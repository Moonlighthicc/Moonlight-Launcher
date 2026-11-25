use super::version_installation;
use crate::core::path_manager::PathManager;
use crate::core::{AppEvent, emit};
use crate::services::java_manager::JavaManager;
use crate::services::settings_manager::SettingsManager;
use aqua::{
    DownloadBatch, DownloadManager, DownloadProgress, JreBatch, JreVendor, java_runtime_preferences,
};
use compact_str::CompactString;
use dashmap::DashMap;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::{mpsc, watch};
use tracing::{error, info};

static DOWNLOAD_QUEUE: OnceLock<Arc<DownloadQueue>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Done,
    Error(String),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadState {
    pub version: Arc<str>,
    pub status: DownloadStatus,
    pub stage: Cow<'static, str>,
    pub item_current: u64,
    pub item_total: u64,
    pub bytes_current: u64,
    pub bytes_total: u64,
    pub current_item: Option<String>,
}

impl DownloadState {
    fn new(version: Arc<str>) -> Self {
        Self {
            version,
            status: DownloadStatus::Pending,
            stage: Cow::Borrowed("pending"),
            item_current: 0,
            item_total: 0,
            bytes_current: 0,
            bytes_total: 0,
            current_item: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            DownloadStatus::Pending | DownloadStatus::Downloading
        )
    }
}

pub struct DownloadQueue {
    sender: mpsc::Sender<Arc<str>>,
    active: DashMap<Arc<str>, DownloadState>,
    pending_batches: DashMap<
        Arc<str>,
        (
            Box<dyn DownloadBatch + 'static>,
            super::shared_storage::SharedGuard,
        ),
    >,
    /// Descargas `aqua` en curso que se pueden cancelar.
    running: DashMap<Arc<str>, aqua::DownloadHandle>,
    /// Tareas que no admiten cancelación (instalación de modpacks, lotes de JRE).
    uncancellable: DashMap<Arc<str>, ()>,
    /// Cancelaciones pedidas por el usuario que aún no han terminado de aplicarse.
    cancel_requests: DashMap<Arc<str>, ()>,
    /// Tareas terminadas (ok/error) recientemente. Permite a la UI recuperar el
    /// resultado en su sondeo periódico aunque se haya perdido el evento.
    finished: DashMap<Arc<str>, (DownloadState, std::time::Instant)>,
}

/// Cuánto tiempo se recuerda el resultado de una tarea terminada.
const FINISHED_RETENTION: std::time::Duration = std::time::Duration::from_secs(120);

/// Proveedor de JRE elegido por el usuario para una versión de Java.
fn preferred_jre_vendor(java_version: u8) -> Option<JreVendor> {
    JreVendor::from_id(SettingsManager::read().get_jre_vendor(java_version))
}

/// Aplica el proveedor de descargas configurado al downloader global de `aqua`.
fn apply_download_provider() {
    let provider_id = SettingsManager::read().download_provider.clone();
    aqua::set_download_provider(aqua::DownloadProvider::from_id(provider_id.as_str()));
}

