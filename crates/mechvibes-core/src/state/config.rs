use crate::theme::{BuiltInTheme, Theme};
use crate::state::paths;
use crate::utils::auto_updater::AutoUpdateConfig;
use crate::utils::{data, path};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock, RwLockReadGuard};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MusicPlayerConfig {
    pub current_track_id: Option<String>,
    pub volume: f32,
    pub is_muted: bool,
    pub auto_play: bool,
    pub music_last_updated: u64,
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
    pub background_image: Option<String>,
    pub use_background_image: bool,
    pub muted_background: String,
    pub muted_background_image: Option<String>,
    pub use_muted_background_image: bool,
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
    pub background_image: Option<String>,
    pub use_image: bool,
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
    pub version: String,
    pub last_updated: DateTime<Utc>,
    pub commit: Option<String>,
    pub keyboard_soundpack: String,
    pub mouse_soundpack: String,
    pub volume: f32,
    pub mouse_volume: f32,
    pub enable_volume_boost: bool,
    pub enable_sound: bool,
    pub enable_keyboard_sound: bool,
    pub enable_mouse_sound: bool,
    pub selected_audio_device: Option<String>,
    pub enabled_keyboards: Vec<String>,
    pub enabled_mice: Vec<String>,
    pub theme: Theme,
    pub custom_css: String,
    pub logo_customization: LogoCustomization,
    pub enable_logo_customization: bool,
    pub background_customization: BackgroundCustomization,
    pub enable_background_customization: bool,
    pub music_player: MusicPlayerConfig,
    pub ambiance_active_sounds: HashMap<String, f32>,
    pub ambiance_global_volume: f32,
    pub ambiance_is_muted: bool,
    pub auto_start: bool,
    pub start_minimized: bool,
    pub landscape_mode: bool,
    pub auto_update: AutoUpdateConfig,
}

static GLOBAL_APP_CONFIG: LazyLock<RwLock<AppConfig>> =
    std::sync::LazyLock::new(|| RwLock::new(AppConfig::load()));

impl AppConfig {
    pub fn get() -> RwLockReadGuard<'static, Self> {
        log::debug!("🔍 Getting app config");
        match GLOBAL_APP_CONFIG.read() {
            Ok(config) => config,
            Err(e) => {
                log::error!("Failed to read app config: {}", e);
                panic!("Failed to read app config");
            }
        }
    }

    pub fn update(updater: impl FnOnce(&mut Self)) {
        log::debug!("🔍 Updating app config");
        match GLOBAL_APP_CONFIG.write() {
            Ok(mut config) => {
                updater(&mut config);
                config.last_updated = chrono::Utc::now();
                match config.save() {
                    Ok(_) => log::debug!("🔄 App config updated"),
                    Err(e) => log::error!("❌ Failed to save app config: {}", e),
                }
            }
            Err(e) => {
                log::error!("Failed to write app config: {}", e);
                panic!("Failed to write app config");
            }
        }
    }

    fn load() -> Self {
        let config_path = paths::data::config_json();

        if let Some(parent) = config_path.parent() {
            if let Err(_) = path::ensure_directory_exists(parent) {
                log::error!("Warning: Could not create data directory");
            }
        }

        match data::load_json_from_file::<AppConfig>(&config_path) {
            Ok(mut config) => {
                let actual_auto_start = crate::utils::auto_startup::get_auto_startup_state();
                if config.auto_start != actual_auto_start {
                    log::info!(
                        "🔄 Syncing auto_start config with registry: {} -> {}",
                        config.auto_start,
                        actual_auto_start
                    );
                    config.auto_start = actual_auto_start;
                    let _ = config.save();
                }
                config
            }
            Err(e) => {
                log::error!(
                    "Warning: Failed to load config file from {}: {}. Using defaults.",
                    config_path.display(),
                    e
                );
                let default_config = Self::default();
                let _ = default_config.save();
                default_config
            }
        }
    }

    fn save(&self) -> Result<(), String> {
        let config_path = paths::data::config_json();
        data::save_json_to_file(self, &config_path)
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
            mouse_volume: 1.0,
            enable_volume_boost: false,
            enable_sound: true,
            enable_keyboard_sound: true,
            enable_mouse_sound: true,
            selected_audio_device: None,
            enabled_keyboards: Vec::new(),
            enabled_mice: Vec::new(),
            theme: Theme::BuiltIn(BuiltInTheme::System),
            custom_css: String::new(),
            logo_customization: LogoCustomization::default(),
            enable_logo_customization: false,
            background_customization: BackgroundCustomization::default(),
            enable_background_customization: false,
            music_player: MusicPlayerConfig::default(),
            ambiance_active_sounds: HashMap::new(),
            ambiance_global_volume: 0.5,
            ambiance_is_muted: false,
            auto_start: false,
            start_minimized: false,
            landscape_mode: false,
            auto_update: AutoUpdateConfig::default(),
        }
    }
}
