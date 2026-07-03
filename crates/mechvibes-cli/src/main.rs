#![allow(non_snake_case)]

mod daemon;
mod devices;
mod ipc;
mod packs;
mod tray;

use clap::{Parser, Subcommand};
use ipc::{IpcCommand, IpcResponse, send_command};
use mechvibes_core::state::config::AppConfig;

#[derive(Parser)]
#[command(name = "mechvibes-cli", about = "MechvibesDX headless daemon and control tool")]
struct Cli {
    /// Enable log output
    #[arg(long, global = true)]
    log: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Stop the running daemon
    Stop,

    /// Show daemon status and current config
    Status,

    /// Show or manage soundpacks
    Pack {
        #[command(subcommand)]
        cmd: Option<PackCommand>,
    },

    /// Get or set master volume
    Volume {
        #[command(subcommand)]
        cmd: Option<VolumeCommand>,
    },

    /// Toggle mute
    Mute,

    /// Show or manage audio output device
    Device {
        #[command(subcommand)]
        cmd: Option<DeviceCommand>,
    },
}

#[derive(Subcommand)]
enum VolumeCommand {
    /// Set master volume (0–100)
    Set { volume: f32 },
}

#[derive(Subcommand)]
enum PackCommand {
    /// List available soundpacks
    List {
        #[arg(long, help = "List keyboard soundpacks")]
        keyboard: bool,
        #[arg(long, help = "List mouse soundpacks")]
        mouse: bool,
    },
    /// Set the active soundpack by its full ID (e.g. builtin/keyboard/cherrymx-blue-abs)
    Set {
        /// Full soundpack ID from `pack list`
        id: String,
    },
}

#[derive(Subcommand)]
enum DeviceCommand {
    /// List available audio output devices
    List,
    /// Set the active audio output device by ID (use 'default' for system default)
    Set {
        /// Device ID from `device list` (e.g. output_0) or 'default'
        id: String,
    },
    /// Force reconnect to the current audio output device
    Reconnect,
}

fn setup_logging(enabled: bool) {
    let filter = if enabled { "info" } else { "off" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(filter))
        .format_timestamp(None)
        .format_target(false)
        .init();
}

fn main() {
    let cli = Cli::parse();

    setup_logging(cli.log);

    match cli.command {
        None => daemon::run(),

        Some(Command::Stop) => match send_command(IpcCommand::Stop) {
            Ok(_) => println!("Daemon stopped."),
            Err(e) => eprintln!("Error: {}", e),
        },

        Some(Command::Status) => match send_command(IpcCommand::Status) {
            Ok(IpcResponse::Status {
                enable_sound,
                keyboard_soundpack,
                mouse_soundpack,
                volume,
                mouse_volume,
                keypresses,
                mouse_presses,
                avg_latency_ms,
                discarded,
                ..
            }) => {
                println!("Daemon:      running");
                println!("Sound:       {}", if enable_sound { "on" } else { "muted" });
                println!("Keys:        {}", if keyboard_soundpack.is_empty() { "(none)" } else { &keyboard_soundpack });
                println!("Mouse:       {}", if mouse_soundpack.is_empty() { "(none)" } else { &mouse_soundpack });
                println!("Vol:         {:.0}  Mouse vol: {:.0}", volume, mouse_volume);
                println!();
                println!("Key presses: {}", keypresses);
                println!("Mouse clicks:{}", mouse_presses);
                println!("Avg latency: {:.2} ms", avg_latency_ms);
                println!("Discarded:   {}", discarded);
            }
            Ok(_) => println!("Daemon: running"),
            Err(e) => println!("Daemon: not running ({})", e),
        },

        Some(Command::Pack { cmd: None }) => {
            let cfg = AppConfig::get();
            println!("Keyboard: {}", if cfg.keyboard_soundpack.is_empty() { "(none)" } else { &cfg.keyboard_soundpack });
            println!("Mouse:    {}", if cfg.mouse_soundpack.is_empty() { "(none)" } else { &cfg.mouse_soundpack });
        }

        Some(Command::Pack { cmd: Some(PackCommand::List { keyboard, mouse }) }) => {
            packs::list(keyboard, mouse);
        }

        Some(Command::Pack { cmd: Some(PackCommand::Set { id }) }) => {
            packs::set(&id);
        }

        Some(Command::Volume { cmd: None }) => {
            let cfg = AppConfig::get();
            println!("Volume: {:.0}", cfg.volume);
            println!("Mouse volume: {:.0}", cfg.mouse_volume);
        }

        Some(Command::Volume { cmd: Some(VolumeCommand::Set { volume }) }) => {
            let volume = volume.clamp(0.0, 100.0);
            match send_command(IpcCommand::SetVolume { volume }) {
                Ok(_) => println!("Volume set to {:.0}", volume),
                Err(_) => {
                    AppConfig::update(|cfg| cfg.volume = volume);
                    println!("Volume set to {:.0} (saved, no daemon running)", volume);
                }
            }
        }

        Some(Command::Mute) => match send_command(IpcCommand::Mute) {
            Ok(_) => println!("Toggled mute."),
            Err(e) => eprintln!("Error: {}", e),
        },

        Some(Command::Device { cmd: None }) => {
            let cfg = AppConfig::get();
            match &cfg.selected_audio_device {
                Some(id) => println!("Device: {}", id),
                None => println!("Device: default (system)"),
            }
        }

        Some(Command::Device { cmd: Some(DeviceCommand::List) }) => {
            devices::list();
        }

        Some(Command::Device { cmd: Some(DeviceCommand::Set { id }) }) => {
            devices::set(&id);
        }

        Some(Command::Device { cmd: Some(DeviceCommand::Reconnect) }) => {
            devices::reconnect();
        }
    }
}
