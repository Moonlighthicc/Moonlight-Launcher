use crate::core::path_manager::PathManager;
use crate::core::{AppEvent, FsError, InstanceError, emit};
use crate::services::launcher::remove_log_ring;
use compact_str::ToCompactString;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use tokio::fs as tokio_fs;
use tokio::sync::RwLock;
use tokio::sync::oneshot;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use sysinfo::{Pid, Process, System};

use super::data::{InstOverrides, InstanceData, validate_instance_name};
use super::handle::InstanceHandle;
use super::status::InstanceStatus;

pub(crate) const SYNC_INTERVAL_SECS: u64 = 30;
// Longer than a valid instance name, and inside the root to stay on its filesystem.
pub(crate) const DELETION_DIR: &str = ".moonlauncher-pending-instance-deletions";

pub struct InstanceManager {
    instances: Arc<RwLock<HashMap<String, InstanceHandle>>>,
    /// Base de instancias. RwLock (parking_lot) porque la migración de
    /// almacenamiento la reapunta en caliente.
    instance_dir: parking_lot::RwLock<PathBuf>,
    _sync_handle: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    ready_tx: tokio::sync::watch::Sender<bool>,
}

static INSTANCE_MANAGER: OnceLock<Arc<InstanceManager>> = OnceLock::new();

struct KillSender {
    id: String,
    tx: oneshot::Sender<()>,
    requested: Arc<AtomicBool>,
}

static KILL_SENDERS: LazyLock<Mutex<HashMap<String, Vec<KillSender>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Minecraft processes that survived a launcher restart. These entries use the
/// operating-system PID, unlike supervised processes which use Launchwerk IDs.
static EXTERNAL_PROCESSES: LazyLock<Mutex<HashMap<String, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(test)]
pub fn register_kill_sender(uuid: &str, tx: oneshot::Sender<()>) -> Arc<AtomicBool> {
    register_kill_sender_with_id(uuid, uuid::Uuid::new_v4().to_string(), tx)
}

/// Registers one independent Java process for an instance.
pub fn register_kill_sender_with_id(uuid: &str, id: String, tx: oneshot::Sender<()>) -> Arc<AtomicBool> {
    let requested = Arc::new(AtomicBool::new(false));
    KILL_SENDERS.lock().unwrap_or_else(|e| e.into_inner())
        .entry(uuid.to_string()).or_default().push(KillSender { id, tx, requested: requested.clone() });
    emit_processes_changed(uuid);
    requested
}

/// Legacy cleanup: removes every process registered for an instance.
pub fn unregister_kill_sender(uuid: &str) {
    KILL_SENDERS.lock().unwrap_or_else(|e| e.into_inner()).remove(uuid);
}

pub fn unregister_kill_sender_process(uuid: &str, id: &str) {
    let mut all = KILL_SENDERS.lock().unwrap_or_else(|e| e.into_inner());
    let empty = if let Some(processes) = all.get_mut(uuid) {
        processes.retain(|process| process.id != id);
        processes.is_empty()
    } else { false };
    if empty { all.remove(uuid); }
    drop(all);
    emit_processes_changed(uuid);
}

fn emit_processes_changed(uuid: &str) {
    emit(AppEvent::InstanceProcessesChanged {
        id: uuid.to_compact_string(),
        process_ids: running_process_ids(uuid),
    });
}

pub fn running_process_ids(uuid: &str) -> Vec<String> {
    let mut ids = KILL_SENDERS.lock().unwrap_or_else(|e| e.into_inner())
        .get(uuid).map(|items| items.iter().map(|item| item.id.clone()).collect::<Vec<_>>()).unwrap_or_default();
    if let Some(external) = EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner()).get(uuid) {
        ids.extend(external.iter().cloned());
    }
    ids
}

pub fn has_running_processes(uuid: &str) -> bool {
    KILL_SENDERS.lock().unwrap_or_else(|e| e.into_inner())
        .get(uuid).is_some_and(|items| !items.is_empty())
        || EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner())
            .get(uuid).is_some_and(|items| !items.is_empty())
}

