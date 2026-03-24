use crate::utils::config_converter::{convert_v1_to_v2, convert_v2_multi_to_single};

use super::validator::{SoundpackValidationStatus, validate_soundpack_config};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

// ===== SOUNDPACK TYPES =====

// Default function for options field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundpackOptions {
    #[serde(default = "default_recommended_volume")]
    pub recommended_volume: f32,
    #[serde(default = "default_random_pitch")]
    pub random_pitch: bool,
}

fn default_recommended_volume() -> f32 {
    1.0
}

fn default_random_pitch() -> bool {
    false
}

impl Default for SoundpackOptions {
    fn default() -> Self {
        Self {
            recommended_volume: 1.0,
            random_pitch: false,
        }
    }
}

// Key definition structure for V2 format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyDefinition {
    pub timing: Vec<[f32; 2]>, // Array of [start_ms, end_ms] pairs
    #[serde(default)]
    pub audio_file: Option<String>, // For "multi" definition method
}

// This is the structure of the soundpack config file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundPack {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub config_version: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub audio_file: Option<String>, // Used only in "single" definition_method
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub created_at: Option<String>, // ISO-8601 string
    pub definition_method: String, // "single" or "multi"
    #[serde(default)]
    pub options: SoundpackOptions,
    pub definitions: HashMap<String, KeyDefinition>,
}

impl SoundPack {
    /// Validate that the audio file referenced in the config exists on disk.
    /// For "single" method, checks the top-level audio_file. For "multi", checks each definition's audio_file.
    /// Logs warnings or errors for missing files or missing audio_file field.
    pub fn validate_audio_file(&self, soundpack_path: &str) -> Result<(), String> {
        log::debug!("🔍 audio_file in config: {:?}", self.audio_file);

        let audio_files: Vec<&str> = if self.definition_method == "single" {
            self.audio_file
                .as_deref()
                .map(|s| vec![s])
                .unwrap_or_default()
        } else {
            self.definitions
                .values()
                .filter_map(|d| d.audio_file.as_deref())
                .collect()
        };

        if audio_files.is_empty() {
            return Err("No audio_file field found in config".into());
        }

        for audio_filename in audio_files {
            Self::validate_audio_path(soundpack_path, audio_filename)?;
        }

        Ok(())
    }

    fn validate_audio_path(soundpack_path: &str, audio_filename: &str) -> Result<(), String> {
        let full_audio_path = format!(
            "{}/{}",
            soundpack_path,
            audio_filename.trim_start_matches("./")
        );
        log::debug!("🔍 soundpack_path: {}", soundpack_path);
        log::debug!("🔍 full_audio_path: {}", full_audio_path);
        log::info!(
            "🔍 audio file exists: {}",
            std::path::Path::new(&full_audio_path).exists()
        );

        if !std::path::Path::new(&full_audio_path).exists() {
            return Err(format!(
                "Audio file not found during cache refresh: {}",
                full_audio_path
            ));
        }

        Ok(())
    }
}

/// Load and migrate a soundpack config from disk.
/// Validates the config, runs V1→V2 and multi→single migrations if needed,
/// and returns the config path and parsed config JSON.
pub fn load_and_migrate_soundpack(soundpack_path: &str) -> Result<SoundPack, String> {
    let config_path = PathBuf::from(soundpack_path)
        .join("config.json")
        .to_string_lossy()
        .to_string();

    log::info!("🔍 Validating soundpack config: {}", config_path);

    let validation_result = validate_soundpack_config(&config_path);
    if validation_result.status == SoundpackValidationStatus::VersionOneNeedsConversion
        && validation_result.can_be_converted
    {
        let backup_path = format!("{}.v1.backup", config_path);
        let _ = fs::copy(&config_path, &backup_path);
        match convert_v1_to_v2(&config_path, &config_path, None) {
            Ok(()) => {}
            Err(e) => {
                let error_msg =
                    format!("Failed to convert {} from V1 to V2: {}", soundpack_path, e);
                if fs::copy(&backup_path, &config_path).is_ok() {}
                return Err(error_msg);
            }
        }
    }

    let content =
        fs::read_to_string(&config_path).map_err(|e| format!("Failed to read config: {}", e))?;

    let mut config: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;

    if let Some(definition_method) = config.get("definition_method").and_then(|v| v.as_str()) {
        if definition_method == "multi" {
            log::debug!("🔄 Found V2 multi method config, converting to single method");
            if let Err(e) = convert_v2_multi_to_single(&config_path, &soundpack_path) {
                log::error!("❌ Failed to convert multi to single: {}", e);
                return Err(format!("Failed to convert multi to single method: {}", e));
            }

            let new_content = fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to re-read converted config: {}", e))?;
            config = serde_json::from_str(&new_content)
                .map_err(|e| format!("Failed to parse converted config: {}", e))?;

            log::info!("✅ Successfully converted to single method");
        }
    }

    let soundpack = serde_json::from_value::<SoundPack>(config)
        .map_err(|e| format!("Failed to parse config: {}", e))?;

    soundpack.validate_audio_file(soundpack_path)?;

    Ok(soundpack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Parses all committed soundpacks and verifies parsing works for each.
    #[test]
    fn parse_all_committed_soundpacks() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let soundpacks_dir = manifest_dir.join("soundpacks");

        if !soundpacks_dir.exists() {
            panic!(
                "soundpacks directory not found at {}",
                soundpacks_dir.display()
            );
        }

        let mut parsed_count = 0;
        let mut errors = Vec::new();

        for device_type in ["keyboard", "mouse"] {
            let type_dir = soundpacks_dir.join(device_type);
            if !type_dir.exists() {
                continue;
            }

            let entries = fs::read_dir(&type_dir).unwrap_or_else(|e| {
                panic!("Failed to read {}: {}", type_dir.display(), e);
            });

            for entry in entries.filter_map(|e| e.ok()) {
                let config_path = entry.path().join("config.json");
                if !config_path.exists() {
                    continue;
                }

                let path_str = config_path.display().to_string();
                match fs::read_to_string(&config_path) {
                    Ok(content) => match serde_json::from_str::<SoundPack>(&content) {
                        Ok(soundpack) => {
                            assert!(
                                !soundpack.definitions.is_empty(),
                                "{}: soundpack has no definitions",
                                path_str
                            );
                            assert!(
                                !soundpack.name.is_empty(),
                                "{}: soundpack has empty name",
                                path_str
                            );
                            parsed_count += 1;
                        }
                        Err(e) => {
                            errors.push(format!("{}: parse error: {}", path_str, e));
                        }
                    },
                    Err(e) => {
                        errors.push(format!("{}: read error: {}", path_str, e));
                    }
                }
            }
        }

        if !errors.is_empty() {
            panic!(
                "Failed to parse {} soundpack(s):\n{}",
                errors.len(),
                errors.join("\n")
            );
        }

        assert!(
            parsed_count > 0,
            "No soundpacks found to parse in {}",
            soundpacks_dir.display()
        );
    }
}
