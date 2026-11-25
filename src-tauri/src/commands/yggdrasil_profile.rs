use crate::{core::http_client::HTTP, services::SettingsManager};
use base64::{Engine as _, engine::general_purpose};
use image::ImageDecoder;
use launchwerk::auth::{AccountType, MinecraftUser};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Cursor};
use tracing::info;

const MAX_SKIN_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize)]
pub struct YggdrasilSkinProfile {
    skin_url: Option<String>,
    model: &'static str,
}

#[derive(Deserialize)]
struct ProfileResponse {
    #[serde(default)]
    properties: Vec<ProfileProperty>,
}

#[derive(Deserialize)]
struct ProfileProperty {
    name: String,
    value: String,
}

#[derive(Default, Deserialize)]
struct TexturesPayload {
    #[serde(default)]
    textures: HashMap<String, Texture>,
}

#[derive(Deserialize)]
struct Texture {
    url: String,
    #[serde(default)]
    metadata: TextureMetadata,
}

#[derive(Default, Deserialize)]
struct TextureMetadata {
    model: Option<String>,
}

/// Saca la skin (URL y modelo) del JSON del perfil Yggdrasil, que trae las
/// texturas en base64 dentro de la propiedad `textures`.
fn parse_skin(body: &[u8]) -> Result<Option<(String, &'static str)>, String> {
    let profile: ProfileResponse = serde_json::from_slice(body)
        .map_err(|e| format!("Invalid Yggdrasil profile response: {e}"))?;
    let Some(encoded) = profile
        .properties
        .iter()
        .find(|property| property.name == "textures")
        .map(|property| &property.value)
    else {
        return Ok(None);
    };
    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("Invalid Yggdrasil textures: {e}"))?;
    let payload: TexturesPayload =
        serde_json::from_slice(&decoded).map_err(|e| format!("Invalid Yggdrasil textures: {e}"))?;
    let Some(skin) = payload.textures.get("SKIN") else {
        return Ok(None);
    };
    let model = if skin.metadata.model.as_deref() == Some("slim") {
        "slim"
    } else {
        "classic"
    };
    Ok(Some((skin.url.clone(), model)))
}

async fn download(url: &str, limit: usize) -> Result<Vec<u8>, String> {
    let mut response = HTTP
        .get(url)
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .await
        .map_err(|e| format!("Connection error with the server: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "The server responded with HTTP {} when requesting the skin",
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err("The skin exceeds the allowed size".to_string());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if chunk.len() > limit - bytes.len() {
            return Err("The skin exceeds the allowed size".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(crate) fn validate_skin_png(bytes: &[u8]) -> Result<(), String> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(64);
    limits.max_image_height = Some(64);
    limits.max_alloc = Some(1024 * 1024);
    let decoder = image::codecs::png::PngDecoder::with_limits(Cursor::new(bytes), limits)
        .map_err(|e| format!("Invalid skin: {e}"))?;
    // El visor 3D usa la rejilla UV de 64x64, así que las skins HD no sirven.
    if !matches!(decoder.dimensions(), (64, 32) | (64, 64)) {
        return Err("The skin must be 64x32 or 64x64".to_string());
    }
    image::DynamicImage::from_decoder(decoder).map_err(|e| format!("Invalid skin: {e}"))?;
    Ok(())
}

fn skin_data_url(bytes: &[u8]) -> Result<String, String> {
    validate_skin_png(bytes)?;
    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(bytes)
    ))
}

fn load_yggdrasil_user(uuid: &str) -> Result<MinecraftUser, String> {
    let user = SettingsManager::read()
        .user
        .iter()
        .find(|u| u.uuid == uuid)
        .cloned()
        .ok_or_else(|| "Account not found".to_string())?;
    if user.user_type != AccountType::Yggdrasil {
        return Err("This account does not use a Yggdrasil server".to_string());
    }
    if user.yggdrasil_server_url.is_none() {
        return Err("The account has no server configured".to_string());
    }
    Ok(user)
}

async fn api_root_of(user: &MinecraftUser) -> Result<String, String> {
    let server = user
        .yggdrasil_server_url
        .as_deref()
        .ok_or_else(|| "The account has no server configured".to_string())?;
    let api_root = launchwerk::auth::yggdrasil::YggdrasilAuth::new()
        .resolve_api_url(server)
        .await
        .map_err(|e| format!("Could not connect to the server: {e}"))?;
    Ok(if api_root.ends_with('/') {
        api_root
    } else {
        format!("{api_root}/")
    })
}

/// El servidor puede haber invalidado el token de juego; se refresca una vez
/// con el client token guardado para no obligar a volver a iniciar sesión.
async fn refresh_user_tokens(
    mut user: MinecraftUser,
    api_root: &str,
) -> Result<MinecraftUser, String> {
    let client_token = user
        .client_token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| "Your session expired. Please sign in again.".to_string())?;
    let refreshed = launchwerk::auth::yggdrasil::YggdrasilAuth::new()
        .refresh(
            api_root,
            &user.access_token,
            client_token,
            &user.uuid,
        )
        .await
        .map_err(|_| "Your session expired. Please sign in again.".to_string())?;
    refreshed
        .save_tokens()
        .map_err(|_| "Could not save the refreshed session".to_string())?;
    user.access_token = refreshed.access_token;
    user.client_token = refreshed.client_token;
    Ok(user)
}

fn model_field(model: &str) -> &'static str {
    if model == "slim" { "slim" } else { "" }
}

