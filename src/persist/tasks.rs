use std::path::PathBuf;

use crate::task::{reserve_task_ids, TaskRegistry};

fn task_registry_path() -> PathBuf {
    crate::config::config_dir().join("tasks.json")
}

pub(crate) fn load() -> TaskRegistry {
    let path = task_registry_path();
    let registry = match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<TaskRegistry>(&content) {
            Ok(registry) => registry,
            Err(err) => {
                tracing::warn!(path = %path.display(), error = %err, "failed to parse task registry");
                TaskRegistry::default()
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => TaskRegistry::default(),
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "failed to read task registry");
            TaskRegistry::default()
        }
    };
    reserve_task_ids(&registry);
    registry
}

pub(crate) fn save(registry: &TaskRegistry) -> std::io::Result<()> {
    super::io::save_json_to_path(&task_registry_path(), registry)
}
