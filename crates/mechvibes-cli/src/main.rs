use crossbeam_channel as channel;
use mechvibes_core::input_manager::{InputEvent, init_input_channels, init_window_focus_state_with_value};
use mechvibes_core::start_listeners;
use mechvibes_core::state::config::AppConfig;
use mechvibes_core::utils::constants::APP_NAME;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn setup_logging() {
    let log_env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(log_env).init();
}

fn main() {
    setup_logging();

    log::info!("🚀 {} CLI starting...", APP_NAME);

    let _manifest = mechvibes_core::state::manifest::AppManifest::load();

    if let Err(e) = mechvibes_core::state::paths::soundpacks::ensure_soundpack_directories() {
        log::warn!("⚠️ Failed to create soundpack directories: {}", e);
    }

    mechvibes_core::state::app::init_app_state();

    {
        let config = AppConfig::get();
        log::info!(
            "🎹 Keyboard soundpack: {}",
            if config.keyboard_soundpack.is_empty() { "(none)" } else { &config.keyboard_soundpack }
        );
        log::info!(
            "🖱️  Mouse soundpack: {}",
            if config.mouse_soundpack.is_empty() { "(none)" } else { &config.mouse_soundpack }
        );
        log::info!("🔊 Sound enabled: {}", config.enable_sound);
    }

    // Load audio — AUDIO_CONTEXT is a LazyLock that initializes on first access
    {
        let _ctx = mechvibes_core::audio::audio_context::AUDIO_CONTEXT.lock();
        log::info!("🔊 Audio context initialized");
    }

    let (keyboard_tx, keyboard_rx) = channel::unbounded::<InputEvent>();
    let (mouse_tx, mouse_rx) = channel::unbounded::<InputEvent>();
    let (hotkey_tx, hotkey_rx) = channel::unbounded::<String>();

    // CLI has no window — treat focus as always false so the global (rdev) listener handles all input
    init_window_focus_state_with_value(false);
    init_input_channels(keyboard_rx, mouse_rx, hotkey_rx);

    start_listeners(keyboard_tx, mouse_tx, hotkey_tx);

    // Spawn a thread to handle hotkeys (Ctrl+Alt+M toggles sound)
    std::thread::spawn(move || {
        let channels = mechvibes_core::input_manager::get_input_channels();
        loop {
            match channels.hotkey_rx.recv() {
                Ok(hotkey) if hotkey == "TOGGLE_SOUND" => {
                    AppConfig::update(|cfg| {
                        cfg.enable_sound = !cfg.enable_sound;
                    });
                    let enabled = AppConfig::get().enable_sound;
                    log::info!("🔊 Sound {}", if enabled { "enabled" } else { "muted" });
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    log::info!("✅ {} CLI running — press Ctrl+C to exit", APP_NAME);

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        log::info!("👋 Shutting down...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}
