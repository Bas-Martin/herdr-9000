use std::io;
use std::path::PathBuf;

use crate::project::{reserve_project_ids, ProjectRegistry};

fn project_registry_path() -> PathBuf {
    crate::config::config_dir().join("projects.json")
}

pub(crate) fn load() -> io::Result<ProjectRegistry> {
    let path = project_registry_path();
    let registry = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<ProjectRegistry>(&content).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse project registry {}: {err}", path.display()),
            )
        })?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => ProjectRegistry::default(),
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("failed to read project registry {}: {err}", path.display()),
            ));
        }
    };
    reserve_project_ids(&registry);
    Ok(registry)
}

pub(crate) fn save(registry: &ProjectRegistry) -> std::io::Result<()> {
    super::io::save_json_to_path(&project_registry_path(), registry)
}
