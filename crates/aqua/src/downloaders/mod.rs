mod batch;
mod fabric;
mod forge;
mod jre;
mod minecraft;
mod neoforge;
mod optifine;
mod quilt;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

pub use batch::{DownloadBatch, DownloadItemSpec, GenericBatch};
pub use fabric::FabricBatch;
pub use forge::{ForgeBatch, ForgeVersionInfo};
use futures::stream::{self, StreamExt};
pub use jre::JreBatch;
use log::warn;
pub use minecraft::MinecraftBatch;
pub use neoforge::{NeoForgeBatch, NeoForgeVersionInfo};
pub use optifine::OptiFineBatch;
pub use quilt::QuiltBatch;
use tokio::sync::{Mutex, Semaphore, watch};
use tokio::task::JoinHandle;

use crate::AquaError;
use crate::progress::{DownloadReporter, ProgressSender, ProgressState};
use crate::types::NormalizedVersion;
use crate::utilities::download_file_with_headers;

const DEFAULT_MAX_HANDLES: usize = 2;
// Varias conexiones en paralelo: con muchos archivos pequeños (mods, assets)
// la latencia por archivo domina y 6 hilos eran un cuello de botella.
// Los espejos limitan su propia concurrencia (ver provider.rs).
const DEFAULT_DOWNLOADS_PER_HANDLE: usize = 16;

/// Recibe cada `DownloadHandle` creado por un instalador, para poder cancelarlo desde fuera.
pub type HandleObserver = Arc<dyn Fn(&DownloadHandle) + Send + Sync>;

// ─── DownloadManager ──────────────────────────────────────────────────────────

pub struct DownloadManager {
    game_path: PathBuf,
    handle_semaphore: Arc<Semaphore>,
    downloads_per_handle: usize,
}

impl DownloadManager {
    pub fn new(game_path: PathBuf) -> Self {
        Self {
            game_path,
            handle_semaphore: Arc::new(Semaphore::new(DEFAULT_MAX_HANDLES)),
            downloads_per_handle: DEFAULT_DOWNLOADS_PER_HANDLE,
        }
    }

    pub fn with_max_handles(mut self, max: usize) -> Self {
        self.handle_semaphore = Arc::new(Semaphore::new(max));
        self
    }

    pub fn with_max_downloads(mut self, max: usize) -> Self {
        self.downloads_per_handle = max;
        self
    }

    /// Minecraft-specific: resolve version from Mojang manifest and download everything.
    pub async fn prepare(&self, version_id: &str) -> Result<DownloadHandle, AquaError> {
        let batch = MinecraftBatch::new(&self.game_path, version_id).await?;
        let name = batch.name();
        let total = batch.items().len();
        let version = Some(batch.version().clone());
        let version_id = batch.version().id.clone();

        Ok(DownloadHandle {
            inner: Arc::new(DownloadInner {
                name,
                _version_id: version_id,
                version,
                batch: Box::new(batch),
                handle_sem: Arc::clone(&self.handle_semaphore),
                max_downloads: self.downloads_per_handle,
                cancel_flag: AtomicBool::new(false),
                cancel_tx: watch::channel(false).0,
                join_handle: Mutex::new(None),
                completed_items: Arc::new(AtomicUsize::new(0)),
                total_items: Arc::new(AtomicUsize::new(total)),
            }),
        })
    }

    /// Generic: accept any `DownloadBatch` implementation.
    pub async fn prepare_batch(
        &self,
        batch: Box<dyn DownloadBatch + 'static>,
    ) -> Result<DownloadHandle, AquaError> {
        let name = batch.name();
        let total = batch.items().len();

        Ok(DownloadHandle {
            inner: Arc::new(DownloadInner {
                name,
                _version_id: String::new(),
                version: None,
                batch,
                handle_sem: Arc::clone(&self.handle_semaphore),
                max_downloads: self.downloads_per_handle,
                cancel_flag: AtomicBool::new(false),
                cancel_tx: watch::channel(false).0,
                join_handle: Mutex::new(None),
                completed_items: Arc::new(AtomicUsize::new(0)),
                total_items: Arc::new(AtomicUsize::new(total)),
            }),
        })
    }
}

// ─── DownloadHandle ───────────────────────────────────────────────────────────

struct DownloadInner {
    name: String,
    _version_id: String,
    version: Option<NormalizedVersion>,
    batch: Box<dyn DownloadBatch>,
    handle_sem: Arc<Semaphore>,
    max_downloads: usize,
    cancel_flag: AtomicBool,
    /// Emite `true` al cancelar, para interrumpir también las transferencias en curso.
    cancel_tx: watch::Sender<bool>,
    join_handle: Mutex<Option<JoinHandle<Result<(), AquaError>>>>,
    completed_items: Arc<AtomicUsize>,
    total_items: Arc<AtomicUsize>,
}

