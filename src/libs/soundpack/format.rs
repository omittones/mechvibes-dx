use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ===== SOUNDPACK TYPES =====

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SoundpackType {
    Keyboard,
    Mouse,
}

// Default function for config_version field
fn default_config_version() -> u32 {
    2
}

// Default function for soundpack_type field
fn default_soundpack_type() -> SoundpackType {
    SoundpackType::Keyboard
}

// Default function for options field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundpackOptions {
    #[serde(default = "default_recommended_volume")]
    pub recommended_volume: f32,
    #[serde(default = "default_random_pitch")]
    pub random_pitch: bool,
}

fn default_recommended_volume() -> f32 {
    1.0
}

fn default_random_pitch() -> bool {
    false
}

impl Default for SoundpackOptions {
    fn default() -> Self {
        Self {
            recommended_volume: 1.0,
            random_pitch: false,
        }
    }
}

// Key definition structure for V2 format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyDefinition {
    pub timing: Vec<[f32; 2]>, // Array of [start_ms, end_ms] pairs
    #[serde(default)]
    pub audio_file: Option<String>, // For "multi" definition method
}

// This is the structure of the soundpack config file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SoundPack {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub config_version: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub audio_file: Option<String>, // Used only in "single" definition_method
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub created_at: Option<String>, // ISO-8601 string
    pub definition_method: String, // "single" or "multi"
    #[serde(default)]
    pub options: SoundpackOptions,
    #[serde(default = "default_soundpack_type")]
    pub soundpack_type: SoundpackType, // Type of soundpack (Keyboard or Mouse) - for internal use
    #[serde(default = "default_config_version")]
    pub config_version_num: u32, // Internal config version number
    pub definitions: HashMap<String, KeyDefinition>,
}
