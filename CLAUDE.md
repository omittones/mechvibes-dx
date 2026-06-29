# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build commands

```bash
# Desktop GUI (primary app) — requires Dioxus CLI (cargo install dioxus-cli)
dx serve --package mechvibes-desktop          # dev server with hot reload
dx build --package mechvibes-desktop --release

# CLI binary
cargo build -p mechvibes-cli
cargo run -p mechvibes-cli

# Build everything in the workspace
cargo build --workspace
cargo build --workspace --release

# CSS (Tailwind) — must be running alongside dx serve
pnpm run dev        # watch mode
pnpm run build      # one-shot

# Check all crates without producing artifacts
cargo check --workspace

# Run tests
cargo test --workspace
cargo test -p mechvibes-core          # test only core
```

The desktop binary needs `--console` to see log output on Windows: `cargo run -p mechvibes-desktop -- --console`.

## Workspace structure

```
crates/
  mechvibes-core/      # Shared library — no Dioxus dependency
  mechvibes-desktop/   # GUI binary — depends on core, adds Dioxus + tray + image + rfd
  mechvibes-cli/       # Headless binary — depends on core only
assets/                # Icons, fonts (shared, referenced by mechvibes-desktop)
soundpacks/            # Built-in soundpacks (keyboard/ and mouse/ subdirs)
Dioxus.toml            # Desktop bundle config (asset paths, bundle resources)
```

## Core architecture

### mechvibes-core

Pure Rust library with no UI dependency. Everything sound-related lives here.

**Key modules:**
- `audio/audio_context.rs` — `AUDIO_CONTEXT: LazyLock<Arc<Mutex<AudioContext>>>` is the global audio output. Initialized on first access; call `.lock()` to play sounds.
- `audio/sound_processor.rs` — `start_sound_processor()` spawns two threads (keyboard + mouse) that block on crossbeam channel `recv()` and play audio immediately, then forward keyboard events to the UI channel.
- `audio/soundpack_loader.rs` — `load_soundpack_from_config()` and `load_soundpack_file()` decode audio with Symphonia and push PCM data + timing maps into `AudioContext`.
- `listeners/` — `start_listeners()` spawns rdev (global) + device_query (focused-window polling) listener threads. On Linux/Wayland uses evdev instead of rdev for keyboard.
- `input_manager.rs` — `OnceLock`-backed globals: `INPUT_CHANNELS` (crossbeam receivers shared to the UI) and `WINDOW_FOCUS_STATE` (Arc<Mutex<bool>> controlling which listener handles keyboard events).
- `soundpack/cache.rs` — `SoundpackRef` is the canonical ID format: `"{builtin|custom}/{keyboard|mouse}/{folder}"`. Cache is JSON on disk.
- `state/config.rs` — `AppConfig` stored in a `LazyLock<RwLock<AppConfig>>`. Use `AppConfig::get()` for reads, `AppConfig::update(|cfg| …)` for atomic writes (saves to disk immediately).
- `theme.rs` — `Theme` and `BuiltInTheme` enums (Serde-serializable, no Dioxus). Used in `AppConfig`.

### mechvibes-desktop

Dioxus desktop binary. Adds the GUI, tray icon, and window management.

**Key additions over core:**
- `theme.rs` — `THEME: GlobalSignal<Theme>` + `use_theme()` hook. Re-exports `Theme`/`BuiltInTheme` from core.
- `state/app.rs` — Dioxus hooks (`use_app_state`, `use_state_trigger`, `use_update_info`, `use_update_info_setter`) that wrap the core globals. Re-exports init functions from `mechvibes_core::state::app`.
- `utils/config.rs` — `use_config()` Dioxus hook returning `(Signal<AppConfig>, updater_fn)`.
- `utils/theme.rs` — `use_themes()` Dioxus hook for custom theme CSS management.
- `libs/ui.rs` — Root Dioxus component `app()`. Sets up asset handler, wry event handler, sound processor, and reactive keyboard state updates.
- `libs/routes.rs` — Dioxus Router enum + `Layout` component (applies theme, background).
- `libs/tray.rs` / `libs/tray_service.rs` — System tray menu via `tray-icon` crate; `TRAY_UPDATE_SERVICE` is a global mpsc channel for cross-component tray refresh requests.

**Import convention in desktop crate:**
- Core types/functions: `mechvibes_core::audio::…`, `mechvibes_core::state::config::AppConfig`, etc.
- Desktop-local Dioxus hooks: `crate::utils::config::use_config`, `crate::theme::use_theme`, `crate::state::app::use_app_state`.
- Desktop-local libs: `crate::libs::tray::…`, `crate::libs::window_manager::…`.

### mechvibes-cli

Headless binary. Initializes config, audio, and listeners — then parks on `ctrlc`. No window, no tray. `init_window_focus_state_with_value(false)` so the rdev global listener handles all input.

## Key cross-cutting patterns

**Event flow:**
```
rdev/device_query listener threads
  → crossbeam Sender<InputEvent> (keyboard_tx / mouse_tx)
  → sound_processor threads (recv, play audio immediately)
  → ui_keyboard_tx (forwarded for UI state update only)
  → Dioxus component polls via get_input_channels().keyboard_rx
```

**Hotkey (Ctrl+Alt+M):** detected inside `input_listener.rs`, sent as `"TOGGLE_SOUND"` string on `hotkey_tx`. Desktop listens in `libs/ui.rs`; CLI listens in its own thread.

**Soundpack format:** Each soundpack is a folder with `config.json`. The `definitions` map keys (e.g. `"Space"`) to timing arrays `[[start_ms, end_ms], …]` within a single audio file (`audio_file` field). `SoundChannel` slices the PCM buffer at playback time.

**Adding a new page (desktop):** Add a route variant to `libs/routes.rs::Route`, create a component in `components/pages/`, and add a dock entry in `components/dock.rs`.

**Adding a core feature used by both binaries:** Add it to `mechvibes-core`. Import it in desktop with `mechvibes_core::…` and in CLI with `mechvibes_core::…`.

## Asset paths

In `mechvibes-desktop`, the icon is embedded at compile time relative to the crate root:
```rust
include_bytes!("../../../assets/icon.ico")  // crates/mechvibes-desktop/src/ → workspace root
```
Soundpacks are located relative to the executable at runtime via `state::paths::soundpacks::get_builtin_soundpacks_dir()`.
