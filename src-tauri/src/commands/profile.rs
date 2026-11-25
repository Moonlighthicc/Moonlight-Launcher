use crate::core::errors::DownloadError;
use crate::core::http_client::HTTP;
use crate::services::launcher::refresh_microsoft_token;
use crate::services::{
    SettingsManager,
    skin_closet_manager::{SkinClosetEntry, SkinClosetManager, now_ts},
};
use base64::Engine as _;
use base64::engine::general_purpose;
use launchwerk::auth::AccountType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tauri::command;
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info};

pub(crate) const MSA_SKIN_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";
pub(crate) const MSA_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
pub(crate) const MSA_CAPE_URL: &str =
    "https://api.minecraftservices.com/minecraft/profile/capes/active";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftProfileSkin {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: String,
    #[serde(default)]
    pub alias: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftProfileCape {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub alias: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftProfileResponse {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<MinecraftProfileSkin>,
    #[serde(default)]
    pub capes: Vec<MinecraftProfileCape>,
    #[serde(default)]
    pub profile_actions: Vec<String>,
}

/// Cache con TTL y coalescing de requests en vuelo para evitar ráfagas de
/// peticiones al endpoint de perfil de Minecraft Services y prevenir 429.
#[derive(Clone)]
struct CachedProfile {
    profile: Result<MinecraftProfileResponse, String>,
    started_at: Instant,
    fetched_at: Instant,
}

struct ProfileCacheState {
    entries: HashMap<String, CachedProfile>,
    in_flight: HashMap<String, Arc<tokio::sync::Notify>>,
}

#[derive(Clone)]
struct ProfileCache {
    state: Arc<TokioMutex<ProfileCacheState>>,
}

impl ProfileCache {
    const TTL: Duration = Duration::from_secs(30);
    const MAX_ENTRIES: usize = 64;

    fn new() -> Self {
        Self {
            state: Arc::new(TokioMutex::new(ProfileCacheState {
                entries: HashMap::new(),
                in_flight: HashMap::new(),
            })),
        }
    }

    /// Borra la entrada de caché de un UUID sin bloquear. Si no puede tomar el
    /// lock inmediatamente, deja que el TTL expire por sí solo.
    fn invalidate(&self, uuid: &str) {
        if let Ok(mut state) = self.state.try_lock() {
            state.entries.remove(uuid);
        }
    }

    async fn get_or_fetch<F, Fut>(
        &self,
        uuid: String,
        force_refresh: bool,
        fetch: F,
    ) -> Result<MinecraftProfileResponse, String>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<MinecraftProfileResponse, String>> + Send,
    {
        let requested_at = Instant::now();
        loop {
            let in_flight_notify = {
                let state = self.state.lock().await;
                if let Some(entry) = state.entries.get(&uuid)
                    && entry.fetched_at.elapsed() < Self::TTL
                    && (!force_refresh || entry.started_at >= requested_at)
                {
                    return entry.profile.clone();
                }
                // Register under the lock so notify_waiters cannot race the wait.
                state
                    .in_flight
                    .get(&uuid)
                    .cloned()
                    .map(|notify| notify.notified_owned())
            };

            if let Some(notified) = in_flight_notify {
                notified.await;
                continue;
            }

            let our_notify = {
                let mut state = self.state.lock().await;
                if let Some(entry) = state.entries.get(&uuid)
                    && entry.fetched_at.elapsed() < Self::TTL
                    && (!force_refresh || entry.started_at >= requested_at)
                {
                    return entry.profile.clone();
                }
                if state.in_flight.contains_key(&uuid) {
                    continue;
                }
                let arc = Arc::new(tokio::sync::Notify::new());
                state.in_flight.insert(uuid.clone(), arc.clone());
                arc
            };

            let started_at = Instant::now();
            let result = fetch().await;
            let cached = CachedProfile {
                profile: result.clone(),
                started_at,
                fetched_at: Instant::now(),
            };
            {
                let mut state = self.state.lock().await;
                state.entries.insert(uuid.clone(), cached);
                if state.entries.len() > Self::MAX_ENTRIES {
                    let oldest = state
                        .entries
                        .iter()
                        .min_by_key(|(_, entry)| entry.fetched_at)
                        .map(|(key, _)| key.clone());
                    if let Some(oldest) = oldest {
                        state.entries.remove(&oldest);
                    }
                }
                state.in_flight.remove(&uuid);
            }
            our_notify.notify_waiters();
            return result;
        }
    }
}

static PROFILE_CACHE: LazyLock<ProfileCache> = LazyLock::new(ProfileCache::new);

pub(crate) fn mojang_error_message(status: reqwest::StatusCode, body: String) -> String {
    if body.trim().is_empty() {
        return format!("Mojang error (HTTP {})", status.as_u16());
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body)
        && let Some(msg) = value
            .get("errorMessage")
            .or_else(|| value.get("message"))
            .or_else(|| value.get("error"))
            .and_then(|v| v.as_str())
    {
        return format!("{} (HTTP {})", msg, status.as_u16());
    }
    format!("{} (HTTP {})", body, status.as_u16())
}

/// Carga el token de Minecraft Services para el usuario indicado.
/// El frontend no recibe `access_token` por seguridad (`#[serde(skip)]`),
/// por lo que lo recuperamos desde el almacenamiento seguro en el backend.
pub(crate) fn load_minecraft_token(uuid: &str) -> Result<String, String> {
    let mut user = SettingsManager::read()
        .user
        .iter()
        .find(|u| u.uuid == uuid)
        .cloned()
        .ok_or_else(|| format!("User with UUID {} not found", uuid))?;

    if user.user_type != AccountType::Microsoft {
        return Err("Only Microsoft users can manage skins/capes".to_string());
    }

    user.load_tokens()
        .map_err(|e| format!("Could not load the user's tokens: {}", e))?;

    Ok(user.access_token)
}

/// Refresca el token de Microsoft para el usuario indicado y devuelve el nuevo
/// access token. No requiere que el usuario vuelva a iniciar sesión.
pub(crate) async fn refresh_msa_token(uuid: &str) -> Result<String, String> {
    let user = SettingsManager::read()
        .user
        .iter()
        .find(|u| u.uuid == uuid)
        .cloned()
        .ok_or_else(|| format!("User with UUID {} not found", uuid))?;

    if user.user_type != AccountType::Microsoft {
        return Err("Only Microsoft users can manage skins/capes".to_string());
    }

    // Se llega aquí tras un 401: aunque el JWT parezca vigente, hay que forzar
    // el refresco para reintentar con un token nuevo.
    let refreshed = refresh_microsoft_token(user, true)
        .await
        .map_err(|e| e.to_string())?;

    Ok(refreshed.access_token)
}

/// Envía una petición a Minecraft Services con el token del usuario. Si la API
/// responde 401 (token expirado), refresca el token automáticamente y reintenta
/// una única vez.
pub(crate) async fn send_msa_request(
    uuid: &str,
    build: impl Fn(String) -> reqwest::RequestBuilder,
) -> Result<(reqwest::StatusCode, String), String> {
    let access_token = load_minecraft_token(uuid)?;

    let res = build(access_token.clone())
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
        info!("Microsoft token expired for {}, refreshing...", uuid);
        let new_token = refresh_msa_token(uuid).await?;
        let res = build(new_token)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;
        let status = res.status();
        let body = res.text().await.map_err(|e| e.to_string())?;
        return Ok((status, body));
    }

    let status = res.status();
    let body = res.text().await.map_err(|e| e.to_string())?;
    Ok((status, body))
}

pub(crate) async fn capture_active_skin_to_closet(uuid: &str) {
    let profile = match get_minecraft_profile_impl(uuid).await {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to capture the active skin for the closet: {}", e);
            return;
        }
    };

    let Some(active) = profile.skins.into_iter().find(|s| s.state == "ACTIVE") else {
        return;
    };

    let entries = match SkinClosetManager::get_entries(uuid).await {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to load the skin closet: {e}");
            return;
        }
    };
    if entries.iter().any(|e| e.id == active.id) {
        return;
    }

    let saved = async {
        let bytes = download_skin(&active.url).await?;
        super::yggdrasil_profile::validate_skin_png(&bytes)?;
        Ok::<_, String>(bytes)
    }
    .await;
    let bytes = match saved {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to save the closet image: {}", e);
            return;
        }
    };

    let entry = SkinClosetEntry {
        id: active.id,
        url: active.url,
        local_path: String::new(),
        variant: active.variant,
        alias: String::new(),
        saved_at: now_ts(),
    };

    if let Err(e) = SkinClosetManager::save_png(uuid, entry, &bytes).await {
        error!("Failed to sync the closet entry: {}", e);
    }
}

