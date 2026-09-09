use std::io;

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
    super::io::save_json_to_path(&resource_registry_path(), registry)
}
