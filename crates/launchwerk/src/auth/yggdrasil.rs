// Copyright (C) 2026 Santiagolxx, Notstaff and moonlauncher contributors
// SPDX-License-Identifier: GPL-3.0-only
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::MinecraftUser;

/// Timeout for token validation so a bad connection cannot hang the launch.
const VALIDATE_TIMEOUT: Duration = Duration::from_secs(10);

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

// ── Public types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YggdrasilServerInfo {
    pub server_name: String,
    pub skin_domains: Vec<String>,
    pub signature_publickey: Option<String>,
    pub non_email_login: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YggdrasilProfile {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YggdrasilUser {
    pub id: String,
    #[serde(default)]
    pub properties: Vec<YggdrasilProperty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YggdrasilProperty {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
struct YggdrasilAuthResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "clientToken")]
    client_token: String,
    #[serde(default, rename = "availableProfiles")]
    available_profiles: Vec<YggdrasilProfile>,
    #[serde(rename = "selectedProfile")]
    selected_profile: Option<YggdrasilProfile>,
    #[serde(default)]
    #[allow(dead_code)]
    user: Option<YggdrasilUser>,
}

#[derive(Debug, Clone, Deserialize)]
struct YggdrasilMetadataResponse {
    #[serde(default)]
    skin_domains: Vec<String>,
    #[serde(default, rename = "signaturePublickey")]
    signature_publickey: Option<String>,
    #[serde(default)]
    meta: Option<YggdrasilMeta>,
}

#[derive(Debug, Clone, Deserialize)]
struct YggdrasilMeta {
    #[serde(default, rename = "serverName")]
    server_name: Option<String>,
    #[serde(default)]
    feature: Option<YggdrasilFeatures>,
}

#[derive(Debug, Clone, Deserialize)]
struct YggdrasilFeatures {
    #[serde(default, rename = "non_email_login")]
    non_email_login: bool,
}

// ── YggdrasilAuth ───────────────────────────────────────────────────────────

pub struct YggdrasilAuth {
    client: Client,
}

impl Default for YggdrasilAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl YggdrasilAuth {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .user_agent("moonlauncher/2.0")
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Resolve API Location Indication (ALI) — converts a user-provided URL
    /// into the actual Yggdrasil API root.
    pub async fn resolve_api_url(&self, input: &str) -> Result<String, String> {
        let url = ensure_https(input);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to connect to {}: {}", url, e))?;

        // Follow redirects and check for ALI header
        let headers = response.headers().clone();
        if let Some(ali) = headers.get("x-authlib-injector-api-location") {
            let ali_str = ali.to_str().unwrap_or("");
            if !ali_str.is_empty() {
                let resolved = resolve_relative_url(&url, ali_str);
                if resolved != url {
                    return Ok(resolved);
                }
            }
        }

        Ok(url)
    }

    /// Fetch API metadata from the Yggdrasil server root.
    pub async fn fetch_metadata(&self, api_root: &str) -> Result<YggdrasilServerInfo, String> {
        let url = normalize_url(api_root);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch metadata from {}: {}", url, e))?;

        let metadata: YggdrasilMetadataResponse = response
            .json()
            .await
            .map_err(|e| format!("Invalid metadata response: {}", e))?;

        let server_name = metadata
            .meta
            .as_ref()
            .and_then(|m| m.server_name.clone())
            .unwrap_or_else(|| "Unknown Server".to_string());

        let non_email_login = metadata
            .meta
            .as_ref()
            .and_then(|m| m.feature.as_ref())
            .map(|f| f.non_email_login)
            .unwrap_or(false);

        Ok(YggdrasilServerInfo {
            server_name,
            skin_domains: metadata.skin_domains,
            signature_publickey: metadata.signature_publickey,
            non_email_login,
        })
    }