fn remove_external_process(uuid: &str, id: &str) {
    let mut all = EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner());
    let empty = if let Some(processes) = all.get_mut(uuid) {
        processes.retain(|process_id| process_id != id);
        processes.is_empty()
    } else { false };
    if empty { all.remove(uuid); }
}

fn kill_external_process(uuid: &str, id: &str) -> bool {
    let registered = EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner())
        .get(uuid).is_some_and(|ids| ids.iter().any(|process_id| process_id == id));
    if !registered { return false; }
    let Ok(raw_pid) = id.parse::<u32>() else { return false; };
    let mut system = System::new();
    system.refresh_processes();
    let Some(process) = system.process(Pid::from_u32(raw_pid)) else {
        remove_external_process(uuid, id);
        return false;
    };
    // Keep the PID registered until the monitor observes the actual exit. This
    // prevents the UI from briefly losing the running state after TerminateProcess.
    process.kill()
}

/// Sends a signal to the most recently registered process.
pub fn signal_kill(uuid: &str) -> bool {
    let sender = {
        let mut all = KILL_SENDERS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(processes) = all.get_mut(uuid) else {
            drop(all);
            let id = EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner())
                .get(uuid).and_then(|items| items.last()).cloned();
            return id.is_some_and(|id| kill_external_process(uuid, &id));
        };
        let Some(sender) = processes.pop() else { return false; };
        if processes.is_empty() { all.remove(uuid); }
        sender
    };
    sender.requested.store(true, Ordering::Release);
    let sent = sender.tx.send(()).is_ok();
    emit_processes_changed(uuid);
    sent
}

pub fn signal_kill_process(uuid: &str, id: &str) -> bool {
    let sender = {
        let mut all = KILL_SENDERS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(processes) = all.get_mut(uuid) else {
            drop(all);
            return kill_external_process(uuid, id);
        };
        let Some(index) = processes.iter().position(|process| process.id == id) else {
            drop(all);
            return kill_external_process(uuid, id);
        };
        let sender = processes.remove(index);
        if processes.is_empty() { all.remove(uuid); }
        sender
    };
    sender.requested.store(true, Ordering::Release);
    let sent = sender.tx.send(()).is_ok();
    emit_processes_changed(uuid);
    sent
}

fn is_minecraft_process(process: &Process) -> bool {
    let executable = process.exe().and_then(|path| path.file_stem()).and_then(|name| name.to_str());
    let name = process.name();
    let is_java = executable.is_some_and(|value| value.eq_ignore_ascii_case("java") || value.eq_ignore_ascii_case("javaw"))
        || name.eq_ignore_ascii_case("java.exe")
        || name.eq_ignore_ascii_case("javaw.exe")
        || name.eq_ignore_ascii_case("java")
        || name.eq_ignore_ascii_case("javaw");
    if !is_java { return false; }
    let args = process.cmd();
    args.iter().any(|arg| arg == "net.minecraft.client.main.Main"
        || arg == "cpw.mods.bootstraplauncher.BootstrapLauncher"
        || arg == "net.fabricmc.loader.impl.launch.knot.KnotClient"
        || arg == "org.quiltmc.loader.impl.launch.knot.KnotClient")
        || args.iter().any(|arg| arg == "--gameDir")
}

/// Minecraft PIDs (Java) that currently work inside the instance directory.
fn minecraft_pids_in(dir: &Path) -> HashSet<u32> {
    let mut system = System::new();
    system.refresh_processes();
    system
        .processes()
        .iter()
        .filter(|(_, process)| is_minecraft_process(process) && process_belongs_to_instance(process, dir))
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

/// True when one of the processes owns a visible top-level window of a real size.
#[cfg(windows)]
fn has_visible_window(pids: &HashSet<u32>) -> bool {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct Search<'a> {
        pids: &'a HashSet<u32>,
        found: bool,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if search.pids.contains(&pid) && unsafe { IsWindowVisible(hwnd) } != 0 {
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            if unsafe { GetWindowRect(hwnd, &mut rect) } != 0
                && rect.right - rect.left > 100
                && rect.bottom - rect.top > 100
            {
                search.found = true;
                return 0;
            }
        }
        1
    }

    let mut search = Search { pids, found: false };
    unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM) };
    search.found
}

