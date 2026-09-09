//! Session persistence — save/restore workspaces, layouts, and working directories.
//!
//! Stored at `~/.config/herdr/session.json`.
//! Optional pane screen history is stored separately at `session-history.json`.
//! Installed plugins are persisted separately at `plugins.json`.

mod automations;
mod io;
pub mod plugin_registry;
mod projects;
mod resources;
mod restore;
mod snapshot;
mod tasks;

pub use self::io::{clear, clear_history, load, load_history, save};

pub(crate) use self::automations::{load as load_automations, save as save_automations};
pub(crate) use self::projects::{load as load_projects, save as save_projects};
pub(crate) use self::resources::{load as load_resources, save as save_resources};
pub use self::restore::restore;
#[cfg(unix)]
pub use self::restore::{handoff_pane_aliases, restore_handoff};
pub use self::snapshot::{
    capture, capture_history, DirectionSnapshot, LayoutSnapshot, SessionHistorySnapshot,
    SessionSnapshot, TabSnapshot, WorkspaceSnapshot,
};
pub(crate) use self::tasks::{load as load_tasks, save as save_tasks};
