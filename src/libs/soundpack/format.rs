use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ===== SOUNDPACK TYPES =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SoundpackType {
    Keyboard,
    Mouse,
}

// Default function for config_version field
fn default_config_version() -> u32 {
    2
}

// Default function for soundpack_type field
fn default_soundpack_type() -> SoundpackType {
    SoundpackType::Keyboard
}

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
    #[serde(default = "default_soundpack_type")]
    pub soundpack_type: SoundpackType, // Type of soundpack (Keyboard or Mouse) - for internal use
    #[serde(default = "default_config_version")]
    pub config_version_num: u32, // Internal config version number
    pub definitions: HashMap<String, KeyDefinition>,
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