/// Emits `InstanceGameReady` as soon as the new game process shows its window (Windows).
/// The log marker used elsewhere appears only after all mods are loaded, which for big
/// modpacks is long after the window (or the loading screen) is already visible.
/// Call it before spawning Java so already running windows are not mistaken for the new one.
pub async fn spawn_window_watcher(uuid: String, dir: PathBuf, stop: Arc<AtomicBool>) {
    #[cfg(windows)]
    {
        // The baseline must be taken before Java starts, hence awaited here.
        let baseline_dir = dir.clone();
        let known = tokio::task::spawn_blocking(move || minecraft_pids_in(&baseline_dir))
            .await
            .unwrap_or_default();
        tokio::spawn(async move {
            for _ in 0..1000 {
                if stop.load(Ordering::Acquire) {
                    return;
                }
                let (scan_dir, known) = (dir.clone(), known.clone());
                let found = tokio::task::spawn_blocking(move || {
                    let fresh: HashSet<u32> = minecraft_pids_in(&scan_dir)
                        .difference(&known)
                        .copied()
                        .collect();
                    !fresh.is_empty() && has_visible_window(&fresh)
                })
                .await
                .unwrap_or(false);
                if found {
                    emit(AppEvent::InstanceGameReady { id: uuid.to_compact_string() });
                    return;
                }
                time::sleep(Duration::from_millis(300)).await;
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (uuid, dir, stop);
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    let left = std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
    let right = std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
    let left = left.to_string_lossy();
    let right = right.to_string_lossy();
    #[cfg(windows)]
    { left.eq_ignore_ascii_case(&right) }
    #[cfg(not(windows))]
    { left == right }
}

fn process_belongs_to_instance(process: &Process, instance_path: &Path) -> bool {
    let args = process.cmd();
    if let Some(index) = args.iter().position(|arg| arg == "--gameDir") {
        if let Some(value) = args.get(index + 1) {
            return paths_equal(Path::new(value), instance_path);
        }
    }
    process.cwd().is_some_and(|cwd| paths_equal(cwd, instance_path))
}

impl InstanceManager {
    /// Publish an empty manager synchronously so frontend commands cannot race
    /// the startup task and panic on `InstanceManager::get()`.
    pub fn start() -> Arc<Self> {
        let (ready_tx, _ready_rx) = tokio::sync::watch::channel(false);
        let manager = Arc::new(Self {
            instances: Arc::new(RwLock::new(HashMap::new())),
            instance_dir: parking_lot::RwLock::new(
                PathManager::get().get_instance_dir().to_path_buf(),
            ),
            _sync_handle: parking_lot::Mutex::new(None),
            ready_tx,
        });
        let _ = INSTANCE_MANAGER.set(manager.clone());
        manager
    }

    /// Load instance records after the manager has already been published.
    /// Public commands wait for `ready`, preventing an early empty list from
    /// being mistaken for the user's actual instance list.
    pub async fn load_instances(&self) {
        if self._sync_handle.lock().is_none() {
            let handle = tokio::spawn(Self::sync_task());
            *self._sync_handle.lock() = Some(handle);
        }

        let base_dir = PathManager::get().get_instance_dir().to_path_buf();
        cleanup_deleted_instances(&trash_dir(&base_dir)).await;
        let names = if let Ok(mut dir) = tokio::fs::read_dir(&base_dir).await {
            let mut names = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.file_name() != DELETION_DIR && entry.path().is_dir() {
                    names.push(entry.file_name().to_string_lossy().to_string());
                }
            }
            names
        } else {
            Vec::new()
        };

        let handles: Vec<Option<InstanceHandle>> =
            futures::future::join_all(names.iter().map(|name| InstanceHandle::load(name))).await;

        let mut guard = self.instances.write().await;
        for handle in handles.into_iter().flatten() {
            guard.insert(handle.uuid.to_string(), handle);
        }
        drop(guard);

        // `start()` drops the initial watch receiver intentionally. `send()` does
        // not retain the new value when there are no receivers, so commands that
        // subscribe after loading would wait forever and the instance list would
        // remain stuck. `send_replace()` always stores the ready state.
        self.ready_tx.send_replace(true);
        let migration_manager = InstanceManager::get().clone();
        tokio::spawn(async move {
            migrate_instance_icons(migration_manager).await;
        });

        // Reattach to Minecraft processes that outlived the previous launcher
        // process. This runs only after instance loading is complete and on a
        // blocking worker, so process enumeration cannot stall the UI startup.
        let process_manager = InstanceManager::get().clone();
        tokio::spawn(async move {
            process_manager.reattach_running_minecraft().await;
            process_manager.monitor_reattached_processes().await;
        });
    }

    /// Convenience initializer for tests and non-window callers.
    pub async fn init() -> Arc<Self> {
        let manager = Self::start();
        manager.load_instances().await;
        manager
    }

    async fn reattach_running_minecraft(&self) {
        let handles = self.get_all_handles().await;
        let mut targets = Vec::with_capacity(handles.len());
        for handle in &handles {
            targets.push((handle.uuid.to_string(), handle.get_instance_dir().await));
        }

        let discovered = match tokio::task::spawn_blocking(move || {
            let mut system = System::new_all();
            system.refresh_processes();
            let mut found: Vec<(String, String)> = Vec::new();
            for (pid, process) in system.processes() {
                if !is_minecraft_process(process) { continue; }
                for (uuid, instance_path) in &targets {
                    if process_belongs_to_instance(process, instance_path) {
                        found.push((uuid.clone(), pid.as_u32().to_string()));
                        break;
                    }
                }
            }
            found
        }).await {
            Ok(found) => found,
            Err(error) => {
                warn!("Could not scan existing Minecraft processes: {error}");
                return;
            }
        };

        {
            let mut external = EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner());
            external.clear();
            for (uuid, pid) in discovered {
                let processes = external.entry(uuid).or_default();
                if !processes.contains(&pid) { processes.push(pid); }
            }
        }

        for handle in handles {
            if has_running_processes(&handle.uuid) {
                // Start a new local accounting segment; the time before this
                // launcher was opened cannot be reconstructed reliably.
                handle.begin_play_session();
                handle.set_status(InstanceStatus::Started);
            }
        }
    }

    async fn monitor_reattached_processes(&self) {
        let mut interval = time::interval(Duration::from_secs(3));
        interval.tick().await;
        loop {
            interval.tick().await;
            let known: Vec<(String, String)> = {
                let external = EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner());
                external.iter().flat_map(|(uuid, ids)| ids.iter().map(|id| (uuid.clone(), id.clone()))).collect()
            };
            if known.is_empty() { continue; }

            let handles = self.get_all_handles().await;
            let mut instance_paths = HashMap::with_capacity(handles.len());
            for handle in &handles {
                instance_paths.insert(handle.uuid.to_string(), handle.get_instance_dir().await);
            }
            let live = match tokio::task::spawn_blocking(move || {
                let mut system = System::new_all();
                system.refresh_processes();
                known.into_iter().filter(|(uuid, id)| {
                    let Some(path) = instance_paths.get(uuid) else { return false; };
                    id.parse::<u32>().ok()
                        .and_then(|pid| system.process(Pid::from_u32(pid)))
                        .is_some_and(|process| is_minecraft_process(process) && process_belongs_to_instance(process, path))
                }).collect::<Vec<_>>()
            }).await {
                Ok(live) => live,
                Err(error) => {
                    warn!("Could not refresh reattached Minecraft processes: {error}");
                    continue;
                }
            };
            let live_set: HashSet<(String, String)> = live.into_iter().collect();
            let changed_instances = {
                let mut external = EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner());
                let before: HashMap<String, Vec<String>> = external.clone();
                external.retain(|uuid, ids| {
                    ids.retain(|id| live_set.contains(&(uuid.clone(), id.clone())));
                    !ids.is_empty()
                });
                before.into_iter()
                    .filter_map(|(uuid, old_ids)| {
                        let new_ids = external.get(&uuid).cloned().unwrap_or_default();
                        (old_ids != new_ids).then_some(uuid)
                    })
                    .collect::<Vec<_>>()
            };
            for uuid in changed_instances {
                emit_processes_changed(&uuid);
            }
            for handle in handles {
                let running = has_running_processes(&handle.uuid);
                match (running, handle.get_status()) {
                    (true, InstanceStatus::Off) => {
                        handle.begin_play_session();
                        handle.set_status(InstanceStatus::Started);
                    }
                    (false, InstanceStatus::Started) => {
                        handle.end_play_session().await;
                        handle.set_status(InstanceStatus::Off);
                    }
                    _ => {}
                }
            }
        }
    }

    async fn wait_ready(&self) {
        let mut ready = self.ready_tx.subscribe();
        if *ready.borrow() { return; }
        while ready.changed().await.is_ok() {
            if *ready.borrow() { return; }
        }
    }

    pub fn get() -> &'static Arc<InstanceManager> {
        INSTANCE_MANAGER
            .get()
            .expect("BUG: InstanceManager used before initialization")
    }

    /// Directorio base actual de las instancias.
    pub(crate) fn base_dir(&self) -> PathBuf {
        self.instance_dir.read().clone()
    }

    /// Reapunta el directorio base tras una migración de almacenamiento
    /// completada. Las instancias existentes actualizan su raíz por su cuenta.
    pub(crate) fn update_base_dir(&self, dir: PathBuf) {
        *self.instance_dir.write() = dir;
    }

    async fn sync_task() {
        let mut interval = time::interval(Duration::from_secs(SYNC_INTERVAL_SECS));
        interval.tick().await;
        loop {
            interval.tick().await;
            info!("Running sync task");

            let manager = match INSTANCE_MANAGER.get() {
                Some(m) => m.clone(),
                None => continue,
            };

            manager.wait_ready().await;
            let handles: Vec<InstanceHandle> =
                { manager.instances.read().await.values().cloned().collect() };

            for handle in handles {
                // Skip active operations instead of blocking every other autosave.
                let Ok(files_guard) = handle.try_lock_files() else {
                    continue;
                };
                if let Err(e) = files_guard.save_if_dirty().await {
                    error!("Failed to save instance {}: {:?}", handle.uuid, e);
                }
            }
            cleanup_deleted_instances(&trash_dir(&manager.base_dir())).await;
        }
    }

    pub async fn create_instance(
        &self,
        name: String,
        version: String,
        icon: Option<String>,
    ) -> Result<InstanceHandle, InstanceError> {
        self.wait_ready().await;
        validate_instance_name(&name).map_err(InstanceError::InstNameParse)?;
        zellkern::path_security::validate_version(&version)
            .map_err(|e| InstanceError::VersionNotFound(e.to_string()))?;

        let mut data = InstanceData::new(name, version, icon);
        // Reserve the name while publishing the new instance, including against renames.
        let mut instances = self.instances.write().await;
        data.instance_root = self.base_dir();
        for existing in instances.values() {
            if existing.get_name().await == data.name {
                return Err(InstanceError::AlreadyExists);
            }
        }
        tokio_fs::create_dir(data.get_instance_dir())
            .await
            .map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    InstanceError::AlreadyExists
                } else {
                    InstanceError::Fs(FsError::CreateDir {
                        path: data.get_instance_dir().to_string_lossy().to_string(),
                        source: e,
                    })
                }
            })?;
        data.save().await.map_err(|e| {
            InstanceError::Fs(FsError::WriteFile {
                path: data
                    .get_instance_dir()
                    .join("instance.cub")
                    .to_string_lossy()
                    .to_string(),
                source: e,
            })
        })?;

        let handle = InstanceHandle::new(data);
        instances.insert(handle.uuid.to_string(), handle.clone());

        Ok(handle)
    }

    pub async fn get_handle(&self, uuid: &str) -> Option<InstanceHandle> {
        self.wait_ready().await;
        self.instances.read().await.get(uuid).cloned()
    }

    pub async fn get_all_handles(&self) -> Vec<InstanceHandle> {
        self.wait_ready().await;
        self.instances.read().await.values().cloned().collect()
    }

    pub async fn count(&self) -> usize {
        self.wait_ready().await;
        self.instances.read().await.len()
    }

    pub(crate) async fn lock_instance_list(&self) -> tokio::sync::RwLockReadGuard<'_, HashMap<String, InstanceHandle>> {
        self.wait_ready().await;
        self.instances.read().await
    }

    pub async fn get_all_dtos(&self) -> Vec<super::data::InstanceDto> {
        let handles = self.get_all_handles().await;
        let mut dtos = futures::future::join_all(handles.iter().map(|h| h.to_dto())).await;
        dtos.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| a.name.cmp(&b.name)));
        dtos
    }

    pub async fn get_running_ids(&self) -> Vec<String> {
        self.wait_ready().await;
        self.instances
            .read()
            .await
            .values()
            .filter(|h| h.is_busy())
            .map(|h| h.uuid.to_string())
            .collect()
    }

    pub async fn delete_instance(&self, uuid: &str) -> Result<(), String> {
        let handle = self
            .get_handle(uuid)
            .await
            .ok_or_else(|| "Instance not found".to_string())?;

        // Deleting always wins: close the game and anything else using the folder first.
        terminate_instance_processes(&handle, &handle.get_instance_dir().await).await;
        let files_guard = handle.try_lock_files()?;
        let instances = self.instances.clone();
        let trash = trash_dir(&self.base_dir());
        // Finish the transaction even if the IPC caller is cancelled after the rename.
        tokio::spawn(async move {
            let uuid = handle.uuid.as_ref();
            let mut instances = instances.write().await;
            let dir = handle.get_instance_dir().await;
            quarantine_with_retry(&dir, &trash, uuid)
                .await
                .map_err(|e| format!("Failed to remove the directory: {e}"))?;
            files_guard.mark_deleted();
            instances.remove(uuid);
            drop(instances);
            for process_id in running_process_ids(uuid) {
                signal_kill_process(uuid, &process_id);
            }
            unregister_kill_sender(uuid);
            remove_log_ring(uuid);
            emit(AppEvent::InstanceDeleted {
                id: uuid.to_compact_string(),
            });
            drop(files_guard);
            // Failure here leaves only a quarantined directory, retried at startup/sync.
            cleanup_deleted_instances(&trash).await;
            Ok(())
        })
        .await
        .map_err(|e| format!("Error in the deletion task: {e}"))?
    }

    pub(crate) async fn change_root_and_delete_instances(
        &self,
        new_dir: PathBuf,
        reset: bool,
    ) -> Result<crate::services::storage_migration::MoveResult, String> {
        self.wait_ready().await;
        let mut instances = self.instances.write().await;
        let mut guards = Vec::with_capacity(instances.len());
        let mut entries = Vec::with_capacity(instances.len());
        let old_root = self.base_dir();
        for handle in instances.values() {
            let guard = handle.try_lock_files()?;
            if handle.is_busy() {
                return Err(format!("Close '{}' before changing the instances folder", handle.get_name().await));
            }
            let name = handle.get_name().await;
            validate_instance_name(&name)?;
            entries.push((handle.uuid.to_string(), name.to_string()));
            guards.push(guard);
        }
        let root = zellkern::path_security::ConfinedDir::open(&old_root).map_err(|e| e.to_string())?;
        // Do not touch the old instances until the new location is committed.
        crate::services::storage_migration::save_instances_location(&new_dir, reset).await?;
        PathManager::set_instances_dir(new_dir.clone());
        self.update_base_dir(new_dir);
        for guard in &guards {
            guard.mark_deleted();
        }
        instances.clear();
        drop(instances);
        for (id, _) in &entries {
            unregister_kill_sender(id);
            remove_log_ring(id);
            emit(AppEvent::InstanceDeleted { id: id.to_compact_string() });
        }
        drop(guards);
        let failed = tokio::task::spawn_blocking(move || {
            entries.into_iter().filter_map(|(_, name)| {
                root.remove_dir_all(Path::new(&name)).err()
                    .filter(|e| e.kind() != io::ErrorKind::NotFound)
                    .map(|e| format!("{}: {e}", old_root.join(name).display()))
            }).collect()
        }).await.unwrap_or_else(|e| vec![e.to_string()]);
        Ok(crate::services::storage_migration::MoveResult { moved: 0, failed })
    }

    pub async fn update_instance(
        &self,
        uuid: &str,
        new_name: Option<String>,
        new_version: Option<String>,
        new_icon: Option<Option<String>>,
        new_overrides: Option<InstOverrides>,
    ) -> Result<(), String> {
        if let Some(version) = &new_version {
            zellkern::path_security::validate_version(version).map_err(|e| e.to_string())?;
        }
        let handle = self
            .get_handle(uuid)
            .await
            .ok_or_else(|| "Instance not found".to_string())?;

        let files_guard = handle.try_lock_files()?;

        if handle.is_busy() {
            return Err(
                "An instance cannot be modified while it is running".to_string(),
            );
        }

        if let Some(version) = &new_version
            && handle.get_version().await.as_ref() != version
            && let Some(pack) =
                crate::services::modpack::read_state(&handle.get_instance_dir().await).await?
            && pack.locked
        {
            return Err(format!(
                "Minecraft and the loader are managed by {}. Unlock the modpack to change them.",
                pack.name
            ));
        }

        if let Some(name) = new_name {
            validate_instance_name(&name)?;
            let instances = self.instances.write().await;

            let old_name = handle.get_name().await;
            if *old_name != name {
                for existing in instances.values() {
                    if existing.get_name().await.as_ref() == name {
                        return Err("An instance with that name already exists".to_string());
                    }
                }
                let base_dir = self.base_dir();
                let old_dir = base_dir.join(&*old_name);
                let new_dir = base_dir.join(&name);

                if new_dir.exists() {
                    return Err("An instance with that name already exists".to_string());
                }
                tokio_fs::rename(&old_dir, &new_dir)
                    .await
                    .map_err(|e| format!("Failed to rename the directory: {}", e))?;
                handle.set_name(name).await;
            }
        }

        if let Some(version) = new_version {
            handle.set_version(version).await;
        }

        if let Some(icon) = new_icon {
            handle.set_icon(icon).await;
        }

        handle.set_overrides(new_overrides).await;

        files_guard
            .save_if_dirty()
            .await
            .map_err(|e| format!("Failed to save the instance: {}", e))?;

        Ok(())
    }
}

