//! Uso de disco y purga del directorio **shared** (versions/libraries/assets).
//!
//! Toda su contenido se re-descarga bajo demanda (al lanzar una
//! instancia o instalar un loader), así que la purga nunca destruye datos del
//! usuario: solo caché regenerable. Al cambiar de ruta se puede conservar o
//! descartar la caché anterior sin tocar archivos ajenos del destino.

use crate::core::path_security::resolve_user_dir;
use crate::core::{PathManager, default_shared_dir};
use crate::services::{DownloadQueue, InstanceManager, SettingsManager};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};
use tracing::{error, info, warn};

static SHARED_ACCESS: LazyLock<Arc<RwLock<()>>> = LazyLock::new(|| Arc::new(RwLock::new(())));

pub(crate) type SharedGuard = Arc<OwnedRwLockReadGuard<()>>;

/// Acquire before resolving paths; keep this permission until every task/process
/// using those paths has finished. Clones may be moved into blocking workers.
pub(crate) async fn acquire() -> SharedGuard {
    Arc::new(SHARED_ACCESS.clone().read_owned().await)
}

pub(crate) fn try_exclusive() -> Result<OwnedRwLockWriteGuard<()>, String> {
    SHARED_ACCESS.clone().try_write_owned().map_err(|_| {
        "Shared storage is in use; wait for active operations to finish".into()
    })
}

/// Tope de profundidad del inventario: los árboles de Minecraft nunca llegan
/// a este nivel, y un límite acotado el recorrido ante árboles patológicos.
const MAX_SCAN_DEPTH: usize = 40;
const GAME_FOLDERS: &[&str] = &["assets", "libraries", "versions", "runtimes", "natives"];

/// Tamaño y borrabilidad de un directorio, en bytes y archivos.
#[derive(Clone, Copy, Serialize, Default)]
pub struct DirUsage {
    pub bytes: u64,
    pub files: u64,
    /// true si alguna entrada no pudo inspeccionarse o borrarse.
    pub blocked: bool,
}

impl DirUsage {
    fn add(&mut self, other: DirUsage) {
        self.bytes += other.bytes;
        self.files += other.files;
        self.blocked |= other.blocked;
    }
}

/// Estado del directorio shared para la UI de ajustes.
#[derive(Clone, Serialize)]
pub struct SharedDirInfo {
    pub current_dir: String,
    pub custom_dir: String,
    pub default_dir: String,
    /// Tamaño total de shared.
    pub total: DirUsage,
    /// Desglose por subcarpeta, ordenado por nombre.
    pub breakdown: Vec<(String, DirUsage)>,
}

/// Inventario recursivo de un directorio: bytes de archivos regulares y si el
/// árbol es medible/borrable. Los symlinks (los JRE gestionados traen cientos
/// en `legal/`) no bloquean nada: borrar el enlace no toca su destino. Errores
/// de lectura sí marcan el árbol como no borrable.
pub(crate) fn compute_usage(dir: &Path, usage: &mut DirUsage) {
    compute_usage_inner(dir, usage, 0);
}

fn compute_usage_inner(dir: &Path, usage: &mut DirUsage, depth: usize) {
    if depth > MAX_SCAN_DEPTH {
        usage.blocked = true;
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        usage.blocked = true;
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `file_type` sale de la propia entrada (d_type en Unix): evita un
        // `stat` por carpeta.
        let Ok(file_type) = entry.file_type() else {
            usage.blocked = true;
            return;
        };
        if file_type.is_symlink() {
            // Se salta: no aporta tamaño y su borrado es seguro (el enlace, no
            // el destino). Nunca se recorre su contenido.
            continue;
        }
        if file_type.is_dir() {
            compute_usage_inner(&path, usage, depth + 1);
            if usage.blocked {
                return;
            }
        } else if file_type.is_file() {
            match entry.metadata() {
                Ok(meta) => {
                    usage.bytes += meta.len();
                    usage.files += 1;
                }
                Err(_) => {
                    usage.blocked = true;
                    return;
                }
            }
        } else {
            usage.blocked = true;
            return;
        }
    }
}

