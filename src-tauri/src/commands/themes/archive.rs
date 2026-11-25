use super::load_user_theme;
use super::moon::{MANIFEST, MAX_MANIFEST_SIZE, MoonTheme, ThemeEntry, has_theme_extension};
use crate::core::errors::{CoreError, FsError};
use crate::core::{PathManager, sanitize_path, validate_identifier};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use tauri::command;
use tracing::{info, warn};

pub(super) struct StagedTheme {
    pub(super) root: PathBuf,
}

impl StagedTheme {
    pub(super) fn new(themes_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(themes_dir).map_err(|e| e.to_string())?;
        let root = themes_dir.join(format!(".theme-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        let staged = Self { root };
        std::fs::create_dir(staged.root.join("theme")).map_err(|e| e.to_string())?;
        Ok(staged)
    }

    pub(super) fn install(&self, destination: &Path) -> Result<(), String> {
        let backup = self.root.join("backup");
        let had_previous = match std::fs::symlink_metadata(destination) {
            Ok(meta) if meta.is_dir() => true,
            Ok(_) => {
                return Err(format!(
                    "Theme destination is not a directory: {:?}",
                    destination
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.to_string()),
        };
        if had_previous {
            std::fs::rename(destination, &backup).map_err(|e| e.to_string())?;
        }
        if let Err(e) = std::fs::rename(self.root.join("theme"), destination) {
            if had_previous && let Err(rollback) = std::fs::rename(&backup, destination) {
                return Err(format!(
                    "Failed to install theme: {e}; failed to restore: {rollback}. Copy kept at {}",
                    backup.display()
                ));
            }
            return Err(format!("Failed to install theme: {e}"));
        }
        if had_previous && let Err(e) = std::fs::remove_dir_all(&backup) {
            warn!(
                "Theme installed; could not clean up backup {:?}: {}",
                backup, e
            );
        }
        Ok(())
    }
}

impl Drop for StagedTheme {
    fn drop(&mut self) {
        if self.root.join("backup").exists() {
            warn!("Theme backup kept at {:?}", self.root);
        } else if let Err(e) = std::fs::remove_dir_all(&self.root) {
            warn!("Could not clean up staging {:?}: {}", self.root, e);
        }
    }
}

pub(super) fn stage_zip(
    zip_path: &str,
    themes_dir: &Path,
) -> Result<(ThemeEntry, StagedTheme), String> {
    let source = std::path::Path::new(zip_path);
    if !source.exists() {
        return Err(FsError::NotFound(zip_path.to_string()).to_string());
    }

    let file = std::fs::File::open(source).map_err(|e| {
        FsError::ReadFile {
            path: zip_path.to_string(),
            source: e,
        }
        .to_string()
    })?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| CoreError::Other(format!("Invalid ZIP file: {}", e)).to_string())?;

    const LIMIT: u64 = 64 * 1024 * 1024;
    if archive.len() > 512 {
        return Err("Theme archive has too many entries".into());
    }
    let mut remaining = LIMIT;
    let target = MANIFEST;
    let mut manifest = None;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| CoreError::Other(format!("Failed to read ZIP: {}", e)).to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?.replace('\\', "/");
        if name == target || name.ends_with(&format!("/{target}")) {
            if manifest.is_some() {
                return Err(
                    CoreError::Other(format!("Invalid ZIP: multiple {} found", target)).to_string(),
                );
            }
            manifest = Some((i, name));
        }
    }
    let Some((entry_index, entry_name)) = manifest else {
        return Err(format!("Missing {MANIFEST}"));
    };

    let content = {
        let mut buf = String::new();
        let entry = archive.by_index(entry_index).map_err(|e| {
            CoreError::Other(format!("Failed to read {}: {}", target, e)).to_string()
        })?;
        entry
            .take(MAX_MANIFEST_SIZE + 1)
            .read_to_string(&mut buf)
            .map_err(|e| {
                CoreError::Other(format!("Failed to read {}: {}", target, e)).to_string()
            })?;
        if buf.len() as u64 > MAX_MANIFEST_SIZE {
            return Err(format!("{MANIFEST} is too large"));
        }
        buf
    };

    let theme = MoonTheme::parse(&content)?;
    let theme_id = theme.id.clone();

    let prefix = if entry_name == target {
        String::new()
    } else {
        entry_name.strip_suffix(target).unwrap_or("").to_string()
    };
    let prefix_path = Path::new(&prefix);
    if !prefix.is_empty() {
        for component in prefix_path.components() {
            if !matches!(component, Component::Normal(_) | Component::CurDir) {
                return Err(
                    CoreError::Other(format!("Invalid ZIP prefix: '{}'", prefix)).to_string(),
                );
            }
        }
    }
    let prefix_path: PathBuf = prefix_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();

    let staged = StagedTheme::new(themes_dir)?;
    let theme_dir = staged.root.join("theme");
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| CoreError::Other(format!("Failed to read ZIP: {}", e)).to_string())?;

        let name = entry.name().map_err(|e| e.to_string())?.replace('\\', "/");
        if name.contains([':', '\0'])
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!("Unsafe ZIP entry: {name}"));
        }
        sanitize_path(&theme_dir, Path::new(&name))?;
        let enclosed: PathBuf = Path::new(&name)
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name),
                _ => None,
            })
            .collect();

        let relative = match enclosed.strip_prefix(&prefix_path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if relative.as_os_str().is_empty() {
            continue;
        }

        let out_path = sanitize_path(&theme_dir, relative)?;

        if name.ends_with('/') {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("Failed to create directory {:?}: {}", out_path, e))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
        }

        let mut out_file = std::fs::File::create_new(&out_path).map_err(|e| {
            FsError::WriteFile {
                path: out_path.to_string_lossy().to_string(),
                source: e,
            }
            .to_string()
        })?;

        let written = std::io::copy(&mut entry.by_ref().take(remaining + 1), &mut out_file)
            .map_err(|e| {
                FsError::WriteFile {
                    path: out_path.to_string_lossy().to_string(),
                    source: e,
                }
                .to_string()
            })?;
        remaining = remaining
            .checked_sub(written)
            .ok_or("Theme archive exceeds 64 MiB unpacked")?;
    }

    load_user_theme(&theme_dir, &theme_id)?;
    Ok((theme.entry(), staged))
}