impl DownloadQueue {
    pub fn get() -> &'static Arc<DownloadQueue> {
        DOWNLOAD_QUEUE
            .get()
            .expect("BUG: DownloadQueue used before initialization")
    }

    pub async fn init(_app_handle: Option<tauri::AppHandle>) -> Arc<Self> {
        let (tx, rx) = mpsc::channel::<Arc<str>>(64);

        let queue = Arc::new(Self {
            sender: tx,
            active: DashMap::new(),
            pending_batches: DashMap::new(),
            running: DashMap::new(),
            uncancellable: DashMap::new(),
            cancel_requests: DashMap::new(),
            finished: DashMap::new(),
        });

        let queue_clone = queue.clone();
        tokio::spawn(async move {
            Self::worker(rx, queue_clone).await;
        });

        let _ = DOWNLOAD_QUEUE.set(queue.clone());
        queue
    }

    pub async fn enqueue(&self, version: impl Into<Arc<str>>) {
        let version: Arc<str> = version.into();
        if let Err(e) = zellkern::path_security::validate_version(&version) {
            emit(AppEvent::DError {
                version: version.clone(),
                message: e.to_string().into(),
            });
            return;
        }
        let shared_guard = super::shared_storage::acquire().await;

        if let Some(state) = self.active.get(&version)
            && state.is_active()
        {
            return;
        }

        let shared = PathManager::get().get_shared_dir();
        let requested = version.clone();
        let complete = tokio::task::spawn_blocking(move || {
            let _shared_guard = shared_guard;
            version_installation::missing_dependencies(&shared, &requested).is_empty()
        })
        .await
        .unwrap_or(false);
        if complete {
            info!("{} is already installed, skipping download", &*version);
            return;
        }

        info!("{} queued", &*version);

        // Another caller may have enqueued the same version during the disk check.
        match self.active.entry(version.clone()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                if entry.get().is_active() {
                    return;
                }
                entry.insert(DownloadState::new(version.clone()));
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(DownloadState::new(version.clone()));
            }
        }

        self.uncancellable.remove(&version);
        self.cancel_requests.remove(&version);
        self.finished.remove(&version);
        emit(AppEvent::DEnqueue {
            version: version.clone(),
            cancellable: true,
        });

        if let Err(e) = self.sender.send(version).await {
            error!("Failed to enqueue download: {}", e);
        }
    }

    /// Mueve las tareas terminadas a `finished` y descarta los resultados antiguos.
    fn prune(&self) {
        let done: Vec<Arc<str>> = self
            .active
            .iter()
            .filter(|r| !r.value().is_active())
            .map(|r| r.key().clone())
            .collect();
        for key in done {
            if let Some((k, state)) = self.active.remove(&key) {
                self.finished.insert(k, (state, std::time::Instant::now()));
            }
        }
        self.finished
            .retain(|_, (_, at)| at.elapsed() < FINISHED_RETENTION);
    }

    /// Estado actual para el sondeo de la UI: tareas activas + recién terminadas.
    pub async fn get_snapshot(&self) -> Vec<DownloadState> {
        let mut out: Vec<DownloadState> = self.active.iter().map(|r| r.value().clone()).collect();
        out.extend(
            self.finished
                .iter()
                .filter(|r| r.value().1.elapsed() < FINISHED_RETENTION)
                .map(|r| r.value().0.clone()),
        );
        out
    }

    pub async fn get_active_downloads(&self) -> Vec<DownloadState> {
        self.active
            .iter()
            .filter(|r| r.value().is_active())
            .map(|r| r.value().clone())
            .collect()
    }

    /// Tarea visible en la cola que no se puede cancelar (p. ej. instalación de modpack).
    pub async fn enqueue_work(&self, label: impl Into<Arc<str>>) {
        self.enqueue_work_inner(label.into(), false);
    }

    /// Tarea visible en la cola cuyas descargas `aqua` sí se pueden cancelar.
    pub async fn enqueue_tracked(&self, label: impl Into<Arc<str>>) {
        self.enqueue_work_inner(label.into(), true);
    }

    fn enqueue_work_inner(&self, label: Arc<str>, cancellable: bool) {
        self.active.insert(
            label.clone(),
            DownloadState {
                version: label.clone(),
                status: DownloadStatus::Downloading,
                stage: Cow::Borrowed("downloading"),
                item_current: 0,
                item_total: 0,
                bytes_current: 0,
                bytes_total: 0,
                current_item: None,
            },
        );
        if cancellable {
            self.uncancellable.remove(&label);
        } else {
            self.uncancellable.insert(label.clone(), ());
        }
        self.cancel_requests.remove(&label);
        self.finished.remove(&label);
        emit(AppEvent::DEnqueue {
            version: label,
            cancellable,
        });
    }

    /// Pide cancelar una descarga. Devuelve `false` si no es cancelable o ya terminó.
    /// Si la descarga aún no ha empezado a transferir, se cancela en cuanto arranca.
    pub fn cancel(&self, version: &str) -> bool {
        let key: Arc<str> = Arc::from(version);
        if self.uncancellable.contains_key(&key) {
            return false;
        }

        // Remove a task that is still waiting in the queue immediately. The worker
        // may be busy with another download; leaving this as a cancel request alone
        // would make the UI appear stuck until that earlier task finishes. The
        // queued message is harmless: process_version skips it when no state exists.
        let pending = self.active.remove_if(&key, |_, state| {
            matches!(&state.status, DownloadStatus::Pending)
        });
        let pending = match pending {
            Some(_) => true,
            None => {
                // It may already be running (or may have finished). Re-check its
                // state before deciding whether a live cancellation can be sent.
                if !self.active.get(&key).is_some_and(|state| state.is_active()) {
                    return false;
                }
                false
            }
        };

        if pending {
            self.release(&key);
            self.pending_batches.remove(&key);
            info!("Pending download cancelled: {}", key);
            emit(AppEvent::DCancel { version: key });
            return true;
        }

        self.cancel_requests.insert(key.clone(), ());
        if let Some(handle) = self.running.get(&key) {
            handle.cancel();
        }
        info!("Cancellation requested for {}", key);
        true
    }

    fn is_cancel_requested(&self, key: &Arc<str>) -> bool {
        self.cancel_requests.contains_key(key)
    }

    /// Libera el estado auxiliar de una tarea y la marca como cancelada en la UI.
    fn finish_cancelled(&self, label: &Arc<str>) {
        self.release(label);
        self.active.remove(label);
        info!("Download cancelled: {}", label);
        emit(AppEvent::DCancel {
            version: label.clone(),
        });
    }

    /// Vacía el progreso del lote de la tarea. Una tarea puede tener varios lotes
    /// seguidos (base de Minecraft, JRE, loader…): entre uno y otro, la barra no
    /// debe conservar los bytes del lote anterior (eso daba un 100% falso).
    /// `emit_event` avisa a la UI con `pending`; al empezar un lote no hace falta,
    /// porque la etapa de inicio ya llegó como evento.
    pub(crate) fn reset_batch_progress(&self, label: &Arc<str>, emit_event: bool) {
        if let Some(mut state) = self.active.get_mut(label) {
            state.stage = Cow::Borrowed("pending");
            state.item_current = 0;
            state.item_total = 0;
            state.bytes_current = 0;
            state.bytes_total = 0;
            state.current_item = None;
        }
        if emit_event {
            emit(AppEvent::DStage {
                version: label.clone(),
                stage: Cow::Borrowed("pending"),
                info: None,
            });
        }
    }

    /// Quita las banderas y handles auxiliares de una tarea que ya terminó.
    fn release(&self, label: &Arc<str>) {
        self.running.remove(label);
        self.uncancellable.remove(label);
        self.cancel_requests.remove(label);
    }

    pub async fn enqueue_batch(
        &self,
        version: impl Into<Arc<str>>,
        batch: Box<dyn DownloadBatch + 'static>,
        shared_guard: super::shared_storage::SharedGuard,
    ) {
        let version: Arc<str> = version.into();

        if let Some(state) = self.active.get(&version)
            && state.is_active()
        {
            return;
        }

        info!("Batch {} queued", &*version);

        self.pending_batches
            .insert(version.clone(), (batch, shared_guard));
        self.active
            .insert(version.clone(), DownloadState::new(version.clone()));
        self.uncancellable.insert(version.clone(), ());
        self.cancel_requests.remove(&version);
        self.finished.remove(&version);

        emit(AppEvent::DEnqueue {
            version: version.clone(),
            cancellable: false,
        });

        if let Err(e) = self.sender.send(version).await {
            error!("Failed to enqueue batch: {}", e);
        }
    }

    pub async fn finish_work(&self, label: &str) {
        let label: Arc<str> = label.into();
        self.release(&label);
        if let Some(mut state) = self.active.get_mut(&label) {
            state.status = DownloadStatus::Done;
        }
        emit(AppEvent::DFinish {
            version: label.clone(),
        });
        self.prune();
    }

    async fn worker(mut rx: mpsc::Receiver<Arc<str>>, queue: Arc<DownloadQueue>) {
        while let Some(version) = rx.recv().await {
            Self::process_version(&queue, version).await;
        }

        error!("Download worker terminated unexpectedly — the channel was closed");
    }

    async fn process_version(queue: &Arc<DownloadQueue>, version: Arc<str>) {
        apply_download_provider();
        let _shared_guard = super::shared_storage::acquire().await;
        let shared_dir = PathManager::get().get_shared_dir();
        let manager = DownloadManager::new(shared_dir.clone()).with_max_downloads(4);

        if queue.is_cancel_requested(&version) {
            queue.finish_cancelled(&version);
            return;
        }

        if let Some(mut state) = queue.active.get_mut(&version) {
            state.status = DownloadStatus::Downloading;
        } else {
            error!("State not found for {}, skipping", version);
            queue.prune();
            return;
        }

        // JRE batch (pending_batches)
        if let Some(batch) = queue.pending_batches.remove(&version) {
            let (_, (batch, _batch_guard)) = batch;
            let (tx, progress_rx) = watch::channel(DownloadProgress::empty(0));
            let monitor = monitor_download_progress(version.clone(), progress_rx, queue.clone());

            let (dl_result, ()) = tokio::join!(async { batch.finalize(Some(tx)).await }, monitor);

            match dl_result {
                Ok(_) => {
                    info!("Batch {} completed successfully", version);
                    if let Some(mut state) = queue.active.get_mut(&version) {
                        state.status = DownloadStatus::Done;
                    }
                    emit(AppEvent::DFinish {
                        version: version.clone(),
                    });
                    emit(AppEvent::JREChanged);
                }
                Err(e) => {
                    emit_and_set_error(
                        queue,
                        &version,
                        format!("Could not complete batch {}: {:?}", version, e),
                    );
                }
            }

            queue.prune();
            return;
        }

        let result = async {
            set_tracked_stage(&version, "resolving", Some(version.to_string()));
            version_installation::begin(&shared_dir, &version).await?;
            let result = if version.contains("-OptiFine_") {
                Self::process_optifine_version(shared_dir.clone(), &manager, queue, version.clone())
                    .await
            } else if version.contains("-neoforge-") {
                Self::process_neoforge_version(shared_dir.clone(), &manager, queue, version.clone())
                    .await
            } else if version.contains("-forge-") && !version.contains("-neoforge-") {
                Self::process_forge_version(shared_dir.clone(), &manager, queue, version.clone())
                    .await
            } else if let Some((game_version, loader_version)) = parse_fabric_version(&version) {
                Self::process_fabric_version(
                    shared_dir.clone(),
                    &manager,
                    queue,
                    version.clone(),
                    game_version,
                    loader_version,
                )
                .await
            } else if let Some((game_version, loader_version)) = parse_quilt_version(&version) {
                Self::process_quilt_version(
                    shared_dir.clone(),
                    &manager,
                    queue,
                    version.clone(),
                    game_version,
                    loader_version,
                )
                .await
            } else {
                // Vanilla (or invalid version)
                let handle = manager.prepare(&version).await?;
                download_with_progress(version.clone(), handle, queue.clone()).await
            };

            result?;
            version_installation::finish(&shared_dir, &version).await?;
            Ok::<_, aqua::AquaError>(())
        }
        .await;

        match result {
            Ok(_) => {
                info!("Version {} downloaded successfully", version);
                queue.release(&version);
                if let Some(mut state) = queue.active.get_mut(&version) {
                    state.status = DownloadStatus::Done;
                }
                emit(AppEvent::DFinish { version });
            }
            // Si el usuario pidió cancelar, un fallo concurrente también cuenta como cancelación.
            Err(e) if matches!(e, aqua::AquaError::Cancelled) || queue.is_cancel_requested(&version) => {
                queue.finish_cancelled(&version);
            }
            Err(e) => {
                queue.release(&version);
                emit_and_set_error(
                    queue,
                    &version,
                    format!("Could not download {}: {:?}", version, e),
                );
            }
        }

        queue.prune();
    }

    async fn process_fabric_version(
        shared_dir: PathBuf,
        manager: &DownloadManager,
        queue: &Arc<DownloadQueue>,
        version: Arc<str>,
        game_version: String,
        loader_version: String,
    ) -> Result<(), aqua::AquaError> {
        emit_stage(
            &version,
            "resolving",
            Some(format!("Fabric loader {}", loader_version)),
        );

        let (batch, base) = tokio::join!(
            aqua::FabricBatch::new(&shared_dir, &game_version, &loader_version),
            download_base_mc(version.clone(), &game_version, manager, queue),
        );
        let loader = async {
            set_tracked_stage(&version, "resolving", Some(format!("Fabric {loader_version}")));
            let batch = batch?;
            let handle = manager.prepare_batch(Box::new(batch)).await?;
            download_with_progress(version.clone(), handle, queue.clone()).await
        }.await;
        if queue.is_cancel_requested(&version) {
            return Err(aqua::AquaError::Cancelled);
        }
        loader.and(base)
    }

    async fn process_quilt_version(
        shared_dir: PathBuf,
        manager: &DownloadManager,
        queue: &Arc<DownloadQueue>,
        version: Arc<str>,
        game_version: String,
        loader_version: String,
    ) -> Result<(), aqua::AquaError> {
        emit_stage(
            &version,
            "resolving",
            Some(format!("Quilt loader {}", loader_version)),
        );

        let batch = aqua::QuiltBatch::new(&shared_dir, &game_version, &loader_version).await?;
        let handle = manager.prepare_batch(Box::new(batch)).await?;
        download_with_progress(version.clone(), handle, queue.clone()).await?;

        download_base_mc(version, &game_version, manager, queue).await
    }

    async fn process_optifine_version(
        shared_dir: PathBuf,
        manager: &DownloadManager,
        queue: &Arc<DownloadQueue>,
        version: Arc<str>,
    ) -> Result<(), aqua::AquaError> {
        emit_stage(&version, "resolving", Some("OptiFine".into()));
        let release = crate::commands::optifine::resolve_version(&version)
            .await
            .map_err(aqua::AquaError::Other)?;
        let (base, _) = aqua::resolve_version_data(&release.game_version).await?;
        let java_version = base.java_version;
        if !JavaManager::is_installed(java_version) {
            emit_stage(
                &version,
                "jre",
                Some(format!("Java {java_version} for OptiFine")),
            );
            let pkg =
                JavaManager::get_latest_package(java_version, preferred_jre_vendor(java_version))
                    .await
                    .map_err(|e| aqua::AquaError::Other(e.to_string()))?;
            let batch = JreBatch::new(java_version, pkg, JavaManager::get_jre_dir(java_version));
            let handle = manager.prepare_batch(Box::new(batch)).await?;
            download_with_progress(version.clone(), handle, queue.clone()).await?;
        }
        download_base_mc(version.clone(), &release.game_version, manager, queue).await?;
        emit_stage(&version, "resolving", Some("OptiFine installer".into()));
        let batch = aqua::OptiFineBatch::new(
            &shared_dir,
            release,
            JavaManager::get_java_binary(java_version),
        )
        .await?;
        let handle = manager.prepare_batch(Box::new(batch)).await?;
        download_with_progress(version, handle, queue.clone()).await
    }

    async fn process_forge_version(
        shared_dir: PathBuf,
        manager: &DownloadManager,
        queue: &Arc<DownloadQueue>,
        version: Arc<str>,
    ) -> Result<(), aqua::AquaError> {
        let parts: Vec<&str> = version.split("-forge-").collect();
        let (gv, fv) = match parts.as_slice() {
            [gv, fv] => (*gv, *fv),
            _ => {
                return Err(aqua::AquaError::Other(format!(
                    "Forge version format invalid: {}",
                    version
                )));
            }
        };

        // Determine Java preference based on MC version.
        let java_pref = java_runtime_preferences(gv);

        // Ensure a suitable Java runtime is installed, making the download visible.
        if !java_pref.iter().any(|v| JavaManager::is_installed(*v)) {
            let java_version = java_pref[0];
            emit_stage(
                &version,
                "jre",
                Some(format!("Java {} for Forge {}", java_version, version)),
            );
            let pkg =
                JavaManager::get_latest_package(java_version, preferred_jre_vendor(java_version))
                    .await
                    .map_err(|e| aqua::AquaError::Other(e.to_string()))?;
            let dest_dir = JavaManager::get_jre_dir(java_version);
            let jre_batch = JreBatch::new(java_version, pkg, dest_dir);
            let jre_handle = manager.prepare_batch(Box::new(jre_batch)).await?;
            download_with_progress(version.clone(), jre_handle, queue.clone()).await?;
        }

        let java_path = java_pref
            .iter()
            .copied()
            .find(|v| JavaManager::is_installed(*v))
            .map(JavaManager::get_java_binary);

        let Some(java_path) = java_path else {
            return Err(aqua::AquaError::Other(
                "Could not install a compatible Java runtime".into(),
            ));
        };

        let installer_url = aqua::ForgeBatch::resolve_installer_url(gv, fv);
        // Fetch and unpack the installer concurrently; its processors still require the base client.
        let (base, batch) = tokio::join!(
            download_base_mc(version.clone(), gv, manager, queue),
            aqua::ForgeBatch::new(&shared_dir, gv, fv, &installer_url, Some(java_path)),
        );
        base?;
        set_tracked_stage(&version, "resolving", Some(format!("Forge {fv}")));
        let batch = batch?;
        let handle = manager.prepare_batch(Box::new(batch)).await?;
        download_with_progress(version, handle, queue.clone()).await
    }

    async fn process_neoforge_version(
        shared_dir: PathBuf,
        manager: &DownloadManager,
        queue: &Arc<DownloadQueue>,
        version: Arc<str>,
    ) -> Result<(), aqua::AquaError> {
        let parts: Vec<&str> = version.split("-neoforge-").collect();
        let (gv, nv) = match parts.as_slice() {
            [gv, nv] => (*gv, *nv),
            _ => {
                return Err(aqua::AquaError::Other(format!(
                    "NeoForge version format invalid: {}",
                    version
                )));
            }
        };

        // NeoForge only exists for MC 1.20.2+ / future year-based versions.
        let java_pref = java_runtime_preferences(gv);

        if !java_pref.iter().any(|v| JavaManager::is_installed(*v)) {
            let java_version = java_pref[0];
            emit_stage(
                &version,
                "jre",
                Some(format!("Java {} for NeoForge {}", java_version, version)),
            );
            let pkg =
                JavaManager::get_latest_package(java_version, preferred_jre_vendor(java_version))
                    .await
                    .map_err(|e| aqua::AquaError::Other(e.to_string()))?;
            let dest_dir = JavaManager::get_jre_dir(java_version);
            let jre_batch = JreBatch::new(java_version, pkg, dest_dir);
            let jre_handle = manager.prepare_batch(Box::new(jre_batch)).await?;
            download_with_progress(version.clone(), jre_handle, queue.clone()).await?;
        }

        let java_path = java_pref
            .iter()
            .copied()
            .find(|v| JavaManager::is_installed(*v))
            .map(JavaManager::get_java_binary);

        let Some(java_path) = java_path else {
            return Err(aqua::AquaError::Other(
                "Could not install a compatible Java runtime".into(),
            ));
        };

        let installer_url = aqua::NeoForgeBatch::resolve_installer_url(nv);
        let (base, batch) = tokio::join!(
            download_base_mc(version.clone(), gv, manager, queue),
            aqua::NeoForgeBatch::new(&shared_dir, gv, nv, &installer_url, Some(java_path)),
        );
        base?;
        set_tracked_stage(&version, "resolving", Some(format!("NeoForge {nv}")));
        let batch = batch?;
        let handle = manager.prepare_batch(Box::new(batch)).await?;
        download_with_progress(version, handle, queue.clone()).await
    }
}