/// Clonar comparte el mismo estado (y la misma bandera de cancelación).
#[derive(Clone)]
pub struct DownloadHandle {
    inner: Arc<DownloadInner>,
}

impl DownloadHandle {
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// Minecraft version info, `None` for non-Minecraft batches.
    pub fn version(&self) -> Option<&NormalizedVersion> {
        self.inner.version.as_ref()
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancel_flag.load(Ordering::Relaxed)
    }

    pub fn progress(&self) -> (usize, usize) {
        let c = self.inner.completed_items.load(Ordering::Relaxed);
        let t = self.inner.total_items.load(Ordering::Relaxed);
        (c, t)
    }

    pub fn cancel(&self) {
        self.inner.cancel_flag.store(true, Ordering::Relaxed);
        self.inner.cancel_tx.send_replace(true);
    }

    pub async fn download_all(&self, progress_tx: Option<ProgressSender>) -> Result<(), AquaError> {
        self.start(progress_tx).await?;
        self.wait().await
    }

    pub async fn start(&self, progress_tx: Option<ProgressSender>) -> Result<(), AquaError> {
        let mut slot = self.inner.join_handle.lock().await;
        if slot.is_some() {
            return Err(AquaError::Other(
                "Download already in progress or completed".into(),
            ));
        }

        let inner = Arc::clone(&self.inner);
        let handle = tokio::spawn(async move {
            let _handle_permit = Arc::clone(&inner.handle_sem).acquire_owned().await;
            run_download(inner, progress_tx).await
        });

        *slot = Some(handle);
        Ok(())
    }

    pub async fn wait(&self) -> Result<(), AquaError> {
        let handle = self.inner.join_handle.lock().await.take();
        match handle {
            Some(h) => h.await?,
            None => Err(AquaError::Other("Download not started".into())),
        }
    }
}

// ─── Generic download loop ────────────────────────────────────────────────────

async fn run_download(
    inner: Arc<DownloadInner>,
    progress_tx: Option<ProgressSender>,
) -> Result<(), AquaError> {
    if inner.cancel_flag.load(Ordering::Relaxed) {
        return Err(AquaError::Cancelled);
    }

    inner.batch.prepare().await?;

    let total_items = inner.batch.items().len();
    let bytes_total: u64 = inner.batch.items().iter().filter_map(|i| i.size).sum();
    inner.total_items.store(total_items, Ordering::Relaxed);

    let progress_state: Option<Arc<ProgressState>> = progress_tx
        .as_ref()
        .map(|_| ProgressState::new(total_items, bytes_total));

    // Spawn a lightweight forwarder so byte-level progress is smooth.
    let forwarder = if let (Some(state), Some(tx)) = (&progress_state, &progress_tx) {
        let state = Arc::clone(state);
        let tx = tx.clone();
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                let snapshot = state.snapshot();
                if tx.send(snapshot).is_err() {
                    break;
                }
                let done = state.item_current.load(Ordering::Relaxed) >= state.item_total;
                if done {
                    break;
                }
            }
        }))
    } else {
        None
    };

    // Run the actual download work in a separate block so we can always shut
    // the forwarder down on success, error, or cancellation. Otherwise a
    // required item that fails leaves the forwarder looping forever, keeping
    // the watch channel alive and blocking any monitor waiting on it.
    let result = run_download_impl(
        Arc::clone(&inner),
        progress_state.clone(),
        progress_tx.clone(),
    )
    .await;

    // The forwarder may still be looping if we exited early (error or cancel),
    // so abort and wait for it to release its cloned sender.
    if let Some(handle) = forwarder {
        handle.abort();
        let _ = handle.await;
    }

    result
}

/// Equivalente en el CDN "mediafiles" de una URL del CDN "edge" de CurseForge.
/// Ambos sirven los mismos archivos; si uno falla, el otro suele responder.
fn cdn_mirror_url(url: &str) -> Option<String> {
    url.strip_prefix("https://edge.forgecdn.net/")
        .map(|rest| format!("https://mediafiles.forgecdn.net/{rest}"))
}

/// Descarga un archivo y termina en cuanto se pide cancelar, sin esperar a que acabe la transferencia.
async fn download_or_cancel(
    cancel: &watch::Sender<bool>,
    url: &str,
    path: &Path,
    expected_hash: &str,
    size_hint: Option<u64>,
    reporter: Option<&dyn crate::utilities::ProgressReporter>,
    headers: &[(String, String)],
) -> Result<(), AquaError> {
    let mut rx = cancel.subscribe();
    if *rx.borrow() {
        return Err(AquaError::Cancelled);
    }
    tokio::select! {
        result = download_file_with_headers(url, path, expected_hash, size_hint, reporter, headers) => result,
        _ = rx.wait_for(|cancelled| *cancelled) => Err(AquaError::Cancelled),
    }
}

