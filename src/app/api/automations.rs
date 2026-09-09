use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::schema::{
    AutomationCreateParams, AutomationInfo, AutomationListParams, AutomationRunInfo,
    AutomationRunNowParams, AutomationTarget, AutomationUpdateParams, ResponseResult,
};
use crate::app::App;
use crate::automation::{Automation, AutomationRun, AutomationRunStatus};
use crate::task::{Task, TaskLocationMode};

use super::responses::{encode_error, encode_success};

impl App {
    fn automation_info(&self, automation: &Automation) -> AutomationInfo {
        AutomationInfo {
            automation_id: automation.id.clone(),
            name: automation.name.clone(),
            project_id: automation.project_id.clone(),
            cron: automation.cron.clone(),
            prompt: automation.prompt.clone(),
            provider: automation.provider.clone(),
            model: automation.model.clone(),
            workspace_mode: automation.workspace_mode,
            enabled: automation.enabled,
            paused: automation.paused,
            last_scheduled_slot: automation.last_scheduled_slot,
            runs: automation
                .runs
                .iter()
                .map(|run| AutomationRunInfo {
                    run_id: run.run_id.clone(),
                    scheduled_slot: run.scheduled_slot,
                    status: run.status,
                    started_at: run.started_at,
                    finished_at: run.finished_at,
                    task_id: run.task_id.clone(),
                    error: run.error.clone(),
                })
                .collect(),
            created_at: automation.created_at,
            updated_at: automation.updated_at,
        }
    }

    pub(super) fn handle_automation_create(
        &mut self,
        id: String,
        params: AutomationCreateParams,
    ) -> String {
        let name = params.name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return encode_error(id, "invalid_params", "automation name must be printable");
        }
        let prompt = params.prompt.trim();
        if prompt.is_empty() || prompt.chars().any(char::is_control) {
            return encode_error(id, "invalid_params", "automation prompt must be printable");
        }
        let cron = params.cron.trim();
        if let Err(message) = validate_cron(cron) {
            return encode_error(id, "invalid_params", message);
        }
        if self.state.projects.find(&params.project_id).is_none() {
            return encode_error(id, "project_not_found", "automation project does not exist");
        }
        let provider = match params.provider {
            Some(provider) => match crate::project::normalize_agent_provider(&provider) {
                Ok(provider) => Some(provider),
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => None,
        };
        let mut automation = Automation::new(
            name.to_owned(),
            params.project_id,
            cron.to_owned(),
            prompt.to_owned(),
            provider,
            clean_optional(params.model),
            params.workspace_mode,
        );
        automation.enabled = params.enabled;
        automation.paused = params.paused;
        let previous = self.state.automations.clone();
        self.state.automations.insert(automation.clone());
        if let Err(err) = crate::persist::save_automations(&self.state.automations) {
            self.state.automations = previous;
            return encode_error(id, "automation_save_failed", err.to_string());
        }
        encode_success(
            id,
            ResponseResult::AutomationInfo {
                automation: self.automation_info(&automation),
            },
        )
    }

    pub(super) fn handle_automation_list(
        &mut self,
        id: String,
        params: AutomationListParams,
    ) -> String {
        if let Some(project_id) = params.project_id.as_deref() {
            if self.state.projects.find(project_id).is_none() {
                return encode_error(id, "project_not_found", "automation project does not exist");
            }
        }
        let automations = self
            .state
            .automations
            .automations
            .iter()
            .filter(|automation| params.include_disabled || automation.enabled)
            .filter(|automation| {
                params
                    .project_id
                    .as_deref()
                    .is_none_or(|project_id| automation.project_id == project_id)
            })
            .map(|automation| self.automation_info(automation))
            .collect();
        encode_success(id, ResponseResult::AutomationList { automations })
    }

    pub(super) fn handle_automation_update(
        &mut self,
        id: String,
        params: AutomationUpdateParams,
    ) -> String {
        let Some(existing) = self.state.automations.find(&params.automation_id).cloned() else {
            return automation_not_found(id, &params.automation_id);
        };
        let mut updated = existing.clone();
        if let Some(name) = params.name {
            let name = name.trim();
            if name.is_empty() || name.chars().any(char::is_control) {
                return encode_error(id, "invalid_params", "automation name must be printable");
            }
            updated.name = name.to_owned();
        }
        if let Some(cron) = params.cron {
            let cron = cron.trim();
            if let Err(message) = validate_cron(cron) {
                return encode_error(id, "invalid_params", message);
            }
            updated.cron = cron.to_owned();
        }
        if let Some(prompt) = params.prompt {
            let prompt = prompt.trim();
            if prompt.is_empty() || prompt.chars().any(char::is_control) {
                return encode_error(id, "invalid_params", "automation prompt must be printable");
            }
            updated.prompt = prompt.to_owned();
        }
        if let Some(provider) = params.provider {
            updated.provider = match crate::project::normalize_agent_provider(&provider) {
                Ok(provider) => Some(provider),
                Err(message) => return encode_error(id, "invalid_params", message),
            };
        }
        if let Some(model) = params.model {
            updated.model = clean_optional(Some(model));
        }
        if let Some(workspace_mode) = params.workspace_mode {
            updated.workspace_mode = workspace_mode;
        }
        if let Some(enabled) = params.enabled {
            updated.enabled = enabled;
        }
        if let Some(paused) = params.paused {
            updated.paused = paused;
        }
        updated.updated_at = current_unix_ms();
        let previous = self.state.automations.clone();
        let Some(stored) = self.state.automations.find_mut(&updated.id) else {
            return automation_not_found(id, &updated.id);
        };
        *stored = updated.clone();
        if let Err(err) = crate::persist::save_automations(&self.state.automations) {
            self.state.automations = previous;
            return encode_error(id, "automation_save_failed", err.to_string());
        }
        encode_success(
            id,
            ResponseResult::AutomationInfo {
                automation: self.automation_info(&updated),
            },
        )
    }

