pub(crate) mod archive;
mod assets;
pub(crate) mod commands;
pub(crate) mod install;
mod moon;
mod validate;

use self::moon::MoonTheme;
use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FontFace {
    pub family: CompactString,
    pub src: CompactString,
    #[serde(default)]
    pub format: Option<CompactString>,
    #[serde(default)]
    pub weight: Option<CompactString>,
    #[serde(default)]
    pub style: Option<CompactString>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThemeResponse {
    pub name: String,
    pub author: String,
    pub version: String,
    pub r#type: &'static str,
    pub variables: HashMap<String, String>,
    pub bg_image: Option<String>,
    pub bg_image_blur: Option<f64>,
    pub bg_image_opacity: Option<f64>,
    pub fonts: Vec<FontFace>,
    pub icons: HashMap<String, String>,
}

fn load_user_theme(theme_base: &Path, id: &str) -> Result<ThemeResponse, String> {
    MoonTheme::read(theme_base, id)?.resolve(theme_base)
}

pub(crate) fn available_theme(id: &str) -> bool {
    if matches!(id, "dark" | "light" | "lima") {
        return true;
    }
    id.strip_prefix("user:").is_some_and(|raw| {
        crate::core::validate_identifier(raw).is_ok()
            && MoonTheme::read(&crate::core::PathManager::get().get_themes_dir().join(raw), raw).is_ok()
    })
}