#[command]
pub fn export_theme(id: String, dest: String) -> Result<String, String> {
    info!("Exporting theme '{}' to '{}'", id, dest);
    validate_identifier(&id)?;

    let output = std::path::PathBuf::from(&dest);
    if output.file_name().is_none() {
        return Err(FsError::InvalidPath(dest.clone()).to_string());
    }
    if !has_theme_extension(&output) {
        return Err("The export must be a .moontheme file".into());
    }

    for component in output.components() {
        if matches!(component, Component::ParentDir) {
            return Err(FsError::InvalidPath(dest.clone()).to_string());
        }
    }

    let theme_dir = PathManager::get().get_themes_dir().join(&id);
    if !theme_dir.exists() {
        return Err(FsError::NotFound(theme_dir.to_string_lossy().to_string()).to_string());
    }

    load_user_theme(&theme_dir, &id)?;
    export_theme_directory(&theme_dir, &output)?;
    let out_path = output.to_string_lossy().to_string();
    info!("Theme exported to '{}'", out_path);
    Ok(out_path)
}

pub(super) fn export_theme_directory(theme_dir: &Path, output: &Path) -> Result<(), String> {
    let file = std::fs::File::create(output).map_err(|e| FsError::WriteFile {
        path: output.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    fn add_dir_to_zip(
        zip: &mut zip::ZipWriter<std::fs::File>,
        dir: &std::path::Path,
        prefix: &std::path::Path,
        options: zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| FsError::ReadDir {
            path: dir.to_string_lossy().to_string(),
            source: e,
        })? {
            let entry = entry.map_err(|e| FsError::ReadDir {
                path: dir.to_string_lossy().to_string(),
                source: e,
            })?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if !kind.is_dir() && !kind.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(prefix)
                .map_err(|_| CoreError::Other("Failed to compute the relative path".into()))?;
            if kind.is_dir() {
                zip.add_directory(relative.to_string_lossy().replace('\\', "/"), options)
                    .map_err(|e| CoreError::Other(format!("Failed to add directory: {}", e)))?;
                add_dir_to_zip(zip, &path, prefix, options)?;
            } else {
                let data = std::fs::read(&path).map_err(|e| FsError::ReadFile {
                    path: path.to_string_lossy().to_string(),
                    source: e,
                })?;
                zip.start_file(relative.to_string_lossy().replace('\\', "/"), options)
                    .map_err(|e| CoreError::Other(format!("Failed to add file: {}", e)))?;
                std::io::Write::write_all(&mut *zip, &data)
                    .map_err(|e| CoreError::Other(format!("Failed to write ZIP: {}", e)))?;
            }
        }
        Ok(())
    }

    add_dir_to_zip(&mut zip_writer, theme_dir, theme_dir, options)?;

    zip_writer
        .finish()
        .map_err(|e| CoreError::Other(format!("Failed to finalize ZIP: {}", e)))?;

    Ok(())
}

#[cfg(test)]
#[path = "../../tests/commands/themes/import.rs"]
mod import_tests;