    /// Authenticate with username and password.
    pub async fn authenticate(
        &self,
        api_root: &str,
        username: &str,
        password: &str,
    ) -> Result<MinecraftUser, String> {
        let url = format!("{}authserver/authenticate", normalize_url(api_root));

        let body = serde_json::json!({
            "agent": {
                "name": "Minecraft",
                "version": 1
            },
            "username": username,
            "password": password,
            "requestUser": true
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Authentication request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            let error_msg = parse_yggdrasil_error(&error_text)
                .unwrap_or_else(|| format!("HTTP {}: {}", status, error_text));
            return Err(error_msg);
        }

        let auth_response: YggdrasilAuthResponse = response
            .json()
            .await
            .map_err(|e| format!("Invalid auth response: {}", e))?;

        if auth_response.selected_profile.is_some() {
            return build_user_from_response(auth_response, api_root);
        }

        // The game cannot join a server with an unbound token.
        let profile = auth_response
            .available_profiles
            .first()
            .ok_or_else(|| "No profiles available on this account".to_string())?;
        let bound = self
            .request_refresh(
                api_root,
                &auth_response.access_token,
                &auth_response.client_token,
                Some(profile),
            )
            .await?;
        checked_refresh_user(bound, api_root, &profile.id, &auth_response.client_token)
    }

    /// Refresh an existing access token.
    pub async fn refresh(
        &self,
        api_root: &str,
        access_token: &str,
        client_token: &str,
        profile_id: &str,
    ) -> Result<MinecraftUser, String> {
        // selectedProfile is only for binding a previously unbound token.
        let response = self
            .request_refresh(api_root, access_token, client_token, None)
            .await?;
        checked_refresh_user(response, api_root, profile_id, client_token)
    }

    async fn request_refresh(
        &self,
        api_root: &str,
        access_token: &str,
        client_token: &str,
        profile: Option<&YggdrasilProfile>,
    ) -> Result<YggdrasilAuthResponse, String> {
        let url = format!("{}authserver/refresh", normalize_url(api_root));
        let mut body = serde_json::json!({
            "accessToken": access_token,
            "clientToken": client_token,
            "requestUser": true
        });
        if let Some(profile) = profile {
            body["selectedProfile"] = serde_json::json!(profile);
        }

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|_| "Refresh request failed".to_string())?;

        if !response.status().is_success() {
            return Err(format!(
                "Refresh request returned HTTP {}",
                response.status()
            ));
        }

        response
            .json()
            .await
            .map_err(|_| "Invalid refresh response".to_string())
    }

