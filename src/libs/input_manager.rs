/// Global input manager to handle input channels between main and UI
use crossbeam_channel as channel;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// A single keyboard or mouse event flowing through the input channels.
#[derive(Clone, Debug)]
pub struct InputEvent {
    pub code: String,
    pub is_down: bool,
    /// Monotonic time when the listener sent this event into the channel.
    pub received_at: Instant,
}

/// Static global holder for input channels
static INPUT_CHANNELS: OnceLock<InputChannels> = OnceLock::new();

/// Static global holder for window focus state
static WINDOW_FOCUS_STATE: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();

/// Struct to hold input event channels
pub struct InputChannels {
    pub keyboard_rx: channel::Receiver<InputEvent>,
    pub mouse_rx: channel::Receiver<InputEvent>,
    pub hotkey_rx: channel::Receiver<String>,
}

/// Initialize input channels (called from main)
pub fn init_input_channels(
    keyboard_rx: channel::Receiver<InputEvent>,
    mouse_rx: channel::Receiver<InputEvent>,
    hotkey_rx: channel::Receiver<String>,
) {
    let channels = InputChannels {
        keyboard_rx,
        mouse_rx,
        hotkey_rx,
    };

    let _ = INPUT_CHANNELS.set(channels);
}

/// Get input channels (called from UI)
pub fn get_input_channels() -> &'static InputChannels {
    INPUT_CHANNELS
        .get()
        .expect("Input channels not initialized")
}

/// Initialize window focus state with a specific value (called from main)
pub fn init_window_focus_state_with_value(focused: bool) {
    let _ = WINDOW_FOCUS_STATE.set(Arc::new(Mutex::new(focused)));
}

/// Get window focus state (called from UI)
pub fn get_window_focus_state() -> Arc<Mutex<bool>> {
    WINDOW_FOCUS_STATE
        .get()
        .expect("Window focus state not initialized")
        .clone()
}

/// Set window focus state (called from UI event handler)
pub fn set_window_focus(focused: bool) {
    if let Some(state) = WINDOW_FOCUS_STATE.get() {
        *state.lock().unwrap() = focused;
        log::debug!(
            "🔍 Window focus state changed: {}",
            if focused { "FOCUSED" } else { "UNFOCUSED" }
        );
    }
}
