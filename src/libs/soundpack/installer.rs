//! Soundpack installation from ZIP archives.

use crate::libs::soundpack::cache::SoundpackMetadata;
use crate::libs::soundpack::cache::SoundpackRef;
use crate::libs::soundpack::cache::SoundpackType;
use crate::libs::soundpack::validator::{SoundpackValidationStatus, validate_soundpack_config};
use crate::state::paths::soundpacks::get_custom_soundpacks_dir;
use crate::utils::config_converter;
use crate::utils::path;
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use uuid::Uuid;
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct SoundpackInfo {
    pub name: String,
    pub id: String,
}

/// Check if a soundpack ID already exists in the app state.
/// Supports both full IDs (custom/keyboard/name) and folder names for conflict detection.
pub fn check_soundpack_id_conflict(id: &SoundpackRef, soundpacks: &[SoundpackMetadata]) -> bool {
    soundpacks.iter().any(|pack| pack.id.eq(id))
}

/// Extract soundpack ID from ZIP without extracting files
pub fn get_soundpack_id_from_zip(file_path: &str) -> Result<SoundpackRef, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let file_path = file.name().to_string();

        if file_path.ends_with("config.json") {
            let mut config_content = String::new();
            file.read_to_string(&mut config_content)
                .map_err(|e| format!("Failed to read config.json: {}", e))?;

            let config: Value = serde_json::from_str(&config_content)
                .map_err(|e| format!("Failed to parse config.json: {}", e))?;

            if let Some(id) = config.get("id").and_then(|v| v.as_str()) {
                if !id.trim().is_empty() {
                    return Ok(SoundpackRef {
                        id: id.to_string(),
                        is_builtin: false,
                        soundpack_type: SoundpackType::Keyboard,
                    });
                }
            }

            return Ok(SoundpackRef {
                id: format!("imported-{}", Uuid::new_v4()),
                is_builtin: false,
                soundpack_type: SoundpackType::Keyboard,
            });
        }
    }

    Err("No config.json found in ZIP file".to_string())
}

