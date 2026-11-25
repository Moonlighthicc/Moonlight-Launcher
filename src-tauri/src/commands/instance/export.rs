use super::launch::validate_uuid;
use crate::core::InstanceError;
use crate::services::InstanceManager;
use crate::services::instance_export::{export_to_zip, prepare_export};
use std::path::{Component, PathBuf};
use tracing::{error, info};

#[tauri::command]
pub async fn export_instance_zip(id: String, dest: String) -> Result<String, String> {
    validate_uuid(&id)?;
    info!("Exporting instance {} to '{}'", id, dest);

    let manager = InstanceManager::get();
    let Some(handle) = manager.get_handle(&id).await else {
        error!("Instance {} not found for export", id);
        return Err(InstanceError::NotFound.into());
    };

    let files_guard = handle.try_lock_files()?;
    if handle.is_busy() {
        error!("Attempt to export busy instance {}", id);
        return Err(InstanceError::Busy.into());
    }

    let input = prepare_export(&handle)
        .await
        .map_err(|e| format!("Failed to prepare export: {e}"))?;
    let dest_path = PathBuf::from(dest);

    if dest_path.file_name().is_none() {
        return Err("Invalid destination path".into());
    }
    if dest_path.extension().map(|e| e != "zip").unwrap_or(true) {
        return Err("The export must be a .zip file".into());
    }

    // Reject destinations containing parent directory references to avoid overwriting
    // files outside the intended export location.
    for component in dest_path.components() {
        if matches!(component, Component::ParentDir) {
            return Err("Invalid destination path: contains '..'".into());
        }
    }

    let output = tokio::task::spawn_blocking(move || {
        let _files_guard = files_guard;
        export_to_zip(&input, &dest_path)
    })
    .await
    .map_err(|e| format!("Export task failed: {e}"))?
    .map_err(|e| format!("Failed to export instance: {e}"))?;

    let out_str = output.to_string_lossy().to_string();
    info!("Instance {} exported to '{}'", id, out_str);
    Ok(out_str)
}
