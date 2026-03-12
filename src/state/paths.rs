/// Centralized path definitions
///
/// ## Path Structure
/// - `data/` - Application data and configuration files (relative to app root)
/// - `soundpacks/` - Built-in soundpack directories (relative to app root)
/// - Custom soundpacks - Stored in system app data directory (e.g., %APPDATA%/Mechvibes/soundpacks)
/// - Custom images - Stored in system app data directory (e.g., %APPDATA%/Mechvibes/custom_images)
///
/// All paths are relative to the application executable directory unless specified otherwise.
use std::path::PathBuf;
use std::sync::OnceLock;

/// Get the application root directory (where the executable is located)
/// This ensures resources are found regardless of working directory
fn get_app_root() -> &'static PathBuf {
    static APP_ROOT: OnceLock<PathBuf> = OnceLock::new();
    APP_ROOT.get_or_init(|| {
        // Try to get the directory where the executable is located
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // Check if running in dev mode (dx serve creates target/dx/... path)
                let exe_path_str = exe_path.to_string_lossy();
                if exe_path_str.contains("target\\dx\\") || exe_path_str.contains("target/dx/") {
                    // In dev mode, use current working directory (project root)
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    log::info!("📂 App root (dev mode - from cwd): {}", cwd.display());
                    return cwd;
                }

                log::info!("📂 App root (from exe): {}", exe_dir.display());
                return exe_dir.to_path_buf();
            }
        }

        // Fallback to current working directory (for development)
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        log::info!("📂 App root (fallback - from cwd): {}", cwd.display());
        cwd
    })
}

/// Get the system app data directory for Mechvibes
/// Returns platform-specific app data directory:
/// - Windows: %APPDATA%/Mechvibes
/// - macOS: ~/Library/Application Support/Mechvibes
/// - Linux: ~/.local/share/mechvibes
fn get_system_app_data_dir() -> PathBuf {
    use directories::BaseDirs;

    if let Some(base_dirs) = BaseDirs::new() {
        #[cfg(target_os = "windows")]
        {
            // Windows: %APPDATA%/Mechvibes
            base_dirs.data_dir().join("Mechvibes")
        }
        #[cfg(target_os = "macos")]
        {
            // macOS: ~/Library/Application Support/Mechvibes
            base_dirs.data_dir().join("Mechvibes")
        }
        #[cfg(target_os = "linux")]
        {
            // Linux: ~/.local/share/mechvibes
            base_dirs.data_dir().join("mechvibes")
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            // Other Unix-like systems
            base_dirs.data_dir().join("mechvibes")
        }
    } else {
        // Fallback to app root if system directories not available
        get_app_root().join("data")
    }
}

/// Application data directory paths
pub mod data {
    use super::{get_app_root, get_system_app_data_dir};
    use std::path::PathBuf;

    /// Application configuration file
    pub fn config_json() -> PathBuf {
        get_app_root().join("data").join("config.json")
    }

    /// Application manifest file
    pub fn manifest_json() -> PathBuf {
        get_app_root().join("data").join("manifest.json")
    }

    /// Custom themes configuration file
    pub fn themes_json() -> PathBuf {
        get_app_root().join("data").join("themes.json")
    }

    /// Soundpack cache file
    pub fn soundpack_cache_json() -> PathBuf {
        get_app_root().join("data").join("soundpack_cache.json")
    }

    /// Custom images directory for user-uploaded images
    /// Uses system app data directory (e.g., %APPDATA%/Mechvibes/custom_images on Windows)
    pub fn custom_images_dir() -> PathBuf {
        get_system_app_data_dir().join("custom_images")
    }
}

/// Soundpack directory paths
pub mod soundpacks {
    use super::{get_app_root, get_system_app_data_dir};
    use std::path::PathBuf;

    /// Get the base soundpacks directory for built-in soundpacks (app root)
    pub fn get_builtin_soundpacks_dir() -> PathBuf {
        get_app_root().join("soundpacks")
    }

    /// Get the base soundpacks directory for custom soundpacks (system app data)
    pub fn get_custom_soundpacks_dir() -> PathBuf {
        get_system_app_data_dir().join("soundpacks")
    }