/// Uso de disco de un directorio (vacío si no existe).
fn dir_usage(dir: &Path) -> DirUsage {
    let mut usage = DirUsage::default();
    if dir.exists() {
        compute_usage(dir, &mut usage);
    }
    usage
}

fn shared_breakdown() -> (DirUsage, Vec<(String, DirUsage)>) {
    let shared = PathManager::get().get_shared_dir();
    let mut total = DirUsage::default();
    let mut breakdown = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&shared) {
        let mut items: Vec<(String, DirUsage)> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !GAME_FOLDERS.contains(&name.as_str()) || !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let usage = dir_usage(&entry.path());
            total.add(usage);
            items.push((name, usage));
        }
        items.sort_by(|a, b| a.0.cmp(&b.0));
        breakdown = items;
    }
    (total, breakdown)
}

pub async fn get_shared_dir_info() -> SharedDirInfo {
    let custom_dir = SettingsManager::snapshot().custom_shared_dir;
    let (total, breakdown) = tokio::task::spawn_blocking(shared_breakdown)
        .await
        .unwrap_or_default();
    SharedDirInfo {
        current_dir: PathManager::get()
            .get_shared_dir()
            .to_string_lossy()
            .to_string(),
        custom_dir: custom_dir.to_string_lossy().to_string(),
        default_dir: default_shared_dir().to_string_lossy().to_string(),
        total,
        breakdown,
    }
}

/// Un lanzamiento en curso toca shared (nativos, versiones, descargas
/// pendientes que reapuntan la cola): purgar en paralelo podría dejar archivos
/// a medio escribir o fallar por archivos abiertos.
pub(crate) async fn shared_is_busy() -> Option<String> {
    for handle in InstanceManager::get().get_all_handles().await {
        if handle.is_busy() {
            return Some(handle.get_name().await.to_string());
        }
    }
    if !DownloadQueue::get().get_active_downloads().await.is_empty() {
        return Some(String::new());
    }
    None
}

/// Borra el contenido de `dir` (no el propio directorio). Devuelve los nombres
/// de las entradas que no se pudieron borrar. Los symlinks se eliminan como
/// enlaces: `remove_file` no sigue el enlace, así que el destino queda intacto.
pub(crate) fn clear_dir(dir: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(file_type) = entry.file_type() else {
            failures.push(name);
            continue;
        };
        let result = if file_type.is_dir() {
            std::fs::remove_dir_all(&path)
        } else if file_type.is_file() || file_type.is_symlink() {
            std::fs::remove_file(&path)
        } else {
            warn!("Unknown entry during purge: {}", path.display());
            failures.push(name);
            continue;
        };
        if let Err(e) = result {
            warn!("Could not delete {}: {}", path.display(), e);
            failures.push(name);
        }
    }
    Ok(failures)
}

async fn purge_shared_at(path: PathBuf) -> Result<DirUsage, String> {
    // El tamaño se mide antes de borrar para informar cuánto se liberó.
    let total = tokio::task::spawn_blocking({
        let path = path.clone();
        move || {
            let mut usage = DirUsage::default();
            if path.exists() {
                compute_usage(&path, &mut usage);
            }
            usage
        }
    })
    .await
    .map_err(|e| format!("The purge task failed: {e}"))?;

    if total.blocked {
        return Err(
            "The folder contains links or inaccessible files: delete them manually".to_string(),
        );
    }

    let removed = tokio::task::spawn_blocking(move || {
        if !path.exists() {
            return Ok(Vec::new());
        }
        clear_dir(&path)
    })
    .await
    .map_err(|e| format!("The purge task failed: {e}"))?
    .map_err(|e| format!("Could not read the shared folder: {e}"))?;

    for name in &removed {
        error!("Entry not deleted during shared purge: {name}");
    }
    if !removed.is_empty() {
        return Err(format!(
            "Could not delete {} entries (files in use?): {}",
            removed.len(),
            removed.join(", ")
        ));
    }
    Ok(total)
}

