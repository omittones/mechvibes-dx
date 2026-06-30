use mechvibes_core::device_manager::DeviceManager;
use mechvibes_core::state::config::AppConfig;

use crate::ipc::{IpcCommand, send_command};

pub fn list() {
    let dm = DeviceManager::new();
    match dm.get_output_devices() {
        Ok(devices) => {
            let selected = AppConfig::get().selected_audio_device.clone();
            println!("Audio output devices:");
            // Always show the system default option first
            let current_is_default = selected.is_none() || selected.as_deref() == Some("output_default");
            let default_marker = if current_is_default { " *" } else { "" };
            println!("  {:<20} System default{}", "default", default_marker);
            for d in &devices {
                let active = selected.as_deref() == Some(&d.id);
                let mut tags = Vec::new();
                if d.is_default {
                    tags.push("system default");
                }
                if active {
                    tags.push("selected");
                }
                let tag_str = if tags.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", tags.join(", "))
                };
                let marker = if active { " *" } else { "" };
                println!("  {:<20} {}{}{}", d.id, d.name, tag_str, marker);
            }
            println!();
            println!("Use 'mechvibes-cli devices set <id>' to switch. * = active.");
        }
        Err(e) => eprintln!("Error listing devices: {}", e),
    }
}

pub fn set(id: &str) {
    match send_command(IpcCommand::SetDevice { id: id.to_string() }) {
        Ok(_) => println!("Audio device set to '{}'.", id),
        Err(_) => {
            // No daemon — save to config directly (takes effect on next daemon start)
            let device_id = if id == "default" { None } else { Some(id.to_string()) };
            AppConfig::update(|cfg| cfg.selected_audio_device = device_id);
            println!("Audio device set to '{}' (saved, no daemon running).", id);
        }
    }
}

pub fn reconnect() {
    match send_command(IpcCommand::Reconnect) {
        Ok(_) => println!("Reconnected audio device."),
        Err(e) => eprintln!("Error: {} (is the daemon running?)", e),
    }
}
