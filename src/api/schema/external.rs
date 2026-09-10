use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::task::TaskLocationMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalTrackerProvider {
    Linear,
    Jira,
    Gitlab,
    Asana,
    Plane,
    Notion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExternalTrackerConfig {
    pub provider: ExternalTrackerProvider,
    pub enabled: bool,
    pub base_url: String,
    pub credential_env: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExternalTrackerConfigureParams {
    pub project_id: String,
    pub provider: ExternalTrackerProvider,
    pub enabled: bool,
    pub base_url: String,
    pub credential_env: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExternalIssueSearchParams {
    pub project_id: String,
    pub provider: ExternalTrackerProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExternalIssueInfo {
    pub provider: ExternalTrackerProvider,
    pub identifier: String,
    pub title: String,
    pub body: String,
    pub url: String,
    pub state: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExternalIssueTaskCreateParams {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_endpoint_id: Option<String>,
    pub issue: ExternalIssueInfo,
    #[serde(default)]
    pub location: TaskLocationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}
