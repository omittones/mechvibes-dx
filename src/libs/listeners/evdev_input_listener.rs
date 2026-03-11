use crossbeam_channel as channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "linux")]
pub fn start_evdev_keyboard_listener(
    keyboard_tx: channel::Sender<String>,
    hotkey_tx: channel::Sender<String>,
    _is_focused: Arc<Mutex<bool>>,
) {
    thread::spawn(move || {
        use evdev::{EventType, KeyCode};

        log::debug!("🔍 [evdev] Starting Linux keyboard listener (Wayland/X11 compatible)");

        // Track modifier keys for hotkey detection
        let mut ctrl_pressed = false;
        let mut alt_pressed = false;

        // Find all keyboard devices
        let mut keyboards = Vec::new();

        let devices: Vec<_> = evdev::enumerate().collect();
        if devices.is_empty() {
            log::error!(
                "❌ [evdev] No devices found, make sure you have permission to access /dev/input/event*"
            );
            return;
        }

        for (path, device) in devices {
            // Check if device has keyboard capabilities
            if device.supported_keys().is_some() {
                log::info!(
                    "🔍 [evdev] Found keyboard device: {:?} - {}",
                    path.display(),
                    device.name().unwrap_or("Unknown")
                );

                // Set device to non-blocking mode to prevent blocking on idle devices
                if let Err(e) = device.set_nonblocking(true) {
                    log::error!(
                        "⚠️ [evdev] Failed to set non-blocking mode for {:?}: {}",
                        path.display(),
                        e
                    );
                }

                keyboards.push(device);
            }
        }

        if keyboards.is_empty() {
            log::error!(
                "❌ [evdev] No keyboard devices found, make sure you have permission to access /dev/input/event*!"
            );
            return;
        }

        log::info!(
            "🔍 [evdev] Monitoring {} keyboard device(s)",
            keyboards.len()
        );

        // Monitor all keyboards in a loop
        loop {
            for device in &mut keyboards {
                // Fetch events (non-blocking)
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() == EventType::KEY {
                                let key_value = event.value();
                                let key = KeyCode::new(event.code());
                                let key_code = map_evdev_keycode(key);
                                if !key_code.is_empty() {
                                    // Handle key press (value == 1)
                                    if key_value == 1 {
                                        // Track modifier keys for hotkey detection
                                        match key_code {
                                            "ControlLeft" | "ControlRight" => {
                                                ctrl_pressed = true;
                                            }
                                            "AltLeft" | "AltRight" => {
                                                alt_pressed = true;
                                            }
                                            "KeyM" => {
                                                // Check for Ctrl+Alt+M hotkey combination
                                                if ctrl_pressed && alt_pressed {
                                                    log::info!(
                                                        "🔥 [evdev] Hotkey detected: Ctrl+Alt+M - Toggling global sound"
                                                    );
                                                    match hotkey_tx.send("TOGGLE_SOUND".to_string())
                                                    {
                                                        Ok(()) => log::debug!(
                                                            "[evdev] Hotkey event sent successfully"
                                                        ),
                                                        Err(e) => log::error!(
                                                            "[evdev] Failed to send hotkey event: {}",
                                                            e
                                                        ),
                                                    }
                                                    continue; // Don't process this as a regular key event
                                                }
                                            }
                                            _ => {}
                                        }

                                        // Send key press event
                                        match keyboard_tx.send(key_code.to_string()) {
                                            Ok(()) => log::debug!(
                                                "[evdev] Key press detected: {}",
                                                key_code
                                            ),
                                            Err(e) => log::error!(
                                                "[evdev] Failed to send key press '{}': {}",
                                                key_code,
                                                e
                                            ),
                                        }
                                    }
                                    // Handle key release (value == 0)
                                    else if key_value == 0 {
                                        // Track modifier key releases for hotkey detection
                                        match key_code {
                                            "ControlLeft" | "ControlRight" => {
                                                ctrl_pressed = false;
                                            }
                                            "AltLeft" | "AltRight" => {
                                                alt_pressed = false;
                                            }
                                            _ => {}
                                        }

                                        // Send key release event
                                        match keyboard_tx.send(format!("UP:{}", key_code)) {
                                            Ok(()) => log::debug!(
                                                "[evdev] Key release detected: {}",
                                                key_code
                                            ),
                                            Err(e) => log::error!(
                                                "[evdev] Failed to send key release '{}': {}",
                                                key_code,
                                                e
                                            ),
                                        }
                                    }
                                    // Ignore key repeat (value == 2)
                                } else {
                                    log::debug!("[evdev] Ignored unmapped key event: {:?}", key);
                                }
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No events available, this is normal
                    }
                    Err(e) => {
                        log::error!("⚠️[evdev] Error fetching events: {}", e);
                    }
                }
            }

            // Small sleep to prevent busy-waiting
            thread::sleep(Duration::from_millis(10));
        }
    });
}

