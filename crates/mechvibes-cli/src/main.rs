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

    /// Manage soundpacks
    Packs {
        #[command(subcommand)]
        cmd: PacksCommand,
    },

    /// Set master volume (0–100)
    SetVolume {
        volume: f32,
    },

    /// Toggle mute
    Mute,

    /// Manage audio output devices
    Devices {
        #[command(subcommand)]
        cmd: DevicesCommand,
    },
}

#[derive(Subcommand)]
enum DevicesCommand {
    /// List available audio output devices
    List,
    /// Set the active audio output device by ID (use 'default' for system default)
    Set {
        /// Device ID from `devices list` (e.g. output_0) or 'default'
        id: String,
    },
    /// Force reconnect to the current audio output device
    Reconnect,
}

#[derive(Subcommand)]
enum PacksCommand {
    /// List available soundpacks
    List {
        #[arg(long, help = "List keyboard soundpacks")]
        keyboard: bool,
        #[arg(long, help = "List mouse soundpacks")]
        mouse: bool,
    },
    /// Set the active soundpack by its full ID (e.g. builtin/keyboard/cherrymx-blue-abs)
    Set {
        /// Full soundpack ID from `packs list`
        id: String,
    },
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
                ..
            }) => {
                println!("Daemon: running");
                println!("Sound:  {}", if enable_sound { "on" } else { "muted" });
                println!("Keys:   {}", if keyboard_soundpack.is_empty() { "(none)" } else { &keyboard_soundpack });
                println!("Mouse:  {}", if mouse_soundpack.is_empty() { "(none)" } else { &mouse_soundpack });
                println!("Vol:    {:.0}  Mouse vol: {:.0}", volume, mouse_volume);
            }
            Ok(_) => println!("Daemon: running"),
            Err(e) => println!("Daemon: not running ({})", e),
        },

        Some(Command::Packs { cmd: PacksCommand::List { keyboard, mouse } }) => {
            packs::list(keyboard, mouse);
        }

        Some(Command::Packs { cmd: PacksCommand::Set { id } }) => {
            packs::set(&id);
        }

        Some(Command::SetVolume { volume }) => {
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

        Some(Command::Devices { cmd: DevicesCommand::List }) => {
            devices::list();
        }

        Some(Command::Devices { cmd: DevicesCommand::Set { id } }) => {
            devices::set(&id);
        }

        Some(Command::Devices { cmd: DevicesCommand::Reconnect }) => {
            devices::reconnect();
        }
    }
}