/// Borra todo el contenido de shared. Se re-descarga al lanzar.
pub async fn purge_shared_dir() -> Result<DirUsage, String> {
    let exclusive = try_exclusive()?;
    // Own admission inside the task: dropping an IPC future must not unlock
    // storage while spawn_blocking is still deleting its contents.
    tokio::spawn(async move {
        let _exclusive = exclusive;
        if shared_is_busy().await.is_some() {
            return Err("There are active instances or downloads; wait for them to finish".into());
        }
        let root = PathManager::get().get_shared_dir();
        let result = purge_game_folders(&root).await;
        match &result {
            Ok(usage) => info!(
                "Shared purge completed: {} bytes freed across {} files",
                usage.bytes, usage.files
            ),
            Err(e) => error!("Shared purge failed: {e}"),
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn purge_game_folders(root: &Path) -> Result<DirUsage, String> {
    let mut usage = DirUsage::default();
    for name in GAME_FOLDERS {
        let path = root.join(name);
        if safe_game_folder(&path).map_err(|e| e.to_string())? {
            usage.add(purge_shared_at(path).await?);
        }
    }
    Ok(usage)
}

fn safe_game_folder(path: &Path) -> std::io::Result<bool> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if !meta.is_dir() || is_link(&meta) {
        return Err(std::io::Error::other(format!("Not a regular directory: {}", path.display())));
    }
    Ok(true)
}

fn is_link(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 { return true; }
    }
    meta.is_symlink()
}

fn stage_game_files(source: &Path, dest: &Path, depth: usize) -> std::io::Result<()> {
    if depth > MAX_SCAN_DEPTH { return Err(std::io::Error::other("Game folder is too deep")); }
    std::fs::create_dir(dest)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&from)?;
        if is_link(&meta) {
            return Err(std::io::Error::other(format!("Cannot move link: {}", from.display())));
        } else if meta.is_dir() {
            stage_game_files(&from, &to, depth + 1)?;
        } else if meta.is_file() {
            if std::fs::hard_link(&from, &to).is_err() {
                std::fs::copy(&from, &to)?;
            }
        } else {
            return Err(std::io::Error::other(format!("Unsupported file: {}", from.display())));
        }
    }
    Ok(())
}

struct PublishedFolders(Vec<PathBuf>);

impl Drop for PublishedFolders {
    fn drop(&mut self) {
        for path in &self.0 {
            if let Err(e) = std::fs::remove_dir_all(path) {
                warn!("Could not roll back {}: {e}", path.display());
            }
        }
    }
}

fn prepare_move(source: &Path, dest: &Path, move_files: bool) -> std::io::Result<(DirUsage, PublishedFolders)> {
    let mut usage = DirUsage::default();
    let mut folders = Vec::new();
    for name in GAME_FOLDERS {
        let from = source.join(name);
        let to = dest.join(name);
        if safe_game_folder(&to)? && std::fs::read_dir(&to)?.next().transpose()?.is_some() {
            return Err(std::io::Error::other(format!("Destination game folder is not empty: {}", to.display())));
        }
        if safe_game_folder(&from)? {
            usage.add(dir_usage(&from));
            folders.push((from, to, *name));
        }
    }
    if usage.blocked { return Err(std::io::Error::other("Could not inspect the old game folder")); }
    let mut published = PublishedFolders(Vec::new());
    if move_files {
        let stage = tempfile::tempdir_in(dest)?;
        for (from, _, name) in &folders {
            stage_game_files(from, &stage.path().join(name), 0)?;
        }
        for (_, to, name) in folders {
            if safe_game_folder(&to)? { std::fs::remove_dir(&to)?; }
            std::fs::rename(stage.path().join(name), &to)?;
            published.0.push(to);
        }
    }
    Ok((usage, published))
}

