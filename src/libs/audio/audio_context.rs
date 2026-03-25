use crate::libs::audio::sound_channel::PackKind;
use crate::libs::device_manager::DeviceManager;
use crate::state::config::AppConfig;
use rodio::OutputStream;
use std::time::Instant;

use super::sound_channel::SoundChannel;

struct SyncedOutputStream {
    pub _stream: OutputStream,
}

// Safety: OutputStream contains a cpal::Stream with a raw pointer kept alive purely for RAII.
// OutputStreamHandle is internally Arc-based. We never access _stream across threads.
unsafe impl Send for SyncedOutputStream {}
unsafe impl Sync for SyncedOutputStream {}

pub struct AudioContext {
    _stream: SyncedOutputStream,
    keyboard: SoundChannel,
    mouse: SoundChannel,
}

impl AudioContext {
    pub fn new() -> Self {
        let device = AppConfig::get().selected_audio_device.clone();
        let device_manager = DeviceManager::new();

        let (stream, stream_handle) = open_device(&device_manager, device);

        let context = Self {
            _stream: SyncedOutputStream { _stream: stream },
            keyboard: SoundChannel::new(20, stream_handle.clone()), // Increased max voices to reduce audio interruptions
            mouse: SoundChannel::new(20, stream_handle.clone()),
        };

        context
    }

    pub fn reconnect(&mut self) {
        let device = AppConfig::get().selected_audio_device.clone();
        let device_manager = DeviceManager::new();

        let (stream, stream_handle) = open_device(&device_manager, device);

        self._stream = SyncedOutputStream { _stream: stream };
        self.keyboard = SoundChannel::new(20, stream_handle.clone());
        self.mouse = SoundChannel::new(20, stream_handle.clone());
    }

    pub fn set_keyboard_volume(&self, volume: f32) {
        self.keyboard.set_volume(volume);
        AppConfig::update(|config| {
            config.volume = volume;
        });
    }

    pub fn get_keyboard_volume(&self) -> f32 {
        let config = AppConfig::get();
        config.volume
    }

    pub fn set_mouse_volume(&self, volume: f32) {
        self.mouse.set_volume(volume);
        AppConfig::update(|config| {
            config.mouse_volume = volume;
        });
    }

    pub fn get_mouse_volume(&self) -> f32 {
        let config = AppConfig::get();
        config.mouse_volume
    }

    pub fn load_keyboard_mappings(
        &mut self,
        samples: (Vec<f32>, u16, u32), // (samples, channels, sample_rate)
        mappings: std::collections::HashMap<String, Vec<(f64, f64)>>,
    ) -> Result<(), String> {
        self.keyboard
            .load_mappings(samples, mappings, PackKind::Keyboard)
    }

    pub fn load_mouse_mappings(
        &mut self,
        samples: (Vec<f32>, u16, u32), // (samples, channels, sample_rate)
        mappings: std::collections::HashMap<String, Vec<(f64, f64)>>,
    ) -> Result<(), String> {
        self.mouse.load_mappings(samples, mappings, PackKind::Mouse)
    }

    pub fn play_key_event_sound(&mut self, key: &str, is_keydown: bool, received_at: Instant) {
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

    pub fn play_mouse_event_sound(
        &mut self,
        button: &str,
        is_buttondown: bool,
        received_at: Instant,
    ) {
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

fn open_device(
    device_manager: &DeviceManager,
    device: Option<String>,
) -> (OutputStream, rodio::OutputStreamHandle) {
    // Try to use selected device or fall back to default
    let (stream, stream_handle) = match device {
        Some(ref device_id) => match device_manager.get_output_device_by_id(device_id) {
            Ok(Some(device)) => match OutputStream::try_from_device(&device) {
                Ok((stream, handle)) => (stream, handle),
                Err(e) => {
                    log::error!(
                        "❌ Failed to create stream from selected device {}: {}",
                        device_id,
                        e
                    );
                    log::info!("🔄 Falling back to default device...");
                    OutputStream::try_default()
                        .expect("Failed to create default audio output stream")
                }
            },
            Ok(None) => {
                log::error!(
                    "❌ Selected audio device {} not found, using default",
                    device_id
                );
                OutputStream::try_default().expect("Failed to create default audio output stream")
            }
            Err(e) => {
                log::error!("❌ Error accessing selected device {}: {}", device_id, e);
                OutputStream::try_default().expect("Failed to create default audio output stream")
            }
        },
        None => OutputStream::try_default().expect("Failed to create default audio output stream"),
    };
    (stream, stream_handle)
}
