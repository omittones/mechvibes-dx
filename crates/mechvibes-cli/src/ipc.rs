use mechvibes_core::audio::audio_context::AUDIO_CONTEXT;
use mechvibes_core::audio::load_soundpack_from_config;
use mechvibes_core::state::config::AppConfig;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

pub const IPC_PORT: u16 = 47801;
const IPC_ADDR: &str = "127.0.0.1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcCommand {
    Stop,
    Mute,
    Status,
    SetPack { device: String, id: String },
    SetVolume { volume: f32 },
    SetDevice { id: String },
    Reconnect,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpcResponse {
    Status {
        ok: bool,
        enable_sound: bool,
        keyboard_soundpack: String,
        mouse_soundpack: String,
        volume: f32,
        mouse_volume: f32,
        keypresses: u64,
        mouse_presses: u64,
        avg_latency_ms: f64,
        discarded: u64,
    },
    Simple {
        ok: bool,
    },
    Error {
        ok: bool,
        error: String,
    },
}

impl IpcResponse {
    pub fn ok() -> Self {
        IpcResponse::Simple { ok: true }
    }
    pub fn err(msg: &str) -> Self {
        IpcResponse::Error { ok: false, error: msg.to_string() }
    }
}

/// Send a command to the running daemon and return the response. Synchronous.
pub fn send_command(cmd: IpcCommand) -> Result<IpcResponse, String> {
    let addr = format!("{}:{}", IPC_ADDR, IPC_PORT);
    let mut stream = TcpStream::connect(&addr)
        .map_err(|_| "No daemon running".to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    let cmd_json = serde_json::to_string(&cmd).unwrap();
    stream
        .write_all(format!("{}\n", cmd_json).as_bytes())
        .map_err(|e| format!("Send error: {}", e))?;

    let mut response = String::new();
    BufReader::new(&stream)
        .read_line(&mut response)
        .map_err(|e| format!("Read error: {}", e))?;

    serde_json::from_str(response.trim()).map_err(|e| format!("Parse error: {}", e))
}

/// Bind the IPC listener and spawn a handler thread. Errors if address is already bound
/// (meaning another daemon is already running).
pub fn start_ipc_server(stop_flag: Arc<AtomicBool>) -> Result<(), String> {
    let addr = format!("{}:{}", IPC_ADDR, IPC_PORT);
    let listener =
        TcpListener::bind(&addr).map_err(|_| "Port already in use — daemon may already be running".to_string())?;
    listener.set_nonblocking(true).unwrap();

    log::info!("IPC server on {}", addr);

    std::thread::Builder::new()
        .name("ipc-server".into())
        .spawn(move || {
            loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let stop_flag = stop_flag.clone();
                        std::thread::spawn(move || handle_client(stream, stop_flag));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if stop_flag.load(Ordering::Relaxed) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        log::error!("IPC accept error: {}", e);
                        break;
                    }
                }
            }
        })
        .unwrap();

    Ok(())
}

fn handle_client(stream: TcpStream, stop_flag: Arc<AtomicBool>) {
    let peer = stream.peer_addr().ok();
    let mut reader = BufReader::new(&stream);
    let mut writer = &stream;

    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }

    let response = match serde_json::from_str::<IpcCommand>(line.trim()) {
        Ok(cmd) => dispatch(cmd, stop_flag),
        Err(e) => {
            log::warn!("IPC bad command from {:?}: {}", peer, e);
            IpcResponse::err("unknown command")
        }
    };

    let resp_json = serde_json::to_string(&response).unwrap();
    let _ = writer.write_all(format!("{}\n", resp_json).as_bytes());
}

fn dispatch(cmd: IpcCommand, stop_flag: Arc<AtomicBool>) -> IpcResponse {
    match cmd {
        IpcCommand::Stop => {
            stop_flag.store(true, Ordering::Relaxed);
            IpcResponse::ok()
        }

        IpcCommand::Mute => {
            AppConfig::update(|cfg| cfg.enable_sound = !cfg.enable_sound);
            let enabled = AppConfig::get().enable_sound;
            log::info!("Sound {}", if enabled { "unmuted" } else { "muted" });
            IpcResponse::ok()
        }

        IpcCommand::Status => {
            let cfg = AppConfig::get();
            let stats = &mechvibes_core::stats::STATS;
            IpcResponse::Status {
                ok: true,
                enable_sound: cfg.enable_sound,
                keyboard_soundpack: cfg.keyboard_soundpack.clone(),
                mouse_soundpack: cfg.mouse_soundpack.clone(),
                volume: cfg.volume,
                mouse_volume: cfg.mouse_volume,
                keypresses: stats.keypresses.load(std::sync::atomic::Ordering::Relaxed),
                mouse_presses: stats.mouse_presses.load(std::sync::atomic::Ordering::Relaxed),
                avg_latency_ms: stats.avg_latency_ms(),
                discarded: stats.discarded.load(std::sync::atomic::Ordering::Relaxed),
            }
        }

        IpcCommand::SetPack { device, id } => {
            let is_mouse = device == "mouse";
            AppConfig::update(|cfg| {
                if is_mouse {
                    cfg.mouse_soundpack = id.clone();
                } else {
                    cfg.keyboard_soundpack = id.clone();
                }
            });
            // Reload PCM data into AudioContext
            match AUDIO_CONTEXT.lock() {
                Ok(mut ctx) => {
                    if let Err(e) = load_soundpack_from_config(&mut ctx, false) {
                        return IpcResponse::err(&format!("Config saved but reload failed: {}", e));
                    }
                }
                Err(_) => return IpcResponse::err("Failed to lock audio context"),
            }
            log::info!("Soundpack {}: {}", device, id);
            IpcResponse::ok()
        }

        IpcCommand::SetVolume { volume } => {
            let volume = volume.clamp(0.0, 100.0);
            AppConfig::update(|cfg| cfg.volume = volume);
            log::info!("Volume set to {:.0}", volume);
            IpcResponse::ok()
        }

        IpcCommand::SetDevice { id } => {
            let device_id = if id == "default" { None } else { Some(id.clone()) };
            AppConfig::update(|cfg| cfg.selected_audio_device = device_id);
            match AUDIO_CONTEXT.lock() {
                Ok(mut ctx) => {
                    ctx.reconnect();
                    log::info!("Audio device set to {}", id);
                    IpcResponse::ok()
                }
                Err(_) => IpcResponse::err("Failed to lock audio context"),
            }
        }

        IpcCommand::Reconnect => {
            match AUDIO_CONTEXT.lock() {
                Ok(mut ctx) => {
                    ctx.reconnect();
                    log::info!("Audio device reconnected");
                    IpcResponse::ok()
                }
                Err(_) => IpcResponse::err("Failed to lock audio context"),
            }
        }
    }
}
