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
    let solution = if application == Application::VisualStudio {
        nearest_solution(directory).map_err(|error| {
            format!(
                "Cannot find a Visual Studio solution in '{}': {error}",
                directory.display()
            )
        })?
    } else {
        None
    };
    crate::platform::open_with::launch(
        application,
        Some(&executable),
        solution.as_deref().unwrap_or(directory),
    )
}

fn nearest_solution(directory: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut directories = vec![directory.to_path_buf()];
    let mut next_level = Vec::new();
    while !directories.is_empty() {
        directories.sort_unstable();
        for current in directories.drain(..) {
            let mut nearest: Option<PathBuf> = None;
            for entry in std::fs::read_dir(current)? {
                let entry = entry?;
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

                    // Junctions can leave the checkout without being ordinary symbolic links.
                    if entry.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        continue;
                    }
                }
                let file_type = entry.file_type()?;
                let name = entry.file_name();
                if file_type.is_dir() {
                    if name != ".git" && nearest.is_none() {
                        next_level.push(entry.path());
                    }
                } else if file_type.is_file()
                    && Path::new(&name)
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| {
                            extension.eq_ignore_ascii_case("sln")
                                || extension.eq_ignore_ascii_case("slnx")
                        })
                {
                    let path = entry.path();
                    if nearest.as_ref().is_none_or(|previous| path < *previous) {
                        nearest = Some(path);
                    }
                }
            }
            if nearest.is_some() {
                return Ok(nearest);
            }
        }
        std::mem::swap(&mut directories, &mut next_level);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::nearest_solution;
    use std::{fs, time::SystemTime};

    #[test]
    fn nearest_solution_stays_in_checkout_and_prefers_depth_then_path() -> std::io::Result<()> {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp = std::env::temp_dir().join(format!(
            "herdr-nearest-solution-{}-{unique}",
            std::process::id()
        ));
        let checkout = temp.join("worktree with spaces");
        fs::create_dir_all(checkout.join(".git"))?;
        fs::write(temp.join("Outside.sln"), "")?;
        fs::write(checkout.join(".git/Ignored.sln"), "")?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(temp.join("Outside.sln"), checkout.join("Linked.sln"))?;
            std::os::unix::fs::symlink(&temp, checkout.join("external"))?;
        }
        assert_eq!(nearest_solution(&checkout)?, None);
        assert!(nearest_solution(&checkout.join("missing")).is_err());

        let deep = checkout.join("beta/deep/Deep.sln");
        fs::create_dir_all(deep.parent().unwrap())?;
        fs::write(&deep, "")?;
        assert_eq!(nearest_solution(&checkout)?, Some(deep));

        let shallow = checkout.join("beta/B.SLNX");
        fs::write(&shallow, "")?;
        assert_eq!(nearest_solution(&checkout)?, Some(shallow));

        let first_path = checkout.join("alpha/Z.sln");
        fs::create_dir_all(first_path.parent().unwrap())?;
        fs::write(&first_path, "")?;
        assert_eq!(nearest_solution(&checkout)?, Some(first_path));

        let root = checkout.join("Root.sln");
        fs::write(&root, "")?;
        assert_eq!(nearest_solution(&checkout)?, Some(root));
        fs::remove_dir_all(temp)
    }
}
