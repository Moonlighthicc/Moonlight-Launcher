use super::profile::{download_skin, get_minecraft_profile_impl};
use super::yggdrasil_profile::validate_skin_png;
use crate::core::PathManager;
use crate::services::skin_closet_manager::{
    SHARED_SKIN_LIBRARY, SkinClosetEntry, SkinClosetManager, now_ts,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::{Path, PathBuf};
use tauri::command;
use tokio::io::AsyncReadExt;

const MAX_BYTES: usize = 1024 * 1024;

#[command]
pub async fn get_skin_closet(uuid: String) -> Result<Vec<SkinClosetEntry>, String> {
    SkinClosetManager::get_entries(&uuid).await
}

#[command]
pub async fn open_skin_closet_dir() -> Result<(), String> {
    let path = SkinClosetManager::prepare_dir().await?;

    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "macos")]
    let opener = "open";

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    return std::process::Command::new(opener)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Could not open skin directory: {err}"));

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    Err("Opening the skin directory is not supported on this platform".into())
}

#[command]
pub async fn import_skin_to_closet(file_path: String, model: String) -> Result<(), String> {
    let path = PathBuf::from(file_path);
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    save_local_skin(bytes, &model).await
}

#[command]
pub async fn copy_server_skin_to_closet(uuid: String, skin_id: String) -> Result<(), String> {
    let skin = get_minecraft_profile_impl(&uuid)
        .await?
        .skins
        .into_iter()
        .find(|skin| skin.id == skin_id)
        .ok_or("Skin not found on the server")?;
    let bytes = download_skin(&skin.url).await?;
    save_local_skin(bytes, &skin.variant).await
}

#[command]
pub async fn copy_skin_preview_to_closet(data_url: String, model: String) -> Result<(), String> {
    let encoded = data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or("Expected a PNG preview")?;
    if encoded.len() > MAX_BYTES.div_ceil(3) * 4 {
        return Err("The skin exceeds the allowed size".into());
    }
    let bytes = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
    save_local_skin(bytes, &model).await
}

async fn save_local_skin(bytes: Vec<u8>, model: &str) -> Result<(), String> {
    if bytes.len() > MAX_BYTES {
        return Err("The skin exceeds the allowed size".into());
    }
    validate_skin_png(&bytes)?;
    let id = uuid::Uuid::new_v4().to_string();
    SkinClosetManager::save_png(
        SHARED_SKIN_LIBRARY,
        SkinClosetEntry {
            id,
            url: String::new(),
            local_path: String::new(),
            variant: if model.eq_ignore_ascii_case("slim") {
                "SLIM"
            } else {
                "CLASSIC"
            }
            .into(),
            alias: String::new(),
            saved_at: now_ts(),
        },
        &bytes,
    )
    .await
}

#[command]
pub async fn remove_skin_from_closet(uuid: String, entry_id: String) -> Result<(), String> {
    SkinClosetManager::remove_entry(&uuid, &entry_id).await
}

#[command]
pub async fn rename_skin_in_closet(
    uuid: String,
    entry_id: String,
    alias: String,
) -> Result<(), String> {
    SkinClosetManager::update_alias(&uuid, &entry_id, alias).await
}

#[command]
pub async fn get_closet_skin_preview(uuid: String, entry_id: String) -> Result<String, String> {
    let entry = SkinClosetManager::get_entries(&uuid)
        .await?
        .into_iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| "Skin not found in the closet".to_string())?;

    let path = Path::new(&entry.local_path);
    if path.parent() != Some(PathManager::get().get_skin_closet_dir())
        || !tokio::fs::symlink_metadata(path)
            .await
            .map_err(|e| e.to_string())?
            .file_type()
            .is_file()
    {
        return Err("Invalid local skin path".into());
    }
    super::profile::read_skin_preview_data(entry.local_path).await
}
