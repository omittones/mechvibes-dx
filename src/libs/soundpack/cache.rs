//! Soundpack metadata cache loading, saving, and refresh operations.

use crate::libs::soundpack::id::{SoundpackId, SoundpackSource};
use crate::state::paths;
use crate::utils::{data, path};
use std::path::Path;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::libs::soundpack::format::SoundpackType;

// ===== SOUNDPACK METADATA =====

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundpackMetadata {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub icon: Option<String>,
    pub soundpack_type: SoundpackType,
    pub config_path: String,
}

// ===== SOUNDPACK CACHE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundpackCache {
    pub soundpacks: HashMap<String, SoundpackMetadata>,
    pub last_scan: u64,
    pub cache_version: u32, // Add version to force regeneration when format changes
    #[serde(default)]
    pub count: SoundpackCount, // Count of soundpacks by type
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoundpackCount {
    pub keyboard: usize,
    pub mouse: usize,
}

impl SoundpackCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            soundpacks: HashMap::new(),
            last_scan: 0,
            cache_version: 5, // Version 5: path-based unique IDs (source/type/folder)
            count: SoundpackCount::default(),
        }
    }

    /// Add or update soundpack metadata.
    pub fn add_soundpack(&mut self, metadata: SoundpackMetadata) {
        self.soundpacks
            .insert(metadata.config_path.clone(), metadata);
    }

    /// Update count based on current soundpacks in cache.
    pub fn update_count(&mut self) {
        let mut keyboard_count = 0;
        let mut mouse_count = 0;

        for metadata in self.soundpacks.values() {
            match metadata.soundpack_type {
                SoundpackType::Keyboard => keyboard_count += 1,
                SoundpackType::Mouse => mouse_count += 1,
            }
        }
        self.count.keyboard = keyboard_count;
        self.count.mouse = mouse_count;

        log::info!(
            "📊 Updated count: {} keyboard, {} mouse soundpacks",
            keyboard_count,
            mouse_count
        );
    }
}

/// Load the soundpack metadata cache from disk.
pub fn load_cache() -> SoundpackCache {
    let cache_file = cache_file_path();
    match data::load_json_from_file::<SoundpackCache>(Path::new(&cache_file)) {
        Ok(cache) => {
            log::info!(
                "📦 Loaded soundpack metadata cache {} with {} entries",
                cache_file,
                cache.soundpacks.len()
            );
            cache
        }
        Err(e) => {
            log::error!("⚠️ Failed to load cache file: {}", e);
            SoundpackCache::new()
        }
    }
}

/// Save the soundpack metadata cache to disk.
pub fn save_cache(cache: &SoundpackCache) {
    let cache_file = cache_file_path();

    if let Some(parent) = Path::new(&cache_file).parent() {
        if let Err(e) = path::ensure_directory_exists(parent) {
            log::error!("⚠️ Failed to create cache directory: {}", e);
            return;
        }
    }

    match data::save_json_to_file(cache, Path::new(&cache_file)) {
        Ok(_) => log::info!(
            "💾 Saved soundpack metadata cache with {} entries",
            cache.soundpacks.len()
        ),
        Err(e) => log::error!("⚠️ Failed to save metadata cache: {}", e),
    }
}

/// Refresh the cache by scanning soundpack directories.
pub fn refresh_cache(cache: &mut SoundpackCache) {
    log::info!("📂 Scanning soundpacks directories...");

    cache.soundpacks.clear();

    let builtin_soundpacks_dir = paths::soundpacks::get_builtin_soundpacks_dir()
        .to_string_lossy()
        .to_string();
    log::info!(
        "📂 Scanning built-in soundpacks in: {}",
        builtin_soundpacks_dir
    );
    scan_soundpack_type(
        cache,
        &builtin_soundpacks_dir,
        false,
        SoundpackSource::Builtin,
    );
    scan_soundpack_type(
        cache,
        &builtin_soundpacks_dir,
        true,
        SoundpackSource::Builtin,
    );

    let custom_soundpacks_dir = paths::soundpacks::get_custom_soundpacks_dir()
        .to_string_lossy()
        .to_string();
    log::info!(
        "📂 Scanning custom soundpacks in: {}",
        custom_soundpacks_dir
    );
    scan_soundpack_type(
        cache,
        &custom_soundpacks_dir,
        false,
        SoundpackSource::Custom,
    );
    scan_soundpack_type(cache, &custom_soundpacks_dir, true, SoundpackSource::Custom);

    cache.update_count();

    cache.last_scan = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    log::info!("📦 Loaded {} soundpacks metadata", cache.soundpacks.len());
}

