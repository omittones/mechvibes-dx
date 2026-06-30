use crossbeam_channel as channel;
use mechvibes_core::{
    audio::{audio_context::AUDIO_CONTEXT, start_sound_processor},
    input_manager::{InputEvent, get_input_channels, init_input_channels, init_window_focus_state_with_value},
    start_listeners,
    state::config::AppConfig,
    utils::constants::APP_NAME,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::ipc;

pub fn run() {
    log::info!("{} daemon starting", APP_NAME);

    let _manifest = mechvibes_core::state::manifest::AppManifest::load();

    if let Err(e) = mechvibes_core::state::paths::soundpacks::ensure_soundpack_directories() {
        log::warn!("Failed to create soundpack directories: {}", e);
    }

    mechvibes_core::state::app::init_app_state();

    {
        let cfg = AppConfig::get();
        log::info!(
            "Keyboard soundpack: {}",
            if cfg.keyboard_soundpack.is_empty() { "(none)" } else { &cfg.keyboard_soundpack }
        );
        log::info!(
            "Mouse soundpack: {}",
            if cfg.mouse_soundpack.is_empty() { "(none)" } else { &cfg.mouse_soundpack }
        );
        log::info!("Sound enabled: {}", cfg.enable_sound);
    }

    // Force-initialize the audio context (loads soundpacks internally on first lock)
    drop(AUDIO_CONTEXT.lock());
    log::info!("Audio context ready");

    // Input channels
    let (keyboard_tx, keyboard_rx) = channel::unbounded::<InputEvent>();
    let (mouse_tx, mouse_rx) = channel::unbounded::<InputEvent>();
    let (hotkey_tx, hotkey_rx) = channel::unbounded::<String>();

    // CLI has no window — global (rdev) listener handles all input
    init_window_focus_state_with_value(false);
    init_input_channels(keyboard_rx, mouse_rx, hotkey_rx);

    // Spawn sound processor threads (keyboard + mouse)
    start_sound_processor(
        AUDIO_CONTEXT.clone(),
        get_input_channels().keyboard_rx.clone(),
        get_input_channels().mouse_rx.clone(),
    );

    start_listeners(keyboard_tx, mouse_tx, hotkey_tx);

    let stop_flag = Arc::new(AtomicBool::new(false));

    // IPC server
    if let Err(e) = ipc::start_ipc_server(stop_flag.clone()) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Hotkey thread (Ctrl+Alt+M toggles sound)
    {
        let stop_flag = stop_flag.clone();
        std::thread::Builder::new()
            .name("hotkey".into())
            .spawn(move || {
                let channels = get_input_channels();
                loop {
                    match channels.hotkey_rx.recv() {
                        Ok(cmd) if cmd == "TOGGLE_SOUND" => {
                            AppConfig::update(|cfg| cfg.enable_sound = !cfg.enable_sound);
                            let on = AppConfig::get().enable_sound;
                            log::info!("Sound {}", if on { "unmuted" } else { "muted" });
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                    if stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                }
            })
            .unwrap();
    }

    // Ctrl+C handler (for terminals; tray exit also sets the same stop_flag)
    {
        let stop_flag = stop_flag.clone();
        ctrlc::set_handler(move || {
            log::info!("Ctrl+C received, shutting down");
            stop_flag.store(true, Ordering::Relaxed);
        })
        .ok();
    }

    log::info!("{} running — Ctrl+C or tray to exit", APP_NAME);

    // Main thread: tray message pump (Windows) or simple wait (others)
    crate::tray::run_event_loop(stop_flag);

    log::info!("{} stopped", APP_NAME);
}