/// Observador para instalaciones: registra cada descarga `aqua` de la tarea,
/// de modo que la cancelación del usuario llegue hasta ella.
pub(crate) fn handle_observer(label: Arc<str>) -> aqua::HandleObserver {
    let queue = DownloadQueue::get().clone();
    Arc::new(move |handle: &aqua::DownloadHandle| {
        queue.running.insert(label.clone(), handle.clone());
        if queue.is_cancel_requested(&label) {
            handle.cancel();
        }
    })
}

pub(crate) fn was_cancel_requested(label: &str) -> bool {
    DownloadQueue::get().cancel_requests.contains_key(label)
}

/// Avisa a la UI de que la tarea cancelada está eliminando sus archivos.
pub(crate) fn mark_deleting(label: &Arc<str>) {
    emit(AppEvent::DStage {
        version: label.clone(),
        stage: Cow::Borrowed("deleting"),
        info: None,
    });
}

pub(crate) fn finish_cancelled_work(label: &Arc<str>) {
    DownloadQueue::get().finish_cancelled(label);
}

fn emit_and_set_error(queue: &DownloadQueue, version: &Arc<str>, msg: String) {
    error!("{}", msg);
    emit(AppEvent::DError {
        version: version.clone(),
        message: CompactString::from(&*msg),
    });
    if let Some(mut state) = queue.active.get_mut(version) {
        state.status = DownloadStatus::Error(msg);
    }
}

