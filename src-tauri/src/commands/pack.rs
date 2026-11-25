use crate::core::PathManager;
use crate::core::{AppEvent, InstanceError, emit};
use crate::services::{DownloadQueue, InstanceManager};
use serde::Serialize;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Serialize)]
pub struct MrpackInfo {
    pub name: String,
    pub version_id: String,
    pub summary: Option<String>,
    pub author: Option<String>,
    pub minecraft_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub file_count: usize,
    pub version_id_for_instance: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CurseForgeModpackInfo {
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    pub minecraft_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub file_count: usize,
    pub version_id_for_instance: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "format", rename_all = "camelCase")]
pub enum LocalModpackInfo {
    Modrinth { info: MrpackInfo },
    Curseforge { info: CurseForgeModpackInfo },
}

async fn modrinth_pack_version(version_id: String) -> Result<serde_json::Value, String> {
    if version_id.is_empty() || !version_id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("Invalid version ID".into());
    }
    super::market::get_modrinth_version(version_id).await
}

pub(super) async fn modrinth_pack_changelog(version_id: String) -> Result<String, String> {
    Ok(modrinth_pack_version(version_id).await?["changelog"]
        .as_str()
        .unwrap_or_default()
        .to_owned())
}

async fn mrpack_info(path: &str) -> Result<MrpackInfo, String> {
    let metadata = cubrinth::mrpack::parse_mrpack(std::path::Path::new(path))
        .map_err(|e| format!("Failed to parse mrpack: {}", e))?;

    crate::services::modpack::validate_archive_paths(std::path::Path::new(path)).await?;

    let version_id_for_instance = metadata.game_version.as_ref().map(|gv| gv.to_version_id());

    Ok(MrpackInfo {
        name: metadata.name,
        version_id: metadata.version_id,
        summary: metadata.summary,
        author: metadata.author,
        minecraft_version: metadata
            .game_version
            .as_ref()
            .map(|gv| gv.mc_version.clone()),
        loader: metadata.game_version.as_ref().and_then(|gv| {
            if gv.loader.is_vanilla() {
                None
            } else {
                Some(gv.loader.name().to_string())
            }
        }),
        loader_version: metadata
            .game_version
            .as_ref()
            .and_then(|gv| gv.loader.version().map(|s| s.to_string())),
        file_count: metadata.file_count,
        version_id_for_instance,
        icon: None,
    })
}

async fn curseforge_modpack_info(path: &str) -> Result<CurseForgeModpackInfo, String> {
    let metadata =
        crate::services::curseforge_modpack::parse_curseforge_modpack(std::path::Path::new(path))
            .map_err(|e| format!("Failed to parse CurseForge modpack: {}", e))?;

    crate::services::modpack::validate_archive_paths(std::path::Path::new(path)).await?;

    let version_id_for_instance = metadata.game_version.as_ref().map(|gv| gv.to_version_id());

    Ok(CurseForgeModpackInfo {
        name: metadata.name,
        version: metadata.version,
        summary: metadata.summary,
        minecraft_version: metadata
            .game_version
            .as_ref()
            .map(|gv| gv.mc_version.clone()),
        loader: metadata.game_version.as_ref().and_then(|gv| {
            if gv.loader.is_vanilla() {
                None
            } else {
                Some(gv.loader.name().to_string())
            }
        }),
        loader_version: metadata
            .game_version
            .as_ref()
            .and_then(|gv| gv.loader.version().map(|s| s.to_string())),
        file_count: metadata.file_count,
        version_id_for_instance,
        icon: None,
    })
}

#[tauri::command]
pub async fn parse_mrpack(path: String) -> Result<MrpackInfo, String> {
    info!("Parsing mrpack: {}", path);
    mrpack_info(&path).await
}

#[tauri::command]
pub async fn parse_local_modpack(path: String) -> Result<LocalModpackInfo, String> {
    info!("Detecting local modpack format: {}", path);
    let file = std::fs::File::open(&path).map_err(|e| format!("Could not open the file: {e}"))?;
    let archive =
        zip::ZipArchive::new(file).map_err(|e| format!("The file is not a valid modpack: {e}"))?;
    let mut is_modrinth = false;
    let mut is_curseforge = false;
    for name in archive.file_names().filter_map(|name| name.ok()) {
        if name == "modrinth.index.json" {
            is_modrinth = true;
        } else if name == "manifest.json" {
            is_curseforge = true;
        }
    }
    drop(archive);
    if is_modrinth {
        Ok(LocalModpackInfo::Modrinth {
            info: mrpack_info(&path).await?,
        })
    } else if is_curseforge {
        Ok(LocalModpackInfo::Curseforge {
            info: curseforge_modpack_info(&path).await?,
        })
    } else {
        Err("The file is neither a Modrinth nor a CurseForge modpack".into())
    }
}

