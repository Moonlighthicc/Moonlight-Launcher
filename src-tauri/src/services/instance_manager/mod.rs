mod manager;
mod status;

pub(crate) mod data;
pub(crate) mod handle;

pub use data::{InstOverrides, InstanceDto};
pub use handle::InstanceHandle;
pub(crate) use manager::DELETION_DIR;
pub use manager::{InstanceManager, register_kill_sender_with_id, running_process_ids, has_running_processes, signal_kill, signal_kill_process, spawn_window_watcher, unregister_kill_sender_process};
#[cfg(test)]
pub use manager::{register_kill_sender, unregister_kill_sender};
pub use status::InstanceStatus;
