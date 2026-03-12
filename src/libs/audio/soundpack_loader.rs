use std::path::{Path, PathBuf};

use crate::libs::soundpack::cache::SoundpackMetadata;
use crate::libs::soundpack::cache::{capture_soundpack_loading_error, load_cache, save_cache};
use crate::libs::soundpack::format::{SoundPack, SoundpackType};
use crate::state::config::AppConfig;
use crate::state::paths;

use super::audio_context::AudioContext;

pub fn load_soundpack(context: &AudioContext) -> Result<(), String> {
    let config = AppConfig::load();
    // Load both keyboard and mouse soundpacks if they are selected
    if !config.keyboard_soundpack.is_empty() {
        load_keyboard_soundpack(context, &config.keyboard_soundpack)?;
    }
    if !config.mouse_soundpack.is_empty() {
        load_mouse_soundpack(context, &config.mouse_soundpack)?;
    }
    Ok(())
}

pub fn load_keyboard_soundpack(context: &AudioContext, soundpack_id: &str) -> Result<(), String> {
    load_keyboard_soundpack_with_cache_control(context, soundpack_id, true)
}

pub fn load_keyboard_soundpack_with_cache_control(
    context: &AudioContext,
    soundpack_id: &str,
    update_cache_on_error: bool,
) -> Result<(), String> {
    if soundpack_id.is_empty() {
        return Err("Soundpack id is empty!".into());
    }

    log::info!("🎹 Loading keyboard soundpack: {}", soundpack_id);
    match load_keyboard_soundpack_optimized(context, soundpack_id) {
        Ok(()) => Ok(()),
        Err(e) => {
            capture_soundpack_loading_error(soundpack_id, SoundpackType::Keyboard, &e);
            Err(e)
        }
    }
}

pub fn load_mouse_soundpack(context: &AudioContext, soundpack_id: &str) -> Result<(), String> {
    load_mouse_soundpack_with_cache_control(context, soundpack_id, true)
}

pub fn load_mouse_soundpack_with_cache_control(
    context: &AudioContext,
    soundpack_id: &str,
    update_cache_on_error: bool,
) -> Result<(), String> {
    if soundpack_id.is_empty() {
        return Err("Soundpack id is empty!".into());
    }

    log::info!("🖱️ Loading mouse soundpack: {}", soundpack_id);
    match load_mouse_soundpack_optimized(context, soundpack_id) {
        Ok(()) => Ok(()),
        Err(e) => {
            if update_cache_on_error {
                capture_soundpack_loading_error(soundpack_id, SoundpackType::Mouse, &e);
            }
            Err(e)
        }
    }
}

fn load_audio_file(
    soundpack_path: &str,
    soundpack: &SoundPack,
) -> Result<(Vec<f32>, u16, u32), String> {
    let sound_file_path = soundpack
        .audio_file
        .as_ref()
        .map(|src| format!("{}/{}", soundpack_path, src.trim_start_matches("./")))
        .ok_or_else(|| "No audio_file field in soundpack config".to_string())?;

    if !std::path::Path::new(&sound_file_path).exists() {
        return Err(format!("Sound file not found: {}", sound_file_path));
    }

    // Use Symphonia for audio loading instead of Rodio
    match load_audio_with_symphonia(&sound_file_path) {
        Ok((samples, channels, sample_rate)) => Ok((samples, channels, sample_rate)),
        Err(e) => Err(format!("Failed to load audio: {}", e)),
    }
}

