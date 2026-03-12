// Event-driven App State Manager
use crate::libs::soundpack::cache::{
    SoundpackCache, SoundpackMetadata, load_cache, refresh_cache, save_cache,
};
use dioxus::prelude::*;
use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};

// Global app state for sharing between components
#[derive(Clone, Debug)]
pub struct AppState {
    soundpack_cache: Arc<SoundpackCache>,
    last_updated: std::time::Instant,
}

impl PartialEq for AppState {
    fn eq(&self, other: &Self) -> bool {
        // Compare cache contents, ignoring timestamp for reactivity purposes
        Arc::ptr_eq(&self.soundpack_cache, &other.soundpack_cache)
    }
}

impl AppState {
    pub fn new() -> Self {
        log::debug!("🌍 Initializing global AppState...");
        Self {
            soundpack_cache: Arc::new(load_cache()),
            last_updated: std::time::Instant::now(),
        }
    }

    pub fn count_keyboard_soundpacks(&self) -> usize {
        self.soundpack_cache.count.keyboard
    }

    pub fn count_mouse_soundpacks(&self) -> usize {
        self.soundpack_cache.count.mouse
    }

    pub fn get_last_scan(&self) -> u64 {
        self.soundpack_cache.last_scan
    }

    pub fn get_soundpacks(&self) -> Vec<SoundpackMetadata> {
        self.soundpack_cache.soundpacks.values().cloned().collect()
    }

    pub fn refresh_cache(&mut self) {
        log::debug!("🔄 Refreshing soundpack cache...");
        let mut fresh_cache = load_cache();
        refresh_cache(&mut fresh_cache);
        save_cache(&fresh_cache);
        self.soundpack_cache = Arc::new(fresh_cache);
        self.last_updated = std::time::Instant::now();
    }
}

// Global state instance
static GLOBAL_APP_STATE: OnceCell<Mutex<AppState>> = OnceCell::new();

// Simple hook for read-only access
pub fn use_app_state() -> AppState {
    let update_signal: Signal<u32> = use_context();

    let app_state = use_memo(move || {
        let _ = update_signal();
        // Subscribe to changes
        if let Some(global_state) = GLOBAL_APP_STATE.get() {
            if let Ok(state) = global_state.lock() {
                return state.clone();
            }
        }
        AppState::new()
    });

    let result = app_state.read().clone();
    result
}

// Hook to trigger state updates
pub fn use_state_trigger() -> Callback<()> {
    let mut update_signal: Signal<u32> = use_context();
    use_callback(move |_| {
        // Refresh cache and trigger UI update
        if let Some(global_state) = GLOBAL_APP_STATE.get() {
            if let Ok(mut state) = global_state.lock() {
                log::debug!("🔄 Triggering cache refresh...");
                state.refresh_cache();
            }
        }
        // Trigger UI update by incrementing the signal value
        let current_value = {
            let val = update_signal.read();
            *val
        };
        update_signal.set(current_value + 1);
    })
}

// Reload the current soundpacks from configuration
pub fn reload_current_soundpacks(audio_ctx: &crate::libs::audio::AudioContext) {
    let mut config = crate::state::config::AppConfig::load();
    let mut config_changed = false;

    // Load keyboard soundpack
    match crate::libs::audio::soundpack_loader::load_keyboard_soundpack_with_cache_control(
        audio_ctx,
        &config.keyboard_soundpack,
        false,
    ) {
        Ok(_) => log::debug!(
            "✅ Keyboard soundpack '{}' reloaded successfully",
            config.keyboard_soundpack
        ),
        Err(e) => {
            log::error!(
                "❌ Failed to reload keyboard soundpack '{}': {}. Clearing selection.",
                config.keyboard_soundpack,
                e
            );
            config.keyboard_soundpack = "".to_string();
            config_changed = true;
        }
    }

    // Load mouse soundpack
    match crate::libs::audio::soundpack_loader::load_mouse_soundpack_with_cache_control(
        audio_ctx,
        &config.mouse_soundpack,
        false,
    ) {
        Ok(_) => log::debug!(
            "✅ Mouse soundpack '{}' reloaded successfully",
            config.mouse_soundpack
        ),
        Err(e) => {
            log::error!(
                "❌ Failed to reload mouse soundpack '{}': {}. Clearing selection.",
                config.mouse_soundpack,
                e
            );
            config.mouse_soundpack = "".to_string();
            config_changed = true;
        }
    }

    // Save config if any changes were made
    if config_changed {
        let _ = config.save();
        log::debug!("💾 Config updated due to failed soundpack loads");
    }
}

// Initialize the app state - call this once at startup
pub fn init_app_state() {
    if GLOBAL_APP_STATE.get().is_none() {
        log::info!("📝 Initializing global app state (mutex)...");
        let _ = GLOBAL_APP_STATE.set(Mutex::new(AppState::new()));
    }
}

// Global update state
static GLOBAL_UPDATE_STATE: OnceCell<Mutex<Option<crate::utils::auto_updater::UpdateInfo>>> =
    OnceCell::new();

// Hook to get update info
pub fn use_update_info() -> Option<crate::utils::auto_updater::UpdateInfo> {
    let update_signal: Signal<u32> = use_context();
    // Trigger signal check
    let _ = update_signal();

    if let Some(global_update) = GLOBAL_UPDATE_STATE.get() {
        if let Ok(state) = global_update.lock() {
            return state.clone();
        }
    }
    None
}

// Function to set update info
pub fn set_update_info(update_info: Option<crate::utils::auto_updater::UpdateInfo>) {
    if let Some(global_update) = GLOBAL_UPDATE_STATE.get() {
        if let Ok(mut state) = global_update.lock() {
            *state = update_info;
        }
    }
    // Note: We need a way to trigger UI updates when this is called
    // This should be called from within Dioxus components that have access to the update signal
}

// Initialize update state - call this once at startup
pub fn init_update_state() {
    if GLOBAL_UPDATE_STATE.get().is_none() {
        log::info!("📝 Initializing global update state...");
        let _ = GLOBAL_UPDATE_STATE.set(Mutex::new(None));

        // Load saved update info from config if available
        if let Some(saved_update) = crate::utils::auto_updater::get_saved_update_info() {
            log::info!(
                "📦 Found saved update info: {} -> {}",
                saved_update.current_version,
                saved_update.latest_version
            );
            set_update_info(Some(saved_update));
        }
    }
}

// Hook to trigger update info changes (should be called from components)
pub fn use_update_info_setter() -> Callback<Option<crate::utils::auto_updater::UpdateInfo>> {
    let mut update_signal: Signal<u32> = use_context();
    use_callback(
        move |info: Option<crate::utils::auto_updater::UpdateInfo>| {
            set_update_info(info);
            // Trigger UI update
            let current_value = {
                let val = update_signal.read();
                *val
            };
            update_signal.set(current_value + 1);
        },
    )
}
