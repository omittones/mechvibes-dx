#[cfg(windows)]
mod inner {
    use mechvibes_core::state::config::AppConfig;
    use mechvibes_core::utils::constants::APP_NAME;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use tray_icon::{
        Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
        menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    };

    const ICON_BYTES: &[u8] = include_bytes!("../../mechvibes-desktop/assets/icon.ico");

    fn load_icon() -> Icon {
        let img = image::load_from_memory(ICON_BYTES).expect("embedded icon is valid");
        let rgba = img.into_rgba8();
        let (w, h) = rgba.dimensions();
        Icon::from_rgba(rgba.into_raw(), w, h).expect("icon from rgba")
    }

    fn build_menu() -> Menu {
        let is_muted = !AppConfig::get().enable_sound;
        let mute_label = if is_muted { "Unmute" } else { "Mute" };

        let mute_item = MenuItem::with_id(MenuId::new("toggle_mute"), mute_label, true, None);
        let sep = PredefinedMenuItem::separator();
        let exit_item = MenuItem::with_id(MenuId::new("exit"), "Exit", true, None);

        Menu::with_items(&[&mute_item, &sep, &exit_item]).unwrap()
    }

    pub fn build() -> TrayIcon {
        let icon = load_icon();
        let menu = build_menu();

        TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_tooltip(APP_NAME)
            .build()
            .expect("failed to build tray icon")
    }

    /// Run the Windows message pump on the calling thread until `stop_flag` is set.
    /// Handles tray menu events (mute toggle, exit) inline.
    pub fn run_event_loop(stop_flag: Arc<AtomicBool>) {
        use winapi::um::winuser::{DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage};

        let _tray = build();

        loop {
            // Drain Windows messages (non-blocking)
            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // Tray icon click events (left-click, etc.) — no-op for now
            while TrayIconEvent::receiver().try_recv().is_ok() {}

            // Menu events
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                match event.id.0.as_str() {
                    "toggle_mute" => {
                        AppConfig::update(|cfg| cfg.enable_sound = !cfg.enable_sound);
                        let on = AppConfig::get().enable_sound;
                        log::info!("Sound {}", if on { "unmuted" } else { "muted" });
                    }
                    "exit" => {
                        stop_flag.store(true, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }

            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }
}

// Public API — no-op on non-Windows

pub fn run_event_loop(stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    #[cfg(windows)]
    inner::run_event_loop(stop_flag);

    #[cfg(not(windows))]
    {
        while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
}
