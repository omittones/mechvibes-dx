use dioxus::prelude::*;
pub use mechvibes_core::state::app::{
    AppState, get_app_state_mutex, get_update_info, set_update_info,
};
pub use mechvibes_core::utils::auto_updater::UpdateInfo;

pub fn use_app_state() -> AppState {
    let update_signal: Signal<u32> = use_context();

    let app_state = use_memo(move || {
        let _ = update_signal();
        if let Some(global_state) = get_app_state_mutex() {
            if let Ok(state) = global_state.lock() {
                return state.clone();
            }
        }
        AppState::new()
    });

    app_state.read().clone()
}

pub fn use_state_trigger() -> Callback<()> {
    let mut update_signal: Signal<u32> = use_context();
    use_callback(move |_| {
        if let Some(global_state) = get_app_state_mutex() {
            if let Ok(mut state) = global_state.lock() {
                log::debug!("🔄 Triggering cache refresh...");
                state.refresh_cache();
            }
        }
        let current_value = {
            let val = update_signal.read();
            *val
        };
        update_signal.set(current_value + 1);
    })
}

pub fn use_update_info() -> Option<UpdateInfo> {
    let update_signal: Signal<u32> = use_context();
    let _ = update_signal();
    get_update_info()
}

pub fn use_update_info_setter() -> Callback<Option<UpdateInfo>> {
    let mut update_signal: Signal<u32> = use_context();
    use_callback(move |info: Option<UpdateInfo>| {
        set_update_info(info);
        let current_value = {
            let val = update_signal.read();
            *val
        };
        update_signal.set(current_value + 1);
    })
}
