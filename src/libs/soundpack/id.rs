//! Soundpack ID - unique identifier for soundpacks.
//!
//! Format: `{source}/{type}/{folder_name}` where:
//! - source: "builtin" | "custom"
//! - type: "keyboard" | "mouse"
//! - folder_name: directory name (e.g., "Apex by teia")
//!
//! This allows multiple soundpacks with the same name to coexist:
//! - builtin/keyboard/Apex by teia
//! - custom/keyboard/Apex by teia
//! - builtin/mouse/Apex by teia

use crate::libs::soundpack::format::SoundpackType;
use crate::state::paths;
use std::fmt;

/// Source of a soundpack (built-in or user-installed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundpackSource {
    Builtin,
    Custom,
}

/// Unique soundpack identifier. Format: `{source}/{type}/{folder_name}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SoundpackId {
    pub source: SoundpackSource,
    pub soundpack_type: SoundpackType,
    pub folder_name: String,
}

impl SoundpackId {
    /// Create from components.
    pub fn new(source: SoundpackSource, soundpack_type: SoundpackType, folder_name: impl Into<String>) -> Self {
        Self {
            source,
            soundpack_type,
            folder_name: folder_name.into(),
        }
    }

    /// Parse from string. Returns None if format is invalid.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(3, '/').collect();
        if parts.len() != 3 {
            return None;
        }
        let source = match parts[0] {
            "builtin" => SoundpackSource::Builtin,
            "custom" => SoundpackSource::Custom,
            _ => return None,
        };
        let soundpack_type = match parts[1] {
            "keyboard" => SoundpackType::Keyboard,
            "mouse" => SoundpackType::Mouse,
            _ => return None,
        };
        let folder_name = parts[2].to_string();
        if folder_name.is_empty() {
            return None;
        }
        Some(Self {
            source,
            soundpack_type,
            folder_name,
        })
    }

    /// Check if string is in new format (contains exactly 2 slashes).
    pub fn is_new_format(s: &str) -> bool {
        let parts: Vec<&str> = s.splitn(3, '/').collect();
        parts.len() == 3
            && (parts[0] == "builtin" || parts[0] == "custom")
            && (parts[1] == "keyboard" || parts[1] == "mouse")
            && !parts[2].is_empty()
    }

    /// Resolve to absolute directory path.
    pub fn to_absolute_path(&self) -> std::path::PathBuf {
        let base = match self.source {
            SoundpackSource::Builtin => paths::soundpacks::get_builtin_soundpacks_dir(),
            SoundpackSource::Custom => paths::soundpacks::get_custom_soundpacks_dir(),
        };
        let type_dir = match self.soundpack_type {
            SoundpackType::Keyboard => "keyboard",
            SoundpackType::Mouse => "mouse",
        };
        base.join(type_dir).join(&self.folder_name)
    }

    /// Resolve to absolute directory path as string.
    pub fn to_path_string(&self) -> String {
        self.to_absolute_path().to_string_lossy().to_string()
    }

    pub fn is_keyboard(&self) -> bool {
        self.soundpack_type == SoundpackType::Keyboard
    }

    pub fn is_mouse(&self) -> bool {
        self.soundpack_type == SoundpackType::Mouse
    }
}

impl fmt::Display for SoundpackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = match self.source {
            SoundpackSource::Builtin => "builtin",
            SoundpackSource::Custom => "custom",
        };
        let type_str = match self.soundpack_type {
            SoundpackType::Keyboard => "keyboard",
            SoundpackType::Mouse => "mouse",
        };
        write!(f, "{}/{}/{}", source, type_str, self.folder_name)
    }
}

impl From<SoundpackId> for String {
    fn from(id: SoundpackId) -> Self {
        id.to_string()
    }
}
