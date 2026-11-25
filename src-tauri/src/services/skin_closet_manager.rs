use crate::core::{PathManager, atomic_file};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{error, info};

const SKIN_CLOSET_VERSION: u32 = 1;
const MAX_ENTRIES_PER_USER: usize = 20;
pub const SHARED_SKIN_LIBRARY: &str = "library";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinClosetEntry {
    pub id: String,
    pub url: String,
    pub local_path: String,
    pub variant: String,
    #[serde(default)]
    pub alias: String,
    pub saved_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinClosetData {
    pub version: u32,
    pub entries: HashMap<String, Vec<SkinClosetEntry>>,
}

impl Default for SkinClosetData {
    fn default() -> Self {
        Self {
            version: SKIN_CLOSET_VERSION,
            entries: HashMap::new(),
        }
    }
}

static SKIN_CLOSET: LazyLock<RwLock<SkinClosetData>> =
    LazyLock::new(|| RwLock::new(SkinClosetData::load()));

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl SkinClosetData {
    pub fn file_path() -> PathBuf {
        PathManager::get()
            .get_settings_dir()
            .join("skin_closet.json")
    }

    fn load() -> Self {
        let path = Self::file_path();
        if !path.exists() {
            info!("skin_closet.json does not exist, a new one will be created");
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<SkinClosetData>(&content) {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to parse skin_closet.json: {}", e);
                    Self::default()
                }
            },
            Err(e) => {
                error!("Failed to read skin_closet.json: {}", e);
                Self::default()
            }
        }
    }

    pub async fn save(&self) -> Result<(), String> {
        self.save_at(Self::file_path()).await
    }

    async fn save_at(&self, path: PathBuf) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| format!("Failed to serialize the skin closet: {}", e))?;
        tokio::task::spawn_blocking(move || atomic_file::write(&path, &json))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("Failed to save the skin closet: {}", e))?;
        Ok(())
    }
}

pub struct SkinClosetManager;

fn skin_filename(alias: &str) -> Result<String, String> {
    let device = alias
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            device.strip_prefix(*prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
    if alias.is_empty()
        || alias.chars().count() > 100
        || alias.ends_with('.')
        || reserved
        || alias
            .chars()
            .any(|ch| ch.is_control() || "<>:\"/\\|?*".contains(ch))
    {
        return Err("This skin name cannot be used as a file name".into());
    }
    Ok(format!("{alias}.png"))
}

async fn require_regular_file(path: &Path) -> io::Result<()> {
    if tokio::fs::symlink_metadata(path)
        .await?
        .file_type()
        .is_file()
    {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected a regular skin file",
        ))
    }
}

async fn write_new_png(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    let result = async {
        file.write_all(bytes).await?;
        file.sync_all().await
    }
    .await;
    drop(file);
    if result.is_err() {
        let _ = tokio::fs::remove_file(path).await;
    }
    result
}

