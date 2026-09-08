use std::path::PathBuf;

use crate::project::{reserve_project_ids, ProjectRegistry};

fn project_registry_path() -> PathBuf {
    crate::config::config_dir().join("projects.json")
}

pub(crate) fn load() -> ProjectRegistry {
    let path = project_registry_path();
    let registry = match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<ProjectRegistry>(&content) {
            Ok(registry) => registry,
            Err(err) => {
                tracing::warn!(path = %path.display(), error = %err, "failed to parse project registry");
                ProjectRegistry::default()
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ProjectRegistry::default(),
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "failed to read project registry");
            ProjectRegistry::default()
        }
    };
    reserve_project_ids(&registry);
    registry
}

pub(crate) fn save(registry: &ProjectRegistry) -> std::io::Result<()> {
    super::io::save_json_to_path(&project_registry_path(), registry)
}
