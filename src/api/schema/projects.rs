use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
pub struct ProjectLifecycle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepare: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teardown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectCreateParams {
    pub name: String,
    pub root_path: String,
    #[serde(default)]
    pub open: bool,
    #[serde(default = "default_true")]
    pub focus: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<ProjectLifecycle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_patterns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_endpoint_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectTarget {
    pub project_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectRenameParams {
    pub project_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectUpdateParams {
    pub project_id: String,
    pub name: String,
    pub root_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<ProjectLifecycle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_patterns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_endpoint_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectOpenParams {
    pub project_id: String,
    #[serde(default = "default_true")]
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectInfo {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_endpoint_id: Option<String>,
    pub name: String,
    pub root_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub lifecycle: ProjectLifecycle,
    #[serde(default)]
    pub preserve_patterns: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub external_trackers: Vec<super::external::ExternalTrackerConfig>,
}
