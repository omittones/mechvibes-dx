#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![allow(non_snake_case)]

mod components;
mod libs;
mod state;
mod utils;

use crossbeam_channel as channel;
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use libs::input_manager::{InputEvent, init_input_channels, init_window_focus_state_with_value};
use libs::start_listeners;
use libs::ui;
use libs::window_manager::{WINDOW_MANAGER, WindowAction};
use std::sync::mpsc;
use utils::constants::APP_NAME;

use crate::state::config::AppConfig;

/// When the app is built with the Windows GUI subsystem, stdout/stderr often start as
/// invalid handles. Attach or allocate a console, then wire stdio to `CONOUT$` so Rust's
/// logging and `env_logger` actually reach the terminal.
#[cfg(windows)]
fn attach_console_for_logging() {
    use std::fs::OpenOptions;
    use std::mem;
    use std::os::windows::io::AsRawHandle;
    use winapi::shared::minwindef::FALSE;
    use winapi::um::consoleapi::AllocConsole;
    use winapi::um::processenv::SetStdHandle;
    use winapi::um::winbase::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
    use winapi::um::wincon::{ATTACH_PARENT_PROCESS, AttachConsole};

    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == FALSE {
            let _ = AllocConsole();
        }
    }

    let stdout = OpenOptions::new().write(true).open(r"\\.\CONOUT$");
    let stderr = OpenOptions::new().write(true).open(r"\\.\CONOUT$");
    if let (Ok(out), Ok(err)) = (stdout, stderr) {
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, out.as_raw_handle() as _);
            SetStdHandle(STD_ERROR_HANDLE, err.as_raw_handle() as _);
        }
        mem::forget(out);
        mem::forget(err);
    }

    enable_windows_console_unicode_and_vt();
}

/// UTF-8 so emoji render; VT processing so `env_logger` ANSI color escapes show up in classic conhost.
#[cfg(windows)]
fn enable_windows_console_unicode_and_vt() {
    use winapi::shared::minwindef::FALSE;
    use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
    use winapi::um::wincon::{
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, SetConsoleCP, SetConsoleOutputCP,
    };

    const CP_UTF8: winapi::shared::minwindef::UINT = 65001;

    unsafe {
        let _ = SetConsoleCP(CP_UTF8);
        let _ = SetConsoleOutputCP(CP_UTF8);

        for &std_id in &[STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let h = GetStdHandle(std_id);
            if h.is_null() || h == INVALID_HANDLE_VALUE {
                continue;
            }
            let mut mode = 0u32;
            if GetConsoleMode(h, &mut mode) != FALSE {
                mode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING;
                let _ = SetConsoleMode(h, mode);
            }
        }
    }
}

// Use .ico format for better Windows compatibility
const EMBEDDED_ICON: &[u8] = include_bytes!("../assets/icon.ico");

