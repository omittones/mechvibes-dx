//! Soundpack metadata cache loading, saving, and refresh operations.

use crate::libs::soundpack::format::{SoundPack, load_and_migrate_soundpack};
use crate::state::paths;
use crate::utils::{data, path};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::str::FromStr;

// ===== SOUNDPACK METADATA =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SoundpackType {
    Keyboard,
    Mouse,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SoundpackRef {
    pub id: String,
    pub is_builtin: bool,
    pub soundpack_type: SoundpackType,
}

impl SoundpackRef {
    pub fn parse(id: &str) -> Result<Self, &'static str> {
        let mut parts = id.splitn(3, '/');
        let source = parts.next().ok_or("missing source")?;
        let ty = parts.next().ok_or("missing type")?;
        let real_id = parts.next().ok_or("missing id")?;

        let is_builtin = match source {
            "builtin" => true,
            "custom" => false,
            _ => return Err("invalid source, expected 'builtin' or 'custom'"),
        };

        let soundpack_type = match ty {
            "keyboard" => SoundpackType::Keyboard,
            "mouse" => SoundpackType::Mouse,
            _ => return Err("invalid type, expected 'keyboard' or 'mouse'"),
        };

        Ok(SoundpackRef {
            id: real_id.to_string(),
            is_builtin,
            soundpack_type,
        })
    }

    /// Returns the full folder path of the soundpack, based on the scanning layout.
    /// e.g. `{app_root}/soundpacks/keyboard/mechanical-keyboard` for builtin or
    /// `{app_data}/soundpacks/keyboard/my-pack` for custom.
    pub fn to_soundpack_path(&self) -> PathBuf {
        let base = if self.is_builtin {
            paths::soundpacks::get_builtin_soundpacks_dir()
        } else {
            paths::soundpacks::get_custom_soundpacks_dir()
        };
        let type_dir = match self.soundpack_type {
            SoundpackType::Keyboard => "keyboard",
            SoundpackType::Mouse => "mouse",
        };
        base.join(type_dir).join(&self.id)
    }
}

impl From<&String> for SoundpackRef {
    fn from(id: &String) -> Self {
        Self::parse(id.as_str()).unwrap()
    }
}

impl Display for SoundpackRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = if self.is_builtin { "builtin" } else { "custom" };
        let ty = match self.soundpack_type {
            SoundpackType::Keyboard => "keyboard",
            SoundpackType::Mouse => "mouse",
        };
        write!(f, "{}/{}/{}", source, ty, self.id)
    }
}

impl FromStr for SoundpackRef {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for SoundpackRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SoundpackRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SoundpackRefVisitor;

        impl<'de> Visitor<'de> for SoundpackRefVisitor {
            type Value = SoundpackRef;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string in the format 'source/type/id'")
            }

            fn visit_str<E>(self, value: &str) -> Result<SoundpackRef, E>
            where
                E: de::Error,
            {
                SoundpackRef::from_str(value).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(SoundpackRefVisitor {})
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundpackMetadata {
    pub id: SoundpackRef,
    pub name: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub icon: Option<String>,
}

// ===== SOUNDPACK CACHE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundpackCache {
    pub soundpacks: HashMap<SoundpackRef, SoundpackMetadata>,
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
        self.soundpacks.insert(metadata.id.clone(), metadata);
    }

