use crate::libs::audio::sound_channel::PackKind;
use crate::libs::device_manager::DeviceManager;
use crate::state::config::AppConfig;
use rodio::OutputStream;
use std::sync::Arc;

use super::sound_channel::SoundChannel;

#[derive(Clone)]
pub struct AudioContext {
    _stream: Arc<OutputStream>,
    pub(crate) keyboard: SoundChannel,
    pub(crate) mouse: SoundChannel,
    pub(crate) device_manager: DeviceManager,
}

// Safety: OutputStream contains a cpal::Stream with a raw pointer kept alive purely for RAII.
// All cross-thread state (samples, key maps, sinks) is behind Arc<Mutex<...>>, and
// OutputStreamHandle is internally Arc-based. We never access _stream across threads.
unsafe impl Send for AudioContext {}
unsafe impl Sync for AudioContext {}

impl PartialEq for AudioContext {
    fn eq(&self, other: &Self) -> bool {
        // For component props, we consider AudioContext instances equal if they're the same Arc
        Arc::ptr_eq(&self._stream, &other._stream)
    }
}

impl AudioContext {
    pub fn new() -> Self {
        // Initialize device manager
        let device_manager = DeviceManager::new();
        let config = AppConfig::get();

        // Try to use selected device or fall back to default
        let (stream, stream_handle) = match &config.selected_audio_device {
            Some(device_id) => match device_manager.get_output_device_by_id(device_id) {
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
                    OutputStream::try_default()
                        .expect("Failed to create default audio output stream")
                }
                Err(e) => {
                    log::error!("❌ Error accessing selected device {}: {}", device_id, e);
                    OutputStream::try_default()
                        .expect("Failed to create default audio output stream")
                }
            },
            None => {
                OutputStream::try_default().expect("Failed to create default audio output stream")
            }
        };

        let context = Self {
            _stream: Arc::new(stream),
            keyboard: SoundChannel::new(20, stream_handle.clone()), // Increased max voices to reduce audio interruptions
            mouse: SoundChannel::new(20, stream_handle.clone()),
            device_manager,
        };

        // Load soundpack from config
        match super::load_soundpack_from_config(&context, false) {
            Ok(_) => {}
            Err(e) => log::error!("❌ Failed to load initial soundpack: {}", e),
        }

        context
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

    pub fn create_with_device(device_id: Option<String>) -> Result<Self, String> {
        // Initialize device manager
        let device_manager = DeviceManager::new();

        // Create stream with selected device
        let (stream, stream_handle) = match &device_id {
            Some(id) => match device_manager.get_output_device_by_id(id) {
                Ok(Some(device)) => match rodio::OutputStream::try_from_device(&device) {
                    Ok((stream, handle)) => (stream, handle),
                    Err(e) => {
                        log::error!("❌ Failed to create stream from device {}: {}", id, e);
                        return Err(format!("Failed to use device: {}", e));
                    }
                },
                Ok(None) => {
                    log::error!("❌ Device {} not found", id);
                    return Err(format!("Device not found: {}", id));
                }
                Err(e) => {
                    log::error!("❌ Error accessing device {}: {}", id, e);
                    return Err(format!("Error accessing device: {}", e));
                }
            },
            None => rodio::OutputStream::try_default()
                .map_err(|e| format!("Failed to create default stream: {}", e))?,
        };

        let context = Self {
            _stream: Arc::new(stream),
            keyboard: SoundChannel::new(20, stream_handle.clone()),
            mouse: SoundChannel::new(20, stream_handle.clone()),
            device_manager,
        };

        match super::load_soundpack_from_config(&context, false) {
            Ok(_) => {}
            Err(e) => log::error!("❌ Failed to load initial soundpack: {}", e),
        }

        Ok(context)
    }

    pub fn get_current_device_info(&self) -> Option<String> {
        let config = AppConfig::get();
        config.selected_audio_device.clone()
    }

    pub fn test_current_device(&self) -> bool {
        let config = AppConfig::get();
        match &config.selected_audio_device {
            Some(device_id) => self
                .device_manager
                .test_output_device(device_id)
                .unwrap_or(false),
            None => true, // Default device is always considered available
        }
    }

    pub fn load_keyboard_mappings(
        &self,
        samples: (Vec<f32>, u16, u32), // (samples, channels, sample_rate)
        mappings: std::collections::HashMap<String, Vec<(f64, f64)>>,
    ) -> Result<(), String> {
        self.keyboard
            .load_mappings(samples, mappings, PackKind::Keyboard)
    }

    pub fn load_mouse_mappings(
        &self,
        samples: (Vec<f32>, u16, u32), // (samples, channels, sample_rate)
        mappings: std::collections::HashMap<String, Vec<(f64, f64)>>,
    ) -> Result<(), String> {
        self.mouse.load_mappings(samples, mappings, PackKind::Mouse)
    }
}