    /// Get soundpack directory path for a specific soundpack ID.
    ///
    /// Supports two formats:
    /// - **New format**: `{source}/{type}/{folder}` e.g. `builtin/keyboard/oreo`
    /// - **Legacy format**: folder name only, with `is_mouse` for type. Resolves custom first, then builtin.
    pub fn find_soundpack_dir(soundpack_id: &str, is_mouse: bool) -> String {
        use crate::libs::soundpack::id::SoundpackId;

        if let Some(id) = SoundpackId::parse(soundpack_id) {
            return id.to_path_string();
        }

        // Legacy format: folder name only - check custom first, then builtin
        let type_dir = if is_mouse { "mouse" } else { "keyboard" };
        let mut soundpack_dir = get_custom_soundpacks_dir().join(type_dir).join(soundpack_id);
        let config_path = soundpack_dir.join("config.json");

        if !config_path.exists() {
            soundpack_dir = get_builtin_soundpacks_dir().join(type_dir).join(soundpack_id);
        }

        soundpack_dir.to_string_lossy().to_string()
    }

    /// Resolve a soundpack ID to absolute path. For legacy IDs (folder name only),
    /// requires is_mouse to determine type.
    pub fn resolve_soundpack_path(soundpack_id: &str, is_mouse: bool) -> std::path::PathBuf {
        use crate::libs::soundpack::id::SoundpackId;

        if let Some(id) = SoundpackId::parse(soundpack_id) {
            return id.to_absolute_path();
        }

        let type_dir = if is_mouse { "mouse" } else { "keyboard" };
        let custom_path = get_custom_soundpacks_dir().join(type_dir).join(soundpack_id);
        if custom_path.join("config.json").exists() {
            return custom_path;
        }
        get_builtin_soundpacks_dir().join(type_dir).join(soundpack_id)
    }

    /// Ensure soundpack directories exist (keyboard and mouse)
    /// Creates the directories if they don't exist
    pub fn ensure_soundpack_directories() -> Result<(), std::io::Error> {
        use std::fs;

        // Ensure built-in soundpack directories exist
        let builtin_soundpacks_dir = get_builtin_soundpacks_dir();
        let builtin_keyboard_dir = builtin_soundpacks_dir.join("keyboard");
        let builtin_mouse_dir = builtin_soundpacks_dir.join("mouse");

        if !builtin_soundpacks_dir.exists() {
            fs::create_dir_all(&builtin_soundpacks_dir)?;
            log::debug!(
                "📁 Created built-in soundpacks directory: {}",
                builtin_soundpacks_dir.display()
            );
        }

        if !builtin_keyboard_dir.exists() {
            fs::create_dir_all(&builtin_keyboard_dir)?;
            log::debug!(
                "⌨️ Created built-in keyboard soundpacks directory: {}",
                builtin_keyboard_dir.display()
            );
        }

        if !builtin_mouse_dir.exists() {
            fs::create_dir_all(&builtin_mouse_dir)?;
            log::debug!(
                "🖱️ Created built-in mouse soundpacks directory: {}",
                builtin_mouse_dir.display()
            );
        }

        // Ensure custom soundpack directories exist
        let custom_soundpacks_dir = get_custom_soundpacks_dir();
        let custom_keyboard_dir = custom_soundpacks_dir.join("keyboard");
        let custom_mouse_dir = custom_soundpacks_dir.join("mouse");

        if !custom_soundpacks_dir.exists() {
            fs::create_dir_all(&custom_soundpacks_dir)?;
            log::debug!(
                "📁 Created custom soundpacks directory: {}",
                custom_soundpacks_dir.display()
            );
        }

        if !custom_keyboard_dir.exists() {
            fs::create_dir_all(&custom_keyboard_dir)?;
            log::debug!(
                "⌨️ Created custom keyboard soundpacks directory: {}",
                custom_keyboard_dir.display()
            );
        }

        if !custom_mouse_dir.exists() {
            fs::create_dir_all(&custom_mouse_dir)?;
            log::debug!(
                "🖱️ Created custom mouse soundpacks directory: {}",
                custom_mouse_dir.display()
            );
        }

        Ok(())
    }
}
