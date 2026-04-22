use rodio::Player;
use rodio::buffer::SamplesBuffer;
use rodio::mixer::Mixer;
use std::collections::HashMap;
use std::num::NonZero;
use std::time::Instant;

pub(crate) type PcmBuffer = (Vec<f32>, u16, u32);

/// PCM, per-input timing ranges (ms), press state, and active players for one input domain (keyboard or mouse).
pub(crate) struct SoundChannel {
    max_voices: usize,
    mixer: Mixer,
    samples: Option<PcmBuffer>,
    time_map: HashMap<String, Vec<[f32; 2]>>,
    pressed: HashMap<String, bool>,
    players: HashMap<String, Player>,
}

#[derive(Clone, Copy)]
pub(crate) enum PackKind {
    Keyboard,
    Mouse,
}

impl SoundChannel {
    pub fn new(max_voices: usize, mixer: Mixer) -> Self {
        Self {
            max_voices,
            mixer,
            samples: None,
            time_map: HashMap::new(),
            pressed: HashMap::new(),
            players: HashMap::new(),
        }
    }

    pub fn replace_mixer(&self, mixer: Mixer) -> Self {
        Self {
            mixer: mixer,
            max_voices: self.max_voices,
            samples: self.samples.clone(),
            time_map: self.time_map.clone(),
            pressed: self.pressed.clone(),
            players: HashMap::new(),
        }
    }

    pub fn set_volume(&self, volume: f32) {
        for sink in self.players.values() {
            sink.set_volume(volume);
        }
    }

    pub fn play_event_sound(
        &mut self,
        key: &str,
        is_keydown: bool,
        volume: f32,
        received_at: Instant,
        source_label: &'static str,
    ) {
        if !self.should_play_sound(key, is_keydown) {
            return;
        }

        let Some((start, end)) = self.resolve_segment_bounds_ms(key, is_keydown) else {
            return;
        };

        self.play_pcm_segment(
            key,
            start,
            end,
            is_keydown,
            volume,
            received_at,
            source_label,
        );

        self.cleanup_players();
    }

    /// Returns `true` if this down/up edge should be processed: updates [`Self::pressed`] and allows
    /// playback. Returns `false` when the edge is ignored (already down on down, or up while not down).
    fn should_play_sound(&mut self, code: &str, is_down: bool) -> bool {
        if is_down {
            if *self.pressed.get(code).unwrap_or(&false) {
                return false;
            }
            self.pressed.insert(code.to_string(), true);
        } else {
            if !*self.pressed.get(code).unwrap_or(&false) {
                return false;
            }
            self.pressed.insert(code.to_string(), false);
        }
        true
    }

