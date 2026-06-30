use mechvibes_core::soundpack::cache::{SoundpackRef, SoundpackType};
use mechvibes_core::state::config::AppConfig;
use mechvibes_core::state::paths::soundpacks::{get_builtin_soundpacks_dir, get_custom_soundpacks_dir};

use crate::ipc::{IpcCommand, send_command};

pub fn set(id: &str) {
    let pack_ref = match SoundpackRef::parse(id) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: invalid soundpack ID '{}': {}", id, e);
            eprintln!("Run 'mechvibes-cli packs list' to see valid IDs.");
            std::process::exit(1);
        }
    };

    let device = match pack_ref.soundpack_type {
        SoundpackType::Keyboard => "keyboard",
        SoundpackType::Mouse => "mouse",
    };

    match send_command(IpcCommand::SetPack { device: device.to_string(), id: id.to_string() }) {
        Ok(_) => println!("Set {} soundpack: {}", device, id),
        Err(_) => {
            AppConfig::update(|cfg| {
                if pack_ref.soundpack_type == SoundpackType::Keyboard {
                    cfg.keyboard_soundpack = id.to_string();
                } else {
                    cfg.mouse_soundpack = id.to_string();
                }
            });
            println!("Set {} soundpack: {} (saved, no daemon running)", device, id);
        }
    }
}

pub fn list(keyboard: bool, mouse: bool) {
    // Default to both if neither flag given
    let show_keyboard = keyboard || (!keyboard && !mouse);
    let show_mouse = mouse || (!keyboard && !mouse);

    if show_keyboard {
        println!("Keyboard soundpacks:");
        print_packs(SoundpackType::Keyboard);
    }
    if show_mouse {
        if show_keyboard {
            println!();
        }
        println!("Mouse soundpacks:");
        print_packs(SoundpackType::Mouse);
    }
}

fn print_packs(kind: SoundpackType) {
    let type_dir = match kind {
        SoundpackType::Keyboard => "keyboard",
        SoundpackType::Mouse => "mouse",
    };

    let mut found = false;

    for (source, base) in [
        ("builtin", get_builtin_soundpacks_dir()),
        ("custom", get_custom_soundpacks_dir()),
    ] {
        let dir = base.join(type_dir);
        if !dir.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let folder = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let id = format!("{}/{}/{}", source, type_dir, folder);
            let name = read_pack_name(&path).unwrap_or_else(|| folder.clone());
            println!("  {:<45} {}", id, name);
            found = true;
        }
    }

    if !found {
        println!("  (none found)");
    }
}

fn read_pack_name(pack_dir: &std::path::Path) -> Option<String> {
    let config_path = pack_dir.join("config.json");
    let raw = std::fs::read_to_string(config_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
    val.get("name")?.as_str().map(|s| s.to_string())
}