async fn write_named_png(
    data: &mut SkinClosetData,
    dir: &Path,
    preferred: Option<&str>,
    bytes: &[u8],
) -> Result<(String, PathBuf), String> {
    let preferred = preferred
        .map(str::trim)
        .filter(|alias| skin_filename(alias).is_ok())
        .map(str::to_owned);
    let candidates = preferred.into_iter().chain((0..=u64::MAX).map(|number| {
        if number == 0 {
            "skin".to_owned()
        } else {
            format!("skin({number})")
        }
    }));
    for alias in candidates {
        let path = dir.join(skin_filename(&alias)?);
        match write_new_png(&path, bytes).await {
            Ok(()) => {
                // An externally deleted file can still be in the index. Do not let
                // its old entry refer to (or delete) the newly created skin.
                let filename = path.file_name().unwrap().to_string_lossy();
                for entries in data.entries.values_mut() {
                    entries.retain(|entry| {
                        let old = Path::new(&entry.local_path);
                        old != path
                            && !(cfg!(windows)
                                && old.parent() == Some(dir)
                                && old.file_name().is_some_and(|name| {
                                    name.to_string_lossy().to_lowercase() == filename.to_lowercase()
                                }))
                    });
                }
                return Ok((alias, path));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Skin name numbering exhausted".into())
}

async fn migrate_legacy_files(
    current: &mut SkinClosetData,
    dir: &Path,
    index_path: &Path,
) -> Result<(), String> {
    let legacy: Vec<_> = current
        .entries
        .iter()
        .flat_map(|(scope, list)| {
            list.iter().filter_map(move |entry| {
                let source = Path::new(&entry.local_path);
                let parent = source.parent()?;
                // Only migrate paths with the old <scope>/<id>.png layout.
                (parent.parent() == Some(dir)
                    && parent
                        .file_name()
                        .is_some_and(|name| name == std::ffi::OsStr::new(scope))
                    && source.extension().is_some_and(|ext| ext == "png"))
                .then(|| (scope.clone(), entry.clone()))
            })
        })
        .collect();
    for (scope, entry) in legacy {
        let mut data = current.clone();
        let source = Path::new(&entry.local_path);
        let parent = source.parent().unwrap();
        match tokio::fs::symlink_metadata(parent).await {
            Ok(meta) if meta.file_type().is_dir() => {}
            Ok(_) => continue,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        }
        match require_regular_file(source).await {
            Ok(()) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidInput
                ) =>
            {
                continue;
            }
            Err(e) => return Err(e.to_string()),
        }
        let bytes = tokio::fs::read(source).await.map_err(|e| e.to_string())?;
        let (alias, target) = write_named_png(&mut data, dir, Some(&entry.alias), &bytes).await?;
        let updated = data.entries[&scope]
            .iter()
            .position(|candidate| {
                candidate.id == entry.id && candidate.local_path == entry.local_path
            })
            .expect("the legacy entry is outside the target directory");
        let updated = &mut data.entries.get_mut(&scope).unwrap()[updated];
        updated.alias = alias;
        updated.local_path = target.to_string_lossy().into_owned();
        if let Err(e) = data.save_at(index_path.to_path_buf()).await {
            let _ = tokio::fs::remove_file(target).await;
            return Err(e);
        }
        *current = data;
        // Commit the index before removing the old copy.
        if let Err(e) = tokio::fs::remove_file(source).await {
            error!("Could not remove old skin after migration: {e}");
        }
        let _ = tokio::fs::remove_dir(parent).await;
    }
    // Never delete unknown files left in a legacy directory.
    let _ = tokio::fs::remove_dir(dir.join(SHARED_SKIN_LIBRARY)).await;
    Ok(())
}

impl SkinClosetManager {
    pub async fn prepare_dir() -> Result<PathBuf, String> {
        let mut current = SKIN_CLOSET.write().await;
        let dir = PathManager::get().get_skin_closet_dir();
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| e.to_string())?;
        migrate_legacy_files(&mut current, dir, &SkinClosetData::file_path()).await?;
        Ok(dir.to_path_buf())
    }

    pub async fn get_entries(uuid: &str) -> Result<Vec<SkinClosetEntry>, String> {
        Self::prepare_dir().await?;
        let data = SKIN_CLOSET.read().await;
        Ok(data.entries.get(uuid).cloned().unwrap_or_default())
    }

    pub async fn save_png(
        uuid: &str,
        mut entry: SkinClosetEntry,
        bytes: &[u8],
    ) -> Result<(), String> {
        let dir = Self::prepare_dir().await?;
        let mut current = SKIN_CLOSET.write().await;
        let mut data = current.clone();
        if data
            .entries
            .get(uuid)
            .is_some_and(|list| list.iter().any(|e| e.id == entry.id))
        {
            return Ok(());
        }
        let (alias, path) = write_named_png(&mut data, &dir, None, bytes).await?;
        entry.alias = alias;
        entry.local_path = path.to_string_lossy().into_owned();
        let list = data.entries.entry(uuid.to_string()).or_default();
        list.insert(0, entry);
        list.sort_by_key(|a| std::cmp::Reverse(a.saved_at));
        let removed = if uuid != SHARED_SKIN_LIBRARY && list.len() > MAX_ENTRIES_PER_USER {
            list.split_off(MAX_ENTRIES_PER_USER)
        } else {
            Vec::new()
        };
        if let Err(error) = data.save().await {
            let _ = tokio::fs::remove_file(path).await;
            return Err(error);
        }
        *current = data;
        for entry in removed {
            if let Err(e) = Self::remove_image(&entry.local_path).await {
                error!("Failed to delete expired closet image: {}", e);
            }
        }
        Ok(())
    }

    pub async fn remove_entry(uuid: &str, entry_id: &str) -> Result<(), String> {
        Self::prepare_dir().await?;
        let mut current = SKIN_CLOSET.write().await;
        let mut data = current.clone();
        let Some(list) = data.entries.get_mut(uuid) else {
            return Ok(());
        };
        let Some(index) = list.iter().position(|e| e.id == entry_id) else {
            return Ok(());
        };
        let entry = list.remove(index);
        data.save().await?;
        *current = data;
        if let Err(e) = Self::remove_image(&entry.local_path).await {
            error!("Failed to delete a local skin image: {}", e);
        }
        Ok(())
    }

    pub async fn update_alias(uuid: &str, entry_id: &str, alias: String) -> Result<(), String> {
        let dir = Self::prepare_dir().await?;
        let alias = alias.trim();
        let target = dir.join(skin_filename(alias)?);
        let mut current = SKIN_CLOSET.write().await;
        let mut data = current.clone();
        if let Some(list) = data.entries.get_mut(uuid)
            && let Some(entry) = list.iter_mut().find(|e| e.id == entry_id)
        {
            let source = PathBuf::from(&entry.local_path);
            if source == target {
                return Ok(());
            }
            if source.parent() != Some(dir.as_path()) {
                return Err("Invalid local skin path".into());
            }
            require_regular_file(&source)
                .await
                .map_err(|e| e.to_string())?;
            let case_only = cfg!(windows)
                && match tokio::fs::canonicalize(&target).await {
                    Ok(path) => tokio::fs::canonicalize(&source)
                        .await
                        .is_ok_and(|source| source == path),
                    Err(_) => false,
                };
            if case_only {
                tokio::fs::rename(&source, &target)
                    .await
                    .map_err(|e| e.to_string())?;
            } else {
                let bytes = tokio::fs::read(&source).await.map_err(|e| e.to_string())?;
                write_new_png(&target, &bytes).await.map_err(|e| {
                    if e.kind() == io::ErrorKind::AlreadyExists {
                        "A skin with this name already exists".to_owned()
                    } else {
                        e.to_string()
                    }
                })?;
            }
            entry.alias = alias.to_owned();
            entry.local_path = target.to_string_lossy().into_owned();
            if let Err(error) = data.save().await {
                if case_only {
                    let _ = tokio::fs::rename(&target, &source).await;
                } else {
                    let _ = tokio::fs::remove_file(&target).await;
                }
                return Err(error);
            }
            *current = data;
            if !case_only && let Err(e) = tokio::fs::remove_file(&source).await {
                error!("Could not remove the old skin file after renaming: {e}");
            }
        }
        Ok(())
    }

    async fn remove_image(local_path: &str) -> Result<(), String> {
        let path = Path::new(local_path);
        if path.parent() == Some(PathManager::get().get_skin_closet_dir()) && path.exists() {
            tokio::fs::remove_file(path)
                .await
                .map_err(|e| format!("Failed to delete closet image: {}", e))?;
        }
        Ok(())
    }
}

pub fn now_ts() -> i64 {
    current_timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_skin_names_fill_gaps_without_overwriting_files() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let mut data: SkinClosetData = serde_json::from_value(serde_json::json!({
            "version": 1,
            "last_skin_number": 6,
            "entries": {}
        }))
        .unwrap();
        for name in ["skin(1).png", "skin(6).png"] {
            std::fs::write(dir.join(name), b"existing").unwrap();
        }
        for expected in ["skin", "skin(2)"] {
            let (alias, path) = write_named_png(&mut data, dir, None, b"new").await.unwrap();
            assert_eq!(alias, expected);
            assert_eq!(path, dir.join(format!("{expected}.png")));
            assert_eq!(std::fs::read(path).unwrap(), b"new");
        }
        for name in ["skin(1).png", "skin(6).png"] {
            assert_eq!(std::fs::read(dir.join(name)).unwrap(), b"existing");
        }
    }

    #[tokio::test]
    async fn reusing_a_deleted_skin_filename_removes_its_stale_entry() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let mut data = SkinClosetData::default();
        data.entries.insert(
            SHARED_SKIN_LIBRARY.into(),
            vec![SkinClosetEntry {
                id: "deleted".into(),
                url: String::new(),
                local_path: dir.join("skin.png").to_string_lossy().into_owned(),
                variant: "CLASSIC".into(),
                alias: "skin".into(),
                saved_at: 0,
            }],
        );
        let (alias, _) = write_named_png(&mut data, dir, None, b"new").await.unwrap();
        assert_eq!(alias, "skin");
        assert!(data.entries[SHARED_SKIN_LIBRARY].is_empty());
    }

    #[test]
    fn skin_names_are_safe_as_windows_filenames() {
        assert_eq!(skin_filename("Лунный скин").unwrap(), "Лунный скин.png");
        for alias in ["", "../other", "a\\b", "AUX", "lpt1.backup", "trailing."] {
            assert!(skin_filename(alias).is_err(), "accepted {alias:?}");
        }
    }

    #[tokio::test]
    async fn migrates_a_known_legacy_png_without_overwriting_an_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("skins");
        let library = dir.join(SHARED_SKIN_LIBRARY);
        std::fs::create_dir_all(&library).unwrap();
        let source = library.join("old-id.png");
        std::fs::write(&source, b"old skin").unwrap();
        std::fs::write(dir.join("skin(4).png"), b"existing file").unwrap();
        let index = temp.path().join("closet.json");
        let mut data = SkinClosetData::default();
        data.entries.insert(
            SHARED_SKIN_LIBRARY.into(),
            vec![SkinClosetEntry {
                id: "old-id".into(),
                url: String::new(),
                local_path: source.to_string_lossy().into_owned(),
                variant: "CLASSIC".into(),
                alias: "skin(4)".into(),
                saved_at: 0,
            }],
        );
        migrate_legacy_files(&mut data, &dir, &index).await.unwrap();
        migrate_legacy_files(&mut data, &dir, &index).await.unwrap();
        let saved: SkinClosetData = serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
        let entry = &saved.entries[SHARED_SKIN_LIBRARY][0];
        assert_eq!(entry.alias, "skin");
        assert_eq!(Path::new(&entry.local_path), dir.join("skin.png"));
        assert_eq!(std::fs::read(&entry.local_path).unwrap(), b"old skin");
        assert_eq!(
            std::fs::read(dir.join("skin(4).png")).unwrap(),
            b"existing file"
        );
        assert!(!library.exists());
    }
}
