use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Theme {
    BuiltIn(BuiltInTheme),
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, EnumIter)]
pub enum BuiltInTheme {
    Light,
    Dark,
    System,
    Cupcake,
    Bumblebee,
    Emerald,
    Corporate,
    Synthwave,
    Retro,
    Cyberpunk,
    Valentine,
    Halloween,
    Garden,
    Forest,
    Aqua,
    Lofi,
    Pastel,
    Fantasy,
    Wireframe,
    Black,
    Luxury,
    Dracula,
    Cmyk,
    Autumn,
    Business,
    Acid,
    Lemonade,
    Night,
    Coffee,
    Winter,
    Dim,
    Nord,
    Sunset,
    Abyss,
    Silk,
    Caramellatte,
}

impl BuiltInTheme {
    pub fn to_daisy_theme(&self) -> String {
        match self {
            BuiltInTheme::System => "light".to_string(),
            _ => format!("{:?}", self).to_lowercase(),
        }
    }

    pub fn all() -> Vec<BuiltInTheme> {
        BuiltInTheme::iter().collect()
    }
}

impl Theme {
    pub fn to_daisy_theme(&self) -> String {
        match self {
            Theme::BuiltIn(builtin) => builtin.to_daisy_theme(),
            Theme::Custom(name) => format!("custom-{}", name),
        }
    }
}