fn server_error(status: StatusCode, body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(message) = value
            .get("errorMessage")
            .or_else(|| value.get("message"))
            .and_then(|v| v.as_str())
    {
        return format!("{message} (HTTP {})", status.as_u16());
    }
    format!("The server responded with HTTP {}", status.as_u16())
}

/// PUT/DELETE del perfil con el token de la cuenta. Si el servidor responde 401
/// se refresca el token y se reintenta una sola vez.
async fn send_profile_request(
    uuid: &str,
    method: reqwest::Method,
    form: impl Fn() -> Option<reqwest::multipart::Form>,
    path: &str,
) -> Result<(), String> {
    let mut user = load_yggdrasil_user(uuid)?;
    user.load_tokens()
        .map_err(|e| format!("Could not load the account tokens: {e}"))?;
    let api_root = api_root_of(&user).await?;
    let url = format!(
        "{api_root}api/user/profile/{}/{path}",
        uuid.replace('-', "")
    );

    for attempt in 0..2 {
        let mut request = HTTP
            .request(method.clone(), &url)
            .bearer_auth(&user.access_token);
        if let Some(form) = form() {
            request = request.multipart(form);
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        if status == StatusCode::UNAUTHORIZED {
            if attempt == 0 {
                user = refresh_user_tokens(user, &api_root).await?;
                continue;
            }
            return Err("Your session expired. Please sign in again.".to_string());
        }
        if status == StatusCode::NOT_FOUND {
            return Err(
                "The server does not allow changing the skin from the launcher. Do it from the website."
                    .to_string(),
            );
        }
        return Err(server_error(status, &body));
    }
    Err("Could not complete the operation".to_string())
}

#[tauri::command]
pub async fn get_yggdrasil_skin_profile(uuid: String) -> Result<YggdrasilSkinProfile, String> {
    let user = load_yggdrasil_user(&uuid)?;
    let api_root = api_root_of(&user).await?;
    let profile_url = format!(
        "{api_root}sessionserver/session/minecraft/profile/{}",
        uuid.replace('-', "")
    );

    info!("Fetching Yggdrasil profile for the skin: {profile_url}");
    let response = HTTP
        .get(&profile_url)
        .send()
        .await
        .map_err(|e| format!("Network error while fetching the profile: {e}"))?;
    if matches!(
        response.status(),
        StatusCode::NO_CONTENT | StatusCode::NOT_FOUND
    ) {
        return Ok(YggdrasilSkinProfile {
            skin_url: None,
            model: "classic",
        });
    }
    if !response.status().is_success() {
        return Err(format!(
            "The server responded with HTTP {} when requesting the profile",
            response.status()
        ));
    }
    let body = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read the profile: {e}"))?;

    let Some((skin_url, model)) = parse_skin(&body)? else {
        return Ok(YggdrasilSkinProfile {
            skin_url: None,
            model: "classic",
        });
    };
    let bytes = download(&skin_url, MAX_SKIN_BYTES).await?;
    Ok(YggdrasilSkinProfile {
        skin_url: Some(skin_data_url(&bytes)?),
        model,
    })
}

/// El catálogo de skins es de Mosaicary: solo se descargan PNG de ahí, para que
/// una respuesta alterada no pueda hacer que el backend pida otra dirección.
fn is_catalog_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(url.host_str(), Some("mosaicary.com" | "www.mosaicary.com"))
    })
}

async fn put_skin(uuid: &str, bytes: Vec<u8>, model: &str) -> Result<(), String> {
    let model = model_field(model);
    info!("Uploading Yggdrasil skin for {uuid} ({model})");
    send_profile_request(
        uuid,
        reqwest::Method::PUT,
        || {
            let part = reqwest::multipart::Part::bytes(bytes.clone())
                .file_name("skin.png")
                .mime_str("image/png")
                .ok()?;
            Some(
                reqwest::multipart::Form::new()
                    .text("model", model.to_string())
                    .part("file", part),
            )
        },
        "skin",
    )
    .await
}

/// Sube una skin a la cuenta. El servidor espera `model` vacío para el modelo
/// clásico y `slim` para el delgado.
#[tauri::command]
pub async fn upload_yggdrasil_skin(
    uuid: String,
    file_path: String,
    model: String,
) -> Result<(), String> {
    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| format!("Could not read the skin: {e}"))?;
    if bytes.len() > MAX_SKIN_BYTES {
        return Err("The skin exceeds the allowed size".to_string());
    }
    validate_skin_png(&bytes)?;
    put_skin(&uuid, bytes, &model).await
}

/// Sube una skin elegida en el catálogo de Mosaicary: el PNG se descarga acá
/// (el servidor de la cuenta no acepta direcciones remotas).
#[tauri::command]
pub async fn upload_yggdrasil_skin_url(
    uuid: String,
    skin_url: String,
    model: String,
) -> Result<(), String> {
    if !is_catalog_url(&skin_url) {
        return Err("The catalog skin is not valid".to_string());
    }
    let bytes = download(&skin_url, MAX_SKIN_BYTES).await?;
    validate_skin_png(&bytes)?;
    put_skin(&uuid, bytes, &model).await
}

#[tauri::command]
pub async fn delete_yggdrasil_skin(uuid: String) -> Result<(), String> {
    info!("Removing Yggdrasil skin from {uuid}");
    send_profile_request(&uuid, reqwest::Method::DELETE, || None, "skin").await
}

#[cfg(test)]
#[path = "../tests/commands/yggdrasil_profile.rs"]
mod tests;
