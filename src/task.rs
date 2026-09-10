use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskLocationMode {
    #[default]
    Repository,
    Worktree,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskStatus {
    Queued,
    Provisioning,
    Working,
    Blocked,
    ReviewReady,
    Failed,
    Completed,
    Open,
    Closed,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskAgentStatus {
    Idle,
    Working,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TaskHistoryEntry {
    pub(crate) status: TaskStatus,
    pub(crate) at: u64,
    pub(crate) reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GitHubIssueContext {
    pub(crate) repository: String,
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) url: String,
    pub(crate) labels: Vec<String>,
    pub(crate) assignees: Vec<String>,
    pub(crate) state: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskLifecycleStep {
    Prepare,
    Setup,
    Run,
    Teardown,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TaskLifecycleStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TaskLifecycleRun {
    pub(crate) step: TaskLifecycleStep,
    pub(crate) status: TaskLifecycleStatus,
    pub(crate) started_at: u64,
    pub(crate) finished_at: Option<u64>,
    pub(crate) output: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Task {
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remote_endpoint_id: Option<String>,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) location: TaskLocationMode,
    pub(crate) branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_path: Option<PathBuf>,
    #[serde(default)]
    pub(crate) auto_provisioned_worktree: bool,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) prompt: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) environment: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) resource_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent_session: Option<crate::agent_resume::PersistedAgentSession>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) lifecycle_runs: Vec<TaskLifecycleRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pull_request_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) github_issue: Option<GitHubIssueContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) external_issue: Option<crate::api::schema::ExternalIssueInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) current_step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent_status: Option<TaskAgentStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) history: Vec<TaskHistoryEntry>,
    pub(crate) status: TaskStatus,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
    pub(crate) closed_at: Option<u64>,
    pub(crate) workspace_id: Option<String>,
    pub(crate) tab_id: Option<String>,
    pub(crate) pane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tmux_pane_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TaskRegistry {
    #[serde(default = "default_registry_version")]
    pub(crate) version: u32,
    #[serde(default)]
    pub(crate) tasks: Vec<Task>,
}

fn default_registry_version() -> u32 {
    1
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self {
            version: default_registry_version(),
            tasks: Vec::new(),
        }
    }
}

impl TaskRegistry {
    pub(crate) fn find(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|task| task.id == id)
    }

    pub(crate) fn find_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|task| task.id == id)
    }

    pub(crate) fn insert(&mut self, task: Task) {
        self.tasks.push(task);
    }
}

impl Task {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        project_id: String,
        name: String,
        location: TaskLocationMode,
        branch: Option<String>,
        worktree_path: Option<PathBuf>,
        provider: Option<String>,
        model: Option<String>,
        prompt: Option<String>,
        environment: BTreeMap<String, String>,
        resource_ids: Vec<String>,
        workspace_id: Option<String>,
        tab_id: Option<String>,
        pane_id: Option<String>,
    ) -> Self {
        let now = current_unix_ms();
        Self {
            id: format!("t{}", NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)),
            remote_endpoint_id: None,
            project_id,
            name,
            location,
            branch,
            worktree_path,
            auto_provisioned_worktree: false,
            provider,
            model,
            prompt,
            environment,
            resource_ids,
            agent_command: None,
            error: None,
            agent_session: None,
            lifecycle_runs: Vec::new(),
            pull_request_url: None,
            github_issue: None,
            external_issue: None,
            current_step: None,
            agent_status: None,
            history: vec![TaskHistoryEntry {
                status: TaskStatus::Queued,
                at: now,
                reason: Some("task created".to_owned()),
            }],
            status: TaskStatus::Queued,
            created_at: now,
            updated_at: now,
            closed_at: None,
            workspace_id,
            tab_id,
            pane_id,
            tmux_pane_id: None,
        }
    }
}

pub(crate) fn reserve_task_ids(registry: &TaskRegistry) {
    let next_id = registry
        .tasks
        .iter()
        .filter_map(|task| task.id.strip_prefix('t'))
        .filter_map(|value| value.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    NEXT_TASK_ID.fetch_max(next_id, Ordering::Relaxed);
}

pub(crate) fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
