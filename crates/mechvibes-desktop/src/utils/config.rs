use dioxus::prelude::*;
use mechvibes_core::state::config::AppConfig;
use std::rc::Rc;

fn create_config_updater(config: Signal<AppConfig>) -> Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)> {
    Rc::new(move |updater: Box<dyn FnOnce(&mut AppConfig)>| {
        let mut config = config;
        AppConfig::update(updater);
        config.set(AppConfig::get().clone());
    })
}

pub fn use_config() -> (
    Signal<AppConfig>,
    Rc<dyn Fn(Box<dyn FnOnce(&mut AppConfig)>)>,
) {
    let config = use_signal(|| AppConfig::get().clone());
    let update_config = create_config_updater(config);
    (config, update_config)
}
