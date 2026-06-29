#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![allow(non_snake_case)]

mod components;
mod libs;
mod state;
mod theme;
mod utils;

use crossbeam_channel as channel;
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use mechvibes_core::input_manager::{InputEvent, init_input_channels, init_window_focus_state_with_value};
use mechvibes_core::start_listeners;
use mechvibes_core::state::config::AppConfig;
use mechvibes_core::utils::constants::APP_NAME;
use libs::window_manager::{WINDOW_MANAGER, WindowAction};
use std::sync::mpsc;

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

#[cfg(windows)]
fn enable_windows_console_unicode_and_vt() {
    use winapi::shared::minwindef::FALSE;
    use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
    use winapi::um::wincon::{ENABLE_VIRTUAL_TERMINAL_PROCESSING, SetConsoleCP, SetConsoleOutputCP};

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

const EMBEDDED_ICON: &[u8] = include_bytes!("../assets/icon.ico");

fn load_icon() -> Option<dioxus::desktop::tao::window::Icon> {
    match image::load_from_memory_with_format(EMBEDDED_ICON, image::ImageFormat::Ico) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let target_size = 32u32;
            let final_rgba = if rgba.dimensions() != (target_size, target_size) {
                image::imageops::resize(&rgba, target_size, target_size, image::imageops::FilterType::Lanczos3)
            } else {
                rgba
            };
            dioxus::desktop::tao::window::Icon::from_rgba(final_rgba.into_raw(), target_size, target_size).ok()
        }
        Err(e) => {
            log::error!("❌ Failed to load embedded ICO data: {}", e);
            None
        }
    }
}

fn setup_logging(console_mode: bool) {
    let log_env = if console_mode {
        env_logger::Env::default().default_filter_or("info")
    } else {
        env_logger::Env::default()
    };
    let mut log_builder = env_logger::Builder::from_env(log_env);
    #[cfg(windows)]
    if console_mode {
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

    let _manifest = mechvibes_core::state::manifest::AppManifest::load();

    if let Err(e) = mechvibes_core::state::paths::soundpacks::ensure_soundpack_directories() {
        log::warn!("⚠️ Failed to create soundpack directories: {}", e);
    }

    log::debug!("🔍 Command line args: {:?}", args);

    let should_start_minimized = {
        let config = AppConfig::get();
        args.contains(&"--minimized".to_string()) || (config.auto_start && config.start_minimized)
    };

    mechvibes_core::state::app::init_app_state();
    mechvibes_core::state::app::init_update_state();

    if let Err(e) = mechvibes_core::state::music::initialize_music_player() {
        log::warn!("⚠️ Failed to initialize music player: {}", e);
    } else {
        log::info!("🎵 Music player initialized successfully");
    }

    mechvibes_core::state::ambiance::initialize_global_ambiance_player();
    log::info!("🎵 Ambiance player initialized");

    let (keyboard_tx, keyboard_rx) = channel::unbounded::<InputEvent>();
    let (mouse_tx, mouse_rx) = channel::unbounded::<InputEvent>();
    let (hotkey_tx, hotkey_rx) = channel::unbounded::<String>();

    init_input_channels(keyboard_rx, mouse_rx, hotkey_rx);

    let initial_focus_state = !should_start_minimized;
    init_window_focus_state_with_value(initial_focus_state);

    start_listeners(keyboard_tx, mouse_tx, hotkey_tx);

    let (window_tx, _window_rx) = mpsc::channel::<WindowAction>();
    WINDOW_MANAGER.set_action_sender(window_tx);

    let window_width = 470;
    let min_height = 600;
    let default_height = 820;
    let max_height = 820;

    let window_icon = load_icon();

    let window_builder = WindowBuilder::default()
        .with_title(APP_NAME)
        .with_transparent(true)
        .with_always_on_top(false)
        .with_inner_size(LogicalSize::new(window_width, default_height))
        .with_min_inner_size(LogicalSize::new(window_width, min_height))
        .with_max_inner_size(LogicalSize::new(window_width, max_height))
        .with_fullscreen(None)
        .with_decorations(false)
        .with_resizable(true)
        .with_visible(!should_start_minimized)
        .with_window_icon(window_icon);

    let config = Config::new().with_window(window_builder).with_menu(None);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(app_with_stylesheets)
}

fn app_with_stylesheets() -> Element {
    rsx! {
        libs::ui::app {}
    }
}
