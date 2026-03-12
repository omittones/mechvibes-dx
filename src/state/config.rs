use crate::libs::theme::{BuiltInTheme, Theme};
use crate::state::paths;
use crate::utils::auto_updater::AutoUpdateConfig;
use crate::utils::{data, path};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MusicPlayerConfig {
    pub current_track_id: Option<String>,
    pub volume: f32, // 0.0 to 100.0
    pub is_muted: bool,
    pub auto_play: bool,         // Auto-play music when app starts
    pub music_last_updated: u64, // timestamp for music cache
}

impl Default for MusicPlayerConfig {
    fn default() -> Self {
        Self {
            current_track_id: None,
            volume: 50.0,
            is_muted: false,
            auto_play: false,
            music_last_updated: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogoCustomization {
    pub border_color: String,
    pub text_color: String,
    pub shadow_color: String,
    pub background_color: String,
    pub background_image: Option<String>, // Path to background image
    pub use_background_image: bool,       // Whether to use image instead of color for background
    pub muted_background: String,
    pub muted_background_image: Option<String>, // Path to muted background image
    pub use_muted_background_image: bool, // Whether to use image instead of color for muted background
    pub dimmed_when_muted: bool,
}

impl Default for LogoCustomization {
    fn default() -> Self {
        Self {
            border_color: "var(--color-base-content)".to_string(),
            text_color: "var(--color-base-content)".to_string(),
            shadow_color: "var(--color-base-content)".to_string(),
            background_color: "var(--color-base-200)".to_string(),
            background_image: None,
            use_background_image: false,
            muted_background: "var(--color-base-300)".to_string(),
            muted_background_image: None,
            use_muted_background_image: false,
            dimmed_when_muted: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackgroundCustomization {
    pub background_color: String,
    pub background_image: Option<String>, // Path to background image
    pub use_image: bool,                  // Whether to use image instead of color
}

impl Default for BackgroundCustomization {
    fn default() -> Self {
        Self {
            background_color: "".to_string(),
            background_image: None,
            use_image: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // Metadata
    pub version: String,
    pub last_updated: DateTime<Utc>,
    pub commit: Option<String>,
    // Audio settings
    pub keyboard_soundpack: String,
    pub mouse_soundpack: String,
    pub volume: f32,
    pub mouse_volume: f32,         // Separate volume for mouse sounds
    pub enable_volume_boost: bool, // Enable/disable volume boost to 200%
    pub enable_sound: bool,
    pub enable_keyboard_sound: bool, // Enable/disable keyboard sounds specifically
    pub enable_mouse_sound: bool,    // Enable/disable mouse sounds specifically
    // Device settings
    pub selected_audio_device: Option<String>, // Selected audio output device
    pub enabled_keyboards: Vec<String>,        // Enabled physical keyboards (by device instance ID)
    pub enabled_mice: Vec<String>,             // Enabled physical mice (by device instance ID)
    // UI settings
    pub theme: Theme,
    pub custom_css: String, // Legacy field for existing custom CSS
    pub logo_customization: LogoCustomization,
    pub enable_logo_customization: bool, // Enable/disable logo customization panel
    pub background_customization: BackgroundCustomization,
    pub enable_background_customization: bool, // Enable/disable background customization panel
    // Music player settings
    pub music_player: MusicPlayerConfig, // Ambiance settings
    pub ambiance_active_sounds: HashMap<String, f32>, // sound_id -> volume (0.0 to 1.0)
    pub ambiance_global_volume: f32,     // 0.0 to 1.0 - global multiplier
    pub ambiance_is_muted: bool,
    // Note: ambiance play state is not persistent - always starts paused
    // System settings
    pub auto_start: bool,
    pub start_minimized: bool, // Start minimized to tray when auto-starting with Windows
    pub landscape_mode: bool,  // Enable/disable landscape mode layout
    pub auto_update: AutoUpdateConfig, // Auto-update settings
}

impl AppConfig {
    pub fn load() -> Self {
        let config_path = paths::data::config_json();

        // Ensure data directory exists
        if let Some(parent) = config_path.parent() {
            if let Err(_) = path::ensure_directory_exists(parent) {
                log::error!("Warning: Could not create data directory");
            }
        }

        // Load config from file, falling back to defaults if it doesn't exist or is invalid
        match data::load_json_from_file::<AppConfig>(&config_path) {
            Ok(mut config) => {
                // Migrate legacy soundpack IDs to new format (source/type/folder)
                let migrated = Self::migrate_soundpack_ids(&mut config);
                if migrated {
                    let _ = config.save();
                }

                // Sync auto_start with actual registry state
                let actual_auto_start = crate::utils::auto_startup::get_auto_startup_state();
                if config.auto_start != actual_auto_start {
                    log::info!(
                        "🔄 Syncing auto_start config with registry: {} -> {}",
                        config.auto_start,
                        actual_auto_start
                    );
                    config.auto_start = actual_auto_start;
                    let _ = config.save(); // Save the synced state
                }
                config
            }
            Err(e) => {
                log::error!(
                    "Warning: Failed to load config file: {}. Using defaults.",
                    e
                );
                let default_config = Self::default();
                let _ = default_config.save();
                default_config
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let config_path = paths::data::config_json();
        data::save_json_to_file(self, &config_path)
    }

    /// Migrate legacy soundpack IDs (folder name only) to new format (source/type/folder).
    /// Returns true if any migration was performed.
    fn migrate_soundpack_ids(config: &mut Self) -> bool {
        use crate::libs::soundpack::id::SoundpackId;

        let mut migrated = false;

        if !config.keyboard_soundpack.is_empty() && !SoundpackId::is_new_format(&config.keyboard_soundpack) {
            let builtin = paths::soundpacks::get_builtin_soundpacks_dir().join("keyboard").join(&config.keyboard_soundpack);
            let custom = paths::soundpacks::get_custom_soundpacks_dir().join("keyboard").join(&config.keyboard_soundpack);
            let new_id = if custom.join("config.json").exists() {
                Some(format!("custom/keyboard/{}", config.keyboard_soundpack))
            } else if builtin.join("config.json").exists() {
                Some(format!("builtin/keyboard/{}", config.keyboard_soundpack))
            } else {
                None
            };
            if let Some(id) = new_id {
                log::info!("🔧 Migrated keyboard soundpack ID: {} -> {}", config.keyboard_soundpack, id);
                config.keyboard_soundpack = id;
                migrated = true;
            }
        }

        if !config.mouse_soundpack.is_empty() && !SoundpackId::is_new_format(&config.mouse_soundpack) {
            let builtin = paths::soundpacks::get_builtin_soundpacks_dir().join("mouse").join(&config.mouse_soundpack);
            let custom = paths::soundpacks::get_custom_soundpacks_dir().join("mouse").join(&config.mouse_soundpack);
            let new_id = if custom.join("config.json").exists() {
                Some(format!("custom/mouse/{}", config.mouse_soundpack))
            } else if builtin.join("config.json").exists() {
                Some(format!("builtin/mouse/{}", config.mouse_soundpack))
            } else {
                None
            };
            if let Some(id) = new_id {
                log::info!("🔧 Migrated mouse soundpack ID: {} -> {}", config.mouse_soundpack, id);
                config.mouse_soundpack = id;
                migrated = true;
            }
        }

        migrated
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: crate::utils::constants::APP_VERSION.to_string(),
            last_updated: Utc::now(),
            commit: option_env!("GIT_HASH").map(|s| s.to_string()),
            keyboard_soundpack: "oreo".into(),
            mouse_soundpack: "test-mouse".into(),
            volume: 1.0,
            mouse_volume: 1.0,          // Default mouse volume to 100%
            enable_volume_boost: false, // Default volume boost disabled
            enable_sound: true,
            enable_keyboard_sound: true, // Default keyboard sounds enabled
            enable_mouse_sound: true,    // Default mouse sounds enabled
            selected_audio_device: None, // Default to system default audio device
            enabled_keyboards: Vec::new(), // Default to no keyboards enabled (all keyboards will work)
            enabled_mice: Vec::new(),      // Default to no mice enabled (all mice will work)
            theme: Theme::BuiltIn(BuiltInTheme::System), // Default to System theme
            custom_css: String::new(),
            logo_customization: LogoCustomization::default(),
            enable_logo_customization: false, // Default logo customization disabled
            background_customization: BackgroundCustomization::default(),
            enable_background_customization: false, // Default background customization disabled
            music_player: MusicPlayerConfig::default(),
            ambiance_active_sounds: HashMap::new(),
            ambiance_global_volume: 0.5, // Default global ambiance volume to 50%
            ambiance_is_muted: false,
            // Note: ambiance play state is not persistent - always starts paused
            auto_start: false,
            start_minimized: false, // Default to not starting minimized
            landscape_mode: false,  // Default landscape mode disabled
            auto_update: AutoUpdateConfig::default(), // Default auto-update settings
        }
    }
}
