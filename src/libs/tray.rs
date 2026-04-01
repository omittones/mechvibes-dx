use crate::{state::config::AppConfig, utils::constants::APP_NAME};
use std::sync::Mutex;
use std::time::Instant;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

// Embed the icon at compile time for cross-platform reliability
const EMBEDDED_ICON: &[u8] = include_bytes!("../../assets/icon.ico");

pub enum TrayMessage {
    Show,
    Exit,
    ToggleMute,
    OpenGitHub,
    OpenDiscord,
    OpenWebsite,
}

pub struct TrayManager {
    tray_icon: TrayIcon,
}

impl TrayManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Load current config to determine sound state
        let config = AppConfig::get();
        let mute_text = if config.enable_sound {
            "Mute sounds"
        } else {
            "Unmute sounds"
        };

        // Create the tray menu with specific IDs
        let show_item = MenuItem::with_id(
            MenuId::new("show"),
            &format!("Show {}", APP_NAME),
            true,
            None,
        );
        let separator1 = PredefinedMenuItem::separator();

        // Sound control section
        let mute_item = MenuItem::with_id(MenuId::new("toggle_mute"), mute_text, true, None);
        let separator2 = PredefinedMenuItem::separator();

        // External links section
        let github_item = MenuItem::with_id(MenuId::new("github"), "GitHub Repository", true, None);
        let discord_item =
            MenuItem::with_id(MenuId::new("discord"), "Discord Community", true, None);
        let website_item =
            MenuItem::with_id(MenuId::new("website"), "Official Website", true, None);
        let separator = PredefinedMenuItem::separator();

        let exit_item = MenuItem::with_id(MenuId::new("exit"), "Exit", true, None);

        // Create the menu with the items
        let menu = Menu::with_items(&[
            &show_item,
            &separator1,
            &mute_item,
            &separator2,
            &github_item,
            &discord_item,
            &website_item,
            &separator,
            &exit_item,
        ])?;

        // Load the icon from embedded bytes for cross-platform reliability
        let icon = match image::load_from_memory_with_format(EMBEDDED_ICON, image::ImageFormat::Ico)
        {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                match Icon::from_rgba(rgba.into_raw(), width, height) {
                    Ok(icon) => {
                        log::info!("✅ Loaded embedded tray icon ({}x{})", width, height);
                        icon
                    }
                    Err(e) => {
                        log::error!("❌ Failed to create tray icon from embedded data: {}", e);
                        return Err(Box::new(e));
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to load embedded tray icon data: {}", e);
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to load embedded icon: {}", e),
                )));
            }
        };

        // Build the tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(APP_NAME)
            .with_icon(icon)
            .build()?;
        Ok(TrayManager {
            tray_icon: tray_icon,
        })
    }

    pub fn update_menu(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Load current config to determine sound state
        let config = AppConfig::get();
        let mute_text = if config.enable_sound {
            "Mute sounds"
        } else {
            "Unmute sounds"
        };

        // Create new menu with updated text
        let show_item = MenuItem::with_id(
            MenuId::new("show"),
            &format!("Show {}", APP_NAME),
            true,
            None,
        );
        let separator1 = PredefinedMenuItem::separator();

        // Sound control section with updated text
        let mute_item = MenuItem::with_id(MenuId::new("toggle_mute"), mute_text, true, None);
        let separator2 = PredefinedMenuItem::separator();

        // External links section
        let github_item = MenuItem::with_id(MenuId::new("github"), "GitHub Repository", true, None);
        let discord_item =
            MenuItem::with_id(MenuId::new("discord"), "Discord Community", true, None);
        let website_item =
            MenuItem::with_id(MenuId::new("website"), "Official Website", true, None);
        let separator = PredefinedMenuItem::separator();

        let exit_item = MenuItem::with_id(MenuId::new("exit"), "Exit", true, None);

        // Create the new menu
        let menu = Menu::with_items(&[
            &show_item,
            &separator1,
            &mute_item,
            &separator2,
            &github_item,
            &discord_item,
            &website_item,
            &separator,
            &exit_item,
        ])?;

        // Update the tray icon with new menu
        self.tray_icon.set_menu(Some(Box::new(menu)));
        log::debug!("🔄 Tray menu updated with text: {}", mute_text);

        Ok(())
    }
}

static LAST_LEFT_CLICK: Mutex<Option<Instant>> = Mutex::new(None);
const DOUBLE_CLICK_INTERVAL_MS: u128 = 500;

pub fn handle_tray_icon_click() -> Option<TrayMessage> {
    if let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let mut last = LAST_LEFT_CLICK.lock().unwrap();
            let now = Instant::now();
            if let Some(prev) = *last {
                if now.duration_since(prev).as_millis() < DOUBLE_CLICK_INTERVAL_MS {
                    *last = None;
                    return Some(TrayMessage::Show);
                }
            }
            *last = Some(now);
        }
    }
    None
}

pub fn handle_tray_events() -> Option<TrayMessage> {
    // Handle menu events
    if let Ok(event) = MenuEvent::receiver().try_recv() {
        log::info!("🖱️ Tray menu event received: {:?}", event);
        match event.id.0.as_str() {
            "show" => {
                log::info!("🔼 Tray menu: Show {} clicked", APP_NAME);
                return Some(TrayMessage::Show);
            }
            "toggle_mute" => {
                log::info!("🔇 Tray menu: Toggle Mute clicked");
                return Some(TrayMessage::ToggleMute);
            }
            "github" => {
                log::info!("🐙 Tray menu: GitHub Repository clicked");
                return Some(TrayMessage::OpenGitHub);
            }
            "discord" => {
                log::info!("💬 Tray menu: Discord Community clicked");
                return Some(TrayMessage::OpenDiscord);
            }
            "website" => {
                log::info!("🌐 Tray menu: Official Website clicked");
                return Some(TrayMessage::OpenWebsite);
            }
            "exit" => {
                log::info!("❌ Tray menu: Exit clicked");
                return Some(TrayMessage::Exit);
            }
            _ => {
                log::info!("❓ Tray menu: Unknown menu item: {}", event.id.0);
            }
        }
    }

    None
}