    /// Validate an access token.
    ///
    /// - `Ok(true)`: the server accepts the token.
    /// - `Ok(false)`: the server explicitly rejects it (401/403).
    /// - `Err(_)`: the server could not be reached or answered unexpectedly.
    ///   The token state is unknown, so the caller must not treat it as invalid.
    pub async fn validate(
        &self,
        api_root: &str,
        access_token: &str,
        client_token: &str,
    ) -> Result<bool, String> {
        let url = format!("{}authserver/validate", normalize_url(api_root));

        let body = serde_json::json!({
            "accessToken": access_token,
            "clientToken": client_token
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .timeout(VALIDATE_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    "Yggdrasil /validate timed out".to_string()
                } else {
                    // Strip the URL so a user-supplied query or credentials never reach logs.
                    format!("Yggdrasil /validate request failed: {}", e.without_url())
                }
            })?;

        let status = response.status();
        if status.is_success() {
            Ok(true)
        } else if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            Ok(false)
        } else {
            Err(format!("Validate request returned HTTP {}", status))
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn ensure_https(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    }
}

fn normalize_url(url: &str) -> String {
    let mut s = url.to_string();
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

fn resolve_relative_url(base: &str, relative: &str) -> String {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        return relative.to_string();
    }
    // Relative URL — resolve against base
    let base_clean = base.trim_end_matches('/');
    if relative.starts_with('/') {
        // Absolute path relative to origin
        if let Some(origin_end) = base_clean.find("://")
            && let Some(slash_pos) = base_clean[origin_end + 3..].find('/')
        {
            let origin = &base_clean[..origin_end + 3 + slash_pos];
            return format!("{}{}", origin, relative);
        }
        format!("{}{}", base_clean, relative)
    } else {
        format!("{}/{}", base_clean, relative)
    }
}

fn parse_yggdrasil_error(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct YggError {
        error: Option<String>,
        #[serde(rename = "errorMessage")]
        error_message: Option<String>,
    }

    let err: YggError = serde_json::from_str(body).ok()?;
    match err.error_message {
        Some(msg) => Some(msg),
        None => err.error,
    }
}

fn build_user_from_response(
    resp: YggdrasilAuthResponse,
    api_root: &str,
) -> Result<MinecraftUser, String> {
    let profile = resp
        .selected_profile
        .as_ref()
        .ok_or_else(|| "The access token is not bound to a profile".to_string())?;
    if resp.access_token.trim().is_empty() || resp.client_token.trim().is_empty() {
        return Err("The authentication server returned an empty token".to_string());
    }

    Ok(MinecraftUser::yggdrasil(
        profile.name.clone(),
        profile.id.clone(),
        resp.access_token,
        resp.client_token,
        api_root.to_string(),
    ))
}

fn checked_refresh_user(
    response: YggdrasilAuthResponse,
    api_root: &str,
    profile_id: &str,
    client_token: &str,
) -> Result<MinecraftUser, String> {
    if response.client_token != client_token
        || !response
            .selected_profile
            .as_ref()
            .is_some_and(|profile| profile.id.eq_ignore_ascii_case(profile_id))
    {
        return Err("The refreshed session does not match the selected profile".to_string());
    }
    let mut user = build_user_from_response(response, api_root)?;
    user.uuid = profile_id.to_owned();
    Ok(user)
}

// ── Authlib Injector Download ───────────────────────────────────────────────

// Pin both the upstream release and its published checksum: this JAR runs as a JVM agent.
const AUTHLIB_INJECTOR_URL: &str = "https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.8/authlib-injector-1.2.8.jar";
const AUTHLIB_INJECTOR_SHA256: &str =
    "9c7f4343e6c82034958ffb48c14a2cb0c85928be7283103ce17da00c6d5a7b10";
const AUTHLIB_INJECTOR_MAX_SIZE: usize = 2 * 1024 * 1024;

fn verify_injector_checksum(bytes: &[u8], expected: &str) -> Result<(), String> {
    let actual = format!("{:x}", sha2::Sha256::digest(bytes));
    if actual != expected {
        return Err(format!(
            "SHA-256 mismatch: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

/// Return the verified cached JVM agent, or download and atomically replace it.
pub async fn download_authlib_injector(shared_dir: &Path) -> Result<PathBuf, String> {
    let cache_dir = shared_dir.join("authlib-injector");
    let jar_path = cache_dir.join("authlib-injector.jar");

    use tokio::io::AsyncReadExt;
    match tokio::fs::File::open(&jar_path).await {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take((AUTHLIB_INJECTOR_MAX_SIZE + 1) as u64)
                .read_to_end(&mut bytes)
                .await
                .map_err(|e| format!("Cannot read cached JAR at {}: {e}", jar_path.display()))?;
            if verify_injector_checksum(&bytes, AUTHLIB_INJECTOR_SHA256).is_ok() {
                return Ok(jar_path);
            }
            log::warn!(
                "Cached authlib-injector is damaged or differs from the pinned release; replacing it"
            );
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(format!(
                "Cannot open cached JAR at {}: {e}",
                jar_path.display()
            ));
        }
    }

    log::info!("Downloading authlib-injector from {AUTHLIB_INJECTOR_URL}");
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .user_agent("moonlauncher/2.0")
        .https_only(true)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;
    let mut response = client.get(AUTHLIB_INJECTOR_URL).send().await.map_err(|e| {
        if e.is_timeout() {
            format!("Timed out downloading JAR from {AUTHLIB_INJECTOR_URL}")
        } else {
            format!("Failed to download JAR: {e:#}")
        }
    })?;
    if !response.status().is_success() {
        return Err(format!(
            "Failed to download JAR: HTTP {}",
            response.status()
        ));
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed to read JAR response: {e}"))?
    {
        if chunk.len() > AUTHLIB_INJECTOR_MAX_SIZE - bytes.len() {
            return Err("Downloaded JAR exceeds the size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    verify_injector_checksum(&bytes, AUTHLIB_INJECTOR_SHA256)?;

    tokio::fs::create_dir_all(&cache_dir)
        .await
        .map_err(|e| format!("Cannot create cache directory {}: {e}", cache_dir.display()))?;
    let temp_path = cache_dir.join(format!("authlib-injector-{}.tmp", uuid::Uuid::new_v4()));
    let result = async {
        tokio::fs::write(&temp_path, &bytes).await?;
        tokio::fs::rename(&temp_path, &jar_path).await
    }
    .await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(format!(
            "Cannot save verified JAR at {}: {e}",
            jar_path.display()
        ));
    }

    log::info!("authlib-injector.jar downloaded and cached");
    Ok(jar_path)
}

#[cfg(test)]
mod injector_tests {
    use super::*;

    #[test]
    fn checksum_rejects_modified_bytes() {
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify_injector_checksum(b"abc", expected).is_ok());
        assert!(verify_injector_checksum(b"ab", expected).is_err());
    }

    #[tokio::test]
    #[ignore = "downloads the pinned upstream JAR over HTTPS"]
    async fn downloads_reuses_and_repairs_injector() {
        let dir = tempfile::tempdir().unwrap();
        let jar = download_authlib_injector(dir.path()).await.unwrap();
        let modified = std::fs::metadata(&jar).unwrap().modified().unwrap();
        assert_eq!(download_authlib_injector(dir.path()).await.unwrap(), jar);
        assert_eq!(
            std::fs::metadata(&jar).unwrap().modified().unwrap(),
            modified
        );
        std::fs::write(&jar, b"partial download").unwrap();
        assert_eq!(download_authlib_injector(dir.path()).await.unwrap(), jar);
        verify_injector_checksum(&std::fs::read(&jar).unwrap(), AUTHLIB_INJECTOR_SHA256).unwrap();
        assert_eq!(std::fs::read_dir(jar.parent().unwrap()).unwrap().count(), 1);
    }
}

/// Fetch the Yggdrasil API metadata and return it as a Base64 string
/// suitable for the `-Dauthlibinjector.yggdrasil.prefetched` JVM argument.
pub async fn fetch_metadata_prefetch(api_root: &str) -> Result<String, String> {
    let url = normalize_url(api_root);
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("moonlauncher/2.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch metadata from {}: {}", url, e))?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read metadata response: {}", e))?;

    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(body.as_bytes());
    Ok(encoded)
}
