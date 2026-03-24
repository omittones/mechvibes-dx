use crate::state::config::AppConfig;
use dioxus::prelude::*;
use std::rc::Rc;

/// Creates a config updater function that loads fresh config, applies changes, and saves
fn create_config_updater(
    config_signal: Signal<AppConfig>,
) -> Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)> {
    Rc::new(move |updater: Box<dyn FnOnce(&mut AppConfig)>| {
        let mut config_signal = config_signal;
        AppConfig::update(updater);
        config_signal.set(AppConfig::get().clone());
    })
}

/// Hook for managing configuration state with automatic updates
/// Returns a tuple of (config_signal, update_config_fn)
/// The update function can be used to make atomic config updates
pub fn use_config() -> (
    Signal<AppConfig>,
    Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)>,
) {
    let config = AppConfig::get().clone();
    let config_signal = Signal::new(config);
    let update_config = create_config_updater(config_signal);

    (config_signal, update_config)
}
