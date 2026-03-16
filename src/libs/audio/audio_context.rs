use crate::libs::device_manager::DeviceManager;
use crate::state::config::AppConfig;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

static AUDIO_VOLUME: std::sync::OnceLock<Mutex<f32>> = std::sync::OnceLock::new();
static MOUSE_AUDIO_VOLUME: std::sync::OnceLock<Mutex<f32>> = std::sync::OnceLock::new();

#[derive(Clone)]
pub struct AudioContext {
    _stream: Arc<OutputStream>,
    pub(crate) stream_handle: OutputStreamHandle,
    pub(crate) keyboard_samples: Arc<Mutex<Option<(Vec<f32>, u16, u32)>>>,
    pub(crate) mouse_samples: Arc<Mutex<Option<(Vec<f32>, u16, u32)>>>,
    pub(crate) key_map: Arc<Mutex<HashMap<String, Vec<[f32; 2]>>>>,
    pub(crate) mouse_map: Arc<Mutex<HashMap<String, Vec<[f32; 2]>>>>,
    pub(crate) max_voices: usize,
    pub(crate) key_pressed: Arc<Mutex<HashMap<String, bool>>>,
    pub(crate) mouse_pressed: Arc<Mutex<HashMap<String, bool>>>,
    pub(crate) key_sinks: Arc<Mutex<HashMap<String, Sink>>>,
    pub(crate) mouse_sinks: Arc<Mutex<HashMap<String, Sink>>>,
    pub(crate) device_manager: DeviceManager,
    // Timing tracking for rapid event detection
    pub(crate) last_keyboard_sound_time: Arc<Mutex<Option<Instant>>>,
    pub(crate) last_mouse_sound_time: Arc<Mutex<Option<Instant>>>,
}