/// Load audio file using Symphonia for consistent duration detection
fn load_audio_with_symphonia(file_path: &str) -> Result<(Vec<f32>, u16, u32), String> {
    use std::fs::File;
    use symphonia::core::audio::{AudioBufferRef, Signal};
    use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    // First, check if file exists and has content
    let metadata =
        std::fs::metadata(file_path).map_err(|e| format!("Failed to get file metadata: {}", e))?;
    if metadata.len() == 0 {
        return Err(format!("Audio file is empty: {}", file_path));
    }

    let file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = std::path::Path::new(file_path).extension() {
        if let Some(ext_str) = extension.to_str() {
            hint.with_extension(ext_str);
        }
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| {
            format!(
                "Failed to probe format for '{}': {} (file size: {} bytes)",
                file_path,
                e,
                metadata.len()
            )
        })?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No supported audio tracks found")?;

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    let track_id = track.id;
    let mut samples = Vec::new();
    let mut sample_rate = 44100u32;
    let mut channels = 2u16;

    // Decode audio packets
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => {
                break;
            } // End of stream
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                if samples.is_empty() {
                    // Get format info from first decoded buffer
                    sample_rate = decoded.spec().rate;
                    channels = decoded.spec().channels.count() as u16;
                } // Convert audio buffer to f32 samples
                match decoded {
                    AudioBufferRef::F32(buf) => {
                        if channels == 1 {
                            // Mono audio
                            for &sample in buf.chan(0) {
                                samples.push(sample);
                            }
                        } else {
                            // Stereo audio - interleave samples correctly
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(*left);
                                samples.push(*right);
                            }
                        }
                    }
                    AudioBufferRef::S32(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push((sample as f32) / (i32::MAX as f32));
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push((*left as f32) / (i32::MAX as f32));
                                samples.push((*right as f32) / (i32::MAX as f32));
                            }
                        }
                    }
                    AudioBufferRef::S16(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push((sample as f32) / (i16::MAX as f32));
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push((*left as f32) / (i16::MAX as f32));
                                samples.push((*right as f32) / (i16::MAX as f32));
                            }
                        }
                    }
                    AudioBufferRef::U32(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(
                                    ((sample as f32) - (u32::MAX as f32) / 2.0)
                                        / ((u32::MAX as f32) / 2.0),
                                );
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(
                                    ((*left as f32) - (u32::MAX as f32) / 2.0)
                                        / ((u32::MAX as f32) / 2.0),
                                );
                                samples.push(
                                    ((*right as f32) - (u32::MAX as f32) / 2.0)
                                        / ((u32::MAX as f32) / 2.0),
                                );
                            }
                        }
                    }
                    AudioBufferRef::U16(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(
                                    ((sample as f32) - (u16::MAX as f32) / 2.0)
                                        / ((u16::MAX as f32) / 2.0),
                                );
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(
                                    ((*left as f32) - (u16::MAX as f32) / 2.0)
                                        / ((u16::MAX as f32) / 2.0),
                                );
                                samples.push(
                                    ((*right as f32) - (u16::MAX as f32) / 2.0)
                                        / ((u16::MAX as f32) / 2.0),
                                );
                            }
                        }
                    }
                    AudioBufferRef::U8(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(((sample as f32) - 128.0) / 128.0);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(((*left as f32) - 128.0) / 128.0);
                                samples.push(((*right as f32) - 128.0) / 128.0);
                            }
                        }
                    }
                    AudioBufferRef::S8(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push((sample as f32) / (i8::MAX as f32));
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push((*left as f32) / (i8::MAX as f32));
                                samples.push((*right as f32) / (i8::MAX as f32));
                            }
                        }
                    }
                    AudioBufferRef::F64(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(sample as f32);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(*left as f32);
                                samples.push(*right as f32);
                            }
                        }
                    }
                    AudioBufferRef::U24(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                let sample_f32 = ((sample.inner() as f32) - 8388608.0) / 8388608.0; // 2^23
                                samples.push(sample_f32);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                let left_f32 = ((left.inner() as f32) - 8388608.0) / 8388608.0;
                                let right_f32 = ((right.inner() as f32) - 8388608.0) / 8388608.0;
                                samples.push(left_f32);
                                samples.push(right_f32);
                            }
                        }
                    }
                    AudioBufferRef::S24(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                let sample_f32 = (sample.inner() as f32) / 8388607.0; // 2^23 - 1
                                samples.push(sample_f32);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 {
                                buf.chan(1)
                            } else {
                                buf.chan(0)
                            };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                let left_f32 = (left.inner() as f32) / 8388607.0;
                                let right_f32 = (right.inner() as f32) / 8388607.0;
                                samples.push(left_f32);
                                samples.push(right_f32);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("⚠️[DEBUG] Decode error (continuing): {}", e);
                continue;
            }
        }
    }

    if samples.is_empty() {
        return Err("No audio data decoded".to_string());
    }

    Ok((samples, channels, sample_rate))
}

