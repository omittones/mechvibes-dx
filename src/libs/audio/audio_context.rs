use super::sound_channel::SoundChannel;
use crate::libs::audio::sound_channel::PackKind;
use crate::libs::device_manager::DeviceManager;
use crate::state::config::AppConfig;
use rodio::{DeviceSinkBuilder, MixerDeviceSink};
use std::process;
use std::time::Instant;

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
        let device_manager = DeviceManager::new();

        let sink = open_device(&device_manager, device);
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
        let device_manager = DeviceManager::new();

        let sink = open_device(&device_manager, device);
        let mixer = sink.mixer().clone();

        self._sink = SyncedDeviceSink { _sink: sink };
        self.keyboard = SoundChannel::new(20, mixer.clone());
        self.mouse = SoundChannel::new(20, mixer);
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

fn open_device(device_manager: &DeviceManager, device_id: Option<String>) -> MixerDeviceSink {
    let builder = device_id.map_or_else(
        || DeviceSinkBuilder::from_default_device(),
        |device_id| {
            device_manager
                .get_output_device_by_id(&device_id)
                .map_or_else(
                    |error| {
                        log::error!(
                            "❌ Failed to get output device by id {}: {}",
                            device_id,
                            error
                        );
                        DeviceSinkBuilder::from_default_device()
                    },
                    |device| {
                        device.map_or_else(
                            || {
                                log::error!("❌ Failed to get output device by id {}", &device_id);
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
        .with_error_callback(|error| {
            log::error!("❌ !!!!!! Error accessing default device {}", error);
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
