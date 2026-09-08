use crate::api::schema::{
    ResponseResult, TaskCreateParams, TaskInfo, TaskListParams, TaskOpenParams, TaskRenameParams,
    TaskRuntimeInfo, TaskTarget,
};
use crate::app::App;
use crate::task::{Task, TaskLocationMode, TaskStatus};

use super::responses::{encode_error, encode_success};

impl App {
    pub(super) fn handle_task_create(&mut self, id: String, params: TaskCreateParams) -> String {
        let name = params.name.trim();
        if name.is_empty() {
            return encode_error(id, "invalid_params", "task name must not be empty");
        }
        if self.state.projects.find(&params.project_id).is_none() {
            return project_not_found(id, &params.project_id);
        }
        let project_default_agent = self
            .state
            .projects
            .find(&params.project_id)
            .and_then(|project| project.default_agent.clone());
        let worktree_path = match params.location {
            TaskLocationMode::Repository => {
                if params
                    .worktree_path
                    .as_deref()
                    .is_some_and(|path| !path.trim().is_empty())
                {
                    return encode_error(
                        id,
                        "invalid_params",
                        "repository tasks must not specify a worktree path",
                    );
                }
                None
            }
            TaskLocationMode::Worktree => {
                let Some(path) = params.worktree_path.as_deref() else {
                    return encode_error(
                        id,
                        "invalid_params",
                        "worktree tasks require an existing worktree path",
                    );
                };
                let path = match std::fs::canonicalize(path) {
                    Ok(path) if path.is_dir() => path,
                    Ok(_) => {
                        return encode_error(
                            id,
                            "invalid_params",
                            "task worktree path must be a directory",
                        )
                    }
                    Err(err) => {
                        return encode_error(
                            id,
                            "invalid_params",
                            format!("task worktree path is not accessible: {err}"),
                        )
                    }
                };
                Some(path)
            }
        };
        let provider = match clean_optional(params.provider).or(project_default_agent) {
            Some(provider) => match crate::project::normalize_agent_provider(&provider) {
                Ok(provider) => Some(provider),
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => None,
        };
        let previous = self.state.tasks.clone();
        let task = Task::new(
            params.project_id,
            name.to_owned(),
            params.location,
            clean_optional(params.branch),
            worktree_path,
            provider,
            clean_optional(params.model),
            params.prompt.filter(|prompt| !prompt.is_empty()),
            clean_optional(params.workspace_id),
            clean_optional(params.tab_id),
            clean_optional(params.pane_id),
        );
        self.state.tasks.insert(task.clone());
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return encode_error(id, "task_save_failed", err.to_string());
        }
        encode_success(
            id,
            ResponseResult::TaskCreated {
                task: self.task_info(&task),
            },
        )
    }

    pub(super) fn handle_task_list(&mut self, id: String, params: TaskListParams) -> String {
        if let Some(project_id) = params.project_id.as_deref() {
            if self.state.projects.find(project_id).is_none() {
                return project_not_found(id, project_id);
            }
        }
        let tasks = self
            .state
            .tasks
            .tasks
            .iter()
            .filter(|task| {
                params
                    .project_id
                    .as_deref()
                    .is_none_or(|project_id| task.project_id == project_id)
            })
            .filter(|task| params.include_closed || task.status != TaskStatus::Closed)
            .map(|task| self.task_info(task))
            .collect();
        encode_success(id, ResponseResult::TaskList { tasks })
    }

