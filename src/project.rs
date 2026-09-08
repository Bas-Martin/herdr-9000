use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT_ID: AtomicU64 = AtomicU64::new(1);
pub(crate) const DEFAULT_WORKTREE_BASE: &str = "origin/main";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Project {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) root_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_base: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ProjectRegistry {
    #[serde(default = "default_registry_version")]
    pub(crate) version: u32,
    #[serde(default)]
    pub(crate) projects: Vec<Project>,
}

fn default_registry_version() -> u32 {
    1
}

impl Default for ProjectRegistry {
    fn default() -> Self {
        Self {
            version: default_registry_version(),
            projects: Vec::new(),
        }
    }
}

impl ProjectRegistry {
    pub(crate) fn find(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    pub(crate) fn find_mut(&mut self, id: &str) -> Option<&mut Project> {
        self.projects.iter_mut().find(|project| project.id == id)
    }

    pub(crate) fn find_by_root(&self, root_path: &Path) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| same_path(&project.root_path, root_path))
    }

    pub(crate) fn insert(&mut self, project: Project) {
        self.projects.push(project);
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<Project> {
        let index = self.projects.iter().position(|project| project.id == id)?;
        Some(self.projects.remove(index))
    }
}

impl Project {
    pub(crate) fn new(name: String, root_path: PathBuf, worktree_root: Option<PathBuf>) -> Self {
        let id = format!("p{}", NEXT_PROJECT_ID.fetch_add(1, Ordering::Relaxed));
        Self {
            id,
            name,
            root_path,
            worktree_root,
            worktree_base: None,
        }
    }
}

pub(crate) fn reserve_project_ids(registry: &ProjectRegistry) {
    let next_id = registry
        .projects
        .iter()
        .filter_map(|project| project.id.strip_prefix('p'))
        .filter_map(|value| value.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    NEXT_PROJECT_ID.fetch_max(next_id, Ordering::Relaxed);
}

pub(crate) fn normalize_root_path(raw_path: &str) -> Result<PathBuf, String> {
    if raw_path.trim().is_empty() {
        return Err("project root path must not be empty".into());
    }

    let path = std::fs::canonicalize(raw_path)
        .map_err(|err| format!("project root path is not accessible: {err}"))?;
    if !path.is_dir() {
        return Err("project root path must be a directory".into());
    }
    Ok(path)
}

pub(crate) fn normalize_worktree_root(raw_path: &str) -> Result<PathBuf, String> {
    if raw_path.trim().is_empty() {
        return Err("worktree root path must not be empty".into());
    }

    let path = std::fs::canonicalize(raw_path)
        .map_err(|err| format!("worktree root path is not accessible: {err}"))?;
    if !path.is_dir() {
        return Err("worktree root path must be a directory".into());
    }
    Ok(path)
}

pub(crate) fn display_path(path: &Path) -> String {
    let value = path.display().to_string();
    if let Some(path) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{path}");
    }
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}
pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    let left = canonical_or_original(left);
    let right = canonical_or_original(right);
    same_normalized_path(&left, &right)
}

fn canonical_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(windows)]
fn same_normalized_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(right.to_string_lossy().as_ref())
}

#[cfg(not(windows))]
fn same_normalized_path(left: &Path, right: &Path) -> bool {
    left == right
}
