use std::collections::BTreeMap;
use std::time::Duration;

use crate::api::schema::{
    AgentPromptParams, AgentStartParams, ResponseResult, TaskCreateParams, TaskInfo,
    TaskListParams, TaskOpenParams, TaskRenameParams, TaskRuntimeInfo, TaskTarget,
};
use crate::app::App;
use crate::events::AppEvent;
use crate::task::{
    Task, TaskLifecycleRun, TaskLifecycleStatus, TaskLifecycleStep, TaskLocationMode, TaskStatus,
};

use super::responses::{encode_error, encode_success};

impl App {
    pub(super) fn handle_task_create(&mut self, id: String, params: TaskCreateParams) -> String {
        let name = params.name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return encode_error(
                id,
                "invalid_params",
                "task name must be printable and must not be empty",
            );
        }
        let Some(project) = self.state.projects.find(&params.project_id).cloned() else {
            return project_not_found(id, &params.project_id);
        };
        let project_default_agent = project.default_agent.clone();
        let mut environment = project.environment.clone();
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
        let task_environment = match super::projects::normalize_environment(params.environment) {
            Ok(environment) => environment,
            Err(message) => return encode_error(id, "invalid_params", message),
        };
        environment.extend(task_environment);
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
            environment,
            clean_optional(params.workspace_id),
            clean_optional(params.tab_id),
            clean_optional(params.pane_id),
        );
        self.state.tasks.insert(task.clone());
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return encode_error(id, "task_save_failed", err.to_string());
        }
        let task_id = task.id.clone();
        if task.location == TaskLocationMode::Worktree {
            if let Err(err) = self.run_task_lifecycle(&task_id) {
                tracing::warn!(task_id, error = %err, "task lifecycle failed");
            }
        }
        let task = self
            .state
            .tasks
            .find(&task_id)
            .expect("saved task should remain available");
        encode_success(
            id,
            ResponseResult::TaskCreated {
                task: self.task_info(task),
            },
        )
    }
    pub(super) fn start_task_agent(&mut self, task_id: &str) -> Result<(), String> {
        let Some(task) = self.state.tasks.find(task_id).cloned() else {
            return Err(format!("task {task_id} no longer exists"));
        };
        let Some(provider) = task.provider.clone() else {
            return Ok(());
        };
        let Some(project) = self.state.projects.find(&task.project_id).cloned() else {
            let message = format!("project {} no longer exists", task.project_id);
            self.record_task_error(task_id, message.clone());
            return Err(message);
        };
        let Some(workspace_path) = task_workspace_path(&task, &project) else {
            let message = "task has no workspace path for automatic agent launch".to_owned();
            self.record_task_error(task_id, message.clone());
            return Err(message);
        };
        let environment = task_environment(&task, &project, &workspace_path);
        let Some(pane_id) = task.pane_id.clone() else {
            let message = "task has no runtime pane for automatic agent launch".to_owned();
            self.record_task_error(task_id, message.clone());
            return Err(message);
        };
        let provider = match crate::project::normalize_agent_provider(&provider) {
            Ok(provider) => provider,
            Err(message) => {
                self.record_task_error(task_id, message.clone());
                return Err(message);
            }
        };
        let command = crate::detect::interactive_agent_executable(
            crate::detect::parse_agent_label(&provider)
                .expect("normalized task provider must have a known agent"),
        )
        .to_owned();
        let now = crate::task::current_unix_ms();
        let previous = self.state.tasks.clone();
        let Some(stored) = self.state.tasks.find_mut(task_id) else {
            return Err(format!("task {task_id} no longer exists"));
        };
        stored.provider = Some(provider.clone());
        stored.agent_command = Some(command);
        stored.error = None;
        stored.updated_at = now;
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return Err(format!("task launch metadata could not be saved: {err}"));
        }
        let start = self.start_agent_with_environment(
            AgentStartParams {
                name: task.id.clone(),
                kind: provider,
                pane_id,
                args: Vec::new(),
                timeout_ms: None,
            },
            &environment,
        );
        if let Err(err) = start {
            let message = self.agent_start_error_body(err).message;
            self.record_task_error(task_id, message.clone());
            return Err(message);
        }
        self.schedule_task_agent_prompt(task.id, 0);
        Ok(())
    }
    pub(super) fn run_task_lifecycle(&mut self, task_id: &str) -> Result<(), String> {
        let Some(task) = self.state.tasks.find(task_id).cloned() else {
            return Err(format!("task {task_id} no longer exists"));
        };
        let Some(project) = self.state.projects.find(&task.project_id).cloned() else {
            let message = format!("project {} no longer exists", task.project_id);
            self.record_task_error(task_id, message.clone());
            return Err(message);
        };
        let Some(workspace_path) = task_workspace_path(&task, &project) else {
            let message = "task has no workspace path for lifecycle commands".to_owned();
            self.record_task_error(task_id, message.clone());
            return Err(message);
        };
        let environment = task_environment(&task, &project, &workspace_path);
        let steps = [
            (TaskLifecycleStep::Prepare, project.lifecycle.prepare),
            (TaskLifecycleStep::Setup, project.lifecycle.setup),
            (TaskLifecycleStep::Run, project.lifecycle.run),
        ];
        for (step, command) in steps {
            if let Some(command) = command {
                self.run_task_lifecycle_step(
                    task_id,
                    step,
                    &workspace_path,
                    &environment,
                    &command,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn run_task_teardown(&mut self, task_id: &str) -> Result<(), String> {
        let Some(task) = self.state.tasks.find(task_id).cloned() else {
            return Err(format!("task {task_id} no longer exists"));
        };
        let Some(project) = self.state.projects.find(&task.project_id).cloned() else {
            let message = format!("project {} no longer exists", task.project_id);
            self.record_task_error(task_id, message.clone());
            return Err(message);
        };
        let Some(command) = project.lifecycle.teardown.as_deref() else {
            return Ok(());
        };
        let Some(workspace_path) = task_workspace_path(&task, &project) else {
            let message = "task has no workspace path for teardown".to_owned();
            self.record_task_error(task_id, message.clone());
            return Err(message);
        };
        let environment = task_environment(&task, &project, &workspace_path);
        self.run_task_lifecycle_step(
            task_id,
            TaskLifecycleStep::Teardown,
            &workspace_path,
            &environment,
            command,
        )
    }

    fn run_task_lifecycle_step(
        &mut self,
        task_id: &str,
        step: TaskLifecycleStep,
        workspace_path: &std::path::Path,
        environment: &BTreeMap<String, String>,
        command: &str,
    ) -> Result<(), String> {
        let started_at = crate::task::current_unix_ms();
        let previous = self.state.tasks.clone();
        let Some(task) = self.state.tasks.find_mut(task_id) else {
            return Err(format!("task {task_id} no longer exists"));
        };
        task.lifecycle_runs.push(TaskLifecycleRun {
            step,
            status: TaskLifecycleStatus::Running,
            started_at,
            finished_at: None,
            output: None,
            error: None,
        });
        task.updated_at = started_at;
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return Err(format!("lifecycle run could not be saved: {err}"));
        }

        let (output, error) = run_lifecycle_command(workspace_path, environment, command);
        let finished_at = crate::task::current_unix_ms();
        let Some(task) = self.state.tasks.find_mut(task_id) else {
            return Err(format!("task {task_id} no longer exists"));
        };
        let run = task
            .lifecycle_runs
            .last_mut()
            .expect("lifecycle run was inserted above");
        run.status = if error.is_some() {
            TaskLifecycleStatus::Failed
        } else {
            TaskLifecycleStatus::Succeeded
        };
        run.finished_at = Some(finished_at);
        run.output = output;
        run.error = error.clone();
        task.updated_at = finished_at;
        if let Some(error) = error {
            task.error = Some(format!(
                "lifecycle {} failed: {error}",
                lifecycle_step_label(step)
            ));
        }
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            tracing::error!(task_id, error = %err, "failed to persist lifecycle result");
        }
        if let Some(error) = self
            .state
            .tasks
            .find(task_id)
            .and_then(|task| task.lifecycle_runs.last())
            .and_then(|run| run.error.clone())
        {
            return Err(format!(
                "lifecycle {} failed: {error}",
                lifecycle_step_label(step)
            ));
        }
        Ok(())
    }

    fn schedule_task_agent_prompt(&self, task_id: String, attempt: u8) {
        let event_tx = self.event_tx.clone();
        std::thread::spawn(move || {
            let delay = if attempt == 0 {
                Duration::from_millis(100)
            } else {
                Duration::from_millis(250)
            };
            std::thread::sleep(delay);
            let _ = event_tx.blocking_send(AppEvent::TaskAgentPrompt { task_id, attempt });
        });
    }

    pub(super) fn handle_task_agent_prompt(&mut self, task_id: String, attempt: u8) {
        const MAX_PROMPT_ATTEMPTS: u8 = 24;
        let Some(task) = self.state.tasks.find(&task_id).cloned() else {
            return;
        };
        let Some(target) = task.pane_id.clone() else {
            self.record_task_error(
                &task_id,
                "task has no runtime pane for the initial agent prompt".to_owned(),
            );
            return;
        };
        let prompt = task
            .prompt
            .clone()
            .filter(|prompt| !prompt.is_empty())
            .unwrap_or(task.name);
        let request_id = format!("task-agent-prompt:{task_id}");
        match self.queue_agent_prompt(
            request_id,
            AgentPromptParams {
                target,
                text: prompt,
                wait: None,
            },
        ) {
            Ok((_id, _agent, completion)) => {
                let event_tx = self.event_tx.clone();
                std::thread::spawn(move || {
                    let result = match completion.recv() {
                        Ok(Ok(())) => Ok(()),
                        Ok(Err(err)) => Err(err.to_string()),
                        Err(_) => {
                            Err("pty actor closed before initial prompt completed".to_owned())
                        }
                    };
                    let _ = event_tx
                        .blocking_send(AppEvent::TaskAgentPromptFinished { task_id, result });
                });
            }
            Err(response) => {
                let error = serde_json::from_str::<crate::api::schema::ErrorResponse>(&response)
                    .map(|response| response.error)
                    .ok();
                if error
                    .as_ref()
                    .is_some_and(|error| error.code == "agent_not_ready")
                    && attempt < MAX_PROMPT_ATTEMPTS
                {
                    self.schedule_task_agent_prompt(task_id, attempt + 1);
                } else {
                    let message = error.map_or_else(
                        || "initial agent prompt could not be submitted".to_owned(),
                        |error| error.message,
                    );
                    self.record_task_error(&task_id, message);
                }
            }
        }
    }

    pub(super) fn handle_task_agent_prompt_finished(
        &mut self,
        task_id: String,
        result: Result<(), String>,
    ) {
        if let Err(message) = result {
            self.record_task_error(&task_id, format!("initial agent prompt failed: {message}"));
        }
    }

    fn record_task_error(&mut self, task_id: &str, message: String) {
        let Some(task) = self.state.tasks.find_mut(task_id) else {
            return;
        };
        task.error = Some(message);
        task.updated_at = crate::task::current_unix_ms();
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            tracing::error!(task_id, error = %err, "failed to persist task error");
        }
    }
    pub(super) fn record_task_agent_session(
        &mut self,
        pane_id: crate::layout::PaneId,
        source: String,
        agent: String,
        session_ref: crate::agent_resume::AgentSessionRef,
    ) {
        let Some((workspace_index, _)) = self.find_pane(pane_id) else {
            return;
        };
        let Some(public_pane_id) = self.public_pane_id(workspace_index, pane_id) else {
            return;
        };
        let persisted = crate::agent_resume::PersistedAgentSession {
            source,
            agent,
            session_ref,
        };
        let now = crate::task::current_unix_ms();
        let mut changed = false;
        for task in &mut self.state.tasks.tasks {
            if task.pane_id.as_deref() == Some(public_pane_id.as_str())
                && task.agent_session.as_ref() != Some(&persisted)
            {
                task.agent_session = Some(persisted.clone());
                task.updated_at = now;
                changed = true;
            }
        }
        if changed {
            if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
                tracing::error!(error = %err, "failed to persist task agent session");
            }
        }
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
        let should_teardown = self
            .state
            .tasks
            .find(&target.task_id)
            .is_some_and(|task| task.status != TaskStatus::Closed);
        if should_teardown {
            if let Err(err) = self.run_task_teardown(&target.task_id) {
                tracing::warn!(task_id = %target.task_id, error = %err, "task teardown failed");
            }
        }
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
    pub(super) fn handle_task_diff(
        &mut self,
        id: String,
        params: crate::api::schema::TaskDiffParams,
    ) -> String {
        let Some(task) = self.state.tasks.find(&params.task_id).cloned() else {
            return task_not_found(id, &params.task_id);
        };
        let Some(project) = self.state.projects.find(&task.project_id).cloned() else {
            return project_not_found(id, &task.project_id);
        };
        let Some(worktree_path) = task_workspace_path(&task, &project) else {
            return encode_error(id, "task_workspace_missing", "task has no workspace path");
        };
        if !worktree_path.is_dir() {
            return encode_error(
                id,
                "task_workspace_missing",
                "task workspace is no longer available",
            );
        }
        let base = params.base.unwrap_or_else(|| "HEAD".to_owned());
        if base.is_empty() || base.starts_with('-') || base.chars().any(char::is_control) {
            return encode_error(
                id,
                "invalid_params",
                "diff base must be a valid Git revision",
            );
        }
        let status = match run_task_git(
            &worktree_path,
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        ) {
            Ok(status) => status,
            Err(message) => return encode_error(id, "task_diff_failed", message),
        };
        let files = parse_task_diff_status(&status);
        let patch = match run_task_git_with_base(&worktree_path, "diff", &base) {
            Ok(patch) => patch,
            Err(message) => return encode_error(id, "task_diff_failed", message),
        };
        let untracked = files
            .iter()
            .filter(|file| file.status == crate::api::schema::TaskDiffFileStatus::Untracked)
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let mut patch = patch;
        for path in untracked {
            match untracked_file_patch(&worktree_path, &path) {
                Ok(Some(file_patch)) => {
                    if !patch.is_empty() && !patch.ends_with('\n') {
                        patch.push('\n');
                    }
                    patch.push_str(&file_patch);
                }
                Ok(None) => {}
                Err(message) => return encode_error(id, "task_diff_failed", message),
            }
        }
        encode_success(
            id,
            ResponseResult::TaskDiff {
                diff: crate::api::schema::TaskDiffInfo {
                    task_id: task.id,
                    project_id: task.project_id,
                    worktree_path: crate::project::display_path(&worktree_path),
                    view: params.view,
                    files,
                    patch,
                    read_only: true,
                    refreshed_at: crate::task::current_unix_ms(),
                },
            },
        )
    }

    pub(super) fn handle_task_file_write(
        &mut self,
        id: String,
        params: crate::api::schema::TaskFileWriteParams,
    ) -> String {
        let Some(task) = self.state.tasks.find(&params.task_id).cloned() else {
            return task_not_found(id, &params.task_id);
        };
        let Some(project) = self.state.projects.find(&task.project_id).cloned() else {
            return project_not_found(id, &task.project_id);
        };
        let Some(worktree_path) = task_workspace_path(&task, &project) else {
            return encode_error(id, "task_workspace_missing", "task has no workspace path");
        };
        let Some(relative) = safe_task_relative_path(&params.path) else {
            return encode_error(
                id,
                "invalid_params",
                "file path must be repository-relative and outside .git",
            );
        };
        let target = worktree_path.join(&relative);
        let Some(parent) = target.parent() else {
            return encode_error(id, "invalid_params", "file path has no parent directory");
        };
        if let Err(err) = std::fs::create_dir_all(parent) {
            return encode_error(
                id,
                "task_file_write_failed",
                format!("parent directory could not be created: {err}"),
            );
        }
        let canonical_root = match std::fs::canonicalize(&worktree_path) {
            Ok(path) => path,
            Err(err) => {
                return encode_error(id, "task_workspace_missing", err.to_string());
            }
        };
        let canonical_parent = match std::fs::canonicalize(parent) {
            Ok(path) => path,
            Err(err) => {
                return encode_error(
                    id,
                    "task_file_write_failed",
                    format!("parent directory is not accessible: {err}"),
                );
            }
        };
        if !canonical_parent.starts_with(&canonical_root) {
            return encode_error(id, "invalid_params", "file path escapes the task workspace");
        }
        if let Ok(metadata) = std::fs::symlink_metadata(&target) {
            if metadata.file_type().is_symlink() {
                return encode_error(id, "invalid_params", "symbolic-link files cannot be edited");
            }
        }
        if let Err(err) = std::fs::write(&target, params.content.as_bytes()) {
            return encode_error(id, "task_file_write_failed", err.to_string());
        }
        encode_success(
            id,
            ResponseResult::TaskFileWritten {
                file: crate::api::schema::TaskFileWrittenInfo {
                    task_id: task.id,
                    path: relative.to_string_lossy().replace('\\', "/"),
                    bytes: params.content.len() as u64,
                },
            },
        )
    }
    pub(super) fn handle_task_git_action(
        &mut self,
        id: String,
        params: crate::api::schema::TaskGitActionParams,
    ) -> String {
        let Some(task) = self.state.tasks.find(&params.task_id).cloned() else {
            return task_not_found(id, &params.task_id);
        };
        let Some(project) = self.state.projects.find(&task.project_id).cloned() else {
            return project_not_found(id, &task.project_id);
        };
        let Some(worktree_path) = task_workspace_path(&task, &project) else {
            return encode_error(id, "task_workspace_missing", "task has no workspace path");
        };
        if !worktree_path.is_dir() {
            return encode_error(
                id,
                "task_workspace_missing",
                "task workspace is no longer available",
            );
        }
        let action = params.action;
        let (output, pull_request_url) = match action {
            crate::api::schema::TaskGitAction::Stage => {
                let mut args = vec!["add".to_owned()];
                if params.paths.is_empty() {
                    args.push("-A".to_owned());
                } else {
                    args.push("--".to_owned());
                    for path in params.paths {
                        let Some(path) = safe_task_relative_path(&path) else {
                            return encode_error(
                                id,
                                "invalid_params",
                                "Git paths must be repository-relative and outside .git",
                            );
                        };
                        args.push(path.to_string_lossy().replace('\\', "/"));
                    }
                }
                (run_task_git_owned(&worktree_path, &args), None)
            }
            crate::api::schema::TaskGitAction::Unstage => {
                let mut args = vec!["restore".to_owned(), "--staged".to_owned()];
                args.push("--".to_owned());
                if params.paths.is_empty() {
                    args.push(".".to_owned());
                } else {
                    for path in params.paths {
                        let Some(path) = safe_task_relative_path(&path) else {
                            return encode_error(
                                id,
                                "invalid_params",
                                "Git paths must be repository-relative and outside .git",
                            );
                        };
                        args.push(path.to_string_lossy().replace('\\', "/"));
                    }
                }
                (run_task_git_owned(&worktree_path, &args), None)
            }
            crate::api::schema::TaskGitAction::Commit => {
                let Some(message) = params.message.filter(|message| !message.trim().is_empty())
                else {
                    return encode_error(id, "invalid_params", "commit message must not be empty");
                };
                if message.chars().any(char::is_control) {
                    return encode_error(
                        id,
                        "invalid_params",
                        "commit message must not contain control characters",
                    );
                }
                let args = vec!["commit".to_owned(), "-m".to_owned(), message];
                (run_task_git_owned(&worktree_path, &args), None)
            }
            crate::api::schema::TaskGitAction::Push => (
                run_task_git_owned(&worktree_path, &["push".to_owned()]),
                None,
            ),
            crate::api::schema::TaskGitAction::PullRequest => {
                let branch = match task.branch.clone().or_else(|| {
                    run_task_git(&worktree_path, &["branch", "--show-current"])
                        .ok()
                        .map(|branch| branch.trim().to_owned())
                }) {
                    Some(branch) if !branch.is_empty() => branch,
                    _ => {
                        return encode_error(
                            id,
                            "task_branch_missing",
                            "task has no branch for pull request creation",
                        )
                    }
                };
                let base = params
                    .base
                    .or(project.worktree_base)
                    .unwrap_or_else(|| "main".to_owned());
                let base = base.strip_prefix("origin/").unwrap_or(&base).to_owned();
                if base.is_empty() || base.starts_with('-') || base.chars().any(char::is_control) {
                    return encode_error(id, "invalid_params", "pull request base is invalid");
                }
                let existing = match run_gh(
                    &worktree_path,
                    &[
                        "pr".to_owned(),
                        "list".to_owned(),
                        "--head".to_owned(),
                        branch.clone(),
                        "--state".to_owned(),
                        "open".to_owned(),
                        "--json".to_owned(),
                        "url".to_owned(),
                        "--limit".to_owned(),
                        "1".to_owned(),
                    ],
                ) {
                    Ok(output) => serde_json::from_str::<Vec<serde_json::Value>>(&output)
                        .ok()
                        .and_then(|rows| rows.into_iter().next())
                        .and_then(|row| {
                            row.get("url")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned)
                        }),
                    Err(message) => return encode_error(id, "task_pr_failed", message),
                };
                if let Some(url) = existing {
                    (
                        Ok("existing open pull request reused".to_owned()),
                        Some(url),
                    )
                } else {
                    let title = params
                        .title
                        .unwrap_or_else(|| task.name.clone())
                        .trim()
                        .to_owned();
                    let body = params.body.unwrap_or_default();
                    if title.is_empty()
                        || title.chars().any(char::is_control)
                        || body.chars().any(char::is_control)
                    {
                        return encode_error(
                            id,
                            "invalid_params",
                            "pull request title and body must be printable",
                        );
                    }
                    let output = match run_gh(
                        &worktree_path,
                        &[
                            "pr".to_owned(),
                            "create".to_owned(),
                            "--base".to_owned(),
                            base,
                            "--head".to_owned(),
                            branch,
                            "--title".to_owned(),
                            title,
                            "--body".to_owned(),
                            body,
                        ],
                    ) {
                        Ok(output) => output,
                        Err(message) => return encode_error(id, "task_pr_failed", message),
                    };
                    let url = output
                        .lines()
                        .find(|line| line.starts_with("http://") || line.starts_with("https://"))
                        .map(str::to_owned);
                    (Ok(output), url)
                }
            }
        };
        let output = match output {
            Ok(output) => output,
            Err(message) => return encode_error(id, "task_git_failed", message),
        };
        if let Some(url) = pull_request_url.as_deref() {
            let previous = self.state.tasks.clone();
            if let Some(task) = self.state.tasks.find_mut(&task.id) {
                task.pull_request_url = Some(url.to_owned());
                task.updated_at = crate::task::current_unix_ms();
            }
            if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
                self.state.tasks = previous;
                return encode_error(id, "task_save_failed", err.to_string());
            }
        }
        encode_success(
            id,
            ResponseResult::TaskGitAction {
                action: crate::api::schema::TaskGitActionInfo {
                    task_id: task.id,
                    action,
                    output,
                    pull_request_url,
                },
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
            environment: task.environment.clone(),
            agent_command: task.agent_command.clone(),
            agent_session: task.agent_session.as_ref().map(|session| {
                crate::api::schema::AgentSessionInfo {
                    source: session.source.clone(),
                    agent: session.agent.clone(),
                    kind: session.session_ref.kind,
                    value: session.session_ref.value.clone(),
                }
            }),
            error: task.error.clone(),
            lifecycle_runs: task
                .lifecycle_runs
                .iter()
                .map(|run| crate::api::schema::TaskLifecycleRunInfo {
                    step: run.step,
                    status: run.status,
                    started_at: run.started_at,
                    finished_at: run.finished_at,
                    output: run.output.clone(),
                    error: run.error.clone(),
                })
                .collect(),
            pull_request_url: task.pull_request_url.clone(),
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
fn task_workspace_path(
    task: &Task,
    project: &crate::project::Project,
) -> Option<std::path::PathBuf> {
    match task.location {
        TaskLocationMode::Repository => Some(project.root_path.clone()),
        TaskLocationMode::Worktree => task.worktree_path.clone(),
    }
}
fn task_environment(
    task: &Task,
    project: &crate::project::Project,
    workspace_path: &std::path::Path,
) -> BTreeMap<String, String> {
    let mut environment = task.environment.clone();
    environment.insert("HERDR_TASK_ID".to_owned(), task.id.clone());
    environment.insert("HERDR_TASK_NAME".to_owned(), task.name.clone());
    environment.insert(
        "HERDR_TASK_PATH".to_owned(),
        crate::project::display_path(workspace_path),
    );
    environment.insert("HERDR_PROJECT_ID".to_owned(), project.id.clone());
    environment.insert(
        "HERDR_PROJECT_ROOT".to_owned(),
        crate::project::display_path(&project.root_path),
    );
    if let Some(branch) = task.branch.as_deref() {
        environment.insert("HERDR_TASK_BRANCH".to_owned(), branch.to_owned());
    }
    environment
}
fn run_task_git(worktree_path: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    run_task_program("git", worktree_path, &args)
}

fn run_task_git_owned(worktree_path: &std::path::Path, args: &[String]) -> Result<String, String> {
    run_task_program("git", worktree_path, args)
}

fn run_gh(worktree_path: &std::path::Path, args: &[String]) -> Result<String, String> {
    run_task_program("gh", worktree_path, args)
}

fn run_task_program(
    program: &str,
    worktree_path: &std::path::Path,
    args: &[String],
) -> Result<String, String> {
    let git_path = crate::project::display_path(worktree_path).replace('\\', "/");
    let mut command = crate::noninteractive_process::command(program);
    if program == "git" {
        command.arg("-C").arg(&git_path);
    } else {
        command.current_dir(worktree_path);
    }
    let output = command
        .args(args)
        .output()
        .map_err(|err| format!("could not run {program}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_task_git_with_base(
    worktree_path: &std::path::Path,
    command_name: &str,
    base: &str,
) -> Result<String, String> {
    run_task_git(worktree_path, &[command_name, base, "--"])
}

fn parse_task_diff_status(status: &str) -> Vec<crate::api::schema::TaskDiffFile> {
    let mut records = status.split('\0');
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let code = record.as_bytes();
        let mut path = record[3..].to_owned();
        let old_path = if code[0] == b'R' || code[1] == b'R' || code[0] == b'C' {
            let old_path = path;
            let Some(new_path) = records.next() else {
                continue;
            };
            path = new_path.to_owned();
            Some(old_path)
        } else {
            None
        };
        let status = match (code[0], code[1]) {
            (b'?', b'?') => crate::api::schema::TaskDiffFileStatus::Untracked,
            (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D') => {
                crate::api::schema::TaskDiffFileStatus::Conflicted
            }
            (b'R', _) | (_, b'R') => crate::api::schema::TaskDiffFileStatus::Renamed,
            (b'C', _) | (_, b'C') => crate::api::schema::TaskDiffFileStatus::Copied,
            (b'A', _) | (_, b'A') => crate::api::schema::TaskDiffFileStatus::Added,
            (b'D', _) | (_, b'D') => crate::api::schema::TaskDiffFileStatus::Deleted,
            _ => crate::api::schema::TaskDiffFileStatus::Modified,
        };
        files.push(crate::api::schema::TaskDiffFile {
            path,
            old_path,
            status,
            additions: 0,
            deletions: 0,
            conflict: matches!(status, crate::api::schema::TaskDiffFileStatus::Conflicted),
        });
    }
    files
}

fn untracked_file_patch(
    worktree_path: &std::path::Path,
    relative: &str,
) -> Result<Option<String>, String> {
    let Some(path) = safe_task_relative_path(relative) else {
        return Ok(None);
    };
    let bytes = std::fs::read(worktree_path.join(path))
        .map_err(|err| format!("could not read untracked file {relative}: {err}"))?;
    if bytes.contains(&0) {
        return Ok(Some(format!(
            "diff --git a/{relative} b/{relative}\nnew file mode 100644\nBinary files /dev/null and b/{relative} differ\n"
        )));
    }
    let text = String::from_utf8_lossy(&bytes);
    let line_count = text.lines().count().max(1);
    let mut patch = format!(
        "diff --git a/{relative} b/{relative}\nnew file mode 100644\n--- /dev/null\n+++ b/{relative}\n@@ -0,0 +1,{line_count} @@\n"
    );
    for line in text.lines() {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    Ok(Some(patch))
}

fn safe_task_relative_path(raw: &str) -> Option<std::path::PathBuf> {
    let normalized = raw.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains(':')
        || normalized.split('/').any(|component| {
            component.is_empty() || component == "." || component == ".." || component == ".git"
        })
    {
        return None;
    }
    Some(std::path::PathBuf::from(
        normalized.replace('/', std::path::MAIN_SEPARATOR_STR),
    ))
}

fn lifecycle_step_label(step: TaskLifecycleStep) -> &'static str {
    match step {
        TaskLifecycleStep::Prepare => "prepare",
        TaskLifecycleStep::Setup => "setup",
        TaskLifecycleStep::Run => "run",
        TaskLifecycleStep::Teardown => "teardown",
    }
}

fn run_lifecycle_command(
    workspace_path: &std::path::Path,
    environment: &BTreeMap<String, String>,
    command_text: &str,
) -> (Option<String>, Option<String>) {
    let mut command =
        crate::noninteractive_process::command(if cfg!(windows) { "cmd" } else { "sh" });
    if cfg!(windows) {
        command.args(["/D", "/C", command_text]);
    } else {
        command.args(["-c", command_text]);
    }
    let output = command
        .current_dir(workspace_path)
        .envs(environment)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();
    let output = match output {
        Ok(output) => output,
        Err(err) => return (None, Some(format!("could not start command: {err}"))),
    };
    let text = lifecycle_output(&output.stdout, &output.stderr);
    let error = (!output.status.success()).then(|| {
        format!(
            "command exited with status {}",
            output
                .status
                .code()
                .map_or_else(|| "unknown".to_owned(), |code| code.to_string())
        )
    });
    (text, error)
}

fn lifecycle_output(stdout: &[u8], stderr: &[u8]) -> Option<String> {
    const MAX_LIFECYCLE_OUTPUT_BYTES: usize = 64 * 1024;
    let mut bytes = Vec::with_capacity(stdout.len().saturating_add(stderr.len() + 10));
    bytes.extend_from_slice(stdout);
    if !stdout.is_empty() && !stderr.is_empty() {
        bytes.extend_from_slice(b"\n");
    }
    bytes.extend_from_slice(stderr);
    let truncated = bytes.len() > MAX_LIFECYCLE_OUTPUT_BYTES;
    bytes.truncate(MAX_LIFECYCLE_OUTPUT_BYTES);
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[herdr truncated lifecycle output]");
    }
    (!text.is_empty()).then_some(text)
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