    pub(super) fn handle_task_open(&mut self, id: String, params: TaskOpenParams) -> String {
        let Some(mut task) = self.state.tasks.find(&params.task_id).cloned() else {
            return task_not_found(id, &params.task_id);
        };
        let mut runtime = self.task_runtime_info(&task);
        if !runtime.available {
            if let Some(target_runtime) = self.open_task_target(&task, params.focus) {
                runtime = target_runtime;
                task.workspace_id = runtime.workspace_id.clone();
                task.tab_id = runtime.tab_id.clone();
                task.pane_id = runtime.pane_id.clone();
                task.updated_at = crate::task::current_unix_ms();
                let previous = self.state.tasks.clone();
                if let Some(stored) = self.state.tasks.find_mut(&task.id) {
                    stored.workspace_id = task.workspace_id.clone();
                    stored.tab_id = task.tab_id.clone();
                    stored.pane_id = task.pane_id.clone();
                    stored.updated_at = task.updated_at;
                }
                if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
                    self.state.tasks = previous;
                    return encode_error(id, "task_save_failed", err.to_string());
                }
            }
        }
        if params.focus && runtime.available {
            self.focus_task_runtime(&runtime);
        }
        encode_success(
            id,
            ResponseResult::TaskOpened {
                task: self.task_info(&task),
                runtime,
            },
        )
    }

    pub(super) fn handle_task_rename(&mut self, id: String, params: TaskRenameParams) -> String {
        let name = params.name.trim();
        if name.is_empty() {
            return encode_error(id, "invalid_params", "task name must not be empty");
        }
        let previous = self.state.tasks.clone();
        let Some(task) = self.state.tasks.find_mut(&params.task_id) else {
            return task_not_found(id, &params.task_id);
        };
        task.name = name.to_owned();
        task.updated_at = crate::task::current_unix_ms();
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return encode_error(id, "task_save_failed", err.to_string());
        }
        let Some(task) = self.state.tasks.find(&params.task_id) else {
            return task_not_found(id, &params.task_id);
        };
        encode_success(
            id,
            ResponseResult::TaskRenamed {
                task: self.task_info(task),
            },
        )
    }

    pub(super) fn handle_task_close(&mut self, id: String, target: TaskTarget) -> String {
        let previous = self.state.tasks.clone();
        let Some(task) = self.state.tasks.find_mut(&target.task_id) else {
            return task_not_found(id, &target.task_id);
        };
        if task.status != TaskStatus::Closed {
            let now = crate::task::current_unix_ms();
            task.status = TaskStatus::Closed;
            task.updated_at = now;
            task.closed_at = Some(now);
        }
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return encode_error(id, "task_save_failed", err.to_string());
        }
        let Some(task) = self.state.tasks.find(&target.task_id) else {
            return task_not_found(id, &target.task_id);
        };
        encode_success(
            id,
            ResponseResult::TaskClosed {
                task: self.task_info(task),
            },
        )
    }

    fn open_task_target(&mut self, task: &Task, focus: bool) -> Option<TaskRuntimeInfo> {
        let target_path = match task.location {
            TaskLocationMode::Repository => self
                .state
                .projects
                .find(&task.project_id)
                .map(|project| project.root_path.clone()),
            TaskLocationMode::Worktree => task.worktree_path.clone(),
        }?;
        if !target_path.is_dir() {
            return None;
        }
        let workspace_index = self
            .state
            .workspaces
            .iter()
            .position(|workspace| crate::project::same_path(&workspace.identity_cwd, &target_path))
            .or_else(|| self.create_workspace_with_options(target_path, focus).ok())?;
        let workspace = self.state.workspaces.get(workspace_index)?;
        let tab_index = workspace.active_tab_index();
        let pane = workspace.focused_pane_id()?;
        Some(TaskRuntimeInfo {
            available: true,
            workspace_id: Some(self.public_workspace_id(workspace_index)),
            tab_id: self.public_tab_id(workspace_index, tab_index),
            pane_id: self.public_pane_id(workspace_index, pane),
            message: None,
        })
    }

    pub(crate) fn task_info(&self, task: &Task) -> TaskInfo {
        let runtime = self.task_runtime_info(task);
        TaskInfo {
            task_id: task.id.clone(),
            project_id: task.project_id.clone(),
            name: task.name.clone(),
            location: task.location,
            branch: task.branch.clone(),
            worktree_path: task
                .worktree_path
                .as_deref()
                .map(crate::project::display_path),
            provider: task.provider.clone(),
            model: task.model.clone(),
            prompt: task.prompt.clone(),
            status: task.status,
            created_at: task.created_at,
            updated_at: task.updated_at,
            closed_at: task.closed_at,
            workspace_id: task.workspace_id.clone(),
            tab_id: task.tab_id.clone(),
            pane_id: task.pane_id.clone(),
            runtime_available: runtime.available,
            runtime_message: runtime.message,
        }
    }

    fn task_runtime_info(&self, task: &Task) -> TaskRuntimeInfo {
        let workspace_id = task.workspace_id.clone();
        let tab_id = task.tab_id.clone();
        let pane_id = task.pane_id.clone();
        let message = match (
            workspace_id.as_deref(),
            tab_id.as_deref(),
            pane_id.as_deref(),
        ) {
            (None, _, _) => Some("task has no attached runtime pane".to_owned()),
            (Some(workspace_ref), None, _) => Some(format!(
                "task runtime tab is missing for workspace {workspace_ref}"
            )),
            (Some(workspace_ref), Some(tab_ref), None) => Some(format!(
                "task runtime pane is missing for tab {tab_ref} in workspace {workspace_ref}"
            )),
            (Some(workspace_ref), Some(tab_ref), Some(pane_ref)) => {
                let Some(workspace_index) = self.parse_workspace_id(workspace_ref) else {
                    return TaskRuntimeInfo {
                        available: false,
                        workspace_id: workspace_id.clone(),
                        tab_id: tab_id.clone(),
                        pane_id: pane_id.clone(),
                        message: Some("task workspace is no longer available".to_owned()),
                    };
                };
                let Some((tab_workspace_index, _)) = self.parse_tab_id(tab_ref) else {
                    return TaskRuntimeInfo {
                        available: false,
                        workspace_id: workspace_id.clone(),
                        tab_id: tab_id.clone(),
                        pane_id: pane_id.clone(),
                        message: Some("task tab is no longer available".to_owned()),
                    };
                };
                if tab_workspace_index != workspace_index {
                    return TaskRuntimeInfo {
                        available: false,
                        workspace_id: workspace_id.clone(),
                        tab_id: tab_id.clone(),
                        pane_id: pane_id.clone(),
                        message: Some("task runtime tab belongs to another workspace".to_owned()),
                    };
                }
                let Some((pane_workspace_index, _)) = self.parse_pane_id(pane_ref) else {
                    return TaskRuntimeInfo {
                        available: false,
                        workspace_id: workspace_id.clone(),
                        tab_id: tab_id.clone(),
                        pane_id: pane_id.clone(),
                        message: Some("task pane is no longer available".to_owned()),
                    };
                };
                if pane_workspace_index != workspace_index {
                    return TaskRuntimeInfo {
                        available: false,
                        workspace_id: workspace_id.clone(),
                        tab_id: tab_id.clone(),
                        pane_id: pane_id.clone(),
                        message: Some("task runtime pane belongs to another workspace".to_owned()),
                    };
                }
                None
            }
        };
        TaskRuntimeInfo {
            available: message.is_none(),
            workspace_id,
            tab_id,
            pane_id,
            message,
        }
    }

    fn focus_task_runtime(&mut self, runtime: &TaskRuntimeInfo) {
        let Some(workspace_id) = runtime.workspace_id.as_deref() else {
            return;
        };
        let Some(tab_id) = runtime.tab_id.as_deref() else {
            return;
        };
        let Some(pane_id) = runtime.pane_id.as_deref() else {
            return;
        };
        let Some(workspace_index) = self.parse_workspace_id(workspace_id) else {
            return;
        };
        let Some((tab_workspace_index, tab_index)) = self.parse_tab_id(tab_id) else {
            return;
        };
        let Some((pane_workspace_index, pane)) = self.parse_pane_id(pane_id) else {
            return;
        };
        if workspace_index != tab_workspace_index || workspace_index != pane_workspace_index {
            return;
        }
        self.state.switch_workspace_tab(workspace_index, tab_index);
        self.state.focus_pane_in_workspace(workspace_index, pane);
        self.state.mode = crate::app::Mode::Terminal;
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

fn project_not_found(id: String, project_id: &str) -> String {
    encode_error(
        id,
        "project_not_found",
        format!("project {project_id} not found"),
    )
}

fn task_not_found(id: String, task_id: &str) -> String {
    encode_error(id, "task_not_found", format!("task {task_id} not found"))
}
