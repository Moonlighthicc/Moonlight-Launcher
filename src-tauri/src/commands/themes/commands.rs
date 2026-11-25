use super::moon::{MoonTheme, ThemeEntry};
use super::validate::{ThemeValidationReport, validate_theme_at};
use super::{ThemeResponse, load_user_theme};
use crate::core::errors::FsError;
use crate::core::{PathManager, validate_identifier};
use crate::services::SettingsManager;
use std::path::Path;
use tauri::command;
use tracing::info;

#[command]
pub fn list_themes() -> Result<Vec<ThemeEntry>, String> {
    let themes_dir = PathManager::get().get_themes_dir();
    let mut themes = Vec::new();

    let entries = match std::fs::read_dir(themes_dir) {
        Ok(e) => e,
        Err(_) => {
            info!("Themes directory not found: {:?}", themes_dir);
            return Ok(themes);
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().map(|name| name.to_string_lossy()) else {
            continue;
        };
        if let Some(theme) = list_theme_entry(&path, &id) {
            themes.push(theme);
        }
    }

    info!("{} themes listed", themes.len());
    Ok(themes)
}

fn list_theme_entry(path: &Path, id: &str) -> Option<ThemeEntry> {
    MoonTheme::read(path, id).ok().map(|theme| theme.entry())
}

#[command]
pub fn get_user_theme(id: String) -> Result<ThemeResponse, String> {
    info!("Reading theme '{}'", id);
    validate_identifier(&id)?;
    let theme_base = PathManager::get().get_themes_dir().join(&id);
    load_user_theme(&theme_base, &id)
}

#[command]
pub fn validate_theme(id: String) -> Result<ThemeValidationReport, String> {
    info!("Validating theme '{}'", id);
    validate_identifier(&id)?;
    let theme_base = PathManager::get().get_themes_dir().join(&id);
    if !theme_base.is_dir() {
        return Err(FsError::NotFound(theme_base.to_string_lossy().to_string()).to_string());
    }
    Ok(validate_theme_at(&theme_base, &id))
}

#[command]
pub async fn set_theme(id: String) -> Result<(), String> {
    info!("Switching theme to '{}'", id);
    SettingsManager::write(|s| {
        s.theme = id.clone().into();
    })?;

    SettingsManager::save().await?;

    info!("Theme switched to '{}'", id);
    Ok(())
}

#[command]
pub fn get_current_theme() -> Result<String, String> {
    let theme = SettingsManager::read().theme.to_string();
    info!("Current theme: '{}'", theme);
    Ok(theme)
}

#[command]
pub fn get_themes_dir_path() -> Result<String, String> {
    let path = PathManager::get()
        .get_themes_dir()
        .to_string_lossy()
        .to_string();
    info!("Themes directory path: {}", path);
    Ok(path)
}

#[cfg(test)]
#[path = "../../tests/commands/themes/list.rs"]
mod tests;
