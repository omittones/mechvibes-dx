//! Load soundpack metadata from config files.

use crate::libs::soundpack::cache::SoundpackMetadata;
use crate::libs::soundpack::format::SoundpackType;
use crate::libs::soundpack::id::SoundpackId;
use crate::libs::soundpack::validator::{SoundpackValidationStatus, validate_soundpack_config};
use crate::state::paths;
use crate::utils::config_converter;
use std::fs;
use std::path::PathBuf;

/// Load soundpack metadata from config.json.
/// `soundpack_path` is the absolute directory path; `soundpack_id` is the full ID (e.g. builtin/keyboard/oreo).
pub fn load_soundpack_metadata(
    soundpack_path: &str,
    soundpack_id: &str,
    is_mouse: bool,
) -> Result<SoundpackMetadata, String> {
    let config_path = PathBuf::from(soundpack_path)
        .join("config.json")
        .to_string_lossy()
        .to_string();

    let validation_result = validate_soundpack_config(&config_path);

    if validation_result.status == SoundpackValidationStatus::VersionOneNeedsConversion
        && validation_result.can_be_converted
    {
        let backup_path = format!("{}.v1.backup", config_path);
        let _ = fs::copy(&config_path, &backup_path);
        match config_converter::convert_v1_to_v2(&config_path, &config_path, None) {
            Ok(()) => {}
            Err(e) => {
                let error_msg = format!("Failed to convert {} from V1 to V2: {}", soundpack_id, e);
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
            let soundpack_dir = paths::soundpacks::find_soundpack_dir(soundpack_id, is_mouse);
            // Use soundpack_path directly when available (from scan)

            if let Err(e) =
                config_converter::convert_v2_multi_to_single(&config_path, &soundpack_dir)
            {
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

    let audio_file = config.get("audio_file").and_then(|v| v.as_str());
    log::debug!("🔍 soundpack_id: {}", soundpack_id);
    log::debug!("🔍 config_path: {}", config_path);
    log::debug!("🔍 audio_file in config: {:?}", audio_file);

    if let Some(audio_filename) = audio_file {
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
            log::info!(
                "⚠️ Audio file not found during cache refresh: {}",
                full_audio_path
            );
        }
    } else {
        log::error!("⚠️[CACHE DEBUG] No audio_file field found in config");
    }

    let name = config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(soundpack_id)
        .to_string();

    let tags = config
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(SoundpackMetadata {
        id: soundpack_id.to_string(),
        config_path: config_path,
        name,
        author: config
            .get("author")
            .or_else(|| config.get("m_author"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        tags,
        icon: {
            if let Some(icon_filename) = config.get("icon").and_then(|v| v.as_str()) {
                let icon_path = format!(
                    "{}/{}",
                    soundpack_path,
                    icon_filename.trim_start_matches("./")
                );
                log::debug!(
                    "🔍 Checking icon for {}: {} -> exists: {}",
                    soundpack_id,
                    icon_path,
                    std::path::Path::new(&icon_path).exists()
                );
                if std::path::Path::new(&icon_path).exists() {
                    // URL format: /soundpack-images/{source}/{type}/{folder}/{filename}
                    let asset_url = if SoundpackId::is_new_format(soundpack_id) {
                        format!("/soundpack-images/{}/{}", soundpack_id, icon_filename)
                    } else {
                        // Legacy: /soundpack-images/{type}/{folder}/{filename}
                        let type_str = if is_mouse { "mouse" } else { "keyboard" };
                        format!(
                            "/soundpack-images/{}/{}/{}",
                            type_str, soundpack_id, icon_filename
                        )
                    };
                    log::info!("✅ Generated asset URL for {}: {}", soundpack_id, asset_url);
                    Some(asset_url)
                } else {
                    log::error!(
                        "❌ Icon not found for {}, setting empty string",
                        soundpack_id
                    );
                    Some(String::new())
                }
            } else {
                log::info!("ℹ️  No icon specified for {}", soundpack_id);
                Some(String::new())
            }
        },
        soundpack_type: if is_mouse {
            SoundpackType::Mouse
        } else {
            SoundpackType::Keyboard
        },
    })
}