#[tauri::command]
pub async fn install_mrpack(
    path: String,
    instance_name: String,
    project_id: Option<String>,
    modrinth_version_id: Option<String>,
    icon_url: Option<String>,
) -> Result<MrpackInfo, String> {
    tokio::spawn(install_mrpack_inner(
        path,
        instance_name,
        project_id,
        modrinth_version_id,
        icon_url,
    ))
    .await
    .map_err(|e| e.to_string())?
}

pub(crate) const CANCELLED_INSTALL_MESSAGE: &str = "Installation cancelled";

fn download_error_text(error: aqua::AquaError) -> String {
    match error {
        aqua::AquaError::Cancelled => "Download cancelled".to_string(),
        other => other.to_string(),
    }
}

/// Tras cancelar una instalación: muestra "eliminando", borra la instancia a medio
/// crear y cierra la tarea en la cola (DCancel).
async fn abort_cancelled_install(instance_uuid: &str, label: &Arc<str>) -> String {
    use crate::services::download_queue::{finish_cancelled_work, mark_deleting};
    info!("Modpack installation cancelled; deleting instance {instance_uuid}");
    mark_deleting(label);
    if let Err(e) = InstanceManager::get().delete_instance(instance_uuid).await {
        tracing::error!("Could not delete the cancelled instance: {e}");
    }
    finish_cancelled_work(label);
    CANCELLED_INSTALL_MESSAGE.to_string()
}

async fn rollback_failed_install(instance_uuid: &str, label: &Arc<str>, message: String) -> String {
    let message = match InstanceManager::get().delete_instance(instance_uuid).await {
        Ok(()) => message,
        Err(cleanup) => format!("{message}. Could not remove the incomplete instance: {cleanup}"),
    };
    DownloadQueue::get().fail_work(label, message.clone());
    message
}

