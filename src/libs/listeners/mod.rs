use crossbeam_channel as channel;

mod focused_input_listener;
mod input_listener;

use crate::libs::input_manager::get_window_focus_state;
use focused_input_listener::start_focused_keyboard_listener;
use input_listener::start_unified_input_listener;

#[cfg(target_os = "linux")]
mod evdev_input_listener;

// Start input listeners based on platform and display server
#[cfg(target_os = "linux")]
pub fn start_listeners(
    keyboard_tx: channel::Sender<String>,
    mouse_tx: channel::Sender<String>,
    hotkey_tx: channel::Sender<String>,
) {
    use evdev_input_listener::start_evdev_keyboard_listener;
    use std::sync::{Arc, Mutex};

    log::debug!("🎮 Starting listeners (Linux mode)...");

    let display_server = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "x11".to_string());

    log::debug!("🔍 Detected display server: {}", display_server);

    if display_server == "wayland" {
        log::debug!("🎮 Starting evdev keyboard listener (Wayland mode)...");
        let focus_state = get_window_focus_state();
        start_evdev_keyboard_listener(keyboard_tx.clone(), hotkey_tx.clone(), focus_state);

        // Use rdev for mouse events only (no keyboard/hotkeys on Wayland)
        // Pass "always focused" state to prevent rdev from sending keyboard events
        log::debug!("🎮 Starting unified input listener for mouse events (Wayland mode)...");
        let always_focused = Arc::new(Mutex::new(true));
        start_unified_input_listener(keyboard_tx, mouse_tx, hotkey_tx, always_focused);
    } else {
        let focus_state = get_window_focus_state();

        log::debug!("🎮 Starting unified input listener (X11 mode - unfocused)...");
        start_unified_input_listener(
            keyboard_tx.clone(),
            mouse_tx,
            hotkey_tx,
            focus_state.clone(),
        );

        log::debug!("🎮 Starting focused keyboard listener (X11 mode - focused)...");
        start_focused_keyboard_listener(keyboard_tx, focus_state);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn start_listeners(
    keyboard_tx: channel::Sender<String>,
    mouse_tx: channel::Sender<String>,
    hotkey_tx: channel::Sender<String>,
) {
    log::debug!("🎮 Starting listeners (Windows/macOS mode)...");

    let focus_state = get_window_focus_state();

    log::debug!("🎮 Starting unified input listener (unfocused)...");
    start_unified_input_listener(
        keyboard_tx.clone(),
        mouse_tx,
        hotkey_tx,
        focus_state.clone(),
    );

    log::debug!("🎮 Starting focused keyboard listener (focused)...");
    start_focused_keyboard_listener(keyboard_tx, focus_state);
}