async fn run_download_impl(
    inner: Arc<DownloadInner>,
    progress_state: Option<Arc<ProgressState>>,
    progress_tx: Option<ProgressSender>,
) -> Result<(), AquaError> {
    // Pre-create unique parent directories once
    let mut parents: Vec<&Path> = inner
        .batch
        .items()
        .iter()
        .filter_map(|item| item.destination.parent())
        .collect();
    parents.sort();
    parents.dedup();
    for parent in parents {
        tokio::fs::create_dir_all(parent).await?;
    }

    let completed = Arc::clone(&inner.completed_items);
    let max_concurrent = inner.max_downloads;
    let items_vec: Vec<_> = inner.batch.items().to_vec();

    let inner_for_finalize = Arc::clone(&inner);
    let progress_state_for_stream = progress_state.clone();
    let progress_tx_for_notify = progress_tx.clone();

    let result = stream::iter(items_vec.into_iter().enumerate().map(move |(id, item)| {
        let c = Arc::clone(&completed);
        let state = progress_state_for_stream.clone();
        let notify_tx = progress_tx_for_notify.clone();
        let inner = Arc::clone(&inner);

        async move {
            if inner.cancel_flag.load(Ordering::Relaxed) {
                return Err(AquaError::Cancelled);
            }

            let reporter = state.as_ref().map(|s| {
                 let reporter = DownloadReporter::for_item(Arc::clone(s), item.size)
                     .track_active_item(id, item.label.clone(), item.stage.clone());
                // Immediate snapshot so the frontend sees the stage change.
                let _ = notify_tx.as_ref().map(|tx| tx.send(s.snapshot()));
                reporter
            });
            let reporter_dyn: Option<&dyn crate::utilities::ProgressReporter> = reporter
                .as_ref()
                .map(|r| r as &dyn crate::utilities::ProgressReporter);

            let mut result = download_or_cancel(&inner.cancel_tx, 
                &item.url,
                &item.destination,
                &item.expected_hash,
                item.size,
                reporter_dyn,
                &item.headers,
            )
            .await;
            if result.is_err()
                && let Some(ref fallback) = item.fallback_url
            {
                warn!("Main URL failed. Using fallback: {fallback}");
                result = download_or_cancel(&inner.cancel_tx, 
                    fallback,
                    &item.destination,
                    &item.expected_hash,
                    item.size,
                    reporter_dyn,
                    &item.headers,
                )
                .await;
                if result.is_err() {
                    warn!("Fallback failed, using fallback with universal.");
                    result = download_or_cancel(&inner.cancel_tx, 
                        &fallback.replace(".jar", "-universal.jar"),
                        &item.destination,
                        &item.expected_hash,
                        item.size,
                        reporter_dyn,
                        &item.headers,
                    )
                    .await;
                }
            }
            // Fuentes alternativas: las explícitas y, para CurseForge, el CDN
            // "mediafiles" equivalente al "edge". Se prueban en orden hasta que una funcione.
            if result.is_err() {
                let alternatives = item
                    .extra_urls
                    .iter()
                    .cloned()
                    .chain(cdn_mirror_url(&item.url));
                for alt in alternatives {
                    if matches!(result, Err(AquaError::Cancelled)) {
                        break;
                    }
                    warn!("{} failed; trying alternative source {alt}", item.label);
                    result = download_or_cancel(
                        &inner.cancel_tx,
                        &alt,
                        &item.destination,
                        &item.expected_hash,
                        item.size,
                        reporter_dyn,
                        &item.headers,
                    )
                    .await;
                    if result.is_ok() {
                        break;
                    }
                }
            }
            if result.is_err()
                && let Some(r) = reporter.as_ref()
            {
                r.abandon();
            }
            let count = c.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(ref s) = state {
                s.item_current.store(count, Ordering::Relaxed);
                s.finish_item(id);
                // Notify immediately when an item is finished.
                let _ = notify_tx.as_ref().map(|tx| tx.send(s.snapshot()));
            }
            if let Err(e) = result {
                if item.required || matches!(e, AquaError::Cancelled) {
                    warn!("Download failed for {}: {e}", item.label);
                    return Err(e);
                }
                warn!("Optional file {} failed all URLs: {e}", item.label);
            }
            Ok::<_, AquaError>(())
        }
    }))
    .buffer_unordered(max_concurrent)
    // Keep downloading independent files, returning the first error after the batch drains.
    .fold(Ok(()), |result, next| async move { result.and(next) })
    .await;

    if inner_for_finalize.cancel_flag.load(Ordering::Relaxed) {
        return Err(AquaError::Cancelled);
    }
    result?;
    inner_for_finalize.batch.finalize(progress_tx).await?;

    Ok(())
}

#[cfg(test)]
#[path = "../tests/downloaders.rs"]
mod tests;
