use super::archive::stage_zip;
use super::moon::{ThemeEntry, has_theme_extension};
use crate::core::errors::FsError;
use crate::core::{PathManager, validate_identifier};
use crate::theme_watcher::ThemeWatcher;
use std::path::Path;
use tauri::command;

static INSTALL_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

#[command]
pub async fn import_moon_theme(path: String) -> Result<ThemeEntry, String> {
    if !has_theme_extension(Path::new(&path)) {
        return Err("Expected a .moontheme package".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let themes_dir = PathManager::get().get_themes_dir();
        let (entry, staged) = stage_zip(&path, themes_dir)?;
        let _install = INSTALL_LOCK.lock();
        let destination = themes_dir.join(entry.id.as_str());
        let watch = ThemeWatcher::pause_import(&destination);
        staged.install(&destination)?;
        watch.finish();
        Ok(entry)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub fn remove_theme(id: String) -> Result<(), String> {
    validate_identifier(&id)?;
    let _install = INSTALL_LOCK.lock();
    let theme_dir = PathManager::get().get_themes_dir().join(&id);
    std::fs::remove_dir_all(&theme_dir).map_err(|e| {
        FsError::Remove {
            path: theme_dir.to_string_lossy().to_string(),
            source: e,
        }
        .to_string()
    })
}