async fn install_mrpack_inner(
    path: String,
    instance_name: String,
    project_id: Option<String>,
    modrinth_version_id: Option<String>,
    icon_url: Option<String>,
) -> Result<MrpackInfo, String> {
    info!(
        "Installing mrpack '{}' as instance '{}' (project={:?}, version={:?}, icon={:?})",
        path, instance_name, project_id, modrinth_version_id, icon_url
    );

    let metadata = cubrinth::mrpack::parse_mrpack(std::path::Path::new(&path))
        .map_err(|e| format!("Failed to parse mrpack: {}", e))?;

    crate::services::modpack::validate_archive_paths(std::path::Path::new(&path)).await?;

    let game_version = metadata
        .game_version
        .clone()
        .ok_or_else(|| "No Minecraft version specified in pack dependencies".to_string())?;

    let version_id = game_version.to_version_id();

    let manager = InstanceManager::get();

    let handle = manager
        .create_instance(instance_name, version_id.clone(), None)
        .await
        .map_err(|e| match e {
            InstanceError::AlreadyExists => "An instance with that name already exists".to_string(),
            other => format!("Failed to create instance: {}", other),
        })?;

    let files_guard = handle.try_lock_files()?;
    let instance_dir = handle.get_instance_dir().await;

    // Save upstream metadata if provided
    if let (Some(pid), Some(vid)) = (&project_id, &modrinth_version_id) {
        let upstream = serde_json::json!({
            "type": "modrinth-modpack",
            "projectId": pid,
            "versionId": vid,
        });
        let upstream_path = instance_dir.join("upstream.json");
        let upstream_content = serde_json::to_string_pretty(&upstream)
            .map_err(|e| format!("Failed to serialize upstream: {}", e))?;
        let _ = tokio::fs::write(&upstream_path, &upstream_content).await;
        info!("Saved upstream metadata to {:?}", upstream_path);
    }

    let download_label: Arc<str> = format!("modpack-{}", handle.uuid).into();
    DownloadQueue::get()
        .enqueue_tracked(download_label.clone())
        .await;
    emit(AppEvent::InstanceCreated {
        id: handle.uuid.to_string().into(),
        dto: handle.to_dto().await,
    });

    let (progress_tx, progress_rx) =
        tokio::sync::watch::channel(aqua::progress::DownloadProgress::empty(0));
    // Mismo monitor que el resto de tareas de la cola: actualiza el estado de la
    // cola (para que las consultas de sincronización vean el progreso real) y emite los eventos.
    let progress_task = tokio::spawn(crate::services::download_queue::monitor_download_progress(
        download_label.clone(),
        progress_rx,
        DownloadQueue::get().clone(),
    ));

    let shared_dir = PathManager::get().get_shared_dir();
    // The instance files and the shared Minecraft runtime are independent.
    DownloadQueue::get().enqueue(version_id.clone()).await;
    let install_result = cubrinth::mrpack::install_mrpack_observed(
        std::path::Path::new(&path),
        &instance_dir,
        &shared_dir,
        Some(progress_tx),
        Some(crate::services::download_queue::handle_observer(
            download_label.clone(),
        )),
    )
    .await;

    let _ = progress_task.await;

    if let Err(e) = install_result {
        if crate::services::download_queue::was_cancel_requested(&download_label) {
            drop(files_guard);
            return Err(abort_cancelled_install(&handle.uuid, &download_label).await);
        }
        drop(files_guard);
        return Err(rollback_failed_install(&handle.uuid, &download_label, format!("Failed to install mrpack: {e}")).await);
    }

    crate::services::download_queue::set_tracked_stage(
        &download_label,
        "finishing",
        Some("Saving modpack details".to_string()),
    );

    let source = if project_id.is_some() && modrinth_version_id.is_some() {
        "modrinth"
    } else {
        "local"
    };
    if let Err(e) = crate::services::modpack::record_install(
        &instance_dir,
        std::path::Path::new(&path),
        source,
        project_id,
        modrinth_version_id,
    )
    .await
    {
        drop(files_guard);
        return Err(rollback_failed_install(&handle.uuid, &download_label, format!("Failed to record mrpack inventory: {e}")).await);
    }

    let icon = save_optional_pack_icon(icon_url.as_deref(), &instance_dir, &handle).await;
    if crate::services::download_queue::was_cancel_requested(&download_label) {
        drop(files_guard);
        return Err(abort_cancelled_install(&handle.uuid, &download_label).await);
    }
    if let Err(e) = files_guard.save_if_dirty().await {
        tracing::warn!("Could not persist instance icon: {e}");
    }
    DownloadQueue::get().finish_work(&download_label).await;

    emit(AppEvent::InstanceEdited {
        id: handle.uuid.to_string().into(),
        dto: Some(handle.to_dto().await),
    });

    Ok(MrpackInfo {
        name: metadata.name,
        version_id: metadata.version_id,
        summary: metadata.summary,
        author: metadata.author,
        minecraft_version: Some(game_version.mc_version),
        loader: if game_version.loader.is_vanilla() {
            None
        } else {
            Some(game_version.loader.name().to_string())
        },
        loader_version: game_version.loader.version().map(|s| s.to_string()),
        file_count: metadata.file_count,
        version_id_for_instance: Some(version_id),
        icon,
    })
}

// ── CurseForge modpack commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn download_curseforge_modpack(url: String, file_id: u32) -> Result<String, String> {
    info!(
        "Downloading CurseForge modpack from {} (file_id: {})",
        url, file_id
    );

    let cache_dir = tempfile::Builder::new()
        .prefix("cubic-curseforge-download-")
        .tempdir()
        .map_err(|e| format!("Failed to create cache dir: {}", e))?;

    let filename = format!("{}.zip", file_id);
    let dest = cache_dir.path().join(&filename);

    let item = aqua::DownloadItemSpec::new(url, dest.clone(), "curseforge-modpack");
    let batch = aqua::GenericBatch::new(format!("curseforge-modpack-{}", file_id), vec![item]);

    let shared_dir = PathManager::get().get_shared_dir();
    let dm = aqua::DownloadManager::new(shared_dir);
    let handle = dm
        .prepare_batch(Box::new(batch))
        .await
        .map_err(|e| format!("Failed to prepare CurseForge modpack download: {}", e))?;

    crate::services::download_queue::run_tracked_download(
        format!("cfpack-dl-{file_id}").into(),
        handle,
    )
    .await
    .map_err(|e| {
        format!(
            "Failed to download CurseForge modpack: {}",
            download_error_text(e)
        )
    })?;

    let path_str = dest.to_string_lossy().to_string();
    let _ = cache_dir.keep();
    info!("CurseForge modpack downloaded to {}", path_str);
    Ok(path_str)
}

#[tauri::command]
pub async fn parse_curseforge_modpack(path: String) -> Result<CurseForgeModpackInfo, String> {
    info!("Parsing CurseForge modpack: {}", path);
    curseforge_modpack_info(&path).await
}