/// Extract and install soundpack from ZIP file
pub fn extract_and_install_soundpack(file_path: &str) -> Result<SoundpackInfo, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP archive: {}", e))?;

    let mut config_content = String::new();
    let mut soundpack_id = String::new();
    let mut found_config = false;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let file_path = file.name().to_string();

        if file_path.ends_with("config.json") {
            file.read_to_string(&mut config_content)
                .map_err(|e| format!("Failed to read config.json: {}", e))?;
            found_config = true;
            break;
        }
    }

    if !found_config {
        return Err("No config.json found in ZIP file".to_string());
    }

    let mut config: Value = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    let soundpack_name = config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Soundpack")
        .to_string();

    if let Some(id) = config.get("id").and_then(|v| v.as_str()) {
        if !id.trim().is_empty() {
            soundpack_id = id.to_string();
        }
    }
    if soundpack_id.is_empty() {
        soundpack_id = format!("imported-{}", Uuid::new_v4());
        config["id"] = Value::String(soundpack_id.clone());
    }

    let is_mouse_soundpack = determine_soundpack_type(&config);
    let soundpack_type = if is_mouse_soundpack {
        "mouse"
    } else {
        "keyboard"
    };

    let soundpacks_dir = get_custom_soundpacks_dir();
    let install_dir = soundpacks_dir.join(soundpack_type).join(&soundpack_id);

    path::ensure_directory_exists(&install_dir)
        .map_err(|e| format!("Failed to create soundpack directory: {}", e))?;

    let mut archive =
        ZipArchive::new(File::open(file_path).map_err(|e| format!("Failed to reopen ZIP: {}", e))?)
            .map_err(|e| format!("Failed to reread ZIP archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let file_path = file.name().to_string();

        if file_path.ends_with('/') {
            continue;
        }

        let output_path = if file_path.contains('/') {
            let parts: Vec<&str> = file_path.split('/').collect();
            if parts.len() > 1 {
                install_dir.join(parts[1..].join("/"))
            } else {
                install_dir.join(&file_path)
            }
        } else {
            install_dir.join(&file_path)
        };

        if let Some(parent) = output_path.parent() {
            path::ensure_directory_exists(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let mut output_file =
            File::create(&output_path).map_err(|e| format!("Failed to create file: {}", e))?;
        std::io::copy(&mut file, &mut output_file)
            .map_err(|e| format!("Failed to extract file: {}", e))?;
    }

    log::info!(
        "🔧 Converting config after file extraction - soundpack_dir: {}",
        install_dir.to_string_lossy()
    );
    let final_config_content = handle_config_conversion(
        &config.to_string(),
        &soundpack_id,
        &install_dir.to_string_lossy(),
    )?;
    let config_path = install_dir.join("config.json");
    path::write_file_contents(&config_path.to_string_lossy(), &final_config_content)
        .map_err(|e| format!("Failed to write config.json: {}", e))?;

    // Return full ID for new format (custom/type/folder)
    let full_id = format!("custom/{}/{}", soundpack_type, soundpack_id);

    Ok(SoundpackInfo {
        name: soundpack_name,
        id: full_id,
    })
}

/// Extract and install soundpack from ZIP file with specified target type
pub fn extract_and_install_soundpack_with_type(
    file_path: &str,
    target_type: Option<SoundpackType>,
) -> Result<SoundpackInfo, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP archive: {}", e))?;

    let mut config_content = String::new();
    let mut soundpack_id = String::new();
    let mut found_config = false;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let file_path = file.name().to_string();

        if file_path.ends_with("config.json") {
            file.read_to_string(&mut config_content)
                .map_err(|e| format!("Failed to read config.json: {}", e))?;
            found_config = true;
            break;
        }
    }

    if !found_config {
        return Err("No config.json found in ZIP file".to_string());
    }

    let mut config: Value = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    let soundpack_name = config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Soundpack")
        .to_string();

    if let Some(id) = config.get("id").and_then(|v| v.as_str()) {
        if !id.trim().is_empty() {
            soundpack_id = id.to_string();
        }
    }

    if soundpack_id.is_empty() {
        soundpack_id = format!("imported-{}", Uuid::new_v4());
        config["id"] = Value::String(soundpack_id.clone());
    }

    let soundpack_type = if let Some(target) = target_type {
        match target {
            SoundpackType::Keyboard => "keyboard",
            SoundpackType::Mouse => "mouse",
        }
    } else {
        let is_mouse_soundpack = determine_soundpack_type(&config);
        if is_mouse_soundpack {
            "mouse"
        } else {
            "keyboard"
        }
    };

    let soundpacks_dir = get_custom_soundpacks_dir();
    let install_dir = soundpacks_dir.join(soundpack_type).join(&soundpack_id);

    path::ensure_directory_exists(&install_dir)
        .map_err(|e| format!("Failed to create soundpack directory: {}", e))?;

    let mut archive =
        ZipArchive::new(File::open(file_path).map_err(|e| format!("Failed to reopen ZIP: {}", e))?)
            .map_err(|e| format!("Failed to reread ZIP archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let file_path = file.name().to_string();

        if file_path.ends_with('/') {
            continue;
        }

        let output_path = if file_path.contains('/') {
            let filename = file_path.split('/').last().unwrap_or(&file_path);
            install_dir.join(filename)
        } else {
            install_dir.join(&file_path)
        };

        if let Some(parent) = output_path.parent() {
            path::ensure_directory_exists(parent)
                .map_err(|e| format!("Failed to create parent directory: {}", e))?;
        }

        let mut output_file = File::create(&output_path)
            .map_err(|e| format!("Failed to create output file: {}", e))?;
        std::io::copy(&mut file, &mut output_file)
            .map_err(|e| format!("Failed to extract file: {}", e))?;
    }

    let config_path = install_dir.join("config.json");
    let updated_config = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize updated config: {}", e))?;
    std::fs::write(&config_path, updated_config)
        .map_err(|e| format!("Failed to write updated config.json: {}", e))?;

    let full_id = format!("custom/{}/{}", soundpack_type, soundpack_id);

    Ok(SoundpackInfo {
        name: soundpack_name,
        id: full_id,
    })
}

fn handle_config_conversion(
    config_content: &str,
    soundpack_id: &str,
    soundpack_dir: &str,
) -> Result<String, String> {
    let temp_validate_file = format!("temp_validate_{}.json", soundpack_id);
    std::fs::write(&temp_validate_file, config_content)
        .map_err(|e| format!("Failed to write temp validation file: {}", e))?;

    let validation_result = validate_soundpack_config(&temp_validate_file);

    let _ = std::fs::remove_file(&temp_validate_file);

    let mut final_config_content = config_content.to_string();

    log::info!("⚒️ Soundpack validation result: {:?}", validation_result);

    if validation_result.status == SoundpackValidationStatus::VersionOneNeedsConversion {
        log::info!(
            "🔄 Converting V1 soundpack '{}' to V2 format during import",
            soundpack_id
        );

        let config_backup_path = std::path::Path::new(soundpack_dir).join("config.json.v1.backup");
        if let Err(e) = std::fs::write(&config_backup_path, config_content) {
            log::error!(
                "⚠️Failed to create V1 config backup for {}: {}",
                soundpack_id,
                e,
            );
        } else {
            log::info!(
                "💾 Created V1 config backup at: {}",
                config_backup_path.display()
            );
        }

        let temp_input = format!("temp_v1_{}.json", soundpack_id);
        let temp_output = format!("temp_v2_{}.json", soundpack_id);

        std::fs::write(&temp_input, config_content)
            .map_err(|e| format!("Failed to write temp config: {}", e))?;
        match config_converter::convert_v1_to_v2(&temp_input, &temp_output, Some(soundpack_dir)) {
            Ok(()) => {
                final_config_content = std::fs::read_to_string(&temp_output)
                    .map_err(|e| format!("Failed to read converted config: {}", e))?;

                log::info!(
                    "✅ Successfully converted {} from V1 to V2 during import",
                    soundpack_id
                );

                let _ = std::fs::remove_file(&temp_input);
                let _ = std::fs::remove_file(&temp_output);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temp_input);
                let _ = std::fs::remove_file(&temp_output);
                let _ = std::fs::remove_file(&config_backup_path);

                return Err(format!("Failed to convert V1 soundpack: {}", e));
            }
        }
    }
    Ok(final_config_content)
}

fn determine_soundpack_type(config: &serde_json::Value) -> bool {
    if let Some(soundpack_type) = config.get("type") {
        if let Some(type_str) = soundpack_type.as_str() {
            return type_str == "mouse";
        }
    }

    if let Some(defs) = config.get("defs") {
        if let Some(defs_obj) = defs.as_object() {
            for key in defs_obj.keys() {
                if key.starts_with("Mouse") || key.starts_with("Button") || key.starts_with("Wheel")
                {
                    return true;
                }
            }
        }
    }

    false
}