fn emit_stage(version: &Arc<str>, stage: &'static str, info: Option<String>) {
    emit(AppEvent::DStage {
        version: version.clone(),
        stage: Cow::Borrowed(stage),
        info,
    });
}

/// Update both the backend snapshot and the live UI event for a tracked task.
/// This keeps polling/resync from overwriting a newer phase with a stale one.
pub(crate) fn set_tracked_stage(
    version: &Arc<str>,
    stage: &'static str,
    info: Option<String>,
) {
    if let Some(mut state) = DownloadQueue::get().active.get_mut(version) {
        state.stage = Cow::Borrowed(stage);
        state.current_item = info.clone();
    }
    emit_stage(version, stage, info);
}

async fn download_with_progress(
    version: Arc<str>,
    handle: aqua::DownloadHandle,
    queue: Arc<DownloadQueue>,
) -> Result<(), aqua::AquaError> {
    if queue.is_cancel_requested(&version) {
        return Err(aqua::AquaError::Cancelled);
    }
    queue.running.insert(version.clone(), handle.clone());
    // Re-check after registering: a cancel that raced with the insert must still apply.
    if queue.is_cancel_requested(&version) {
        handle.cancel();
    }
    queue.reset_batch_progress(&version, false);
    let (tx, rx) = watch::channel(DownloadProgress::empty(handle.progress().1));
    let monitor = monitor_download_progress(version.clone(), rx, queue.clone());
    let (result, ()) = tokio::join!(handle.download_all(Some(tx)), monitor);
    queue.running.remove(&version);
    // El lote terminó (con éxito o no): la barra se vacía hasta que llegue el siguiente.
    queue.reset_batch_progress(&version, true);
    result
}

