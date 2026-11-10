use std::io;
use std::path::PathBuf;

use crate::task::{reserve_task_ids, TaskRegistry};

fn task_registry_path() -> PathBuf {
    crate::config::config_dir().join("tasks.json")
}

pub(crate) fn load() -> io::Result<TaskRegistry> {
    let path = task_registry_path();
    let registry = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<TaskRegistry>(&content).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse task registry {}: {err}", path.display()),
            )
        })?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => TaskRegistry::default(),
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("failed to read task registry {}: {err}", path.display()),
            ));
        }
    };
    reserve_task_ids(&registry);
    Ok(registry)
}

pub(crate) fn save(registry: &TaskRegistry) -> std::io::Result<()> {
    super::io::save_json_to_path(&task_registry_path(), registry)
}