fn load_icon() -> Option<dioxus::desktop::tao::window::Icon> {
    // Try to create icon from embedded ICO data
    // Windows taskbar works best with 32x32 icons
    match image::load_from_memory_with_format(EMBEDDED_ICON, image::ImageFormat::Ico) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            log::debug!("📐 Loaded icon from ICO: {}x{}", width, height);

            // Always resize to 32x32 for maximum Windows taskbar compatibility
            // This is the standard size Windows expects for taskbar icons
            let target_size = 32u32;

            let final_rgba = if width != target_size || height != target_size {
                log::debug!(
                    "🔄 Resizing icon from {}x{} to {}x{} for Windows taskbar",
                    width,
                    height,
                    target_size,
                    target_size
                );
                image::imageops::resize(
                    &rgba,
                    target_size,
                    target_size,
                    image::imageops::FilterType::Lanczos3,
                )
            } else {
                log::debug!("✅ Icon already at optimal size ({}x{})", width, height);
                rgba
            };

            match dioxus::desktop::tao::window::Icon::from_rgba(
                final_rgba.into_raw(),
                target_size,
                target_size,
            ) {
                Ok(icon) => {
                    log::debug!(
                        "✅ Successfully created window icon ({}x{})",
                        target_size,
                        target_size
                    );
                    Some(icon)
                }
                Err(e) => {
                    log::error!("❌ Failed to create window icon from RGBA data: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            log::error!("❌ Failed to load embedded ICO data: {}", e);
            None
        }
    }
}

fn setup_logging(console_mode: bool) {
    // Without RUST_LOG, env_logger is quiet except errors; `--console` implies info-level startup
    // logs. RUST_LOG still wins when set.
    let log_env = if console_mode {
        env_logger::Env::default().default_filter_or("info")
    } else {
        env_logger::Env::default()
    };
    let mut log_builder = env_logger::Builder::from_env(log_env);
    #[cfg(windows)]
    if console_mode {
        // Redirected CONOUT$ is not a tty; env_logger skips ANSI unless we force color output.
        log_builder.write_style(env_logger::fmt::WriteStyle::Always);
    }
    log_builder.init();
}

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    let console_mode = raw_args.iter().any(|a| a == "--console");
    let args: Vec<String> = raw_args.into_iter().filter(|a| a != "--console").collect();

    #[cfg(windows)]
    if console_mode {
        attach_console_for_logging();
    }

    setup_logging(console_mode);

    log::info!("🚀 Initializing {}...", APP_NAME);

    // Initialize app manifest first
    let _manifest = state::manifest::AppManifest::load();

    // Ensure soundpack directories exist
    if let Err(e) = state::paths::soundpacks::ensure_soundpack_directories() {
        log::warn!("⚠️ Failed to create soundpack directories: {}", e);
    }

    log::debug!("🔍 Command line args: {:?}", args);

    // Check if we should start minimized (from auto-startup)
    let should_start_minimized = {
        let config = AppConfig::get();
        args.contains(&"--minimized".to_string()) || (config.auto_start && config.start_minimized)
    };

    // Register protocol on first run
    // if let Err(e) = protocol::register_protocol() {
    //     log::error!("Warning: Failed to register mechvibes:// protocol: {}", e);
    // }    // Initialize global app state before rendering
    state::app::init_app_state();
    state::app::init_update_state();

    // Initialize music player
    if let Err(e) = state::music::initialize_music_player() {
        log::warn!("⚠️ Failed to initialize music player: {}", e);
    } else {
        log::info!("🎵 Music player initialized successfully");
    }

    // Initialize ambiance player
    state::ambiance::initialize_global_ambiance_player();
    log::info!("🎵 Ambiance player initialized");

    // Note: Update service will be initialized within the UI components
    // to ensure proper Dioxus runtime context

    // Create input event channels for communication between input listener and UI
    let (keyboard_tx, keyboard_rx) = channel::unbounded::<InputEvent>();
    let (mouse_tx, mouse_rx) = channel::unbounded::<InputEvent>();
    let (hotkey_tx, hotkey_rx) = channel::unbounded::<String>();

    // Initialize global input channels for UI to access (including senders for window events)
    init_input_channels(keyboard_rx, mouse_rx, hotkey_rx);

    // Initialize window focus state
    // If window starts visible (not minimized), it will be focused
    let initial_focus_state = !should_start_minimized;
    init_window_focus_state_with_value(initial_focus_state);
    log::debug!(
        "🔍 Initial window focus state: {}",
        if initial_focus_state {
            "FOCUSED"
        } else {
            "UNFOCUSED"
        }
    );

    start_listeners(keyboard_tx, mouse_tx, hotkey_tx);

    // Create window action channel
    let (window_tx, _window_rx) = mpsc::channel::<WindowAction>();
    WINDOW_MANAGER.set_action_sender(window_tx);

    // Window dimensions - allow vertical resizing
    let window_width = 470;
    let min_height = 600; // Minimum height for compact mode
    let default_height = 820; // Default height
    let max_height = 820; // Maximum height

    // Load icon before creating window
    let window_icon = load_icon();
    if window_icon.is_none() {
        log::warn!("⚠️ Warning: Failed to load window icon - taskbar icon may not appear");
    }

    // Create a WindowBuilder with custom appearance and vertical resizing
    let window_builder = WindowBuilder::default()
        .with_title(APP_NAME)
        .with_transparent(true) // Enable transparency for custom window styling
        .with_always_on_top(false) // Allow normal window behavior for taskbar
        .with_inner_size(LogicalSize::new(window_width, default_height))
        .with_min_inner_size(LogicalSize::new(window_width, min_height))
        .with_max_inner_size(LogicalSize::new(window_width, max_height))
        .with_fullscreen(None)
        .with_decorations(false) // Use custom title bar
        .with_resizable(true) // Enable vertical resizing
        .with_visible(!should_start_minimized) // Hide window if starting minimized
        .with_window_icon(window_icon); // Set window icon for taskbar

    // Create config with our window settings and custom protocol handlers
    let config = Config::new().with_window(window_builder).with_menu(None);

    // Launch the app with our config
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(app_with_stylesheets)
}

fn app_with_stylesheets() -> Element {
    rsx! {
        ui::app {}
    }
}