pub(crate) async fn get_minecraft_profile_impl(
    uuid: &str,
) -> Result<MinecraftProfileResponse, String> {
    get_minecraft_profile_with_refresh(uuid, false).await
}

pub(crate) async fn get_minecraft_profile_with_refresh(
    uuid: &str,
    force_refresh: bool,
) -> Result<MinecraftProfileResponse, String> {
    let uuid = uuid.to_string();
    PROFILE_CACHE
        .get_or_fetch(uuid.clone(), force_refresh, move || async move {
            info!("Fetching Minecraft Services profile for {}", uuid);

            let (status, body) = send_msa_request(&uuid, |token| {
                let request = HTTP.get(MSA_PROFILE_URL).bearer_auth(token);
                if force_refresh {
                    request.header(reqwest::header::CACHE_CONTROL, "no-cache")
                } else {
                    request
                }
            })
            .await?;

            if !status.is_success() {
                error!("Failed to fetch the profile: HTTP {}", status);
                return Err(mojang_error_message(status, body));
            }

            serde_json::from_str(&body).map_err(|e| {
                error!("Failed to parse the profile: {}", e);
                format!("Invalid response: {}", e)
            })
        })
        .await
}

pub(crate) async fn upload_skin_file_impl(
    uuid: &str,
    file_path: &str,
    model: &str,
) -> Result<(), String> {
    info!("Uploading skin from file for {}", uuid);

    let path = PathBuf::from(file_path);
    if !path.exists() {
        return Err(format!("The file '{}' does not exist", file_path));
    }

    let bytes = tokio::fs::read(&path).await.map_err(|e| {
        error!("Failed to read the skin file: {}", e);
        format!("Failed to read the file: {}", e)
    })?;

    upload_skin_bytes_impl(uuid, bytes, model).await
}