/// Reescala en background los iconos que quedaron a resolución completa en
/// versiones anteriores. No bloquea el arranque y cada icono conserva su
/// archivo anterior si la migración falla.
async fn migrate_instance_icons(manager: Arc<InstanceManager>) {
    let handles = manager.get_all_handles().await;
    if handles.is_empty() {
        return;
    }

    let semaphore = Arc::new(tokio::sync::Semaphore::new(3));
    let mut tasks = Vec::with_capacity(handles.len());
    for handle in handles {
        let semaphore = semaphore.clone();
        tasks.push(tokio::spawn(async move {
            let Ok(_permit) = semaphore.acquire().await else {
                return;
            };
            // El resultado no importa: la migración es best-effort.
            let _ = handle.migrate_icon_if_needed().await;
        }));
    }
    for task in tasks {
        let _ = task.await;
    }
    info!("Instance icon migration completed");
}

fn trash_dir(instances: &Path) -> PathBuf {
    instances.join(DELETION_DIR)
}

fn normalized_path(path: &Path) -> PathBuf {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    { PathBuf::from(canonical.to_string_lossy().trim_start_matches(r"\\?\").to_lowercase()) }
    #[cfg(not(windows))]
    { canonical }
}

fn path_inside(path: &Path, root: &Path) -> bool {
    normalized_path(path).starts_with(root)
}

/// Any process (Java, a helper, a shell, an editor...) that works inside the instance
/// directory and may keep it locked.
fn process_uses_dir(process: &Process, root: &Path, root_text: &str) -> bool {
    if process.pid().as_u32() == std::process::id() { return false; }
    if process.cwd().is_some_and(|cwd| path_inside(cwd, root)) { return true; }
    if process.exe().is_some_and(|exe| path_inside(exe, root)) { return true; }
    let args = process.cmd();
    if let Some(index) = args.iter().position(|arg| arg == "--gameDir")
        && let Some(value) = args.get(index + 1)
        && path_inside(Path::new(value), root)
    {
        return true;
    }
    let (a, b) = (format!("{root_text}\\"), format!("{root_text}/"));
    args.iter().any(|arg| {
        #[cfg(windows)]
        let arg = arg.to_lowercase();
        #[cfg(not(windows))]
        let arg = arg.to_string();
        arg.contains(&a) || arg.contains(&b)
    })
}

/// Force-kills every process that uses the instance directory. Returns how many are still alive.
async fn kill_processes_in_dir(dir: &Path) -> usize {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let root = normalized_path(&dir);
        let root_text = root.to_string_lossy().to_string();
        let mut system = System::new();
        system.refresh_processes();
        let mut alive = 0;
        for process in system.processes().values() {
            if process_uses_dir(process, &root, &root_text) {
                warn!("Force-killing process {} ({}) holding an instance directory", process.pid(), process.name());
                process.kill();
                alive += 1;
            }
        }
        alive
    })
    .await
    .unwrap_or(0)
}

/// Stops every process of an instance, force-kills anything that holds its files,
/// and waits (bounded) for the system to release them.
async fn terminate_instance_processes(handle: &InstanceHandle, dir: &Path) {
    let uuid = handle.uuid.to_string();
    for process_id in running_process_ids(&uuid) {
        signal_kill_process(&uuid, &process_id);
    }
    kill_processes_in_dir(dir).await;
    for _ in 0..50 {
        let busy = handle.is_busy() || has_running_processes(&uuid);
        if !busy && kill_processes_in_dir(dir).await == 0 { break; }
        time::sleep(Duration::from_millis(150)).await;
    }
    EXTERNAL_PROCESSES.lock().unwrap_or_else(|e| e.into_inner()).remove(&uuid);
    if handle.is_busy() {
        handle.end_play_session().await;
        handle.set_status(InstanceStatus::Off);
    }
}

fn is_lock_error(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
        || matches!(error.raw_os_error(), Some(5 | 16 | 32 | 33))
}

/// Windows (antivirus, indexer, Explorer) can hold a folder for a moment; retry and re-kill.
async fn quarantine_with_retry(dir: &Path, trash: &Path, uuid: &str) -> io::Result<()> {
    let mut attempt = 0u64;
    loop {
        match quarantine_instance(dir, trash, uuid).await {
            Err(e) if is_lock_error(&e) && attempt < 8 => {
                attempt += 1;
                warn!("Instance directory is locked ({e}), retry {attempt}");
                kill_processes_in_dir(dir).await;
                time::sleep(Duration::from_millis(250 * attempt)).await;
            }
            other => return other,
        }
    }
}

async fn quarantine_instance(dir: &Path, trash: &Path, uuid: &str) -> io::Result<()> {
    uuid::Uuid::parse_str(uuid).map_err(io::Error::other)?;
    let root = trash
        .parent()
        .ok_or_else(|| io::Error::other("Invalid trash root"))?;
    if dir.parent() != Some(root) {
        return Err(io::Error::other("Instance is outside its storage root"));
    }
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::other("Invalid instance directory"))?;
    validate_instance_name(name).map_err(io::Error::other)?;
    match tokio_fs::symlink_metadata(dir).await {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    let confined = zellkern::path_security::ConfinedDir::open(root)?;
    confined.create_dir_all(Path::new(DELETION_DIR))?;
    confined.rename(Path::new(name), &Path::new(DELETION_DIR).join(uuid))
}

async fn cleanup_deleted_instances(trash: &Path) {
    let trash = trash.to_path_buf();
    let result = tokio::task::spawn_blocking(move || -> io::Result<()> {
        let root = trash
            .parent()
            .ok_or_else(|| io::Error::other("Invalid trash root"))?;
        let directory =
            zellkern::path_security::ConfinedDir::open(root)?.open_dir(Path::new(DELETION_DIR))?;
        for name in directory.entries()? {
            if uuid::Uuid::parse_str(&name.to_string_lossy()).is_err() {
                continue;
            }
            if let Err(e) = directory.remove_dir_all(Path::new(&name))
                && e.kind() != io::ErrorKind::NotFound
            {
                error!("Could not clean up deleted instance {name:?}: {e}");
            }
        }
        Ok(())
    })
    .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) if e.kind() == io::ErrorKind::NotFound => {}
        other => error!("Failed to clean up instances pending deletion: {other:?}"),
    }
}

#[cfg(test)]
#[path = "../../tests/services/instance_manager/manager.rs"]
mod tests;
