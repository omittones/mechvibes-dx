use super::audio_context::AudioContext;
use crate::state::config::AppConfig;
use std::time::Instant;

impl AudioContext {
    pub fn play_key_event_sound(&self, key: &str, is_keydown: bool, received_at: Instant) {
        let config = AppConfig::get();
        if !config.enable_sound || !config.enable_keyboard_sound {
            log::debug!(
                "🔇 Sound disabled, skipping key event sound for key '{}'",
                key
            );
            return;
        }
        drop(config);

        self.keyboard.play_event_sound(
            key,
            is_keydown,
            self.get_keyboard_volume(),
            received_at,
            "keyboard",
        );
    }

    pub fn play_mouse_event_sound(&self, button: &str, is_buttondown: bool, received_at: Instant) {
        let config = AppConfig::get();
        if !config.enable_sound || !config.enable_mouse_sound {
            return;
        }
        drop(config);

        self.mouse.play_event_sound(
            button,
            is_buttondown,
            self.get_mouse_volume(),
            received_at,
            "mouse",
        );
    }
}