pub async fn change_shared_dir(new_dir: PathBuf, move_files: bool, reset: bool) -> Result<DirUsage, String> {
    let exclusive = try_exclusive()?;
    tokio::spawn(async move {
        let _exclusive = exclusive;
        change_shared_dir_impl(new_dir, move_files, reset).await
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn change_shared_dir_impl(new_dir: PathBuf, move_files: bool, reset: bool) -> Result<DirUsage, String> {
    if !new_dir.is_absolute() {
        return Err("The path must be absolute".into());
    }
    let new_dir = resolve_user_dir(&new_dir);

    if let Some(name) = shared_is_busy().await {
        return Err(if name.is_empty() {
            "There are active downloads; wait for them to finish".to_string()
        } else {
            format!("Instance '{name}' is running; close it before changing the path")
        });
    }

    let paths = PathManager::get();
    let current_dir = resolve_user_dir(&paths.get_shared_dir());
    if new_dir == current_dir { return Ok(DirUsage::default()); }
    if new_dir.starts_with(&current_dir) || current_dir.starts_with(&new_dir) {
        return Err("The new location cannot be inside the current one (or contain it)".into());
    }
    std::fs::create_dir_all(&new_dir)
        .map_err(|e| format!("Could not create {}: {e}", new_dir.display()))?;
    let new_dir = resolve_user_dir(&new_dir);
    if new_dir.starts_with(&current_dir) || current_dir.starts_with(&new_dir) {
        return Err("The new location cannot be inside the current one (or contain it)".into());
    }
    let instance_dir = resolve_user_dir(&paths.get_instance_dir());
    if new_dir.starts_with(&instance_dir) || instance_dir.starts_with(&new_dir) {
        return Err("The new location cannot overlap the instances folder".into());
    }
    let (usage, mut published) = tokio::task::spawn_blocking({
        let source = current_dir.clone();
        let dest = new_dir.clone();
        move || prepare_move(&source, &dest, move_files)
    }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    let old_setting = SettingsManager::snapshot().custom_shared_dir;
    SettingsManager::write(|s| {
        s.custom_shared_dir = if reset { PathBuf::new() } else { new_dir.clone() };
    })
    .map_err(|e| format!("Could not save the settings: {e}"))?;
    if let Err(e) = SettingsManager::save().await {
        let _ = SettingsManager::write(|s| s.custom_shared_dir = old_setting);
        return Err(format!("Could not save the settings: {e}"));
    }
    PathManager::set_shared_dir(new_dir.clone());
    published.0.clear();
    // Only remove the original after the new path and all files are committed.
    purge_game_folders(&current_dir).await.map_err(|e| format!("New location active, but old files could not all be removed: {e}"))?;
    info!("Shared directory changed to {}", new_dir.display());
    Ok(usage)
}

#[cfg(test)]
#[path = "../tests/services/shared_storage.rs"]
mod tests;

#[cfg(test)]
#[test]
fn shared_move_preserves_source_until_commit_and_leaves_unrelated_files_alone() {
    let source = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    let old = source.path().join("assets");
    let new = dest.path().join("assets");
    std::fs::create_dir(&old).unwrap();
    std::fs::write(old.join("object"), b"abc").unwrap();
    let unrelated = dest.path().join("notes.txt");
    std::fs::write(&unrelated, b"keep").unwrap();
    let (usage, rollback) = prepare_move(source.path(), dest.path(), true).unwrap();
    assert_eq!(usage.bytes, 3);
    assert_eq!(std::fs::read(new.join("object")).unwrap(), b"abc");
    assert!(old.join("object").exists());
    assert!(prepare_move(source.path(), dest.path(), true).is_err());
    drop(rollback);
    assert!(!new.exists());
    assert_eq!(std::fs::read(unrelated).unwrap(), b"keep");
}
