use std::io;

use crate::automation::AutomationRegistry;

const FILE_NAME: &str = "automations.json";

fn automation_registry_path() -> std::path::PathBuf {
    crate::config::config_dir().join(FILE_NAME)
}

pub(crate) fn load() -> io::Result<AutomationRegistry> {
    let path = automation_registry_path();
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).map_err(io::Error::other),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(AutomationRegistry::default()),
        Err(err) => Err(err),
    }
}

pub(crate) fn save(registry: &AutomationRegistry) -> io::Result<()> {
    super::io::save_json_to_path(&automation_registry_path(), registry)
}