/// Direct keyboard soundpack loading
pub fn load_keyboard_soundpack_optimized(
    context: &AudioContext,
    soundpack_id: &str,
) -> Result<(), String> {
    log::info!("📂 Direct loading keyboard soundpack: {}", soundpack_id);

    // Load soundpack directly from filesystem
    let soundpack_dir = paths::soundpacks::find_soundpack_dir(soundpack_id, false);
    let config_path = PathBuf::from(&soundpack_dir).join("config.json");

    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config at {}: {}", config_path.display(), e))?; // Only load V2 config format - V1 configs must be converted first
    let mut soundpack: SoundPack = serde_json::from_str(&config_content).map_err(|e| {
        // Check if this might be a V1 config
        if config_content.contains("\"key_define_type\"") || config_content.contains("\"defines\"") {
            format!("This appears to be a V1 soundpack config. Please convert it to V2 format first using the refresh/convert function. Parse error: {}", e)
        } else {
            format!("Failed to parse V2 soundpack config: {}", e)
        }
    })?;

    soundpack.soundpack_type = SoundpackType::Keyboard;

    // Load audio samples directly from file
    let samples = load_audio_file(&soundpack_dir, &soundpack)?;

    // Create key mappings (only for keyboard soundpacks)
    let key_mappings = create_key_mappings(&soundpack, &samples.0); // Update audio context with keyboard data
    context.update_keyboard_context(samples, key_mappings)?;

    // Update metadata cache - create metadata with no error since loading succeeded
    let mut cache = load_cache();
    let metadata = create_soundpack_metadata(&soundpack_dir, &soundpack);
    cache.add_soundpack(metadata);
    save_cache(&cache);

    log::info!(
        "✅ Successfully loaded keyboard soundpack: {} (direct from files)",
        soundpack.name
    );
    Ok(())
}

/// Direct mouse soundpack loading
pub fn load_mouse_soundpack_optimized(
    context: &AudioContext,
    soundpack_id: &str,
) -> Result<(), String> {
    log::info!("📂 Direct loading mouse soundpack: {}", soundpack_id);

    // Load soundpack directly from filesystem
    let soundpack_dir = paths::soundpacks::find_soundpack_dir(soundpack_id, true);
    let config_path = PathBuf::from(&soundpack_dir).join("config.json");

    // Load config.json
    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let mut soundpack: SoundPack = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;

    soundpack.soundpack_type = SoundpackType::Mouse;

    // Load audio samples directly from file
    let samples = load_audio_file(&soundpack_dir, &soundpack)?;

    // Create mouse mappings (only for mouse soundpacks)
    let mouse_mappings = create_mouse_mappings(&soundpack, &samples.0); // Update audio context with mouse data
    context.update_mouse_context(samples, mouse_mappings)?;

    // Update metadata cache - create metadata with no error since loading succeeded
    let mut cache = load_cache();
    let metadata = create_soundpack_metadata(&soundpack_dir, &soundpack);
    cache.add_soundpack(metadata);
    save_cache(&cache);

    log::info!(
        "✅ Successfully loaded mouse soundpack: {} (direct from files)",
        soundpack.name
    );
    Ok(())
}