// Manual PartialEq implementation for component compatibility
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
        let config = AppConfig::load();

        // Try to use selected device or fall back to default
        let (stream, stream_handle) = match &config.selected_audio_device {
            Some(device_id) => match device_manager.get_output_device_by_id(device_id) {
                Ok(Some(device)) => match rodio::OutputStream::try_from_device(&device) {
                    Ok((stream, handle)) => (stream, handle),
                    Err(e) => {
                        log::error!(
                            "❌ Failed to create stream from selected device {}: {}",
                            device_id,
                            e
                        );
                        log::error!("🔄 Falling back to default device...");
                        rodio::OutputStream::try_default()
                            .expect("Failed to create default audio output stream")
                    }
                },
                Ok(None) => {
                    log::error!(
                        "❌ Selected audio device {} not found, using default",
                        device_id
                    );
                    rodio::OutputStream::try_default()
                        .expect("Failed to create default audio output stream")
                }
                Err(e) => {
                    log::error!("❌ Error accessing selected device {}: {}", device_id, e);
                    rodio::OutputStream::try_default()
                        .expect("Failed to create default audio output stream")
                }
            },
            None => rodio::OutputStream::try_default()
                .expect("Failed to create default audio output stream"),
        };

        let context = Self {
            _stream: Arc::new(stream),
            stream_handle,
            keyboard_samples: Arc::new(Mutex::new(None)),
            mouse_samples: Arc::new(Mutex::new(None)),
            key_map: Arc::new(Mutex::new(HashMap::new())),
            mouse_map: Arc::new(Mutex::new(HashMap::new())),
            max_voices: 20, // Increased max voices to reduce audio interruptions
            key_pressed: Arc::new(Mutex::new(HashMap::new())),
            mouse_pressed: Arc::new(Mutex::new(HashMap::new())),
            key_sinks: Arc::new(Mutex::new(HashMap::new())),
            mouse_sinks: Arc::new(Mutex::new(HashMap::new())),
            device_manager,
            last_keyboard_sound_time: Arc::new(Mutex::new(None)),
            last_mouse_sound_time: Arc::new(Mutex::new(None)),
        };
        // Initialize volume from config
        let config = AppConfig::load();
        AUDIO_VOLUME.get_or_init(|| Mutex::new(config.volume));
        MOUSE_AUDIO_VOLUME.get_or_init(|| Mutex::new(config.mouse_volume));

        // Load soundpack from config
        match super::load_soundpack_from_config(&context, false) {
            Ok(_) => {}
            Err(e) => log::error!("❌ Failed to load initial soundpack: {}", e),
        }

        context
    }
    pub fn set_volume(&self, volume: f32) {
        // Update volume for current keys only
        let key_sinks = self.key_sinks.lock().unwrap();
        for sink in key_sinks.values() {
            sink.set_volume(volume);
        }

        // Update global variable
        if let Some(global) = AUDIO_VOLUME.get() {
            let mut g = global.lock().unwrap();
            *g = volume;
        }

        // Save to config file
        let mut config = AppConfig::load();
        config.volume = volume;
        let _ = config.save();
    }

    pub fn get_volume(&self) -> f32 {
        AUDIO_VOLUME
            .get()
            .and_then(|v| v.lock().ok())
            .map(|v| *v)
            .unwrap_or(1.0)
    }

    pub fn set_mouse_volume(&self, volume: f32) {
        // Update volume for current mouse events only
        let mouse_sinks = self.mouse_sinks.lock().unwrap();
        for sink in mouse_sinks.values() {
            sink.set_volume(volume);
        }

        // Update global variable
        if let Some(global) = MOUSE_AUDIO_VOLUME.get() {
            let mut g = global.lock().unwrap();
            *g = volume;
        }

        // Save to config file
        let mut config = AppConfig::load();
        config.mouse_volume = volume;
        let _ = config.save();
    }

    pub fn get_mouse_volume(&self) -> f32 {
        MOUSE_AUDIO_VOLUME
            .get()
            .and_then(|v| v.lock().ok())
            .map(|v| *v)
            .unwrap_or(1.0)
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
            stream_handle,
            keyboard_samples: Arc::new(Mutex::new(None)),
            mouse_samples: Arc::new(Mutex::new(None)),
            key_map: Arc::new(Mutex::new(HashMap::new())),
            mouse_map: Arc::new(Mutex::new(HashMap::new())),
            max_voices: 20, // Increased max voices to reduce audio interruptions
            key_pressed: Arc::new(Mutex::new(HashMap::new())),
            mouse_pressed: Arc::new(Mutex::new(HashMap::new())),
            key_sinks: Arc::new(Mutex::new(HashMap::new())),
            mouse_sinks: Arc::new(Mutex::new(HashMap::new())),
            device_manager,
            last_keyboard_sound_time: Arc::new(Mutex::new(None)),
            last_mouse_sound_time: Arc::new(Mutex::new(None)),
        };

        // Initialize volume from config
        let config = AppConfig::load();
        AUDIO_VOLUME.get_or_init(|| Mutex::new(config.volume));
        MOUSE_AUDIO_VOLUME.get_or_init(|| Mutex::new(config.mouse_volume)); // Load soundpack from config
        match super::load_soundpack_from_config(&context, false) {
            Ok(_) => {}
            Err(e) => log::error!("❌ Failed to load initial soundpack: {}", e),
        }

        Ok(context)
    }

    pub fn get_current_device_info(&self) -> Option<String> {
        let config = AppConfig::load();
        config.selected_audio_device
    }

    pub fn test_current_device(&self) -> bool {
        let config = AppConfig::load();
        match &config.selected_audio_device {
            Some(device_id) => self
                .device_manager
                .test_output_device(device_id)
                .unwrap_or(false),
            None => true, // Default device is always considered available
        }
    }

    pub fn update_keyboard_context(
        &self,
        samples: (Vec<f32>, u16, u32), // (samples, channels, sample_rate)
        key_mappings: std::collections::HashMap<String, Vec<(f64, f64)>>,
    ) -> Result<(), String> {
        let (audio_samples, channels, sample_rate) = samples;
        let sample_count = audio_samples.len();
        let key_mapping_count = key_mappings.len();

        // Update keyboard samples
        if let Ok(mut cached) = self.keyboard_samples.lock() {
            *cached = Some((audio_samples, channels, sample_rate));
            log::info!("🎹 Updated keyboard samples: {} samples", sample_count);
        } else {
            return Err("Failed to acquire lock on keyboard_samples".to_string());
        }

        // Update key mappings
        if let Ok(mut key_map) = self.key_map.lock() {
            let old_count = key_map.len();
            key_map.clear();

            for (key, mappings) in key_mappings {
                let converted_mappings: Vec<[f32; 2]> = mappings
                    .into_iter()
                    .map(|(start, end)| [start as f32, end as f32])
                    .collect();
                key_map.insert(key.clone(), converted_mappings);
            }

            log::info!(
                "🗝️ Updated key mappings: {} -> {} keys",
                old_count,
                key_map.len()
            );
        } else {
            return Err("Failed to acquire lock on key_map".to_string());
        }

        // Clear active keyboard audio state
        if let Ok(mut sinks) = self.key_sinks.lock() {
            let old_sinks = sinks.len();
            sinks.clear();
            if old_sinks > 0 {
                log::info!("🔇 Cleared {} active key sinks", old_sinks);
            }
        }

        if let Ok(mut pressed) = self.key_pressed.lock() {
            let old_pressed = pressed.len();
            pressed.clear();
            if old_pressed > 0 {
                log::info!("⌨️ Cleared {} pressed keys", old_pressed);
            }
        }

        log::info!(
            "✅ Successfully loaded {} keyboard sounds",
            key_mapping_count
        );
        Ok(())
    }

    pub fn update_mouse_context(
        &self,
        samples: (Vec<f32>, u16, u32), // (samples, channels, sample_rate)
        mouse_mappings: std::collections::HashMap<String, Vec<(f64, f64)>>,
    ) -> Result<(), String> {
        let (audio_samples, channels, sample_rate) = samples;
        let sample_count = audio_samples.len();
        let mouse_mapping_count = mouse_mappings.len();

        // Update mouse samples
        if let Ok(mut cached) = self.mouse_samples.lock() {
            *cached = Some((audio_samples, channels, sample_rate));
            log::info!("🖱️ Updated mouse samples: {} samples", sample_count);
        } else {
            return Err("Failed to acquire lock on mouse_samples".to_string());
        }

        // Update mouse mappings
        if let Ok(mut mouse_map) = self.mouse_map.lock() {
            let old_count = mouse_map.len();
            mouse_map.clear();

            for (button, mappings) in mouse_mappings {
                let converted_mappings: Vec<[f32; 2]> = mappings
                    .into_iter()
                    .map(|(start, end)| [start as f32, end as f32])
                    .collect();
                mouse_map.insert(button.clone(), converted_mappings);
            }

            log::info!(
                "🖱️ Updated mouse mappings: {} -> {} buttons",
                old_count,
                mouse_map.len()
            );
        } else {
            return Err("Failed to acquire lock on mouse_map".to_string());
        }

        // Clear active mouse audio state
        if let Ok(mut mouse_sinks) = self.mouse_sinks.lock() {
            let old_sinks = mouse_sinks.len();
            mouse_sinks.clear();
            if old_sinks > 0 {
                log::info!("🔇 Cleared {} active mouse sinks", old_sinks);
            }
        }

        if let Ok(mut mouse_pressed) = self.mouse_pressed.lock() {
            let old_pressed = mouse_pressed.len();
            mouse_pressed.clear();
            if old_pressed > 0 {
                log::info!("🖱️ Cleared {} pressed mouse buttons", old_pressed);
            }
        }

        log::info!(
            "✅ Successfully loaded mouse soundpack ({} mouse mappings) - Memory properly cleaned",
            mouse_mapping_count
        );
        Ok(())
    }
}