    /// Decode PCM slice, validate timing, append to a player on [`Self::mixer`].
    fn play_pcm_segment(
        &mut self,
        code: &str,
        start: f32,
        end: f32,
        is_down: bool,
        volume: f32,
        received_at: Instant,
        source_label: &'static str,
    ) {
        log::debug!(
            "Playing sound for {} '{}': start={:.3}ms, end={:.3}ms",
            source_label,
            code,
            start,
            end,
        );

        let Some((samples, channels, sample_rate)) = self.samples.clone() else {
            log::error!("❌ No PCM buffer available for {} '{}'", source_label, code);
            return;
        };

        let Some(channels_nz) = NonZero::new(channels) else {
            log::error!("❌ Invalid channel count 0 for {} '{}'", source_label, code);
            return;
        };
        let Some(sample_rate_nz) = NonZero::new(sample_rate) else {
            log::error!("❌ Invalid sample rate 0 for {} '{}'", source_label, code);
            return;
        };

        let total_duration =
            ((samples.len() as f32) / (sample_rate as f32) / (channels as f32)) * 1000.0;
        let duration = end - start;

        if start < 0.0 || duration <= 0.0 || end <= start {
            log::error!(
                "❌ Invalid time parameters for {} '{}': start={:.3}ms, end={:.3}ms, duration={:.3}ms",
                source_label,
                code,
                start,
                end,
                duration
            );
            return;
        }

        const EPSILON: f32 = 1.0;

        if start >= total_duration + EPSILON {
            log::error!(
                "❌ TIMING ERROR: Start time {:.3}ms exceeds audio duration {:.3}ms for {} '{}'",
                start,
                total_duration,
                source_label,
                code
            );
            return;
        }

        if end > total_duration + EPSILON {
            log::error!(
                "❌ TIMING ERROR: Audio segment {:.3}ms-{:.3}ms exceeds duration {:.3}ms for {} '{}'",
                start,
                end,
                total_duration,
                source_label,
                code
            );
            return;
        }

        let start_sample = ((start / 1000.0) * (sample_rate as f32) * (channels as f32)) as usize;
        let end_sample = ((end / 1000.0) * (sample_rate as f32) * (channels as f32)) as usize;

        if end_sample > samples.len() {
            let clamped_end_sample = samples.len();
            let clamped_end_time =
                ((clamped_end_sample as f32) / (sample_rate as f32) / (channels as f32)) * 1000.0;
            let clamped_duration = clamped_end_time - start;

            if clamped_duration > 1.0 && clamped_end_sample > start_sample {
                let segment_samples = samples[start_sample..clamped_end_sample].to_vec();
                let segment = SamplesBuffer::new(channels_nz, sample_rate_nz, segment_samples);
                self.append_player_for_segment(code, is_down, received_at, volume, segment);
            }
            return;
        }

        if start_sample >= end_sample || start_sample >= samples.len() {
            log::error!(
                "❌ INTERNAL ERROR: Invalid sample range for {} '{}': {}..{} (max {})",
                source_label,
                code,
                start_sample,
                end_sample,
                samples.len()
            );
            log::error!(
                "   Audio: {:.3}ms, Channels: {}, Rate: {}",
                total_duration,
                channels,
                sample_rate
            );
            return;
        }

        let segment_samples = samples[start_sample..end_sample].to_vec();
        let segment = SamplesBuffer::new(channels_nz, sample_rate_nz, segment_samples);
        self.append_player_for_segment(code, is_down, received_at, volume, segment);
    }

    fn log_sound_latency(&self, event: &str, received_at: Instant) {
        let ms = received_at.elapsed().as_secs_f32() * 1000.0;
        log::debug!("⏱️ Sound '{}' input latency: {:.3} ms", event, ms,);
    }

    fn append_player_for_segment(
        &mut self,
        code: &str,
        is_down: bool,
        received_at: Instant,
        volume: f32,
        segment: SamplesBuffer,
    ) {
        let player = Player::connect_new(&self.mixer);
        player.set_volume(volume);
        player.append(segment);

        self.log_sound_latency(code, received_at);

        self.cleanup_players();

        self.players.insert(
            format!("{}-{}", code, if is_down { "down" } else { "up" }),
            player,
        );
    }

    fn resolve_segment_bounds_ms(&self, code: &str, is_down: bool) -> Option<(f32, f32)> {
        match self.time_map.get(code) {
            Some(arr) if arr.len() == 2 => {
                let idx = if is_down { 0 } else { 1 };
                let arr = arr[idx];
                let start = arr[0];
                let end = arr[1];
                let duration = end - start;
                if start < 0.0 || duration <= 0.0 || duration > 10000.0 {
                    log::warn!(
                        "⚠️ Suspicious mapping for '{}' ({}): start={:.3}ms, end={:.3}ms, duration={:.3}ms (raw: [{}, {}])",
                        code,
                        if is_down { "down" } else { "up" },
                        start,
                        end,
                        duration,
                        arr[0],
                        arr[1]
                    );
                }
                Some((start, end))
            }
            Some(arr) if arr.len() == 1 => {
                if !is_down {
                    None
                } else {
                    let arr = arr[0];
                    let start = arr[0];
                    let end = arr[1];
                    let duration = end - start;
                    if start < 0.0 || duration <= 0.0 || duration > 10000.0 {
                        log::warn!(
                            "⚠️ Suspicious mapping for '{}': start={:.3}ms, end={:.3}ms, duration={:.3}ms (raw: [{}, {}])",
                            code,
                            start,
                            end,
                            duration,
                            arr[0],
                            arr[1]
                        );
                    }
                    Some((start, end))
                }
            }
            Some(arr) => {
                log::error!(
                    "❌ Invalid mapping for '{}': expected 1-2 elements, got {}",
                    code,
                    arr.len()
                );
                None
            }
            None => {
                log::debug!("🔍 Ignoring unmapped key '{}'", code);
                None
            }
        }
    }