/// Downloads a project logo with bounded retries. The logo is
/// optional metadata: transient CDN failures must never fail the modpack install.
async fn download_pack_icon(url: &str) -> Result<Vec<u8>, String> {
    const MAX_ICON_BYTES: usize = 8 * 1024 * 1024;
    let mut last_error = String::from("unknown error");
    for attempt in 1..=3 {
        let result = async {
            let mut response = crate::core::HTTP
                .get(url)
                .timeout(std::time::Duration::from_secs(8))
                .header(
                    reqwest::header::ACCEPT,
                    "image/avif,image/webp,image/png,image/jpeg,image/*",
                )
                .send()
                .await
                .map_err(|e| format!("request failed: {e}"))?;

            let status = response.status();
            if !status.is_success() {
                return Err(format!("server returned HTTP {status}"));
            }

            if let Some(length) = response.content_length()
                && length > MAX_ICON_BYTES as u64
            {
                return Err(format!("icon is too large ({length} bytes)"));
            }

            let content_type_is_image = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.to_ascii_lowercase().starts_with("image/"));

            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| format!("failed to read response: {e}"))? {
                if bytes.len() + chunk.len() > MAX_ICON_BYTES {
                    return Err("icon is too large".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            if bytes.is_empty() {
                return Err("empty icon".into());
            }

            let has_image_signature = bytes.starts_with(b"\x89PNG\r\n\x1a\n")
                || bytes.starts_with(&[0xff, 0xd8, 0xff])
                || bytes.starts_with(b"GIF87a")
                || bytes.starts_with(b"GIF89a")
                || (bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP");

            if !content_type_is_image && !has_image_signature {
                return Err("response does not contain a recognized image".into());
            }

            Ok(bytes)
        }
        .await;

        match result {
            Ok(bytes) => return Ok(bytes),
            Err(error) => {
                last_error = error;
                tracing::warn!(
                    attempt,
                    max_attempts = 3,
                    error = %last_error,
                    "Modpack icon download attempt failed"
                );
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(350 * attempt as u64))
                        .await;
                }
            }
        }
    }

    Err(last_error)
}

