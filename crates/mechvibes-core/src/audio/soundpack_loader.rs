use crate::audio::audio_context::AudioContext;
use crate::soundpack::cache::{
    SoundpackRef, SoundpackType, load_cache, metadata_from_soundpack, save_cache,
};
use crate::soundpack::format::{SoundPack, load_and_migrate_soundpack};
use crate::state::config::AppConfig;

/// Reload the current soundpacks from configuration.
pub fn load_soundpack_from_config(
    audio_ctx: &mut AudioContext,
    update_config: bool,
) -> Result<(), String> {
    let mut last_err: Option<String> = None;

    let config = AppConfig::get();
    let clear_keyboard_soundpack = if !config.keyboard_soundpack.is_empty() {
        match SoundpackRef::parse(&config.keyboard_soundpack)
            .map(|id| load_soundpack_file(audio_ctx, &id))
        {
            Ok(_) => {
                log::debug!(
                    "✅ Keyboard soundpack '{}' reloaded successfully",
                    config.keyboard_soundpack
                );
                false
            }
            Err(e) => {
                log::error!(
                    "❌ Failed to reload keyboard soundpack '{}'. Clearing selection.",
                    config.keyboard_soundpack
                );
                last_err = Some(e.to_string());
                true
            }
        }
    } else {
        true
    };

    let clear_mouse_soundpack = if !config.mouse_soundpack.is_empty() {
        match SoundpackRef::parse(&config.mouse_soundpack)
            .map(|id| load_soundpack_file(audio_ctx, &id).ok())
        {
            Ok(_) => {
                log::debug!(
                    "✅ Mouse soundpack '{}' reloaded successfully",
                    config.mouse_soundpack
                );
                false
            }
            Err(e) => {
                log::error!(
                    "❌ Failed to reload mouse soundpack '{}'. Clearing selection.",
                    config.mouse_soundpack
                );
                last_err = Some(e.to_string());
                true
            }
        }
    } else {
        true
    };

    drop(config);

    if update_config && (clear_keyboard_soundpack || clear_mouse_soundpack) {
        AppConfig::update(|config| {
            if clear_keyboard_soundpack {
                config.keyboard_soundpack = "".to_string()
            };
            if clear_mouse_soundpack {
                config.mouse_soundpack = "".to_string()
            }
        });
        log::debug!("💾 Config updated due to failed soundpack loads");
    }

    if let Some(err) = last_err {
        Err(err)
    } else {
        Ok(())
    }
}

pub fn load_soundpack_file(context: &mut AudioContext, id: &SoundpackRef) -> Result<(), String> {
    log::info!("📂 Direct loading soundpack: {}", id);

    let soundpack_dir = id.to_soundpack_path().to_string_lossy().to_string();
    let soundpack = load_and_migrate_soundpack(&soundpack_dir)?;
    let samples = load_audio_file(&soundpack_dir, &soundpack)?;

    match id.soundpack_type {
        SoundpackType::Mouse => {
            let mouse_mappings = create_mouse_mappings(&soundpack, &samples.0);
            context.load_mouse_mappings(samples, mouse_mappings)?;
        }
        SoundpackType::Keyboard => {
            let key_mappings = create_key_mappings(&soundpack, &samples.0);
            context.load_keyboard_mappings(samples, key_mappings)?;
        }
    }

    let mut cache = load_cache();
    let metadata = metadata_from_soundpack(id.clone(), &soundpack);
    cache.add_soundpack(metadata);
    save_cache(&cache);

    log::info!(
        "✅ Successfully loaded soundpack: {} (direct from files)",
        soundpack.name
    );

    Ok(())
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

    match load_audio_with_symphonia(&sound_file_path) {
        Ok((samples, channels, sample_rate)) => Ok((samples, channels, sample_rate)),
        Err(e) => Err(format!("Failed to load audio: {}", e)),
    }
}

fn load_audio_with_symphonia(file_path: &str) -> Result<(Vec<f32>, u16, u32), String> {
    use std::fs::File;
    use symphonia::core::audio::{AudioBufferRef, Signal};
    use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

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

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                if samples.is_empty() {
                    sample_rate = decoded.spec().rate;
                    channels = decoded.spec().channels.count() as u16;
                }
                match decoded {
                    AudioBufferRef::F32(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) {
                                samples.push(sample);
                            }
                        } else {
                            let left_chan = buf.chan(0);
                            let right_chan = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (left, right) in left_chan.iter().zip(right_chan.iter()) {
                                samples.push(*left);
                                samples.push(*right);
                            }
                        }
                    }
                    AudioBufferRef::S32(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push((sample as f32) / (i32::MAX as f32)); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push((*l as f32) / (i32::MAX as f32)); samples.push((*r as f32) / (i32::MAX as f32)); }
                        }
                    }
                    AudioBufferRef::S16(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push((sample as f32) / (i16::MAX as f32)); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push((*l as f32) / (i16::MAX as f32)); samples.push((*r as f32) / (i16::MAX as f32)); }
                        }
                    }
                    AudioBufferRef::U32(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push(((sample as f32) - (u32::MAX as f32) / 2.0) / ((u32::MAX as f32) / 2.0)); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push(((*l as f32) - (u32::MAX as f32)/2.0)/((u32::MAX as f32)/2.0)); samples.push(((*r as f32) - (u32::MAX as f32)/2.0)/((u32::MAX as f32)/2.0)); }
                        }
                    }
                    AudioBufferRef::U16(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push(((sample as f32) - (u16::MAX as f32)/2.0)/((u16::MAX as f32)/2.0)); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push(((*l as f32) - (u16::MAX as f32)/2.0)/((u16::MAX as f32)/2.0)); samples.push(((*r as f32) - (u16::MAX as f32)/2.0)/((u16::MAX as f32)/2.0)); }
                        }
                    }
                    AudioBufferRef::U8(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push(((sample as f32) - 128.0) / 128.0); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push(((*l as f32) - 128.0)/128.0); samples.push(((*r as f32) - 128.0)/128.0); }
                        }
                    }
                    AudioBufferRef::S8(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push((sample as f32) / (i8::MAX as f32)); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push((*l as f32)/(i8::MAX as f32)); samples.push((*r as f32)/(i8::MAX as f32)); }
                        }
                    }
                    AudioBufferRef::F64(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push(sample as f32); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push(*l as f32); samples.push(*r as f32); }
                        }
                    }
                    AudioBufferRef::U24(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push(((sample.inner() as f32) - 8388608.0) / 8388608.0); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push(((l.inner() as f32) - 8388608.0)/8388608.0); samples.push(((r.inner() as f32) - 8388608.0)/8388608.0); }
                        }
                    }
                    AudioBufferRef::S24(buf) => {
                        if channels == 1 {
                            for &sample in buf.chan(0) { samples.push((sample.inner() as f32) / 8388607.0); }
                        } else {
                            let left = buf.chan(0); let right = if buf.spec().channels.count() > 1 { buf.chan(1) } else { buf.chan(0) };
                            for (l, r) in left.iter().zip(right.iter()) { samples.push((l.inner() as f32)/8388607.0); samples.push((r.inner() as f32)/8388607.0); }
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

fn create_key_mappings(
    soundpack: &SoundPack,
    _samples: &[f32],
) -> std::collections::HashMap<String, Vec<(f64, f64)>> {
    let mut key_mappings = std::collections::HashMap::new();

    for (key, key_def) in &soundpack.definitions {
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
