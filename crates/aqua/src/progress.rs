use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard};

pub type ProgressSender = tokio::sync::watch::Sender<DownloadProgress>;

/// Logical stage of a download/installation.
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStage {
    /// Resolving version/profile metadata.
    Resolving,
    /// Waiting for a successful HTTP response before reading the body.
    Connecting,
    /// Downloading a library.
    Library,
    /// Downloading an asset object.
    Asset,
    /// Downloading a native library.
    Native,
    /// Downloading the client/server jar.
    Client,
    /// Verifying an existing file.
    Verifying,
    /// Extracting an archive (e.g. natives, JRE).
    Extracting,
    /// Running a post-processor.
    Processing,
    /// Downloading/installing a Java runtime.
    Jre,
    /// Fallback/unknown stage.
    Generic,
}

impl DownloadStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadStage::Resolving => "resolving",
            DownloadStage::Connecting => "connecting",
            DownloadStage::Library => "library",
            DownloadStage::Asset => "asset",
            DownloadStage::Native => "native",
            DownloadStage::Client => "client",
            DownloadStage::Verifying => "verifying",
            DownloadStage::Extracting => "extracting",
            DownloadStage::Processing => "processing",
            DownloadStage::Jre => "jre",
            DownloadStage::Generic => "generic",
        }
    }
}

/// Snapshot of progress for a download task.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadProgress {
    pub stage: DownloadStage,
    pub item_current: usize,
    pub item_total: usize,
    pub bytes_current: u64,
    pub bytes_total: u64,
    pub current_item: Option<String>,
    pub current_item_bytes: u64,
    pub current_item_total: Option<u64>,
}

impl DownloadProgress {
    pub fn empty(item_total: usize) -> Self {
        Self {
            stage: DownloadStage::Generic,
            item_current: 0,
            item_total,
            bytes_current: 0,
            bytes_total: 0,
            current_item: None,
            current_item_bytes: 0,
            current_item_total: None,
        }
    }
}

/// Shared progress state for a whole batch.
///
/// `bytes_current` only ever counts bytes that were really received for the
/// items. `bytes_total` is the sum of the *expected* size of every item: it
/// starts from the sizes declared by the batch and is corrected as soon as the
/// server reports the real size, so it stays consistent with `bytes_current`.
#[derive(Debug)]
pub struct ProgressState {
    pub item_total: usize,
    pub bytes_total: AtomicU64,
    pub item_current: AtomicUsize,
    pub bytes_current: AtomicU64,
    pub current_item: StdMutex<Option<String>>,
    pub current_stage: StdMutex<DownloadStage>,
    pub current_item_bytes: AtomicU64,
    pub current_item_total: AtomicU64,
    active: StdMutex<std::collections::BTreeMap<usize, ActiveItem>>,
}