async fn save_optional_pack_icon(
    url: Option<&str>,
    instance_dir: &std::path::Path,
    handle: &crate::services::InstanceHandle,
) -> Option<String> {
    let icon_path = instance_dir.join("icon.png");
    if let Some(url) = url {
        let result = async {
            let bytes = download_pack_icon(url).await?;
            let root = instance_dir.to_path_buf();
            tokio::task::spawn_blocking(move || {
                zellkern::path_security::ConfinedDir::open(&root)
                    .and_then(|root| root.write(std::path::Path::new("icon.png"), &bytes))
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
            Ok::<_, String>(())
        }
        .await;
        if let Err(e) = result {
            tracing::warn!("Could not save optional modpack icon: {e}");
        }
    }
    // Fall back to the logo extracted from the archive if the CDN is unavailable.
    if !tokio::fs::symlink_metadata(&icon_path).await.ok()?.is_file() {
        return None;
    }
    let icon = icon_path.to_string_lossy().into_owned();
    handle.set_icon(Some(icon.clone())).await;
    Some(icon)
}

#[tauri::command]
pub async fn install_curseforge_modpack(
    path: String,
    instance_name: String,
    project_id: Option<u32>,
    file_id: Option<u32>,
    icon_url: Option<String>,
) -> Result<CurseForgeModpackInfo, String> {
    tokio::spawn(install_curseforge_modpack_inner(
        path,
        instance_name,
        project_id,
        file_id,
        icon_url,
    ))
    .await
    .map_err(|e| e.to_string())?
}

async fn install_curseforge_modpack_inner(
    path: String,
    instance_name: String,
    project_id: Option<u32>,
    file_id: Option<u32>,
    icon_url: Option<String>,
) -> Result<CurseForgeModpackInfo, String> {
    info!(
        "Installing CurseForge modpack '{}' as instance '{}' (project={:?}, file={:?}, icon={:?})",
        path, instance_name, project_id, file_id, icon_url
    );

    let metadata =
        crate::services::curseforge_modpack::parse_curseforge_modpack(std::path::Path::new(&path))
            .map_err(|e| format!("Failed to parse CurseForge modpack: {}", e))?;

    crate::services::modpack::validate_archive_paths(std::path::Path::new(&path)).await?;

    let game_version = metadata
        .game_version
        .clone()
        .ok_or_else(|| "No Minecraft version specified in CurseForge modpack".to_string())?;

    let version_id = game_version.to_version_id();

    let manager = InstanceManager::get();

    let handle = manager
        .create_instance(instance_name, version_id.clone(), None)
        .await
        .map_err(|e| match e {
            InstanceError::AlreadyExists => "An instance with that name already exists".to_string(),
            other => format!("Failed to create instance: {}", other),
        })?;

    let files_guard = handle.try_lock_files()?;
    let instance_dir = handle.get_instance_dir().await;

    // Save upstream metadata if provided
    if let (Some(pid), Some(fid)) = (&project_id, &file_id) {
        let upstream = serde_json::json!({
            "type": "curseforge-modpack",
            "projectId": pid,
            "fileId": fid,
        });
        let upstream_path = instance_dir.join("upstream.json");
        let upstream_content = serde_json::to_string_pretty(&upstream)
            .map_err(|e| format!("Failed to serialize upstream: {}", e))?;
        let _ = tokio::fs::write(&upstream_path, &upstream_content).await;
        info!("Saved upstream metadata to {:?}", upstream_path);
    }

    let download_label: Arc<str> = format!("modpack-{}", handle.uuid).into();
    DownloadQueue::get()
        .enqueue_tracked(download_label.clone())
        .await;
    emit(AppEvent::InstanceCreated {
        id: handle.uuid.to_string().into(),
        dto: handle.to_dto().await,
    });

    let (progress_tx, progress_rx) =
        tokio::sync::watch::channel(aqua::progress::DownloadProgress::empty(0));
    // Mismo monitor que el resto de tareas de la cola: actualiza el estado de la
    // cola (para que las consultas de sincronización vean el progreso real) y emite los eventos.
    let progress_task = tokio::spawn(crate::services::download_queue::monitor_download_progress(
        download_label.clone(),
        progress_rx,
        DownloadQueue::get().clone(),
    ));

    let shared_dir = PathManager::get().get_shared_dir();
    DownloadQueue::get().enqueue(version_id.clone()).await;
    let install_result = crate::services::curseforge_modpack::install_curseforge_modpack_observed(
        std::path::Path::new(&path),
        &instance_dir,
        &shared_dir,
        Some(progress_tx),
        None,
        Some(crate::services::download_queue::handle_observer(
            download_label.clone(),
        )),
    )
    .await;

    let _ = progress_task.await;

    if let Err(e) = install_result {
        if crate::services::download_queue::was_cancel_requested(&download_label) {
            drop(files_guard);
            return Err(abort_cancelled_install(&handle.uuid, &download_label).await);
        }
        drop(files_guard);
        return Err(rollback_failed_install(&handle.uuid, &download_label, format!("Failed to install CurseForge modpack: {e}")).await);
    }

    crate::services::download_queue::set_tracked_stage(
        &download_label,
        "finishing",
        Some("Saving modpack details".to_string()),
    );

    let source = if project_id.is_some() && file_id.is_some() {
        "curseforge"
    } else {
        "local"
    };
    if let Err(e) = crate::services::modpack::record_install(
        &instance_dir,
        std::path::Path::new(&path),
        source,
        project_id.map(|id| id.to_string()),
        file_id.map(|id| id.to_string()),
    )
    .await
    {
        drop(files_guard);
        return Err(rollback_failed_install(&handle.uuid, &download_label, format!("Failed to record CurseForge modpack inventory: {e}")).await);
    }

    let icon = save_optional_pack_icon(icon_url.as_deref(), &instance_dir, &handle).await;
    if crate::services::download_queue::was_cancel_requested(&download_label) {
        drop(files_guard);
        return Err(abort_cancelled_install(&handle.uuid, &download_label).await);
    }
    if let Err(e) = files_guard.save_if_dirty().await {
        tracing::warn!("Could not persist instance icon: {e}");
    }
    DownloadQueue::get().finish_work(&download_label).await;

    emit(AppEvent::InstanceEdited {
        id: handle.uuid.to_string().into(),
        dto: Some(handle.to_dto().await),
    });

    Ok(CurseForgeModpackInfo {
        name: metadata.name,
        version: metadata.version,
        summary: metadata.summary,
        minecraft_version: Some(game_version.mc_version),
        loader: if game_version.loader.is_vanilla() {
            None
        } else {
            Some(game_version.loader.name().to_string())
        },
        loader_version: game_version.loader.version().map(|s| s.to_string()),
        file_count: metadata.file_count,
        version_id_for_instance: Some(version_id),
        icon,
    })
}
