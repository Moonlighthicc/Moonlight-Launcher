use super::assets::{extract_preview, validate_background_image, validate_theme_icon};
use super::{FontFace, ThemeResponse};
use crate::core::{safe_join, validate_identifier};
use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path};

pub(super) const MANIFEST: &str = "moon-theme.json";
pub(super) const EXTENSION: &str = "moontheme";
pub(super) const MAX_MANIFEST_SIZE: u64 = 128 * 1024;

#[derive(Debug, Serialize, Clone)]
pub struct ThemePreview {
    pub bg: String,
    pub accent: String,
    pub text: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThemeEntry {
    pub id: CompactString,
    pub name: CompactString,
    pub author: CompactString,
    pub version: CompactString,
    pub r#type: &'static str,
    pub preview: ThemePreview,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MoonTheme {
    format: String,
    format_version: u32,
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    tokens: HashMap<String, String>,
    background: Option<Background>,
    #[serde(default)]
    fonts: Vec<FontFace>,
    #[serde(default)]
    icons: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Background {
    path: String,
    opacity: Option<f64>,
    blur: Option<f64>,
}

impl MoonTheme {
    pub(super) fn parse(content: &str) -> Result<Self, String> {
        let theme: Self = serde_json::from_str(content).map_err(|e| format!("{MANIFEST}: {e}"))?;
        if theme.format != "moonlauncher" || theme.format_version != 1 {
            return Err("Expected format 'moonlauncher', format_version 1".into());
        }
        validate_identifier(&theme.id)?;
        if theme.id.is_empty()
            || theme.id.len() > 64
            || !theme.id.as_bytes()[0].is_ascii_alphanumeric()
            || !theme
                .id
                .bytes()
                .all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'))
        {
            return Err("Theme id must use lowercase ASCII letters, digits, '-' or '_' and begin with a letter or digit".into());
        }
        if theme.name.trim().is_empty() {
            return Err("Theme name cannot be empty".into());
        }
        for (key, value) in &theme.tokens {
            let valid_key = key.strip_prefix("--").is_some_and(|name| {
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'-'))
            });
            if !valid_key
                || value.trim().is_empty()
                || value.len() > 512
                || value.contains([';', '{', '}', '\\', '/', '@', '!'])
                || value.chars().any(char::is_control)
                || value.to_ascii_lowercase().contains("url(")
                || value.to_ascii_lowercase().contains("image-set(")
            {
                return Err(format!("Invalid theme token: {key}"));
            }
        }
        if let Some(bg) = &theme.background
            && (bg.opacity.is_some_and(|n| !(0.0..=1.0).contains(&n))
                || bg.blur.is_some_and(|n| !(0.0..=32.0).contains(&n)))
        {
            return Err("Background opacity must be 0–1 and blur 0–32".into());
        }
        if theme.fonts.len() > 20 || theme.icons.len() > 100 {
            return Err("A theme can contain at most 20 fonts and 100 icons".into());
        }
        Ok(theme)
    }

    pub(super) fn read(dir: &Path, id: &str) -> Result<Self, String> {
        let path = dir.join(MANIFEST);
        let file = std::fs::File::open(&path).map_err(|e| format!("{MANIFEST}: {e}"))?;
        let mut text = String::new();
        file.take(MAX_MANIFEST_SIZE + 1)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() as u64 > MAX_MANIFEST_SIZE {
            return Err(format!("{MANIFEST} is too large"));
        }
        let theme = Self::parse(&text)?;
        if theme.id != id {
            return Err(format!("Theme id '{}' does not match '{id}'", theme.id));
        }
        Ok(theme)
    }

    pub(super) fn entry(&self) -> ThemeEntry {
        ThemeEntry {
            id: self.id.as_str().into(),
            name: self.name.as_str().into(),
            author: self.author.as_str().into(),
            version: self.version.as_str().into(),
            r#type: "user",
            preview: extract_preview(&self.tokens),
        }
    }

    pub(super) fn resolve(self, dir: &Path) -> Result<ThemeResponse, String> {
        let mut bg_image = None;
        if let Some(bg) = &self.background {
            let path = local_asset(dir, &bg.path)?;
            validate_background_image(Path::new(&path))
                .map_err(|_| format!("Missing, invalid or oversized background: {}", bg.path))?;
            bg_image = Some(path);
        }
        let mut fonts = self.fonts;
        for font in &mut fonts {
            let path = local_asset(dir, &font.src)?;
            let ext = Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if !matches!(
                ext.to_ascii_lowercase().as_str(),
                "woff2" | "woff" | "ttf" | "otf"
            ) {
                return Err(format!("Unsupported font: {}", font.src));
            }
            font.src = path.into();
        }
        let mut icons = HashMap::new();
        for (key, reference) in self.icons {
            let path = local_asset(dir, &reference)?;
            if !validate_theme_icon(&path) {
                return Err(format!("Invalid icon: {reference}"));
            }
            icons.insert(key, path);
        }
        Ok(ThemeResponse {
            name: self.name,
            author: self.author,
            version: self.version,
            r#type: "user",
            variables: self.tokens,
            bg_image,
            bg_image_blur: self.background.as_ref().and_then(|bg| bg.blur),
            bg_image_opacity: self.background.as_ref().and_then(|bg| bg.opacity),
            fonts,
            icons,
        })
    }
}

fn local_asset(dir: &Path, reference: &str) -> Result<String, String> {
    let invalid = || format!("Expected a local theme asset: {reference}");
    if reference.is_empty()
        || reference.contains([':', '\\'])
        || !Path::new(reference)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(invalid());
    }
    let root = dir.canonicalize().map_err(|_| invalid())?;
    let path = safe_join(dir, reference)?;
    let canonical = path.canonicalize().map_err(|_| invalid())?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(invalid());
    }
    Ok(path.to_string_lossy().into_owned())
}

pub(super) fn has_theme_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(EXTENSION))
}