/// Ejecuta una descarga `aqua` registrada en la cola como tarea cancelable:
/// aparece en la sección de descargas con progreso y botón de cancelar.
pub(crate) async fn run_tracked_download(
    label: Arc<str>,
    handle: aqua::DownloadHandle,
) -> Result<(), aqua::AquaError> {
    let queue = DownloadQueue::get().clone();
    queue.enqueue_tracked(label.clone()).await;
    let result = download_with_progress(label.clone(), handle, queue.clone()).await;
    let cancelled = match &result {
        Err(aqua::AquaError::Cancelled) => true,
        Err(_) => queue.is_cancel_requested(&label),
        Ok(()) => false,
    };
    match &result {
        Ok(()) => queue.finish_work(&label).await,
        Err(_) if cancelled => queue.finish_cancelled(&label),
        Err(e) => {
            queue.release(&label);
            emit_and_set_error(
                &queue,
                &label,
                format!("Could not complete the download: {e}"),
            );
            queue.prune();
        }
    }
    result
}

async fn download_base_mc(
    version: Arc<str>,
    game_version: &str,
    manager: &DownloadManager,
    queue: &Arc<DownloadQueue>,
) -> Result<(), aqua::AquaError> {
    if queue.is_cancel_requested(&version) {
        return Err(aqua::AquaError::Cancelled);
    }
    set_tracked_stage(&version, "mc", Some(format!("Minecraft {}", game_version)));
    let shared = PathManager::get().get_shared_dir();
    let check_root = shared.clone();
    let check_version = game_version.to_owned();
    let complete = tokio::task::spawn_blocking(move || {
        version_installation::is_complete(&check_root, &check_version)
    }).await?;
    if complete {
        return Ok(());
    }
    if queue.is_cancel_requested(&version) {
        return Err(aqua::AquaError::Cancelled);
    }
    version_installation::begin(&shared, game_version).await?;
    let base_handle = manager.prepare(game_version).await?;
    download_with_progress(version, base_handle, queue.clone()).await?;
    version_installation::finish(&shared, game_version).await?;
    Ok(())
}

