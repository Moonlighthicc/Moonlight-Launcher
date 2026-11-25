use crate::core::errors::{AppError, InstanceError};
use crate::core::sanitize_path;
use crate::services::{InstanceDto, InstanceManager, InstanceStatus, Launcher, signal_kill, signal_kill_process, has_running_processes, running_process_ids};
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

pub fn validate_uuid(uuid: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(uuid).map_err(|_| format!("Invalid UUID: '{}'", uuid))?;
    Ok(())
}

pub fn sanitize_sub_path(instance_dir: &Path, sub_path: &Path) -> Result<PathBuf, String> {
    sanitize_path(instance_dir, sub_path)
}

#[tauri::command]
pub async fn launch(instance_id: String, server_address: Option<String>) -> Result<(), String> {
    validate_uuid(&instance_id)?;
    let server = server_address
        .as_deref()
        .map(crate::services::server_status::ServerAddress::parse)
        .transpose()?;
    info!("Launching instance {}", instance_id);
    let manager = InstanceManager::get();
    let Some(handle) = manager.get_handle(&instance_id).await else {
        error!("Instance {} not found for launch", instance_id);
        return Err(AppError::Instance(InstanceError::NotFound).to_json());
    };

    Launcher::get()
        .launch(handle.clone(), server)
        .await
        .map_err(|e| {
            error!("Failed to launch instance {}: {}", instance_id, e);
            e.to_json()
        })?;

    info!("Instance {} launched successfully", instance_id);
    Ok(())
}

#[tauri::command]
pub async fn kill_instance(uuid: String, process_id: Option<String>) -> Result<(), String> {
    validate_uuid(&uuid)?;
    info!("Killing instance {}", uuid);
    let manager = InstanceManager::get();
    let handle = manager
        .get_handle(&uuid)
        .await
        .ok_or_else(|| InstanceError::NotFound.to_string())?;

    let killed = if let Some(id) = process_id.as_deref() { signal_kill_process(&uuid, id) } else { signal_kill(&uuid) };
    if !killed && !has_running_processes(&uuid) {
        warn!("Instance {} was not running, forcing Off", uuid);
        handle.set_status(InstanceStatus::Off);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_instance_processes(uuid: String) -> Vec<String> {
    running_process_ids(&uuid)
}

#[tauri::command]
pub async fn get_instances() -> Vec<InstanceDto> {
    InstanceManager::get().get_all_dtos().await
}

#[cfg(test)]
#[path = "../../tests/commands/instance/launch.rs"]
mod tests;