#[derive(Debug)]
struct ActiveItem {
    name: String,
    download_stage: DownloadStage,
    phase: TransferPhase,
    bytes: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TransferPhase {
    Verifying,
    Connecting,
    Downloading,
}

/// Locks a std mutex without ever failing or blocking for long: the guarded
/// values are tiny and never held across an `.await`.
fn lock<T>(m: &StdMutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn saturating_sub_atomic(value: &AtomicU64, amount: u64) {
    let mut current = value.load(Ordering::Relaxed);
    loop {
        let new = current.saturating_sub(amount);
        match value.compare_exchange_weak(current, new, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }
}

impl ProgressState {
    pub fn new(item_total: usize, bytes_total: u64) -> Arc<Self> {
        Arc::new(Self {
            item_total,
            bytes_total: AtomicU64::new(bytes_total),
            item_current: AtomicUsize::new(0),
            bytes_current: AtomicU64::new(0),
            current_item: StdMutex::new(None),
            current_stage: StdMutex::new(DownloadStage::Verifying),
            current_item_bytes: AtomicU64::new(0),
            current_item_total: AtomicU64::new(0),
            active: StdMutex::new(Default::default()),
        })
    }

    /// Corrige el total esperado del lote (positivo: crece, negativo: baja).
    pub fn adjust_bytes_total(&self, delta: i64) {
        if delta >= 0 {
            self.bytes_total.fetch_add(delta as u64, Ordering::Relaxed);
        } else {
            saturating_sub_atomic(&self.bytes_total, delta.unsigned_abs());
        }
    }

    fn start_item(&self, id: usize, name: String, stage: DownloadStage, reporter: &DownloadReporter) {
        lock(&self.active).insert(id, ActiveItem {
            name,
            download_stage: stage,
            phase: TransferPhase::Verifying,
            bytes: reporter.item_bytes.clone(),
            total: reporter.expected.clone(),
        });
    }

    pub(crate) fn finish_item(&self, id: usize) {
        lock(&self.active).remove(&id);
    }

    pub fn snapshot(&self) -> DownloadProgress {
        let active = lock(&self.active);
        // Keep showing a real transfer if other files are still checking or connecting.
        let item = active.values().rev().max_by_key(|item| item.phase);
        let current_item_total = item.map_or_else(|| self.current_item_total.load(Ordering::Relaxed), |i| i.total.load(Ordering::Relaxed));
        DownloadProgress {
            stage: item.map_or_else(|| lock(&self.current_stage).clone(), |i| match i.phase {
                TransferPhase::Verifying => DownloadStage::Verifying,
                TransferPhase::Connecting => DownloadStage::Connecting,
                TransferPhase::Downloading => i.download_stage.clone(),
            }),
            item_current: self.item_current.load(Ordering::Relaxed),
            item_total: self.item_total,
            bytes_current: self.bytes_current.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
            current_item: item.map_or_else(|| lock(&self.current_item).clone(), |i| Some(i.name.clone())),
            current_item_bytes: item.map_or_else(|| self.current_item_bytes.load(Ordering::Relaxed), |i| i.bytes.load(Ordering::Relaxed)),
            current_item_total: if current_item_total == 0 {
                None
            } else {
                Some(current_item_total)
            },
        }
    }
}

/// Per-item reporter attached to a shared `ProgressState`.
///
/// Invariant: for each item, `bytes_current` gets exactly the bytes of the
/// successful attempt (plus any cached bytes), and the item's share of
/// `bytes_total` equals its real size once the item completes.
#[derive(Clone, Debug)]
pub struct DownloadReporter {
    state: Arc<ProgressState>,
    active_id: Option<usize>,
    /// Bytes of the current attempt already counted in `bytes_current`.
    item_bytes: Arc<AtomicU64>,
    /// Size this item currently contributes to `bytes_total`.
    expected: Arc<AtomicU64>,
}

impl DownloadReporter {
    pub fn new(state: Arc<ProgressState>) -> Self {
        Self::for_item(state, None)
    }

    /// `declared` is the size the batch already added to `bytes_total` for this
    /// item (0 / `None` when the batch did not know it).
    pub fn for_item(state: Arc<ProgressState>, declared: Option<u64>) -> Self {
        Self {
            state,
            active_id: None,
            item_bytes: Arc::new(AtomicU64::new(0)),
            expected: Arc::new(AtomicU64::new(declared.unwrap_or(0))),
        }
    }

    pub(crate) fn track_active_item(mut self, id: usize, name: String, stage: DownloadStage) -> Self {
        self.active_id = Some(id);
        self.state.start_item(id, name, stage, &self);
        self
    }

    fn set_phase(&self, phase: TransferPhase) {
        if let Some(id) = self.active_id
            && let Some(item) = lock(&self.state.active).get_mut(&id)
        {
            item.phase = phase;
        }
    }

    fn set_expected(&self, new: u64) {
        let prev = self.expected.swap(new, Ordering::Relaxed);
        if prev != new {
            self.state.adjust_bytes_total(new as i64 - prev as i64);
        }
    }

    /// Call at the start of each attempt so a previous failed attempt's
    /// partial bytes are removed from the global counter.
    pub fn reset_attempt(&self) {
        let prev = self.item_bytes.swap(0, Ordering::Relaxed);
        if prev == 0 {
            return;
        }
        saturating_sub_atomic(&self.state.bytes_current, prev);
    }

    /// Report `delta` bytes successfully written for the current attempt.
    pub fn report_delta(&self, delta: u64) {
        if delta == 0 {
            return;
        }
        self.item_bytes.fetch_add(delta, Ordering::Relaxed);
        self.state
            .current_item_bytes
            .fetch_add(delta, Ordering::Relaxed);
        self.state.bytes_current.fetch_add(delta, Ordering::Relaxed);
    }

    /// The server reported the real size (Content-Length). Only corrects the
    /// expected total; it never counts bytes that have not been received.
    pub fn learn_total(&self, size: u64) {
        self.set_expected(size);
    }

    /// The item is complete: its expected size becomes the bytes it really
    /// received, so the batch total always matches what was downloaded.
    pub fn settle(&self) {
        let actual = self.item_bytes.load(Ordering::Relaxed);
        self.set_expected(actual);
    }

    /// The item will not complete (all sources failed): drop its partial bytes
    /// and its share of the total, so the batch percentage stays honest.
    pub fn abandon(&self) {
        self.reset_attempt();
        self.set_expected(0);
    }

    /// Use when the file already exists and is valid: all `size` bytes are done
    /// and no transfer happens.
    pub fn commit_known_size(&self, size: u64) {
        self.set_expected(size);
        let current = self.item_bytes.swap(size, Ordering::Relaxed);
        if size >= current {
            let delta = size - current;
            if delta > 0 {
                self.state
                    .current_item_bytes
                    .fetch_add(delta, Ordering::Relaxed);
                self.state.bytes_current.fetch_add(delta, Ordering::Relaxed);
            }
        } else {
            saturating_sub_atomic(&self.state.bytes_current, current - size);
        }
    }
}

impl crate::utilities::ProgressReporter for DownloadReporter {
    fn connecting(&self) {
        self.set_phase(TransferPhase::Connecting);
    }

    fn response_ready(&self) {
        self.set_phase(TransferPhase::Downloading);
    }

    fn reset_attempt(&self) {
        DownloadReporter::reset_attempt(self);
    }

    fn report_delta(&self, delta: u64) {
        DownloadReporter::report_delta(self, delta);
    }

    fn commit_known_size(&self, size: u64) {
        DownloadReporter::commit_known_size(self, size);
    }

    fn learn_total(&self, size: u64) {
        DownloadReporter::learn_total(self, size);
    }

    fn settle(&self) {
        DownloadReporter::settle(self);
    }
}

#[cfg(test)]
#[test]
fn finishing_a_parallel_transfer_keeps_the_active_file_visible() {
    let state = ProgressState::new(2, 30);
    let slow = DownloadReporter::for_item(state.clone(), Some(20))
        .track_active_item(0, "slow.jar".into(), DownloadStage::Library);
    let fast = DownloadReporter::for_item(state.clone(), Some(10))
        .track_active_item(1, "fast.bin".into(), DownloadStage::Asset);
    slow.set_phase(TransferPhase::Downloading);
    fast.set_phase(TransferPhase::Downloading);
    slow.report_delta(4);
    fast.report_delta(10);
    state.finish_item(1);
    slow.report_delta(2);
    let snapshot = state.snapshot();
    assert_eq!(snapshot.current_item.as_deref(), Some("slow.jar"));
    assert_eq!(snapshot.stage, DownloadStage::Library);
    assert_eq!(snapshot.current_item_bytes, 6);
    assert_eq!(snapshot.current_item_total, Some(20));
    assert_eq!(snapshot.bytes_current, 16);
}
