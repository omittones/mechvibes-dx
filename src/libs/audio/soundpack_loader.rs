use crate::libs::audio::audio_context::AudioContext;
use crate::libs::soundpack::cache::{
    SoundpackRef, SoundpackType, load_cache, metadata_from_soundpack, save_cache,
};
use crate::libs::soundpack::format::{SoundPack, load_and_migrate_soundpack};
use crate::state::config::AppConfig;

/// Reload the current soundpacks from configuration.
/// If a soundpack fails to load, clear its selection and optionally save config.
/// Returns Ok(()) if succeeded, Err if any selected soundpack fails to load (after clear/reset).
pub fn load_soundpack(audio_ctx: &AudioContext, update_config: bool) -> Result<(), String> {
    let mut config = AppConfig::load();
    let mut config_changed = false;
    let mut last_err: Option<String> = None;

    // Load keyboard soundpack
    if !config.keyboard_soundpack.is_empty() {
        match SoundpackRef::parse(&config.keyboard_soundpack)
            .ok()
            .and_then(|id| load_soundpack_file(audio_ctx, &id).ok())
        {
            Some(_) => log::debug!(
                "✅ Keyboard soundpack '{}' reloaded successfully",
                config.keyboard_soundpack
            ),
            None => {
                let err_msg = format!(
                    "❌ Failed to reload keyboard soundpack '{}'. Clearing selection.",
                    config.keyboard_soundpack
                );
                log::error!("{}", err_msg);
                last_err = Some(err_msg);
                config.keyboard_soundpack = "".to_string();
                config_changed = true;
            }
        }
    }

    // Load mouse soundpack
    if !config.mouse_soundpack.is_empty() {
        match SoundpackRef::parse(&config.mouse_soundpack)
            .ok()
            .and_then(|id| load_soundpack_file(audio_ctx, &id).ok())
        {
            Some(_) => log::debug!(
                "✅ Mouse soundpack '{}' reloaded successfully",
                config.mouse_soundpack
            ),
            None => {
                let err_msg = format!(
                    "❌ Failed to reload mouse soundpack '{}'. Clearing selection.",
                    config.mouse_soundpack
                );
                log::error!("{}", err_msg);
                last_err = Some(err_msg);
                config.mouse_soundpack = "".to_string();
                config_changed = true;
            }
        }
    }

    // Save config if any changes were made and requested by caller
    if config_changed && update_config {
        let _ = config.save();
        log::debug!("💾 Config updated due to failed soundpack loads");
    }

    // If there was any error, return error, otherwise Ok
    if let Some(err) = last_err {
        Err(err)
    } else {
        Ok(())
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

pub fn load_soundpack_file(context: &AudioContext, id: &SoundpackRef) -> Result<(), String> {
    log::info!("📂 Direct loading soundpack: {}", id);

    // Load soundpack directly from filesystem
    let soundpack_dir = id.to_soundpack_path().to_string_lossy().to_string();

    let (config_path, soundpack) = load_and_migrate_soundpack(&soundpack_dir)?;

    // Load audio samples directly from file
    let samples = load_audio_file(&soundpack_dir, &soundpack)?;

    match id.soundpack_type {
        SoundpackType::Mouse => {
            let mouse_mappings = create_mouse_mappings(&soundpack, &samples.0); // Update audio context with mouse data
            context.update_mouse_context(samples, mouse_mappings)?;
        }
        SoundpackType::Keyboard => {
            let key_mappings = create_key_mappings(&soundpack, &samples.0); // Update audio context with keyboard data
            context.update_keyboard_context(samples, key_mappings)?;
        }
    }

    // Update metadata cache - create metadata with no error since loading succeeded
    let mut cache = load_cache();
    let metadata = metadata_from_soundpack(
        &config_path,
        &soundpack,
        id.is_builtin,
        id.soundpack_type == SoundpackType::Mouse,
    );
    cache.add_soundpack(metadata);
    save_cache(&cache);

    log::info!(
        "✅ Successfully loaded mouse soundpack: {} (direct from files)",
        soundpack.name
    );

    Ok(())
}

fn create_key_mappings(
    soundpack: &SoundPack,
    _samples: &[f32],
) -> std::collections::HashMap<String, Vec<(f64, f64)>> {
    // For keyboard soundpacks, use the definitions field for keyboard mappings
    let mut key_mappings = std::collections::HashMap::new();

    for (key, key_def) in &soundpack.definitions {
        // Convert KeyDefinition timing to Vec<(f64, f64)>
        let converted_mappings: Vec<(f64, f64)> = key_def
            .timing
            .iter()
            .map(|pair| (pair[0] as f64, pair[1] as f64))
            .collect();
        key_mappings.insert(key.clone(), converted_mappings);
    }

    key_mappings
}

fn create_mouse_mappings(
    soundpack: &SoundPack,
    _samples: &[f32],
) -> std::collections::HashMap<String, Vec<(f64, f64)>> {
    let mut mouse_mappings = std::collections::HashMap::new();

    // Add fallback: if any defined mouse button is missing, copy from the paired keyboard key
    let mouse_buttons_with_fallback = [
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

    for (button, fallback) in mouse_buttons_with_fallback {
        let key_def = if let Some(key_def) = soundpack.definitions.get(button) {
            key_def
        } else if let Some(key_def) = soundpack.definitions.get(fallback) {
            key_def
        } else {
            continue;
        };

        let converted_mappings: Vec<(f64, f64)> = key_def
            .timing
            .iter()
            .map(|pair| (pair[0] as f64, pair[1] as f64))
            .collect();
        mouse_mappings.insert(button.to_string(), converted_mappings);
    }

    mouse_mappings
}
