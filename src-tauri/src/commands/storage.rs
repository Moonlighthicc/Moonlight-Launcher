use crate::core::{PathManager, default_instances_dir};
use crate::services::launcher_cache;
use crate::services::shared_storage::{self, DirUsage, SharedDirInfo};
use crate::services::storage_migration::{self, InstancesDirInfo, MoveProgress, MoveResult};
use std::path::PathBuf;
use tauri::Manager;
use tauri::ipc::Channel;
use tracing::{error, info};

/// El asset protocol necesita permitir la carpeta activa, que puede estar
/// fuera de `$HOME/.moonlauncher` tras una migración o al reiniciar el launcher.
pub(crate) fn allow_current_instances_asset_scope(app: &tauri::AppHandle) -> Result<(), String> {
    let dir = PathManager::get().get_instance_dir();
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("Could not resolve {}: {e}", dir.display()))?;
    let dir = crate::core::path_security::normalize_user_path(dir);
    app.asset_protocol_scope()
        .allow_directory(&dir, true)
        .map_err(|e| format!("Could not allow access to {}: {e}", dir.display()))
}

/// Estado del directorio de instancias para la UI de ajustes.
#[tauri::command]
pub async fn get_instances_dir_info() -> Result<InstancesDirInfo, String> {
    Ok(storage_migration::get_instances_dir_info().await)
}

/// Cambia el directorio de instancias moviendo las existentes (hardlinks).
#[tauri::command]
pub async fn change_instances_dir(
    app: tauri::AppHandle,
    new_dir: String,
    on_progress: Channel<MoveProgress>,
    move_files: Option<bool>,
) -> Result<MoveResult, String> {
    info!("Changing instances directory to {}", new_dir);
    let result = storage_migration::change_instances_dir(PathBuf::from(new_dir), on_progress, move_files.unwrap_or(true), false).await;
    if let Err(ref e) = result {
        error!("Failed to change instances directory: {e}");
    }
    if let Err(scope_error) = allow_current_instances_asset_scope(&app) {
        error!("Failed to configure the asset protocol: {scope_error}");
        if result.is_ok() {
            return Err(scope_error);
        }
    }
    result
}

/// Cancela una migración de directorio en curso.
#[tauri::command]
pub fn cancel_instances_dir_change() {
    storage_migration::cancel_instances_dir_change();
}

/// Restablece el directorio de instancias al predeterminado moviendo las existentes.
#[tauri::command]
pub async fn reset_instances_dir(
    app: tauri::AppHandle,
    on_progress: Channel<MoveProgress>,
    move_files: Option<bool>,
) -> Result<MoveResult, String> {
    info!("Resetting instances directory to default");
    let result =
        storage_migration::change_instances_dir(default_instances_dir(), on_progress, move_files.unwrap_or(true), true).await;
    if let Err(ref e) = result {
        error!("Failed to reset instances directory: {e}");
    }
    if let Err(scope_error) = allow_current_instances_asset_scope(&app) {
        error!("Failed to configure the asset protocol: {scope_error}");
        if result.is_ok() {
            return Err(scope_error);
        }
    }
    result
}

/// Estado y uso de disco del directorio shared para la UI de ajustes.
#[tauri::command]
pub async fn get_shared_dir_info() -> Result<SharedDirInfo, String> {
    Ok(shared_storage::get_shared_dir_info().await)
}

/// Estado y uso de disco de la caché del launcher para la UI de ajustes.
#[tauri::command]
pub async fn get_cache_info() -> launcher_cache::CacheInfo {
    launcher_cache::get_cache_info().await
}

/// Borra la caché del launcher (manifiestos, idiomas e iconos).
#[tauri::command]
pub async fn purge_launcher_cache() -> Result<DirUsage, String> {
    info!("Purging the launcher cache");
    launcher_cache::purge_cache().await
}

/// Borra todo el contenido de shared; se re-descarga al lanzar.
#[tauri::command]
pub async fn purge_shared_dir() -> Result<DirUsage, String> {
    info!("Purging shared directory");
    let result = shared_storage::purge_shared_dir().await;
    if let Err(ref e) = result {
        error!("Failed to purge shared directory: {e}");
    }
    result
}

/// Changes the shared path and either moves or discards the old cache.
#[tauri::command]
pub async fn change_shared_dir(new_dir: String, move_files: bool) -> Result<DirUsage, String> {
    info!("Changing shared directory to {new_dir}");
    let result = shared_storage::change_shared_dir(PathBuf::from(new_dir), move_files, false).await;
    if let Err(ref e) = result {
        error!("Failed to change shared directory: {e}");
    }
    result
}

/// Restores the default shared path.
#[tauri::command]
pub async fn reset_shared_dir(move_files: bool) -> Result<DirUsage, String> {
    info!("Resetting shared directory to default");
    let result = shared_storage::change_shared_dir(crate::core::default_shared_dir(), move_files, true).await;
    if let Err(ref e) = result {
        error!("Failed to reset shared directory: {e}");
    }
    result
}
