//! Read-only market requests have a window-scoped lifetime. Dropping an aborted
//! future cancels reqwest work, including requests waiting for a network slot.
use futures::future::{AbortHandle, Abortable};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    future::Future,
    sync::LazyLock,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

const CANCELLED: &str = "MARKET_CANCELLED";
const MAX_REQUESTS: usize = 128;
const TTL: Duration = Duration::from_secs(35);
type Key = (String, String);
#[derive(Default)]
struct Requests {
    active: HashMap<Key, AbortHandle>,
    cancelled: HashMap<Key, Instant>,
}
static REQUESTS: LazyLock<Mutex<Requests>> = LazyLock::new(|| Mutex::new(Requests::default()));
static SLOTS: Semaphore = Semaphore::const_new(4);

fn cancel(key: Key) {
    let mut requests = REQUESTS.lock();
    requests.cancelled.retain(|_, time| time.elapsed() < TTL);
    if let Some(handle) = requests.active.get(&key) {
        handle.abort();
    } else {
        // Cancellation IPC can arrive before the corresponding request IPC.
        if requests.cancelled.len() >= MAX_REQUESTS
            && let Some(oldest) = requests
                .cancelled
                .iter()
                .min_by_key(|(_, time)| *time)
                .map(|(key, _)| key.clone())
        {
            requests.cancelled.remove(&oldest);
        }
        requests.cancelled.insert(key, Instant::now());
    }
}

struct Guard(Key);
impl Drop for Guard {
    fn drop(&mut self) {
        REQUESTS.lock().active.remove(&self.0);
    }
}

async fn run<T>(
    key: Key,
    timeout: Duration,
    future: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    let registration = {
        let mut requests = REQUESTS.lock();
        requests.cancelled.retain(|_, time| time.elapsed() < TTL);
        if requests.cancelled.remove(&key).is_some() {
            return Err(CANCELLED.into());
        }
        if requests.active.len() >= MAX_REQUESTS || requests.active.contains_key(&key) {
            return Err("Too many pending market requests".into());
        }
        let (handle, registration) = AbortHandle::new_pair();
        requests.active.insert(key.clone(), handle);
        registration
    };
    let _guard = Guard(key);
    let request = async {
        let _permit = SLOTS.acquire().await.map_err(|e| e.to_string())?;
        future.await
    };
    match tokio::time::timeout(timeout, Abortable::new(request, registration)).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(CANCELLED.into()),
        Err(_) => Err("Market request timed out".into()),
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "command",
    content = "args",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum MarketRequest {
    SearchModrinth {
        query: String,
        loader: String,
        game_version: Option<String>,
        category: Option<String>,
        index: String,
        limit: u32,
        offset: u32,
        project_type: String,
    },
    SearchCurseforge {
        query: String,
        loader: String,
        game_version: Option<String>,
        category: Option<String>,
        index: String,
        limit: u32,
        offset: u32,
        #[serde(default)]
        class_id: Option<u32>,
    },
    GetModrinthProject {
        project_id: String,
    },
    GetModrinthProjectVersions {
        project_id: String,
        loader: Option<String>,
        game_version: Option<String>,
    },
    GetCurseforgeProject {
        mod_id: u32,
    },
    GetCurseforgeProjectFiles {
        mod_id: u32,
        loader: Option<String>,
        game_version: Option<String>,
    },
    GetCurseforgeProjectDescription {
        mod_id: u32,
    },
    GetCurseforgeModpackVersions {
        mod_id: u32,
    },
    GetCurseforgePackManifest {
        file_id: u32,
        file_name: String,
    },
    GetModrinthPackChangelog {
        version_id: String,
    },
    GetCurseforgePackChangelog {
        mod_id: u32,
        file_id: u32,
    },
}

impl MarketRequest {
    async fn execute(self) -> Result<Value, String> {
        use super::market::*;
        match self {
            Self::GetCurseforgePackManifest { file_id, file_name } => {
                let metadata = crate::services::curseforge_modpack::remote_manifest_version(file_id, &file_name)
                    .await
                    .map_err(|e| {
                        tracing::debug!(file_id, error = %e, "Could not read CurseForge modpack manifest");
                        e.to_string()
                    })?;
                serde_json::to_value(metadata).map_err(|e| e.to_string())
            }
            Self::GetModrinthPackChangelog { version_id } => {
                super::pack::modrinth_pack_changelog(version_id)
                    .await
                    .map(Value::String)
            }
            Self::GetCurseforgePackChangelog { mod_id, file_id } => {
                crate::services::curseforge_api::CurseForgeClient::from_settings_or_default()
                    .get_file_changelog(mod_id, file_id)
                    .await
                    .map(Value::String)
                    .map_err(|e| e.to_string())
            }
            Self::SearchModrinth {
                query,
                loader,
                game_version,
                category,
                index,
                limit,
                offset,
                project_type,
            } => {
                search_modrinth(
                    query,
                    loader,
                    game_version,
                    category,
                    index,
                    limit.min(100),
                    offset,
                    project_type,
                )
                .await
            }
            Self::SearchCurseforge {
                query,
                loader,
                game_version,
                category,
                index,
                limit,
                offset,
                class_id,
            } => serde_json::to_value(
                search_curseforge(
                    query,
                    loader,
                    game_version,
                    category,
                    index,
                    limit.min(100),
                    offset,
                    class_id,
                )
                .await?,
            )
            .map_err(|e| e.to_string()),
            Self::GetModrinthProject { project_id } => get_modrinth_project(project_id).await,
            Self::GetModrinthProjectVersions {
                project_id,
                loader,
                game_version,
            } => get_modrinth_project_versions(project_id, loader, game_version).await,
            Self::GetCurseforgeProject { mod_id } => {
                serde_json::to_value(get_curseforge_project(mod_id).await?)
                    .map_err(|e| e.to_string())
            }
            Self::GetCurseforgeProjectFiles {
                mod_id,
                loader,
                game_version,
            } => serde_json::to_value(
                get_curseforge_project_files(mod_id, loader, game_version).await?,
            )
            .map_err(|e| e.to_string()),
            Self::GetCurseforgeProjectDescription { mod_id } => {
                get_curseforge_project_description(mod_id)
                    .await
                    .map(Value::String)
            }
            Self::GetCurseforgeModpackVersions { mod_id } => {
                serde_json::to_value(get_curseforge_modpack_versions(mod_id).await?)
                    .map_err(|e| e.to_string())
            }
        }
    }
}

#[tauri::command]
pub async fn market_request(
    window: tauri::WebviewWindow,
    request_id: String,
    request: MarketRequest,
) -> Result<Value, String> {
    if request_id.len() > 128 {
        return Err("Invalid market request ID".into());
    }
    run((window.label().into(), request_id), TTL, request.execute()).await
}

#[tauri::command]
pub fn cancel_market_request(window: tauri::WebviewWindow, request_id: String) {
    if request_id.len() <= 128 {
        cancel((window.label().into(), request_id));
    }
}

#[cfg(test)]
#[path = "../tests/commands/market_requests.rs"]
mod tests;
