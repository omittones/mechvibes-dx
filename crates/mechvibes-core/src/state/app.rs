use crate::soundpack::cache::{
    SoundpackCache, SoundpackMetadata, load_cache, load_soundpacks_into_cache, save_cache,
};
use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct AppState {
    soundpack_cache: Arc<SoundpackCache>,
    last_updated: std::time::Instant,
}

impl PartialEq for AppState {
    fn eq(&self, other: &Self) -> bool {
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
        load_soundpacks_into_cache(&mut fresh_cache);
        save_cache(&fresh_cache);
        self.soundpack_cache = Arc::new(fresh_cache);
        self.last_updated = std::time::Instant::now();
    }
}

static GLOBAL_APP_STATE: OnceCell<Mutex<AppState>> = OnceCell::new();

pub fn init_app_state() {
    if GLOBAL_APP_STATE.get().is_none() {
        log::info!("📝 Initializing global app state (mutex)...");
        let _ = GLOBAL_APP_STATE.set(Mutex::new(AppState::new()));
    }
}

pub fn get_app_state_mutex() -> Option<&'static Mutex<AppState>> {
    GLOBAL_APP_STATE.get()
}

static GLOBAL_UPDATE_STATE: OnceCell<Mutex<Option<crate::utils::auto_updater::UpdateInfo>>> =
    OnceCell::new();

pub fn set_update_info(update_info: Option<crate::utils::auto_updater::UpdateInfo>) {
    if let Some(global_update) = GLOBAL_UPDATE_STATE.get() {
        if let Ok(mut state) = global_update.lock() {
            *state = update_info;
        }
    }
}

pub fn get_update_info() -> Option<crate::utils::auto_updater::UpdateInfo> {
    if let Some(global_update) = GLOBAL_UPDATE_STATE.get() {
        if let Ok(state) = global_update.lock() {
            return state.clone();
        }
    }
    None
}

pub fn init_update_state() {
    if GLOBAL_UPDATE_STATE.get().is_none() {
        log::info!("📝 Initializing global update state...");
        let _ = GLOBAL_UPDATE_STATE.set(Mutex::new(None));

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
