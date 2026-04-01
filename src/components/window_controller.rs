use crate::libs::tray::{TrayManager, TrayMessage, handle_tray_events, handle_tray_icon_click};
use crate::libs::tray_service::TRAY_UPDATE_SERVICE;
use crate::libs::window_manager::{WINDOW_MANAGER, WindowAction};
use crate::state::config::AppConfig;
use dioxus::desktop::use_window;
use dioxus::prelude::*;
use std::sync::mpsc;

fn init_tray_manager() -> Option<TrayManager> {
    match TrayManager::new() {
        Ok(tray) => {
            log::debug!("✅ System tray initialized successfully");
            Some(tray)
        }

        Err(e) => {
            log::error!("❌ Failed to initialize system tray: {}", e);
            None
        }
    }
}

#[component]
pub fn WindowController() -> Element {
    let window = use_window();

    // Create a static receiver for window actions
    let window_action_receiver = use_signal(|| {
        let (tx, rx) = mpsc::channel::<WindowAction>();
        WINDOW_MANAGER.set_action_sender(tx);
        Some(rx)
    });

    // Create a signal to hold the tray manager
    let tray_manager = use_signal(init_tray_manager);

    // Use effect to listen for both window actions and tray events
    use_effect(move || {
        let window_clone = window.clone();
        let mut tray_manager_clone = tray_manager.clone();

        spawn(async move {
            loop {
                // Handle window actions from internal sources
                if let Some(receiver) = window_action_receiver.read().as_ref() {
                    if let Ok(action) = receiver.try_recv() {
                        match action {
                            WindowAction::Show => {
                                window_clone.set_visible(true);
                                window_clone.set_focus();
                                WINDOW_MANAGER.set_visible(true);
                                log::info!("🔼 Window shown from internal action");
                            }
                            WindowAction::Hide => {
                                window_clone.set_visible(false);
                                WINDOW_MANAGER.set_visible(false);
                                log::info!("🔽 Window hidden from internal action");
                            }
                        }
                    }
                }

                // Handle tray update requests from other parts of the application
                if let Some(_) = TRAY_UPDATE_SERVICE.try_receive() {
                    tray_manager_clone.with_mut(|tray_opt| {
                        if let Some(tray) = tray_opt {
                            if let Err(e) = tray.update_menu() {
                                log::error!(
                                    "❌ Failed to update tray menu from global request: {}",
                                    e
                                );
                            } else {
                                log::info!("✅ Tray menu updated from global request");
                            }
                        }
                    });
                }

                // Handle tray icon double-click (show/focus only if not already focused)
                if let Some(TrayMessage::Show) = handle_tray_icon_click() {
                    let is_visible = WINDOW_MANAGER
                        .is_visible
                        .lock()
                        .map(|v| *v)
                        .unwrap_or(false);
                    if !is_visible {
                        window_clone.set_visible(true);
                        window_clone.set_focus();
                        WINDOW_MANAGER.set_visible(true);
                        log::debug!("🔼 Window shown from tray double-click");
                    }
                }

                // Handle tray menu events
                if let Some(tray_message) = handle_tray_events() {
                    match tray_message {
                        TrayMessage::Show => {
                            window_clone.set_visible(true);
                            window_clone.set_focus();
                            WINDOW_MANAGER.set_visible(true);
                            log::debug!("🔼 Window shown from tray");
                        }
                        TrayMessage::ToggleMute => {
                            // Toggle the global sound enable flag
                            AppConfig::update(|config| {
                                config.enable_sound = !config.enable_sound;
                            });
                            let status = if AppConfig::get().enable_sound {
                                "enabled"
                            } else {
                                "disabled"
                            };

                            log::debug!("🔇 Sounds {} via tray menu", status);

                            // Update tray menu to reflect new state
                            tray_manager_clone.with_mut(|tray_opt| {
                                if let Some(tray) = tray_opt {
                                    if let Err(e) = tray.update_menu() {
                                        log::error!("❌ Failed to update tray menu: {}", e);
                                    }
                                }
                            });
                        }
                        TrayMessage::OpenGitHub => {
                            let url = "https://github.com/hainguyents13/mechvibes-dx";
                            if let Err(e) = open::that(url) {
                                log::error!("❌ Failed to open GitHub URL: {}", e);
                            } else {
                                log::debug!("🐙 Opened GitHub repository in browser");
                            }
                        }
                        TrayMessage::OpenDiscord => {
                            let url = "https://discord.com/invite/MMVrhWxa4w";
                            if let Err(e) = open::that(url) {
                                log::error!("❌ Failed to open Discord URL: {}", e);
                            } else {
                                log::info!("💬 Opened Discord community in browser");
                            }
                        }
                        TrayMessage::OpenWebsite => {
                            let url = "https://mechvibes.com";
                            if let Err(e) = open::that(url) {
                                log::error!("❌ Failed to open website URL: {}", e);
                            } else {
                                log::info!("🌐 Opened official website in browser");
                            }
                        }
                        TrayMessage::Exit => {
                            log::info!("📢 Tray: Exit requested - closing application");
                            // Close the window which will trigger app exit
                            window_clone.close();
                        }
                    }
                }
                // Small delay to prevent busy-waiting
                futures_timer::Delay::new(std::time::Duration::from_millis(50)).await;
            }
        });
    });

    rsx! {
        // This component doesn't render anything visible
        span { style: "display: none;" }
    }
}