fn create_soundpack_metadata(config_path: &str, soundpack: &SoundPack) -> SoundpackMetadata {
    SoundpackMetadata {
        id: soundpack.id.clone(), // Use calculated relative path ID instead of config ID
        name: soundpack.name.clone(),
        author: soundpack.author.clone(),
        tags: soundpack.tags.clone().unwrap_or_default(),
        icon: {
            // Generate dynamic URL for icon instead of base64 conversion
            if let Some(icon_filename) = &soundpack.icon {
                let mut icon_path = PathBuf::from(config_path);
                icon_path.set_file_name(icon_filename);
                if icon_path.exists() {
                    // Create dynamic URL that will be served by the asset handler
                    Some(format!(
                        "/soundpack-images/{}/{}",
                        soundpack.id, icon_filename
                    ))
                } else {
                    Some(String::new()) // Empty string if icon file not found
                }
            } else {
                Some(String::new()) // Empty string if no icon specified
            }
        },
        soundpack_type: soundpack.soundpack_type, // Include the mouse field
        config_path: config_path.to_string(),     // Use the derived folder path for loading
    }
}

fn create_key_mappings(
    soundpack: &SoundPack,
    _samples: &[f32],
) -> std::collections::HashMap<String, Vec<(f64, f64)>> {
    let mut key_mappings = std::collections::HashMap::new(); // For keyboard soundpacks, use the definitions field for keyboard mappings
    // For mouse soundpacks, return empty key mappings
    if soundpack.soundpack_type == SoundpackType::Keyboard {
        for (key, key_def) in &soundpack.definitions {
            // Convert KeyDefinition timing to Vec<(f64, f64)>
            let converted_mappings: Vec<(f64, f64)> = key_def
                .timing
                .iter()
                .map(|pair| (pair[0] as f64, pair[1] as f64))
                .collect();
            key_mappings.insert(key.clone(), converted_mappings);
        }
    }

    key_mappings
}

fn create_mouse_mappings(
    soundpack: &SoundPack,
    _samples: &[f32],
) -> std::collections::HashMap<String, Vec<(f64, f64)>> {
    let mut mouse_mappings = std::collections::HashMap::new(); // For mouse soundpacks, use the definitions field directly
    if soundpack.soundpack_type == SoundpackType::Mouse {
        // This is a mouse soundpack, use definitions field for mouse mappings
        for (button, key_def) in &soundpack.definitions {
            // Convert KeyDefinition timing to Vec<(f64, f64)>
            let converted_mappings: Vec<(f64, f64)> = key_def
                .timing
                .iter()
                .map(|pair| (pair[0] as f64, pair[1] as f64))
                .collect();
            mouse_mappings.insert(button.clone(), converted_mappings);
        }
    } else {
        // This is a keyboard soundpack, create default mouse mappings from keyboard sounds
        log::info!(
            "🖱️ No mouse definitions found, creating default mouse mappings from keyboard sounds"
        );

        // Use common keyboard keys as fallback for mouse buttons
        let fallback_mappings = [
            ("MouseLeft", "Space"),
            ("MouseRight", "Enter"),
            ("MouseMiddle", "Tab"),
            ("MouseWheelUp", "ArrowUp"),
            ("MouseWheelDown", "ArrowDown"),
            ("Mouse4", "Backspace"),
            ("Mouse5", "Delete"),
            ("Mouse6", "Home"),
            ("Mouse7", "End"),
            ("Mouse8", "PageUp"),
        ];
        for (mouse_button, keyboard_key) in &fallback_mappings {
            if let Some(key_def) = soundpack.definitions.get(*keyboard_key) {
                let converted_mappings: Vec<(f64, f64)> = key_def
                    .timing
                    .iter()
                    .map(|pair| (pair[0] as f64, pair[1] as f64))
                    .collect();
                mouse_mappings.insert(mouse_button.to_string(), converted_mappings);
            }
        }
    }

    mouse_mappings
}
