use std::collections::BTreeMap;
use std::time::Duration;

use crate::api::schema::{
    AgentPromptParams, AgentStartParams, ResponseResult, TaskCreateParams, TaskInfo,
    TaskListParams, TaskOpenParams, TaskRenameParams, TaskResourcesParams, TaskRuntimeInfo,
    TaskTarget,
};
use crate::app::App;
use crate::events::AppEvent;
use crate::resource::Resource;
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
        let resource_ids = params.resource_ids;
        if let Err(message) = self.resolve_task_resources(
            &params.project_id,
            None,
            &resource_ids,
            provider.as_deref(),
        ) {
            return encode_error(id, "invalid_params", message);
        }
        let prompt = params.prompt.filter(|prompt| !prompt.is_empty());
        let previous = self.state.tasks.clone();
        let task = Task::new(
            params.project_id,
            name.to_owned(),
            params.location,
            clean_optional(params.branch),
            worktree_path,
            provider,
            clean_optional(params.model),
            prompt,
            environment,
            resource_ids,
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
    fn resolve_task_resources(
        &self,
        project_id: &str,
        task_id: Option<&str>,
        resource_ids: &[String],
        provider: Option<&str>,
    ) -> Result<Vec<Resource>, String> {
        let mut resources = Vec::new();
        for resource_id in resource_ids {
            let Some(resource) = self.state.resources.find(resource_id).cloned() else {
                return Err(format!("resource {resource_id} does not exist"));
            };
            if !resource.enabled {
                return Err(format!("resource {resource_id} is disabled"));
            }
            match resource.scope {
                crate::resource::ResourceScope::Global => {}
                crate::resource::ResourceScope::Project => {
                    if resource.project_id.as_deref() != Some(project_id) {
                        return Err(format!("resource {resource_id} belongs to another project"));
                    }
                }
                crate::resource::ResourceScope::Task => {
                    if resource.task_id.as_deref() != task_id {
                        return Err(format!("resource {resource_id} belongs to another task"));
                    }
                }
            }
            if let Some(resource_provider) = resource.provider.as_deref() {
                if provider != Some(resource_provider) {
                    return Err(format!(
                        "resource {} requires provider {resource_provider}",
                        resource.id
                    ));
                }
            }
            resources.push(resource);
        }
        Ok(resources)
    }
    fn task_prompt(&self, task: &Task) -> Result<String, String> {
        let resources = self.resolve_task_resources(
            &task.project_id,
            Some(&task.id),
            &task.resource_ids,
            task.provider.as_deref(),
        )?;
        Ok(inject_resource_prompt(
            task.prompt.clone().filter(|prompt| !prompt.is_empty()),
            &resources,
        )
        .unwrap_or_else(|| task.name.clone()))
    }

    pub(crate) fn start_task_agent(&mut self, task_id: &str) -> Result<(), String> {
        let Some(task) = self.state.tasks.find(task_id).cloned() else {
            return Err(format!("task {task_id} no longer exists"));
        };
        let Some(provider) = task.provider.clone() else {
            return self.transition_task(
                task_id,
                TaskStatus::ReviewReady,
                Some("no agent provider configured".to_owned()),
            );
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
        self.transition_task(
            task_id,
            TaskStatus::Working,
            Some("agent started".to_owned()),
        )?;
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
        if !project.lifecycle.is_empty() {
            self.transition_task(
                task_id,
                TaskStatus::Provisioning,
                Some("lifecycle provisioning".to_owned()),
            )?;
        }
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
        task.current_step = Some(lifecycle_step_label(step).to_owned());
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
        task.current_step = None;
        task.updated_at = finished_at;
        if let Some(error) = error {
            task.error = Some(format!(
                "lifecycle {} failed: {error}",
                lifecycle_step_label(step)
            ));
            if task.status != TaskStatus::Failed {
                task.status = TaskStatus::Failed;
                task.history.push(crate::task::TaskHistoryEntry {
                    status: TaskStatus::Failed,
                    at: finished_at,
                    reason: Some(format!("lifecycle {} failed", lifecycle_step_label(step))),
                });
            }
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
        let prompt = match self.task_prompt(&task) {
            Ok(prompt) => prompt,
            Err(message) => {
                self.record_task_error(&task_id, message);
                return;
            }
        };
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

    fn transition_task(
        &mut self,
        task_id: &str,
        status: TaskStatus,
        reason: Option<String>,
    ) -> Result<(), String> {
        let previous = self.state.tasks.clone();
        let Some(task) = self.state.tasks.find_mut(task_id) else {
            return Err(format!("task {task_id} no longer exists"));
        };
        if task.status == status {
            return Ok(());
        }
        let now = crate::task::current_unix_ms();
        task.status = status;
        task.updated_at = now;
        task.history.push(crate::task::TaskHistoryEntry {
            status,
            at: now,
            reason,
        });
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return Err(format!("task status could not be saved: {err}"));
        }
        Ok(())
    }

    fn record_task_error(&mut self, task_id: &str, message: String) {
        let Some(task) = self.state.tasks.find_mut(task_id) else {
            return;
        };
        let now = crate::task::current_unix_ms();
        task.error = Some(message.clone());
        task.updated_at = now;
        if task.status != TaskStatus::Failed {
            task.status = TaskStatus::Failed;
            task.history.push(crate::task::TaskHistoryEntry {
                status: TaskStatus::Failed,
                at: now,
                reason: Some(message),
            });
        }
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            tracing::error!(task_id, error = %err, "failed to persist task error");
        }
    }

    pub(super) fn record_task_agent_status(
        &mut self,
        pane_id: crate::layout::PaneId,
        state: crate::detect::AgentState,
    ) {
        let Some((workspace_index, _)) = self.find_pane(pane_id) else {
            return;
        };
        let Some(public_pane_id) = self.public_pane_id(workspace_index, pane_id) else {
            return;
        };
        let (agent_status, task_status) = match state {
            crate::detect::AgentState::Idle => {
                (crate::task::TaskAgentStatus::Idle, TaskStatus::ReviewReady)
            }
            crate::detect::AgentState::Working => {
                (crate::task::TaskAgentStatus::Working, TaskStatus::Working)
            }
            crate::detect::AgentState::Blocked => {
                (crate::task::TaskAgentStatus::Blocked, TaskStatus::Blocked)
            }
            crate::detect::AgentState::Unknown => {
                (crate::task::TaskAgentStatus::Unknown, TaskStatus::Queued)
            }
        };
        let now = crate::task::current_unix_ms();
        let mut changed = false;
        for task in &mut self.state.tasks.tasks {
            if task.pane_id.as_deref() != Some(public_pane_id.as_str())
                || task.status == TaskStatus::Closed
            {
                continue;
            }
            let mut task_changed = false;
            if task.agent_status != Some(agent_status) {
                task.agent_status = Some(agent_status);
                task_changed = true;
            }
            if state != crate::detect::AgentState::Unknown && task.status != task_status {
                task.status = task_status;
                task.history.push(crate::task::TaskHistoryEntry {
                    status: task_status,
                    at: now,
                    reason: Some(format!("agent state: {state:?}")),
                });
                task_changed = true;
            }
            if task_changed {
                task.updated_at = now;
                changed = true;
            }
        }
        if changed {
            if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
                tracing::error!(error = %err, "failed to persist task agent status");
            }
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

    pub(super) fn handle_task_resources(
        &mut self,
        id: String,
        params: TaskResourcesParams,
    ) -> String {
        let Some(task) = self.state.tasks.find(&params.task_id).cloned() else {
            return task_not_found(id, &params.task_id);
        };
        if let Err(message) = self.resolve_task_resources(
            &task.project_id,
            Some(&task.id),
            &params.resource_ids,
            task.provider.as_deref(),
        ) {
            return encode_error(id, "invalid_params", message);
        }
        let previous = self.state.tasks.clone();
        let Some(stored) = self.state.tasks.find_mut(&task.id) else {
            return task_not_found(id, &task.id);
        };
        stored.resource_ids = params.resource_ids;
        stored.updated_at = crate::task::current_unix_ms();
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return encode_error(id, "task_save_failed", err.to_string());
        }
        let Some(task) = self.state.tasks.find(&task.id) else {
            return task_not_found(id, &task.id);
        };
        encode_success(
            id,
            ResponseResult::TaskResourcesUpdated {
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
            task.current_step = None;
            task.updated_at = now;
            task.closed_at = Some(now);
            task.history.push(crate::task::TaskHistoryEntry {
                status: TaskStatus::Closed,
                at: now,
                reason: Some("task closed".to_owned()),
            });
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
    pub(super) fn handle_github_issue_search(
        &mut self,
        id: String,
        params: crate::api::schema::GitHubIssueSearchParams,
    ) -> String {
        let repository = params.repository.trim();
        if repository.is_empty() || repository.chars().any(char::is_control) {
            return encode_error(id, "invalid_params", "GitHub repository is required");
        }
        let mut args = vec![
            "issue".to_owned(),
            "list".to_owned(),
            "--repo".to_owned(),
            repository.to_owned(),
            "--state".to_owned(),
            "all".to_owned(),
            "--limit".to_owned(),
            params.limit.unwrap_or(50).clamp(1, 100).to_string(),
            "--json".to_owned(),
            "number,title,url,state,labels,assignees".to_owned(),
        ];
        if let Some(query) = params.query.filter(|query| !query.trim().is_empty()) {
            if query.chars().any(char::is_control) {
                return encode_error(id, "invalid_params", "GitHub search text is invalid");
            }
            args.extend(["--search".to_owned(), query]);
        }
        let current_dir = match std::env::current_dir() {
            Ok(path) => path,
            Err(err) => {
                return encode_error(id, "github_auth_failed", err.to_string());
            }
        };
        let output = match run_gh(&current_dir, &args) {
            Ok(output) => output,
            Err(message) => return encode_error(id, "github_auth_failed", message),
        };
        let values = match serde_json::from_str::<Vec<serde_json::Value>>(&output) {
            Ok(values) => values,
            Err(err) => {
                return encode_error(
                    id,
                    "github_response_invalid",
                    format!("GitHub returned invalid issue data: {err}"),
                )
            }
        };
        let issues = values
            .iter()
            .filter_map(|value| github_issue_from_value(repository, value).ok())
            .collect();
        encode_success(id, ResponseResult::GitHubIssueList { issues })
    }

    pub(super) fn handle_github_issue_create(
        &mut self,
        id: String,
        params: crate::api::schema::GitHubIssueTaskCreateParams,
    ) -> String {
        let repository = params.repository.trim();
        if repository.is_empty() || repository.chars().any(char::is_control) || params.number == 0 {
            return encode_error(
                id,
                "invalid_params",
                "GitHub repository and issue number are required",
            );
        }
        let Some(project) = self.state.projects.find(&params.project_id).cloned() else {
            return project_not_found(id, &params.project_id);
        };
        let current_dir = match std::env::current_dir() {
            Ok(path) => path,
            Err(err) => return encode_error(id, "github_auth_failed", err.to_string()),
        };
        let output = match run_gh(
            &current_dir,
            &[
                "issue".to_owned(),
                "view".to_owned(),
                params.number.to_string(),
                "--repo".to_owned(),
                repository.to_owned(),
                "--json".to_owned(),
                "number,title,body,url,state,labels,assignees".to_owned(),
            ],
        ) {
            Ok(output) => output,
            Err(message) => return encode_error(id, "github_auth_failed", message),
        };
        let value = match serde_json::from_str::<serde_json::Value>(&output) {
            Ok(value) => value,
            Err(err) => {
                return encode_error(
                    id,
                    "github_response_invalid",
                    format!("GitHub returned invalid issue data: {err}"),
                )
            }
        };
        let issue = match github_issue_from_value(repository, &value) {
            Ok(issue) => issue,
            Err(message) => return encode_error(id, "github_response_invalid", message),
        };
        let worktree_path = match params.location {
            TaskLocationMode::Repository => None,
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
                        return encode_error(id, "invalid_params", err.to_string());
                    }
                };
                Some(path)
            }
        };
        let provider = match project.default_agent.clone() {
            Some(provider) => match crate::project::normalize_agent_provider(&provider) {
                Ok(provider) => Some(provider),
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => None,
        };
        let context = format!(
            "GitHub Issue #{}: {}\n\n{}\n\nURL: {}\nState: {}\nLabels: {}\nAssignees: {}",
            issue.number,
            issue.title,
            issue.body,
            issue.url,
            issue.state,
            issue.labels.join(", "),
            issue.assignees.join(", "),
        );
        let prompt = params
            .prompt
            .filter(|prompt| !prompt.trim().is_empty())
            .map_or_else(
                || context.clone(),
                |prompt| format!("{context}\n\n{prompt}"),
            );
        let task = Task::new(
            params.project_id,
            issue.title.clone(),
            params.location,
            clean_optional(params.branch),
            worktree_path,
            provider,
            None,
            Some(prompt),
            project.environment.clone(),
            Vec::new(),
            None,
            None,
            None,
        );
        let issue_context = crate::task::GitHubIssueContext {
            repository: issue.repository.clone(),
            number: issue.number,
            title: issue.title.clone(),
            body: issue.body.clone(),
            url: issue.url.clone(),
            labels: issue.labels.clone(),
            assignees: issue.assignees.clone(),
            state: issue.state.clone(),
        };
        let mut task = task;
        task.github_issue = Some(issue_context);
        let previous = self.state.tasks.clone();
        self.state.tasks.insert(task.clone());
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return encode_error(id, "task_save_failed", err.to_string());
        }
        let task_id = task.id.clone();
        if task.location == TaskLocationMode::Worktree {
            if let Err(err) = self.run_task_lifecycle(&task_id) {
                tracing::warn!(task_id, error = %err, "GitHub issue task lifecycle failed");
            } else if let Err(err) = self.transition_task(
                &task_id,
                TaskStatus::ReviewReady,
                Some("GitHub issue task ready".to_owned()),
            ) {
                tracing::warn!(task_id, error = %err, "GitHub issue task status update failed");
            }
        } else if let Err(err) = self.transition_task(
            &task_id,
            TaskStatus::ReviewReady,
            Some("GitHub issue task ready".to_owned()),
        ) {
            tracing::warn!(task_id, error = %err, "GitHub issue task status update failed");
        }
        let task = self
            .state
            .tasks
            .find(&task_id)
            .expect("saved GitHub issue task should remain available");
        encode_success(
            id,
            ResponseResult::GitHubIssueTaskCreated {
                task: self.task_info(task),
                issue,
            },
        )
    }
    pub(super) fn handle_external_tracker_configure(
        &mut self,
        id: String,
        params: crate::api::schema::ExternalTrackerConfigureParams,
    ) -> String {
        let base_url = params.base_url.trim().trim_end_matches('/').to_owned();
        let credential_env = params.credential_env.trim().to_owned();
        if !(base_url.starts_with("https://") || base_url.starts_with("http://"))
            || base_url.chars().any(char::is_control)
        {
            return encode_error(
                id,
                "invalid_params",
                "tracker base URL must use HTTP or HTTPS",
            );
        }
        if !valid_environment_name(&credential_env) || credential_env.starts_with("HERDR_") {
            return encode_error(
                id,
                "invalid_params",
                "tracker credential_env must be a non-Herdr environment variable name",
            );
        }
        let config = crate::api::schema::ExternalTrackerConfig {
            provider: params.provider,
            enabled: params.enabled,
            base_url,
            credential_env,
        };
        let previous = self.state.projects.clone();
        let Some(project) = self.state.projects.find_mut(&params.project_id) else {
            return project_not_found(id, &params.project_id);
        };
        if let Some(existing) = project
            .external_trackers
            .iter_mut()
            .find(|existing| existing.provider == config.provider)
        {
            *existing = config;
        } else {
            project.external_trackers.push(config);
        }
        if let Err(err) = crate::persist::save_projects(&self.state.projects) {
            self.state.projects = previous;
            return encode_error(id, "project_save_failed", err.to_string());
        }
        let project = self
            .state
            .projects
            .find(&params.project_id)
            .expect("configured project should remain available");
        encode_success(
            id,
            ResponseResult::ExternalTrackerConfigured {
                project: self.project_info(project),
            },
        )
    }

    pub(super) fn handle_external_issue_search(
        &mut self,
        id: String,
        params: crate::api::schema::ExternalIssueSearchParams,
    ) -> String {
        let Some(project) = self.state.projects.find(&params.project_id).cloned() else {
            return project_not_found(id, &params.project_id);
        };
        let Some(config) = project
            .external_trackers
            .iter()
            .find(|config| config.provider == params.provider && config.enabled)
        else {
            return encode_error(
                id,
                "external_tracker_disabled",
                "tracker is not enabled for this project",
            );
        };
        let Ok(token) = std::env::var(&config.credential_env) else {
            return encode_error(
                id,
                "external_auth_missing",
                format!(
                    "set {} before using the {} tracker",
                    config.credential_env,
                    external_provider_label(config.provider)
                ),
            );
        };
        let mut url = format!("{}/issues", config.base_url);
        if let Some(query) = params.query.filter(|query| !query.trim().is_empty()) {
            if query.chars().any(char::is_control) {
                return encode_error(id, "invalid_params", "tracker search text is invalid");
            }
            url.push_str("?query=");
            url.push_str(&percent_encode(&query));
        }
        let output = match run_external_request(config.provider, &url, &token) {
            Ok(output) => output,
            Err(message) => return encode_error(id, "external_tracker_failed", message),
        };
        let value = match serde_json::from_str::<serde_json::Value>(&output) {
            Ok(value) => value,
            Err(err) => {
                return encode_error(
                    id,
                    "external_response_invalid",
                    format!("tracker returned invalid JSON: {err}"),
                )
            }
        };
        let issues = external_issue_values(&value)
            .into_iter()
            .take(params.limit.unwrap_or(50).clamp(1, 100) as usize)
            .filter_map(|value| external_issue_from_value(params.provider, &value, &url).ok())
            .collect();
        encode_success(id, ResponseResult::ExternalIssueList { issues })
    }

    pub(super) fn handle_external_issue_task_create(
        &mut self,
        id: String,
        params: crate::api::schema::ExternalIssueTaskCreateParams,
    ) -> String {
        let Some(project) = self.state.projects.find(&params.project_id).cloned() else {
            return project_not_found(id, &params.project_id);
        };
        if !project
            .external_trackers
            .iter()
            .any(|config| config.provider == params.issue.provider && config.enabled)
        {
            return encode_error(
                id,
                "external_tracker_disabled",
                "tracker is not enabled for this project",
            );
        }
        let worktree_path = match params.location {
            TaskLocationMode::Repository => None,
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
                            "worktree path is not a directory",
                        )
                    }
                    Err(err) => return encode_error(id, "invalid_params", err.to_string()),
                };
                Some(path)
            }
        };
        let context = format!(
            "{} {}: {}\n\n{}\n\nURL: {}\nState: {}\nLabels: {}\nAssignees: {}",
            external_provider_label(params.issue.provider),
            params.issue.identifier,
            params.issue.title,
            params.issue.body,
            params.issue.url,
            params.issue.state,
            params.issue.labels.join(", "),
            params.issue.assignees.join(", "),
        );
        let prompt = params
            .prompt
            .filter(|prompt| !prompt.trim().is_empty())
            .map_or_else(
                || context.clone(),
                |prompt| format!("{context}\n\n{prompt}"),
            );
        let mut task = Task::new(
            params.project_id,
            params.issue.title.clone(),
            params.location,
            clean_optional(params.branch),
            worktree_path,
            project.default_agent.clone(),
            None,
            Some(prompt),
            project.environment.clone(),
            Vec::new(),
            None,
            None,
            None,
        );
        task.external_issue = Some(params.issue.clone());
        let previous = self.state.tasks.clone();
        self.state.tasks.insert(task.clone());
        if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
            self.state.tasks = previous;
            return encode_error(id, "task_save_failed", err.to_string());
        }
        let task_id = task.id.clone();
        if task.location == TaskLocationMode::Worktree {
            if let Err(err) = self.run_task_lifecycle(&task_id) {
                tracing::warn!(task_id, error = %err, "external issue task lifecycle failed");
            }
        }
        let task = self
            .state
            .tasks
            .find(&task_id)
            .expect("saved external task should remain available");
        encode_success(
            id,
            ResponseResult::ExternalIssueTaskCreated {
                task: self.task_info(task),
                issue: params.issue,
            },
        )
    }

    pub(super) fn handle_task_checks(
        &mut self,
        id: String,
        params: crate::api::schema::TaskChecksParams,
    ) -> String {
        let Some(task) = self.state.tasks.find(&params.task_id).cloned() else {
            return task_not_found(id, &params.task_id);
        };
        let Some(pull_request_url) = task.pull_request_url.clone() else {
            return encode_success(
                id,
                ResponseResult::TaskChecks {
                    checks: crate::api::schema::TaskChecksInfo {
                        task_id: task.id,
                        pull_request_url: None,
                        head_sha: None,
                        checks: Vec::new(),
                        message: Some("task has no pull request".to_owned()),
                        refreshed_at: crate::task::current_unix_ms(),
                    },
                },
            );
        };
        let Some(project) = self.state.projects.find(&task.project_id).cloned() else {
            return project_not_found(id, &task.project_id);
        };
        let Some(worktree_path) = task_workspace_path(&task, &project) else {
            return encode_error(id, "task_workspace_missing", "task has no workspace path");
        };
        let head_output = match run_gh(
            &worktree_path,
            &[
                "pr".to_owned(),
                "view".to_owned(),
                pull_request_url.clone(),
                "--json".to_owned(),
                "headRefOid".to_owned(),
            ],
        ) {
            Ok(output) => output,
            Err(message) => return encode_error(id, "github_checks_failed", message),
        };
        let head_sha = serde_json::from_str::<serde_json::Value>(&head_output)
            .ok()
            .and_then(|value| {
                value
                    .get("headRefOid")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            });
        let checks_output = match run_gh(
            &worktree_path,
            &[
                "pr".to_owned(),
                "checks".to_owned(),
                pull_request_url.clone(),
                "--json".to_owned(),
                "name,state,link,workflow,startedAt,completedAt".to_owned(),
            ],
        ) {
            Ok(output) => output,
            Err(message) => return encode_error(id, "github_checks_failed", message),
        };
        let values = match serde_json::from_str::<Vec<serde_json::Value>>(&checks_output) {
            Ok(values) => values,
            Err(err) => {
                return encode_error(
                    id,
                    "github_response_invalid",
                    format!("GitHub returned invalid check data: {err}"),
                )
            }
        };
        let checks = values
            .into_iter()
            .map(|value| crate::api::schema::TaskCheckInfo {
                name: value
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unnamed check")
                    .to_owned(),
                state: value
                    .get("state")
                    .or_else(|| value.get("bucket"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("UNKNOWN")
                    .to_owned(),
                source: value
                    .get("workflow")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("GitHub Actions")
                    .to_owned(),
                link: value
                    .get("link")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                started_at: value
                    .get("startedAt")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                completed_at: value
                    .get("completedAt")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            })
            .collect();
        encode_success(
            id,
            ResponseResult::TaskChecks {
                checks: crate::api::schema::TaskChecksInfo {
                    task_id: task.id,
                    pull_request_url: Some(pull_request_url),
                    head_sha,
                    checks,
                    message: None,
                    refreshed_at: crate::task::current_unix_ms(),
                },
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

    pub(crate) fn start_task_with_runtime(&mut self, task_id: &str) -> Result<(), String> {
        let Some(task) = self.state.tasks.find(task_id).cloned() else {
            return Err(format!("task {task_id} no longer exists"));
        };
        if !self.task_runtime_info(&task).available {
            let Some(runtime) = self.open_task_target(&task, false) else {
                return Err("task workspace could not be opened".to_owned());
            };
            let previous = self.state.tasks.clone();
            let Some(stored) = self.state.tasks.find_mut(task_id) else {
                return Err(format!("task {task_id} no longer exists"));
            };
            stored.workspace_id = runtime.workspace_id;
            stored.tab_id = runtime.tab_id;
            stored.pane_id = runtime.pane_id;
            stored.updated_at = crate::task::current_unix_ms();
            if let Err(err) = crate::persist::save_tasks(&self.state.tasks) {
                self.state.tasks = previous;
                return Err(format!("task runtime could not be saved: {err}"));
            }
        }
        self.start_task_agent(task_id)
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
            resource_ids: task.resource_ids.clone(),
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
            github_issue: task.github_issue.as_ref().map(|issue| {
                crate::api::schema::GitHubIssueInfo {
                    repository: issue.repository.clone(),
                    number: issue.number,
                    title: issue.title.clone(),
                    body: issue.body.clone(),
                    url: issue.url.clone(),
                    labels: issue.labels.clone(),
                    assignees: issue.assignees.clone(),
                    state: issue.state.clone(),
                }
            }),
            external_issue: task.external_issue.clone(),
            status: task.status,
            current_step: task.current_step.clone(),
            agent_status: task.agent_status,
            history: task
                .history
                .iter()
                .map(|entry| crate::api::schema::TaskHistoryInfo {
                    status: entry.status,
                    at: entry.at,
                    reason: entry.reason.clone(),
                })
                .collect(),
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
fn inject_resource_prompt(prompt: Option<String>, resources: &[Resource]) -> Option<String> {
    let mut output = String::new();
    for resource in resources {
        if resource.content.is_empty() {
            continue;
        }
        let kind = match resource.kind {
            crate::resource::ResourceKind::Prompt => "prompt",
            crate::resource::ResourceKind::Skill => "skill",
            crate::resource::ResourceKind::Mcp => "mcp",
        };
        output.push_str(&format!(
            "[Herdr {kind}: {}]\n{}\n\n",
            resource.name, resource.content
        ));
    }
    if let Some(prompt) = prompt {
        output.push_str(&prompt);
    }
    (!output.is_empty()).then_some(output)
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
fn valid_environment_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn external_provider_label(provider: crate::api::schema::ExternalTrackerProvider) -> &'static str {
    match provider {
        crate::api::schema::ExternalTrackerProvider::Linear => "Linear",
        crate::api::schema::ExternalTrackerProvider::Jira => "Jira",
        crate::api::schema::ExternalTrackerProvider::Gitlab => "GitLab",
        crate::api::schema::ExternalTrackerProvider::Asana => "Asana",
        crate::api::schema::ExternalTrackerProvider::Plane => "Plane",
        crate::api::schema::ExternalTrackerProvider::Notion => "Notion",
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                vec![byte as char]
            } else {
                let hex = b"0123456789ABCDEF";
                vec![
                    '%',
                    hex[(byte >> 4) as usize] as char,
                    hex[(byte & 0x0f) as usize] as char,
                ]
            }
        })
        .collect()
}

fn run_external_request(
    provider: crate::api::schema::ExternalTrackerProvider,
    url: &str,
    token: &str,
) -> Result<String, String> {
    let mut command = crate::noninteractive_process::command("curl");
    command.args([
        "--fail-with-body",
        "--silent",
        "--show-error",
        "--location",
        "--max-time",
        "30",
        "--header",
        &format!("Authorization: Bearer {token}"),
    ]);
    if provider == crate::api::schema::ExternalTrackerProvider::Notion {
        command.arg("--header").arg("Notion-Version: 2022-06-28");
    }
    let output = command
        .arg(url)
        .output()
        .map_err(|err| format!("could not run curl: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "external tracker request failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn external_issue_values(value: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(values) = value.as_array() {
        return values.clone();
    }
    for key in ["issues", "data", "nodes", "results", "workItems"] {
        if let Some(values) = value.get(key).and_then(serde_json::Value::as_array) {
            return values.clone();
        }
    }
    Vec::new()
}

fn external_issue_from_value(
    provider: crate::api::schema::ExternalTrackerProvider,
    value: &serde_json::Value,
    base_url: &str,
) -> Result<crate::api::schema::ExternalIssueInfo, String> {
    let string_field = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
            .unwrap_or_default()
            .to_owned()
    };
    let identifier = string_field(&["identifier", "key", "id"]);
    let identifier = if identifier.is_empty() {
        value
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .map_or_else(String::new, |number| number.to_string())
    } else {
        identifier
    };
    let title = string_field(&["title", "name", "summary"]);
    if identifier.is_empty() || title.is_empty() {
        return Err("tracker issue is missing an identifier or title".to_owned());
    }
    let labels = value
        .get("labels")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| {
                    value
                        .as_str()
                        .or_else(|| value.get("name").and_then(serde_json::Value::as_str))
                })
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let assignees = value
        .get("assignees")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| {
                    value
                        .as_str()
                        .or_else(|| value.get("name").and_then(serde_json::Value::as_str))
                        .or_else(|| value.get("login").and_then(serde_json::Value::as_str))
                })
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let url = {
        let url = string_field(&["url", "web_url", "html_url"]);
        if url.is_empty() {
            format!("{base_url}/{}", identifier)
        } else {
            url
        }
    };
    Ok(crate::api::schema::ExternalIssueInfo {
        provider,
        identifier,
        title,
        body: string_field(&["body", "description"]),
        url,
        state: string_field(&["state", "status", "stateName"]),
        labels,
        assignees,
        metadata: BTreeMap::new(),
    })
}

fn github_issue_from_value(
    repository: &str,
    value: &serde_json::Value,
) -> Result<crate::api::schema::GitHubIssueInfo, String> {
    let number = value
        .get("number")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "GitHub issue response has no number".to_owned())?;
    let title = value
        .get("title")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "GitHub issue response has no title".to_owned())?
        .to_owned();
    let body = value
        .get("body")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let url = value
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "GitHub issue response has no URL".to_owned())?
        .to_owned();
    let state = value
        .get("state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("UNKNOWN")
        .to_owned();
    let labels = value
        .get("labels")
        .and_then(serde_json::Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.get("name").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let assignees = value
        .get("assignees")
        .and_then(serde_json::Value::as_array)
        .map(|assignees| {
            assignees
                .iter()
                .filter_map(|assignee| assignee.get("login").and_then(serde_json::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(crate::api::schema::GitHubIssueInfo {
        repository: repository.to_owned(),
        number,
        title,
        body,
        url,
        labels,
        assignees,
        state,
    })
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