#[cfg(target_os = "linux")]
fn map_evdev_keycode(key: evdev::KeyCode) -> &'static str {
    use evdev::KeyCode;

    match key {
        // Letters
        KeyCode::KEY_A => "KeyA",
        KeyCode::KEY_B => "KeyB",
        KeyCode::KEY_C => "KeyC",
        KeyCode::KEY_D => "KeyD",
        KeyCode::KEY_E => "KeyE",
        KeyCode::KEY_F => "KeyF",
        KeyCode::KEY_G => "KeyG",
        KeyCode::KEY_H => "KeyH",
        KeyCode::KEY_I => "KeyI",
        KeyCode::KEY_J => "KeyJ",
        KeyCode::KEY_K => "KeyK",
        KeyCode::KEY_L => "KeyL",
        KeyCode::KEY_M => "KeyM",
        KeyCode::KEY_N => "KeyN",
        KeyCode::KEY_O => "KeyO",
        KeyCode::KEY_P => "KeyP",
        KeyCode::KEY_Q => "KeyQ",
        KeyCode::KEY_R => "KeyR",
        KeyCode::KEY_S => "KeyS",
        KeyCode::KEY_T => "KeyT",
        KeyCode::KEY_U => "KeyU",
        KeyCode::KEY_V => "KeyV",
        KeyCode::KEY_W => "KeyW",
        KeyCode::KEY_X => "KeyX",
        KeyCode::KEY_Y => "KeyY",
        KeyCode::KEY_Z => "KeyZ",

        // Numbers
        KeyCode::KEY_1 => "Digit1",
        KeyCode::KEY_2 => "Digit2",
        KeyCode::KEY_3 => "Digit3",
        KeyCode::KEY_4 => "Digit4",
        KeyCode::KEY_5 => "Digit5",
        KeyCode::KEY_6 => "Digit6",
        KeyCode::KEY_7 => "Digit7",
        KeyCode::KEY_8 => "Digit8",
        KeyCode::KEY_9 => "Digit9",
        KeyCode::KEY_0 => "Digit0",

        // Function keys
        KeyCode::KEY_F1 => "F1",
        KeyCode::KEY_F2 => "F2",
        KeyCode::KEY_F3 => "F3",
        KeyCode::KEY_F4 => "F4",
        KeyCode::KEY_F5 => "F5",
        KeyCode::KEY_F6 => "F6",
        KeyCode::KEY_F7 => "F7",
        KeyCode::KEY_F8 => "F8",
        KeyCode::KEY_F9 => "F9",
        KeyCode::KEY_F10 => "F10",
        KeyCode::KEY_F11 => "F11",
        KeyCode::KEY_F12 => "F12",

        // Special keys
        KeyCode::KEY_SPACE => "Space",
        KeyCode::KEY_ENTER => "Enter",
        KeyCode::KEY_BACKSPACE => "Backspace",
        KeyCode::KEY_TAB => "Tab",
        KeyCode::KEY_ESC => "Escape",
        KeyCode::KEY_CAPSLOCK => "CapsLock",
        KeyCode::KEY_LEFTSHIFT => "ShiftLeft",
        KeyCode::KEY_RIGHTSHIFT => "ShiftRight",
        KeyCode::KEY_LEFTCTRL => "ControlLeft",
        KeyCode::KEY_RIGHTCTRL => "ControlRight",
        KeyCode::KEY_LEFTALT => "AltLeft",
        KeyCode::KEY_RIGHTALT => "AltRight",
        KeyCode::KEY_LEFTMETA => "MetaLeft",
        KeyCode::KEY_RIGHTMETA => "MetaRight",

        // Arrow keys
        KeyCode::KEY_UP => "ArrowUp",
        KeyCode::KEY_DOWN => "ArrowDown",
        KeyCode::KEY_LEFT => "ArrowLeft",
        KeyCode::KEY_RIGHT => "ArrowRight",

        // Editing keys
        KeyCode::KEY_INSERT => "Insert",
        KeyCode::KEY_DELETE => "Delete",
        KeyCode::KEY_HOME => "Home",
        KeyCode::KEY_END => "End",
        KeyCode::KEY_PAGEUP => "PageUp",
        KeyCode::KEY_PAGEDOWN => "PageDown",

        // Punctuation
        KeyCode::KEY_MINUS => "Minus",
        KeyCode::KEY_EQUAL => "Equal",
        KeyCode::KEY_LEFTBRACE => "BracketLeft",
        KeyCode::KEY_RIGHTBRACE => "BracketRight",
        KeyCode::KEY_BACKSLASH => "Backslash",
        KeyCode::KEY_SEMICOLON => "Semicolon",
        KeyCode::KEY_APOSTROPHE => "Quote",
        KeyCode::KEY_GRAVE => "Backquote",
        KeyCode::KEY_COMMA => "Comma",
        KeyCode::KEY_DOT => "Period",
        KeyCode::KEY_SLASH => "Slash",

        _ => "",
    }
}