    /// Update count based on current soundpacks in cache.
    pub fn update_count(&mut self) {
        let mut keyboard_count = 0;
        let mut mouse_count = 0;

        for metadata in self.soundpacks.values() {
            match metadata.id.soundpack_type {
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
            let mut cache = SoundpackCache::new();
            load_soundpacks_into_cache(&mut cache);
            cache
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
pub fn load_soundpacks_into_cache(cache: &mut SoundpackCache) {
    log::info!("📂 Scanning soundpacks directories...");

    cache.soundpacks.clear();

    let builtin_soundpacks_dir = paths::soundpacks::get_builtin_soundpacks_dir();
    log::info!(
        "📂 Scanning built-in soundpacks in: {}",
        builtin_soundpacks_dir.display()
    );
    scan_soundpack_type(cache, &builtin_soundpacks_dir, true, false);
    scan_soundpack_type(cache, &builtin_soundpacks_dir, true, true);

    let custom_soundpacks_dir = paths::soundpacks::get_custom_soundpacks_dir();
    log::info!(
        "📂 Scanning custom soundpacks in: {}",
        custom_soundpacks_dir.display()
    );
    scan_soundpack_type(cache, &custom_soundpacks_dir, false, false);
    scan_soundpack_type(cache, &custom_soundpacks_dir, false, true);

    cache.update_count();

    cache.last_scan = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    log::info!("📦 Loaded {} soundpacks metadata", cache.soundpacks.len());
}

fn cache_file_path() -> String {
    paths::data::soundpack_cache_json()
        .to_string_lossy()
        .to_string()
}

fn scan_soundpack_type(
    cache: &mut SoundpackCache,
    soundpacks_dir: &Path,
    is_builtin: bool,
    is_mouse: bool,
) {
    let type_dir = soundpacks_dir.join(if is_mouse { "mouse" } else { "keyboard" });
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

        let soundpack_path = entry.path();

        log::info!("🔍 Processing soundpack {}", soundpack_path.display());

        match load_and_migrate_soundpack(&soundpack_path.to_string_lossy()) {
            Ok(soundpack) => {
                let id = SoundpackRef {
                    id: entry.file_name().to_string_lossy().to_string(),
                    is_builtin: is_builtin,
                    soundpack_type: if is_mouse {
                        SoundpackType::Mouse
                    } else {
                        SoundpackType::Keyboard
                    },
                };
                let metadata = metadata_from_soundpack(id, &soundpack);
                cache.add_soundpack(metadata);
            }
            Err(e) => {
                log::error!(
                    "❌ Failed to load metadata for {}: {}",
                    soundpack_path.display(),
                    e
                );
                insert_error_metadata(cache, &soundpack_path, is_builtin, is_mouse, e);
            }
        }
    }
}

fn insert_error_metadata(
    cache: &mut SoundpackCache,
    soundpack_path: &Path,
    is_builtin: bool,
    is_mouse: bool,
    _error: String,
) {
    let file_name = soundpack_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let error_metadata = SoundpackMetadata {
        id: SoundpackRef {
            id: file_name.clone(),
            is_builtin: is_builtin,
            soundpack_type: if is_mouse {
                SoundpackType::Mouse
            } else {
                SoundpackType::Keyboard
            },
        },
        name: format!("Error: {}", file_name),
        author: None,
        tags: vec!["error".to_string()],
        icon: None,
    };
    cache
        .soundpacks
        .insert(error_metadata.id.clone(), error_metadata);
}

pub fn metadata_from_soundpack(id: SoundpackRef, soundpack: &SoundPack) -> SoundpackMetadata {
    let config_path = id
        .to_soundpack_path()
        .join("config.json")
        .to_string_lossy()
        .to_string();

    SoundpackMetadata {
        id: id.clone(),
        name: soundpack.name.clone(),
        author: soundpack.author.clone(),
        tags: soundpack.tags.clone().unwrap_or_default(),
        icon: {
            if let Some(icon_filename) = &soundpack.icon {
                let mut icon_path = PathBuf::from(config_path);
                icon_path.set_file_name(icon_filename.trim_start_matches("./"));
                if icon_path.exists() {
                    let asset_url = format!(
                        "/soundpack-images/{}/{}/{}/{}",
                        if id.is_builtin { "builtin" } else { "custom" },
                        if id.soundpack_type == SoundpackType::Mouse {
                            "mouse"
                        } else {
                            "keyboard"
                        },
                        id.id,
                        icon_filename
                    );
                    log::debug!("✅ Generated asset URL for {}: {}", id, asset_url);
                    Some(asset_url)
                } else {
                    log::warn!("⚠️ Icon not found for {}, setting empty string", id);
                    Some(String::new())
                }
            } else {
                log::debug!("ℹ️  No icon specified for {}, setting empty string", id);
                Some(String::new())
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundpack_ref_serialize_builtin_keyboard() {
        let ref_ = SoundpackRef::parse("builtin/keyboard/mechanical-keyboard").unwrap();
        let json = serde_json::to_string(&ref_).unwrap();
        assert_eq!(json, r#""builtin/keyboard/mechanical-keyboard""#);
    }

    #[test]
    fn soundpack_ref_serialize_custom_mouse() {
        let ref_ = SoundpackRef::parse("custom/mouse/my-mouse-pack").unwrap();
        let json = serde_json::to_string(&ref_).unwrap();
        assert_eq!(json, r#""custom/mouse/my-mouse-pack""#);
    }

    #[test]
    fn soundpack_ref_deserialize_builtin_keyboard() {
        let json = r#""builtin/keyboard/mechanical-keyboard""#;
        let ref_: SoundpackRef = serde_json::from_str(json).unwrap();
        assert_eq!(ref_.id, "mechanical-keyboard");
        assert!(ref_.is_builtin);
        assert_eq!(ref_.soundpack_type, SoundpackType::Keyboard);
    }

    #[test]
    fn soundpack_ref_deserialize_custom_mouse() {
        let json = r#""custom/mouse/my-mouse-pack""#;
        let ref_: SoundpackRef = serde_json::from_str(json).unwrap();
        assert_eq!(ref_.id, "my-mouse-pack");
        assert!(!ref_.is_builtin);
        assert_eq!(ref_.soundpack_type, SoundpackType::Mouse);
    }

    #[test]
    fn soundpack_ref_in_struct() {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct TestModel {
            id: SoundpackRef,
        }
        let json = r#"{"id":"builtin/keyboard/some-pack"}"#;
        let model: TestModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.id.id, "some-pack");
        assert!(model.id.is_builtin);
        assert_eq!(model.id.soundpack_type, SoundpackType::Keyboard);
    }

    #[test]
    fn soundpack_ref_roundtrip() {
        let original = SoundpackRef::parse("builtin/keyboard/some-pack").unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: SoundpackRef = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn soundpack_ref_deserialize_invalid_source() {
        let json = r#""invalid/keyboard/some-pack""#;
        let result: Result<SoundpackRef, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn soundpack_ref_deserialize_invalid_type() {
        let json = r#""builtin/unknown/some-pack""#;
        let result: Result<SoundpackRef, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn soundpack_ref_deserialize_malformed() {
        let json = r#""builtin/keyboard""#; // missing id
        let result: Result<SoundpackRef, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn soundpack_ref_deserialize_not_string() {
        let json = r#"123"#;
        let result: Result<SoundpackRef, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