/// Sube un PNG como archivo (multipart), la forma de cambiar la skin que Mojang acepta.
async fn upload_skin_bytes_impl(uuid: &str, bytes: Vec<u8>, model: &str) -> Result<(), String> {
    let (status, body) = send_msa_request(uuid, |token| {
        let file_part = reqwest::multipart::Part::bytes(bytes.clone())
            .file_name("skin.png")
            .mime_str("image/png")
            .expect("valid mime");

        let form = reqwest::multipart::Form::new()
            .text("variant", model.to_uppercase())
            .part("file", file_part);

        HTTP.post(MSA_SKIN_URL).bearer_auth(token).multipart(form)
    })
    .await?;

    if !status.is_success() {
        error!("Failed to upload the skin: HTTP {}", status);
        return Err(mojang_error_message(status, body));
    }

    info!("Skin uploaded successfully");
    Ok(())
}

/// Lo más que se descarga de una skin por URL: un PNG de skin pesa unos pocos KB.
const SKIN_URL_MAX_BYTES: usize = 1024 * 1024;

/// Descarga la imagen de una skin para subirla como archivo.
pub(crate) async fn download_skin(skin_url: &str) -> Result<Vec<u8>, String> {
    let url = reqwest::Url::parse(skin_url).map_err(|e| format!("Invalid skin URL: {}", e))?;
    // http también: el armario guarda las URL de textures.minecraft.net tal como las da Mojang.
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err("The skin URL must be http or https".into());
    }
    let mut response = HTTP
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| DownloadError::Request(e.to_string()).to_string())?
        .error_for_status()
        .map_err(|e| DownloadError::Request(e.to_string()).to_string())?;
    let too_large = || DownloadError::ReadResponse("the skin is too heavy".into()).to_string();
    if response
        .content_length()
        .is_some_and(|length| length > SKIN_URL_MAX_BYTES as u64)
    {
        return Err(too_large());
    }
    // Content-Length puede faltar o mentir: se cuenta lo que llega y se corta al pasar el límite,
    // sin esperar al resto de la respuesta.
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| DownloadError::ReadResponse(e.to_string()).to_string())?
    {
        if chunk.len() > SKIN_URL_MAX_BYTES - bytes.len() {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(DownloadError::ReadResponse("the skin is not a PNG".into()).to_string());
    }
    Ok(bytes)
}