    pub(super) fn handle_automation_delete(
        &mut self,
        id: String,
        target: AutomationTarget,
    ) -> String {
        let previous = self.state.automations.clone();
        if self
            .state
            .automations
            .remove(&target.automation_id)
            .is_none()
        {
            return automation_not_found(id, &target.automation_id);
        }
        if let Err(err) = crate::persist::save_automations(&self.state.automations) {
            self.state.automations = previous;
            return encode_error(id, "automation_save_failed", err.to_string());
        }
        encode_success(
            id,
            ResponseResult::AutomationDeleted {
                automation_id: target.automation_id,
            },
        )
    }

    pub(super) fn handle_automation_run_now(
        &mut self,
        id: String,
        params: AutomationRunNowParams,
    ) -> String {
        let Some(automation) = self.state.automations.find(&params.automation_id) else {
            return automation_not_found(id, &params.automation_id);
        };
        let scheduled_slot = crate::automation::current_cron_slot(SystemTime::now());
        let run_id = if !automation.enabled || automation.paused {
            self.record_automation_skip(&params.automation_id, scheduled_slot)
        } else {
            self.execute_automation(&params.automation_id, scheduled_slot)
        };
        let Some(automation) = self.state.automations.find(&params.automation_id) else {
            return automation_not_found(id, &params.automation_id);
        };
        let Some(run) = automation.runs.iter().find(|run| run.run_id == run_id) else {
            return encode_error(
                id,
                "automation_run_failed",
                "automation run was not recorded",
            );
        };
        encode_success(
            id,
            ResponseResult::AutomationRunStarted {
                automation: self.automation_info(automation),
                run: self.automation_run_info(run),
            },
        )
    }

