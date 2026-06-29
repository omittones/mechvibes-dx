use dioxus::prelude::*;
pub use mechvibes_core::theme::{BuiltInTheme, Theme};

pub static THEME: GlobalSignal<Theme> = Signal::global(|| Theme::BuiltIn(BuiltInTheme::System));

pub fn use_theme() -> Signal<Theme> {
    THEME.signal()
}
