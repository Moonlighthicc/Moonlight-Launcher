use crate::services::{InstanceDto, InstanceImportPlan};
use std::path::Path;

#[tauri::command]
pub async fn detect_instance_zip(path: String) -> Result<InstanceImportPlan, String> {
    tracing::info!("Detecting instance in ZIP file: {}", path);
    crate::services::detect_instance_zip(Path::new(&path))
        .await
        .map_err(|e| {
            tracing::error!("Failed to detect instance in ZIP: {}", e);
            e.to_string()
        })
}

#[tauri::command]
pub async fn import_instance_zip(
    preview_token: String,
    name: String,
) -> Result<InstanceDto, String> {
    tracing::info!(
        "Importing instance from preview '{}' as '{}'",
        preview_token,
        name
    );
    crate::services::import_instance_zip(&preview_token, &name)
        .await
        .map_err(|e| {
            tracing::error!("Failed to import instance from ZIP: {}", e);
            e.to_string()
        })
}

#[tauri::command]
pub async fn cancel_instance_import(preview_token: String) -> Result<(), String> {
    tracing::info!("Cancelling preview session: {}", preview_token);
    crate::services::cancel_preview(&preview_token).await;
    Ok(())
}
