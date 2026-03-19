use crossbeam_channel as channel;
use std::sync::Arc;
use std::thread;

use super::audio_context::AudioContext;

/// Channels forwarded to the UI for state updates only (no sound playback).
#[derive(Clone)]
pub struct UiEventChannels {
    pub keyboard_rx: channel::Receiver<String>,
}

/// Spawns dedicated threads that play sounds on blocking `recv()`, eliminating
/// the polling latency of the previous `try_recv` + async-sleep approach.
///
/// Keyboard events are forwarded to a UI channel after playback so the UI can
/// still update `KeyboardState`. Mouse events have no UI representation, so
/// they are consumed entirely on the sound thread.
pub fn start_sound_processor(
    audio_ctx: Arc<AudioContext>,
    keyboard_rx: channel::Receiver<String>,
    mouse_rx: channel::Receiver<String>,
) -> UiEventChannels {
    let (ui_keyboard_tx, ui_keyboard_rx) = channel::unbounded::<String>();

    // Keyboard sound thread — blocks until an event arrives, plays immediately,
    // then forwards the event string to the UI channel.
    {
        let ctx = audio_ctx.clone();
        thread::Builder::new()
            .name("sound-keyboard".into())
            .spawn(move || {
                log::info!("🎹 Keyboard sound processor thread started");
                loop {
                    match keyboard_rx.recv() {
                        Ok(keycode) => {
                            if keycode.starts_with("UP:") {
                                ctx.play_key_event_sound(&keycode[3..], false);
                            } else if !keycode.is_empty() {
                                ctx.play_key_event_sound(&keycode, true);
                            }
                            let _ = ui_keyboard_tx.send(keycode);
                        }
                        Err(_) => break,
                    }
                }
                log::info!("🎹 Keyboard sound processor thread exiting");
            })
            .expect("failed to spawn keyboard sound thread");
    }

    // Mouse sound thread — blocks until an event arrives, plays immediately.
    // Nothing to forward since mouse events don't update UI state.
    {
        let ctx = audio_ctx;
        thread::Builder::new()
            .name("sound-mouse".into())
            .spawn(move || {
                log::info!("🖱️ Mouse sound processor thread started");
                loop {
                    match mouse_rx.recv() {
                        Ok(button_code) => {
                            if button_code.starts_with("UP:") {
                                ctx.play_mouse_event_sound(&button_code[3..], false);
                            } else if !button_code.is_empty() {
                                ctx.play_mouse_event_sound(&button_code, true);
                            }
                        }
                        Err(_) => break,
                    }
                }
                log::info!("🖱️ Mouse sound processor thread exiting");
            })
            .expect("failed to spawn mouse sound thread");
    }

    UiEventChannels {
        keyboard_rx: ui_keyboard_rx,
    }
}