fn parse_fabric_version(version: &str) -> Option<(String, String)> {
    let rest = version.strip_prefix("fabric-loader-")?;
    let (loader, game) = rest.rsplit_once('-')?;
    Some((game.to_string(), loader.to_string()))
}

fn parse_quilt_version(version: &str) -> Option<(String, String)> {
    let rest = version.strip_prefix("quilt-loader-")?;
    let (loader, game) = rest.rsplit_once('-')?;
    Some((game.to_string(), loader.to_string()))
}

pub(crate) async fn monitor_download_progress(
    version: Arc<str>,
    mut progress_rx: watch::Receiver<DownloadProgress>,
    queue: Arc<DownloadQueue>,
) {
    let mut last_stage: Option<String> = None;

    loop {
        if progress_rx.changed().await.is_err() {
            break;
        }
        let progress = progress_rx.borrow().clone();

        if let Some(mut state) = queue.active.get_mut(&version) {
            state.stage = Cow::Borrowed(progress.stage.as_str());
            state.item_current = progress.item_current as u64;
            state.item_total = progress.item_total as u64;
            state.bytes_current = progress.bytes_current;
            state.bytes_total = progress.bytes_total;
            state.current_item = progress.current_item.clone();
        }

        let stage_str = progress.stage.as_str().to_string();
        let current_item = progress.current_item.clone();

        emit(AppEvent::DProgress {
            version: version.clone(),
            stage: Cow::Borrowed(progress.stage.as_str()),
            item_current: progress.item_current as u64,
            item_total: progress.item_total as u64,
            bytes_current: progress.bytes_current,
            bytes_total: progress.bytes_total,
            current_item,
        });

        if last_stage.as_ref() != Some(&stage_str) {
            emit(AppEvent::DStage {
                version: version.clone(),
                stage: Cow::Borrowed(progress.stage.as_str()),
                info: progress.current_item,
            });
            last_stage = Some(stage_str);
        }
    }
}

#[cfg(test)]
#[path = "../tests/services/download_queue.rs"]
mod tests;
