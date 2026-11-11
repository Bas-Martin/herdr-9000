use std::path::{Path, PathBuf};

use crate::config::OpenWithConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Application {
    VsCode,
    VisualStudio,
    Explorer,
}

impl Application {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::VsCode => "Visual Studio Code",
            Self::VisualStudio => "Visual Studio",
            Self::Explorer => "Windows Explorer",
        }
    }
}

fn cache_path() -> PathBuf {
    crate::config::state_dir().join("open-with.toml")
}

/// Installation and update refresh only this cache, never the user's configuration.
pub(crate) fn refresh_detection() -> Result<(), String> {
    let mut document = toml_edit::DocumentMut::new();
    for (application, key) in [
        (Application::VsCode, "vscode"),
        (Application::VisualStudio, "visual_studio"),
    ] {
        if let Some(path) = crate::platform::open_with::detect(application) {
            document[key] = toml_edit::value(path.to_string_lossy().as_ref());
        }
    }
    let path = cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create Open With detection cache: {error}"))?;
    }
    std::fs::write(path, document.to_string())
        .map_err(|error| format!("Cannot save Open With detection cache: {error}"))
}

pub(crate) fn launch(
    application: Application,
    directory: &Path,
    config: &OpenWithConfig,
) -> Result<(), String> {
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(format!(
            "Cannot open directory '{}': expected an existing absolute directory",
            directory.display()
        ));
    }
    crate::platform::open_with::ensure_supported(application)?;
    if application == Application::Explorer {
        return crate::platform::open_with::launch(application, None, directory);
    }
    let configured = match application {
        Application::VsCode => config.vscode.as_deref(),
        Application::VisualStudio => config.visual_studio.as_deref(),
        Application::Explorer => None,
    }
    .filter(|value| !value.trim().is_empty());
    let executable = if let Some(configured) = configured {
        let path = PathBuf::from(configured);
        if !crate::platform::open_with::is_executable(&path) {
            return Err(format!(
                "Settings → Open With: invalid {} executable path; use a full path.",
                application.name()
            ));
        }
        path
    } else {
        let cached = std::fs::read_to_string(cache_path())
            .ok()
            .and_then(|content| toml::from_str::<OpenWithConfig>(&content).ok());
        let cached_path = cached
            .and_then(|config| match application {
                Application::VsCode => config.vscode,
                Application::VisualStudio => config.visual_studio,
                Application::Explorer => None,
            })
            .map(PathBuf::from)
            .filter(|path| crate::platform::open_with::is_executable(path));
        cached_path
            .or_else(|| crate::platform::open_with::detect(application))
            .ok_or_else(|| {
                format!(
                    "{} not found; set its path in Settings → Open With.",
                    application.name()
                )
            })?
    };
    crate::platform::open_with::launch(application, Some(&executable), directory)
}