/// Cambia la skin por la imagen de una URL.
///
/// Mojang dejó de aceptar el cambio por URL (`{"url": ...}` en JSON): desde octubre de 2026 su
/// cortafuegos lo rechaza con un 403 en HTML, mientras que la subida del PNG como archivo
/// sigue funcionando. Así que la imagen se descarga aquí y se sube como una del disco.
pub(crate) async fn upload_skin_url_impl(
    uuid: &str,
    skin_url: &str,
    variant: &str,
) -> Result<(), String> {
    info!("Changing skin by URL for {}", uuid);
    let bytes = download_skin(skin_url).await?;
    upload_skin_bytes_impl(uuid, bytes, variant).await
}

#[command]
pub async fn get_minecraft_profile(uuid: String) -> Result<MinecraftProfileResponse, String> {
    get_minecraft_profile_impl(&uuid).await
}

#[command]
pub async fn read_skin_preview_data(file_path: String) -> Result<String, String> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err(format!("The file '{}' does not exist", file_path));
    }

    let bytes = tokio::fs::read(&path).await.map_err(|e| {
        error!("Failed to read the skin file: {}", e);
        format!("Failed to read the file: {}", e)
    })?;

    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(&bytes)
    ))
}

#[command]
pub async fn upload_skin_file(
    uuid: String,
    file_path: String,
    model: String,
    remember_locally: Option<bool>,
) -> Result<(), String> {
    upload_skin_file_impl(&uuid, &file_path, &model).await?;
    PROFILE_CACHE.invalidate(&uuid);
    if remember_locally.unwrap_or(false) {
        capture_active_skin_to_closet(&uuid).await;
    }
    Ok(())
}

#[command]
pub async fn upload_skin_url(
    uuid: String,
    skin_url: String,
    variant: String,
    remember_locally: Option<bool>,
) -> Result<(), String> {
    upload_skin_url_impl(&uuid, &skin_url, &variant).await?;
    PROFILE_CACHE.invalidate(&uuid);
    if remember_locally.unwrap_or(false) {
        capture_active_skin_to_closet(&uuid).await;
    }
    Ok(())
}

#[command]
pub async fn equip_cape(uuid: String, cape_id: String) -> Result<(), String> {
    info!("Equipping cape {} for {}", cape_id, uuid);

    let body = serde_json::json!({ "capeId": cape_id });

    let (status, res_body) = send_msa_request(&uuid, |token| {
        HTTP.put(MSA_CAPE_URL).bearer_auth(token).json(&body)
    })
    .await?;

    if !status.is_success() {
        error!("Failed to equip cape: HTTP {}", status);
        return Err(mojang_error_message(status, res_body));
    }

    PROFILE_CACHE.invalidate(&uuid);
    info!("Cape equipped successfully");
    Ok(())
}

#[command]
pub async fn unequip_cape(uuid: String) -> Result<(), String> {
    info!("Unequipping cape for {}", uuid);

    let (status, res_body) =
        send_msa_request(&uuid, |token| HTTP.delete(MSA_CAPE_URL).bearer_auth(token)).await?;

    if !status.is_success() {
        error!("Failed to unequip cape: HTTP {}", status);
        return Err(mojang_error_message(status, res_body));
    }

    PROFILE_CACHE.invalidate(&uuid);
    info!("Cape unequipped successfully");
    Ok(())
}

#[cfg(test)]
#[path = "../tests/commands/profile.rs"]
mod tests;
