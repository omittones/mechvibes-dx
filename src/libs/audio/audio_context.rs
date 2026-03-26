use super::sound_channel::SoundChannel;
use crate::libs::audio::load_soundpack_from_config;
use crate::libs::audio::sound_channel::{PackKind, PcmBuffer};
use crate::libs::device_manager::DeviceManager;
use crate::state::config::AppConfig;
use cpal::StreamError;
use rodio::{DeviceSinkBuilder, MixerDeviceSink};
use std::collections::HashMap;
use std::process;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

pub static AUDIO_CONTEXT: LazyLock<Arc<Mutex<AudioContext>>> = LazyLock::new(|| {
    let mut context = AudioContext::new();

    // Load soundpack from config
    // TODO - move this somewhere else to AC does not depend on anything
    match load_soundpack_from_config(&mut context, false) {
        Ok(_) => {}
        Err(e) => log::error!("❌ Failed to load initial soundpack: {}", e),
    }

    Arc::new(Mutex::new(context))
});

struct SyncedDeviceSink {
    pub _sink: MixerDeviceSink,
}

// Safety: MixerDeviceSink contains a cpal::Stream with a raw pointer kept alive purely for RAII.
// Mixer is internally Arc-based. We never access _sink across threads.
unsafe impl Send for SyncedDeviceSink {}
unsafe impl Sync for SyncedDeviceSink {}

pub struct AudioContext {
    _sink: SyncedDeviceSink,
    keyboard: SoundChannel,
    mouse: SoundChannel,
}

impl AudioContext {
    pub fn new() -> Self {
        let device = AppConfig::get().selected_audio_device.clone();

        let sink = open_device(device);
        let mixer = sink.mixer().clone();

        let context = Self {
            _sink: SyncedDeviceSink { _sink: sink },
            keyboard: SoundChannel::new(20, mixer.clone()),
            mouse: SoundChannel::new(20, mixer),
        };

        context
    }

    pub fn reconnect(&mut self) {
        let device = AppConfig::get().selected_audio_device.clone();

        let sink = open_device(device);
        let mixer = sink.mixer().clone();

        self._sink = SyncedDeviceSink { _sink: sink };
        self.keyboard = self.keyboard.replace_mixer(&mixer);
        self.mouse = self.mouse.replace_mixer(&mixer);
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
        samples: PcmBuffer,
        mappings: HashMap<String, Vec<(f64, f64)>>,
    ) -> Result<(), String> {
        self.keyboard
            .load_mappings(samples, mappings, PackKind::Keyboard)
    }

    pub fn load_mouse_mappings(
        &mut self,
        samples: PcmBuffer,
        mappings: HashMap<String, Vec<(f64, f64)>>,
    ) -> Result<(), String> {
        self.mouse.load_mappings(samples, mappings, PackKind::Mouse)
    }

    pub fn clear_keyboard_mappings(&mut self) {
        self.keyboard.clear_mappings();
    }

    pub fn clear_mouse_mappings(&mut self) {
        self.mouse.clear_mappings();
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

fn open_device(device_id: Option<String>) -> MixerDeviceSink {
    let device_manager = DeviceManager::new();

    log::info!(
        "🔊 Opening device: {}",
        device_id
            .as_ref()
            .unwrap_or(&"default".to_string())
            .to_string()
    );

    let builder = device_id.map_or_else(
        || DeviceSinkBuilder::from_default_device(),
        |device_id| {
            device_manager
                .get_output_device_by_id(&device_id)
                .map_or_else(
                    |error| {
                        log::error!("❌ Failed to get output device ({}), using default", error);
                        DeviceSinkBuilder::from_default_device()
                    },
                    |device| {
                        device.map_or_else(
                            || {
                                log::error!(
                                    "❌ Failed to get output device (missing device), using default"
                                );
                                DeviceSinkBuilder::from_default_device()
                            },
                            |device| DeviceSinkBuilder::from_device(device),
                        )
                    },
                )
        },
    );

    let builder = match builder {
        Ok(builder) => builder,
        Err(error) => {
            log::error!("❌ Failed to get output device builder: {}", error);
            process::exit(1);
        }
    };

    let sink = match builder
        .with_error_callback(|error| match error {
            StreamError::DeviceNotAvailable | StreamError::StreamInvalidated => {
                log::warn!("⚠️ Device not available, reconnecting...");
                let mut ctx = AUDIO_CONTEXT.lock().unwrap();
                ctx.reconnect();
            }
            _ => {
                log::error!("❌ Error accessing device {:?}", error);
                process::exit(1);
            }
        })
        .open_sink_or_fallback()
    {
        Ok(sink) => sink,
        Err(error) => {
            log::error!("❌ Failed to create audio output stream: {}", error);
            process::exit(1);
        }
    };

    sink
}