    pub(crate) fn run_due_automations(&mut self, now: SystemTime) -> bool {
        let slot = crate::automation::current_cron_slot(now);
        let due = self
            .state
            .automations
            .automations
            .iter()
            .filter(|automation| automation.enabled && !automation.paused)
            .filter(|automation| automation.last_scheduled_slot != Some(slot))
            .filter_map(|automation| match crate::automation::cron_matches(&automation.cron, slot) {
                Ok(true) => Some(automation.id.clone()),
                Ok(false) => None,
                Err(message) => {
                    tracing::warn!(automation_id = %automation.id, error = %message, "invalid persisted automation cron");
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        for automation_id in due {
            if self.claim_automation_slot(&automation_id, slot) {
                self.execute_automation(&automation_id, slot);
                changed = true;
            }
        }
        changed
    }

    fn claim_automation_slot(&mut self, automation_id: &str, slot: u64) -> bool {
        let Some(automation) = self.state.automations.find_mut(automation_id) else {
            return false;
        };
        if automation.last_scheduled_slot == Some(slot) {
            return false;
        }
        automation.last_scheduled_slot = Some(slot);
        automation.updated_at = current_unix_ms();
        if let Err(err) = crate::persist::save_automations(&self.state.automations) {
            tracing::error!(automation_id, error = %err, "failed to persist automation schedule claim");
        }
        true
    }

    fn record_automation_skip(&mut self, automation_id: &str, slot: u64) -> String {
        let run = AutomationRun::new(slot, AutomationRunStatus::Skipped);
        let run_id = run.run_id.clone();
        if let Some(automation) = self.state.automations.find_mut(automation_id) {
            automation.append_run(run);
            automation.updated_at = current_unix_ms();
            if let Err(err) = crate::persist::save_automations(&self.state.automations) {
                tracing::error!(automation_id, error = %err, "failed to persist skipped automation run");
            }
        }
        run_id
    }

    fn execute_automation(&mut self, automation_id: &str, slot: u64) -> String {
        let Some(automation) = self.state.automations.find(automation_id).cloned() else {
            return self.record_automation_skip(automation_id, slot);
        };
        let mut run = AutomationRun::new(slot, AutomationRunStatus::Queued);
        let run_id = run.run_id.clone();
        if let Some(stored) = self.state.automations.find_mut(automation_id) {
            stored.append_run(run.clone());
            stored.updated_at = current_unix_ms();
        }
        if let Err(err) = crate::persist::save_automations(&self.state.automations) {
            tracing::error!(automation_id, error = %err, "failed to persist queued automation run");
        }

        let Some(project) = self.state.projects.find(&automation.project_id).cloned() else {
            run.status = AutomationRunStatus::Failed;
            run.error = Some("automation project does not exist".to_owned());
            self.finish_automation_run(&automation.id, &run);
            return run_id;
        };
        let task_name = format!("{} ({slot})", automation.name);
        let (branch, worktree_path) = match automation.workspace_mode {
            TaskLocationMode::Repository => (None, None),
            TaskLocationMode::Worktree => {
                let branch = format!("automation/{}/{}", automation.id, slot);
                let root = project
                    .worktree_root
                    .clone()
                    .unwrap_or_else(|| self.state.worktree_directory.clone());
                let repo_name = project
                    .root_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| !name.is_empty())
                    .unwrap_or("repository");
                let path = crate::worktree::default_checkout_path(&root, repo_name, &branch);
                run.status = AutomationRunStatus::Provisioning;
                self.update_automation_run(&automation.id, &run);
                if let Err(error) = crate::worktree::run_worktree_add_command(
                    &project.root_path,
                    &path,
                    &branch,
                    project
                        .worktree_base
                        .as_deref()
                        .unwrap_or(crate::project::DEFAULT_WORKTREE_BASE),
                    false,
                ) {
                    run.status = AutomationRunStatus::Failed;
                    run.error = Some(error);
                    self.finish_automation_run(&automation.id, &run);
                    return run_id;
                }
                if let Err(error) = crate::worktree::preserve_ignored_files(
                    &project.root_path,
                    &path,
                    &project.preserve_patterns,
                ) {
                    run.status = AutomationRunStatus::Failed;
                    run.error = Some(error);
                    self.finish_automation_run(&automation.id, &run);
                    return run_id;
                }
                (Some(branch), Some(path))
            }
        };
        let task = Task::new(
            automation.project_id.clone(),
            task_name,
            automation.workspace_mode,
            branch,
            worktree_path,
            automation.provider.clone(),
            automation.model.clone(),
            Some(automation.prompt.clone()),
            project.environment.clone(),
            Vec::new(),
            None,
            None,
            None,
        );
        let task_id = task.id.clone();
        self.state.tasks.insert(task);
        if let Err(error) = crate::persist::save_tasks(&self.state.tasks) {
            run.status = AutomationRunStatus::Failed;
            run.error = Some(error.to_string());
            self.finish_automation_run(&automation.id, &run);
            return run_id;
        }
        if let Err(error) = self.run_task_lifecycle(&task_id) {
            run.status = AutomationRunStatus::Failed;
            run.task_id = Some(task_id);
            run.error = Some(error);
            self.finish_automation_run(&automation.id, &run);
            return run_id;
        }
        run.status = AutomationRunStatus::Launching;
        run.task_id = Some(task_id.clone());
        self.update_automation_run(&automation.id, &run);
        match self.start_task_with_runtime(&task_id) {
            Ok(()) => run.status = AutomationRunStatus::Completed,
            Err(error) => {
                run.status = AutomationRunStatus::Failed;
                run.error = Some(error);
            }
        }
        self.finish_automation_run(&automation.id, &run);
        run_id
    }

    fn automation_run_info(&self, run: &AutomationRun) -> AutomationRunInfo {
        AutomationRunInfo {
            run_id: run.run_id.clone(),
            scheduled_slot: run.scheduled_slot,
            status: run.status,
            started_at: run.started_at,
            finished_at: run.finished_at,
            task_id: run.task_id.clone(),
            error: run.error.clone(),
        }
    }

    fn update_automation_run(&mut self, automation_id: &str, run: &AutomationRun) {
        if let Some(automation) = self.state.automations.find_mut(automation_id) {
            if let Some(stored) = automation
                .runs
                .iter_mut()
                .find(|stored| stored.run_id == run.run_id)
            {
                *stored = run.clone();
                automation.updated_at = current_unix_ms();
            }
        }
        if let Err(err) = crate::persist::save_automations(&self.state.automations) {
            tracing::error!(automation_id, error = %err, "failed to persist automation run");
        }
    }

    fn finish_automation_run(&mut self, automation_id: &str, run: &AutomationRun) {
        let mut finished = run.clone();
        finished.finished_at = Some(current_unix_ms());
        self.update_automation_run(automation_id, &finished);
    }
}

fn validate_cron(cron: &str) -> Result<(), String> {
    crate::automation::cron_matches(
        cron,
        crate::automation::current_cron_slot(SystemTime::now()),
    )
    .map(|_| ())
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn automation_not_found(id: String, automation_id: &str) -> String {
    encode_error(
        id,
        "automation_not_found",
        format!("automation {automation_id} not found"),
    )
}
