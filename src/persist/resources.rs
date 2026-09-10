use std::io::{self, Write as _};

use crate::resource::ResourceRegistry;

const FILE_NAME: &str = "resources.json";

fn resource_registry_path() -> std::path::PathBuf {
    crate::config::config_dir().join(FILE_NAME)
}

pub(crate) fn load() -> io::Result<ResourceRegistry> {
    let path = resource_registry_path();
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).map_err(io::Error::other),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(ResourceRegistry::default()),
        Err(err) => Err(err),
    }
}

pub(crate) fn save(registry: &ResourceRegistry) -> io::Result<()> {
    let path = resource_registry_path();
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("resource registry path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".resources-{}.tmp", std::process::id()));
    let json = serde_json::to_string_pretty(registry)?;
    let mut file = crate::platform::create_private_state_file(&temp_path)?;
    if let Err(error) = file
        .write_all(json.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }
    drop(file);
    if let Err(error) = crate::platform::replace_file(&temp_path, &path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error);
    }
    crate::platform::sync_parent_directory(parent)
}
