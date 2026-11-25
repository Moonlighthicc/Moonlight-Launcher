use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceDensity {
    Compact,
    Comfortable,
    #[default]
    #[serde(other)]
    Theme,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct InterfacePreferences {
    pub scale: u32,
    pub density: InterfaceDensity,
    /// "theme" usa la fuente del tema; cualquier otro valor es una familia
    /// empaquetada en el proyecto que sobreescribe --font-family.
    pub font: String,
}

impl Default for InterfacePreferences {
    fn default() -> Self {
        Self {
            scale: 100,
            density: InterfaceDensity::Theme,
            font: "theme".to_string(),
        }
    }
}

impl InterfacePreferences {
    pub fn normalize(&mut self) {
        if self.scale == 125 {
            self.scale = 110;
        } else if ![90, 100, 110, 150, 180, 200, 250, 300].contains(&self.scale) {
            self.scale = 100;
        }
        if self.font.trim().is_empty() {
            self.font = "theme".to_string();
        }
    }
}