    pub(crate) fn load_mappings(
        &mut self,
        samples: PcmBuffer,
        mappings: HashMap<String, Vec<(f64, f64)>>,
        kind: PackKind,
    ) -> Result<(), String> {
        let (audio_samples, channels, sample_rate) = samples;
        let sample_count = audio_samples.len();
        let mapping_count = mappings.len();

        self.samples = Some((audio_samples, channels, sample_rate));
        match kind {
            PackKind::Keyboard => {
                log::info!("🎹 Updated keyboard samples: {} samples", sample_count);
            }
            PackKind::Mouse => {
                log::info!("🖱️ Updated mouse samples: {} samples", sample_count);
            }
        }

        let old_count = self.time_map.len();
        self.time_map.clear();
        for (id, ranges) in mappings {
            let converted: Vec<[f32; 2]> = ranges
                .into_iter()
                .map(|(start, end)| [start as f32, end as f32])
                .collect();
            self.time_map.insert(id, converted);
        }
        match kind {
            PackKind::Keyboard => {
                log::info!(
                    "🗝️ Updated key mappings: {} -> {} keys",
                    old_count,
                    self.time_map.len()
                );
            }
            PackKind::Mouse => {
                log::info!(
                    "🖱️ Updated mouse mappings: {} -> {} buttons",
                    old_count,
                    self.time_map.len()
                );
            }
        }

        let old_players = self.players.len();
        self.players.clear();
        if old_players > 0 {
            log::info!("🔇 Cleared {} active sinks", old_players);
        }

        let old_pressed = self.pressed.len();
        self.pressed.clear();
        if old_pressed > 0 {
            match kind {
                PackKind::Keyboard => {
                    log::info!("⌨️ Cleared {} pressed keys", old_pressed);
                }
                PackKind::Mouse => {
                    log::info!("🖱️ Cleared {} pressed mouse buttons", old_pressed);
                }
            }
        }

        match kind {
            PackKind::Keyboard => {
                log::info!("✅ Successfully loaded {} keyboard sounds", mapping_count);
            }
            PackKind::Mouse => {
                log::info!(
                    "✅ Successfully loaded mouse soundpack ({} mouse mappings) - Memory properly cleaned",
                    mapping_count
                );
            }
        }
        Ok(())
    }

    pub(crate) fn clear_mappings(&mut self) {
        self.samples = None;
        self.time_map.clear();
        let old_sinks = self.players.len();
        self.players.clear();
        if old_sinks > 0 {
            log::info!("🔇 Cleared {} active sinks", old_sinks);
        }
        let old_pressed = self.pressed.len();
        self.pressed.clear();
        if old_pressed > 0 {
            log::info!("⌨️ Cleared {} pressed keys", old_pressed);
        }
        log::info!("🎹 Cleared keyboard soundpack from memory");
    }

    fn cleanup_players(&mut self) {
        self.players.retain(|_, player| !player.empty());

        while self.players.len() >= self.max_voices {
            if let Some(key) = self.players.keys().next().cloned() {
                self.players.remove(&key);
            } else {
                break;
            }
        }
    }
}