/// Capture a soundpack loading error and update the cache.
pub fn capture_soundpack_loading_error(
    soundpack_id: &str,
    soundpack_type: SoundpackType,
    error: &str,
) {
    if soundpack_id.is_empty() {
        log::error!("⚠️ Skipping cache entry for empty soundpack ID: {}", error);
        return;
    }

    log::info!("📝 Capturing loading error for {}: {}", soundpack_id, error);

    let mut cache = load_cache();

    if !cache.soundpacks.contains_key(soundpack_id) {
        let error_metadata = SoundpackMetadata {
            id: soundpack_id.to_string(),
            config_path: soundpack_id.to_string(),
            name: format!("Error: {}", soundpack_id),
            author: None,
            tags: vec!["error".to_string()],
            icon: None,
            soundpack_type,
        };
        cache
            .soundpacks
            .insert(soundpack_id.to_string(), error_metadata);
    }

    save_cache(&cache);
    log::info!(
        "💾 Updated cache with error information for {}",
        soundpack_id
    );
}

fn cache_file_path() -> String {
    paths::data::soundpack_cache_json()
        .to_string_lossy()
        .to_string()
}

fn scan_soundpack_type(
    cache: &mut SoundpackCache,
    soundpacks_dir: &str,
    is_mouse: bool,
    source: SoundpackSource,
) {
    let soundpack_type = if is_mouse {
        SoundpackType::Mouse
    } else {
        SoundpackType::Keyboard
    };
    let type_dir =
        std::path::Path::new(soundpacks_dir).join(if is_mouse { "mouse" } else { "keyboard" });
    log::info!("📂 Scanning soundpacks in: {}", type_dir.display());

    if !type_dir.exists() {
        log::error!("⚠️ Directory does not exist: {}", type_dir.display());
        return;
    }

    let entries = match std::fs::read_dir(&type_dir) {
        Ok(e) => e,
        Err(e) => {
            log::error!("⚠️ Failed to read directory: {}: {}", type_dir.display(), e);
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                log::error!("⚠️ Failed to read directory: {}: {}", type_dir.display(), e);
                continue;
            }
        };

        let folder_name = entry.file_name().to_string_lossy().to_string();
        let soundpack_path = entry.path().to_string_lossy().to_string();
        let soundpack_id = SoundpackId::new(source, soundpack_type, &folder_name).to_string();
        log::info!("🔍 Processing soundpack {}", soundpack_id);

        match super::metadata::load_soundpack_metadata(&soundpack_path, &soundpack_id, is_mouse) {
            Ok(mut metadata) => {
                metadata.id = soundpack_id.clone();
                metadata.config_path = soundpack_id.clone();
                log::info!("✅ Successfully loaded metadata for: {}", soundpack_id);
                cache.soundpacks.insert(soundpack_id, metadata);
            }
            Err(e) => {
                log::info!("❌ Failed to load metadata for {}: {}", soundpack_id, e);
                insert_error_metadata(cache, &soundpack_id, &folder_name, e, soundpack_type);
            }
        }
    }
}

fn insert_error_metadata(
    cache: &mut SoundpackCache,
    full_soundpack_id: &str,
    soundpack_name: &str,
    _error: String,
    soundpack_type: SoundpackType,
) {
    let error_metadata = SoundpackMetadata {
        id: full_soundpack_id.to_string(),
        config_path: full_soundpack_id.to_string(),
        name: format!("Error: {}", soundpack_name),
        author: None,
        tags: vec!["error".to_string()],
        icon: None,
        soundpack_type,
    };
    cache
        .soundpacks
        .insert(full_soundpack_id.to_string(), error_metadata);
}
