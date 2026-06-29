use dioxus::prelude::*;
use mechvibes_core::state::themes::ThemesConfig;
use once_cell::sync::Lazy;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

static THEMES_CONFIG: Lazy<Arc<Mutex<ThemesConfig>>> =
    Lazy::new(|| Arc::new(Mutex::new(ThemesConfig::load())));

static REFRESH_TRIGGER: GlobalSignal<u32> = Signal::global(|| 0);

pub fn use_themes() -> (
    Signal<ThemesConfig>,
    Rc<dyn Fn(Box<dyn FnOnce(&mut ThemesConfig)>)>,
) {
    let mut themes = use_signal(|| THEMES_CONFIG.lock().unwrap().clone());

    use_effect(move || {
        let _trigger_value = REFRESH_TRIGGER();
        themes.set(THEMES_CONFIG.lock().unwrap().clone());
    });

    let update_themes = Rc::new(|updater: Box<dyn FnOnce(&mut ThemesConfig)>| {
        {
            let mut config_guard = THEMES_CONFIG.lock().unwrap();
            updater(&mut *config_guard);

            if let Err(e) = config_guard.save() {
                log::error!("❌ Failed to save themes: {}", e);
                return;
            }
        }
        let current = REFRESH_TRIGGER();
        *REFRESH_TRIGGER.write() = current + 1;
    });

    (themes, update_themes)
}

pub fn get_themes_config() -> ThemesConfig {
    THEMES_CONFIG.lock().unwrap().clone()
}
