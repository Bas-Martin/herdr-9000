use crate::api::schema::{
    EventData, EventEnvelope, EventKind, ProjectCreateParams, ProjectInfo, ProjectOpenParams,
    ProjectRenameParams, ProjectTarget, ProjectUpdateParams, ResponseResult,
};
use crate::app::App;

use super::responses::{encode_error, encode_success};

impl App {
    fn project_workspace_index(&self, project: &crate::project::Project) -> Option<usize> {
        self.state.workspaces.iter().position(|workspace| {
            crate::project::same_path(&workspace.identity_cwd, &project.root_path)
        })
    }

    fn project_info(&self, project: &crate::project::Project) -> ProjectInfo {
        ProjectInfo {
            project_id: project.id.clone(),
            name: project.name.clone(),
            root_path: crate::project::display_path(&project.root_path),
            workspace_id: self
                .project_workspace_index(project)
                .map(|index| self.public_workspace_id(index)),
            worktree_root: project
                .worktree_root
                .as_ref()
                .map(|path| crate::project::display_path(path)),
            worktree_base: project.worktree_base.clone(),
            default_agent: project.default_agent.clone(),
            lifecycle: crate::api::schema::ProjectLifecycle {
                prepare: project.lifecycle.prepare.clone(),
                setup: project.lifecycle.setup.clone(),
                run: project.lifecycle.run.clone(),
                teardown: project.lifecycle.teardown.clone(),
            },
            preserve_patterns: project.preserve_patterns.clone(),
        }
    }

    pub(super) fn handle_project_list(&mut self, id: String) -> String {
        let projects = self
            .state
            .projects
            .projects
            .iter()
            .map(|project| self.project_info(project))
            .collect();
        encode_success(id, ResponseResult::ProjectList { projects })
    }

    pub(super) fn handle_project_get(&mut self, id: String, target: ProjectTarget) -> String {
        let Some(project) = self.state.projects.find(&target.project_id) else {
            return project_not_found(id, &target.project_id);
        };
        encode_success(
            id,
            ResponseResult::ProjectInfo {
                project: self.project_info(project),
            },
        )
    }

    pub(super) fn handle_project_create(
        &mut self,
        id: String,
        params: ProjectCreateParams,
    ) -> String {
        let name = params.name.trim();
        if name.is_empty() {
            return encode_error(id, "invalid_params", "project name must not be empty");
        }
        let root_path = match crate::project::normalize_root_path(&params.root_path) {
            Ok(path) => path,
            Err(message) => return encode_error(id, "invalid_params", message),
        };
        let worktree_root = match params.worktree_root.as_deref() {
            Some(path) => match crate::project::normalize_worktree_root(path) {
                Ok(path) => Some(path),
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => None,
        };
        if let Some(existing) = self.state.projects.find_by_root(&root_path).cloned() {
            if params.open {
                return self.handle_project_open(
                    id,
                    ProjectOpenParams {
                        project_id: existing.id,
                        focus: params.focus,
                    },
                );
            }
            return encode_error(
                id,
                "project_exists",
                format!("project already exists: {}", existing.id),
            );
        }
        let default_agent = match normalize_default_agent(params.default_agent) {
            Ok(agent) => agent,
            Err(message) => return encode_error(id, "invalid_params", message),
        };
        let preserve_patterns = match normalize_preserve_patterns(params.preserve_patterns) {
            Ok(patterns) => patterns,
            Err(message) => return encode_error(id, "invalid_params", message),
        };
        let lifecycle = match normalize_lifecycle(params.lifecycle) {
            Ok(lifecycle) => lifecycle,
            Err(message) => return encode_error(id, "invalid_params", message),
        };
        let previous = self.state.projects.clone();
        let worktree_base = clean_worktree_base(params.worktree_base);
        let mut project = crate::project::Project::new(name.to_owned(), root_path, worktree_root);
        project.default_agent = default_agent;
        project.preserve_patterns = preserve_patterns;
        project.lifecycle = lifecycle;
        project.worktree_base = worktree_base;
        self.state.projects.insert(project.clone());
        if let Err(err) = crate::persist::save_projects(&self.state.projects) {
            self.state.projects = previous.clone();
            return encode_error(id, "project_save_failed", err.to_string());
        }
        if params.open {
            let (index, created) = if let Some(index) = self.project_workspace_index(&project) {
                if params.focus {
                    self.state.switch_workspace(index);
                }
                (index, false)
            } else {
                let index = match self
                    .create_workspace_with_options(project.root_path.clone(), params.focus)
                {
                    Ok(index) => index,
                    Err(err) => {
                        self.state.projects = previous;
                        let _ = crate::persist::save_projects(&self.state.projects);
                        return encode_error(id, "workspace_create_failed", err.to_string());
                    }
                };
                (index, true)
            };
            if let Some(workspace) = self.state.workspaces.get_mut(index) {
                workspace.set_custom_name(project.name.clone());
                crate::logging::workspace_renamed(&workspace.id);
            }
            if created {
                self.emit_workspace_open_events(index);
            }
            self.schedule_session_save();
        }
        encode_success(
            id,
            ResponseResult::ProjectCreated {
                project: self.project_info(&project),
            },
        )
    }

    pub(super) fn handle_project_open(&mut self, id: String, params: ProjectOpenParams) -> String {
        let Some(project) = self.state.projects.find(&params.project_id).cloned() else {
            return project_not_found(id, &params.project_id);
        };
        let (workspace_index, created) = if let Some(index) = self.project_workspace_index(&project)
        {
            if params.focus {
                self.state.switch_workspace(index);
            }
            (index, false)
        } else {
            let index =
                match self.create_workspace_with_options(project.root_path.clone(), params.focus) {
                    Ok(index) => index,
                    Err(err) => {
                        return encode_error(id, "workspace_create_failed", err.to_string());
                    }
                };
            if let Some(workspace) = self.state.workspaces.get_mut(index) {
                workspace.set_custom_name(project.name.clone());
                crate::logging::workspace_renamed(&workspace.id);
            }
            self.emit_workspace_open_events(index);
            (index, true)
        };

        encode_success(
            id,
            ResponseResult::ProjectOpened {
                project: self.project_info(&project),
                workspace: self.workspace_info(workspace_index),
                created,
            },
        )
    }

    pub(super) fn handle_project_rename(
        &mut self,
        id: String,
        params: ProjectRenameParams,
    ) -> String {
        self.update_project(
            id,
            params.project_id,
            params.name,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }
    pub(super) fn handle_project_update(
        &mut self,
        id: String,
        params: ProjectUpdateParams,
    ) -> String {
        self.update_project(
            id,
            params.project_id,
            params.name,
            Some(params.root_path),
            params.worktree_root,
            params.worktree_base,
            params.default_agent,
            params.preserve_patterns,
            params.lifecycle,
        )
    }
    fn update_project(
        &mut self,
        id: String,
        project_id: String,
        name: String,
        root_path_param: Option<String>,
        worktree_root_param: Option<String>,
        worktree_base_param: Option<String>,
        default_agent_param: Option<String>,
        preserve_patterns_param: Option<Vec<String>>,
        lifecycle_param: Option<crate::api::schema::ProjectLifecycle>,
    ) -> String {
        let name = name.trim();
        if name.is_empty() {
            return encode_error(id, "invalid_params", "project name must not be empty");
        }
        let Some(existing) = self.state.projects.find(&project_id).cloned() else {
            return project_not_found(id, &project_id);
        };
        let root_path = match root_path_param.as_deref() {
            Some(path) => match crate::project::normalize_root_path(path) {
                Ok(path) => path,
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => existing.root_path.clone(),
        };
        if self
            .state
            .projects
            .find_by_root(&root_path)
            .is_some_and(|project| project.id != project_id)
        {
            return encode_error(
                id,
                "project_exists",
                "another project already uses that path",
            );
        }
        let worktree_root = match worktree_root_param.as_deref() {
            Some(path) if path.trim().is_empty() => None,
            Some(path) => match crate::project::normalize_worktree_root(path) {
                Ok(path) => Some(path),
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => existing.worktree_root.clone(),
        };
        let worktree_base = match worktree_base_param {
            Some(base) => clean_worktree_base(Some(base)),
            None => existing.worktree_base.clone(),
        };
        let default_agent = match default_agent_param {
            Some(agent) => match normalize_default_agent(Some(agent)) {
                Ok(agent) => agent,
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => existing.default_agent.clone(),
        };
        let preserve_patterns = match preserve_patterns_param {
            Some(patterns) => match normalize_preserve_patterns(Some(patterns)) {
                Ok(patterns) => patterns,
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => existing.preserve_patterns.clone(),
        };
        let lifecycle = match lifecycle_param {
            Some(lifecycle) => match normalize_lifecycle(Some(lifecycle)) {
                Ok(lifecycle) => lifecycle,
                Err(message) => return encode_error(id, "invalid_params", message),
            },
            None => existing.lifecycle.clone(),
        };
        let previous = self.state.projects.clone();
        let Some(project) = self.state.projects.find_mut(&project_id) else {
            return project_not_found(id, &project_id);
        };
        project.preserve_patterns = preserve_patterns;
        project.default_agent = default_agent;
        let old_root_path = project.root_path.clone();
        project.name = name.to_owned();
        project.root_path = root_path;
        project.worktree_root = worktree_root;
        project.lifecycle = lifecycle;
        project.worktree_base = worktree_base;
        if let Err(err) = crate::persist::save_projects(&self.state.projects) {
            self.state.projects = previous;
            return encode_error(id, "project_save_failed", err.to_string());
        }
        let Some(project) = self.state.projects.find(&project_id).cloned() else {
            return project_not_found(id, &project_id);
        };
        if let Some(index) = self.state.workspaces.iter().position(|workspace| {
            crate::project::same_path(&workspace.identity_cwd, &old_root_path)
        }) {
            let workspace_id = self.state.workspaces[index].id.clone();
            self.state.workspaces[index].set_custom_name(project.name.clone());
            crate::logging::workspace_renamed(&workspace_id);
            self.schedule_session_save();
            self.emit_event(EventEnvelope {
                event: EventKind::WorkspaceRenamed,
                data: EventData::WorkspaceRenamed {
                    workspace_id,
                    label: project.name.clone(),
                },
            });
        }

        encode_success(
            id,
            ResponseResult::ProjectInfo {
                project: self.project_info(&project),
            },
        )
    }

    pub(super) fn handle_project_delete(&mut self, id: String, target: ProjectTarget) -> String {
        if self.state.projects.find(&target.project_id).is_none() {
            return project_not_found(id, &target.project_id);
        }
        let previous = self.state.projects.clone();
        let Some(project) = self.state.projects.remove(&target.project_id) else {
            return project_not_found(id, &target.project_id);
        };
        if let Err(err) = crate::persist::save_projects(&self.state.projects) {
            self.state.projects = previous;
            return encode_error(id, "project_save_failed", err.to_string());
        }

        encode_success(
            id,
            ResponseResult::ProjectDeleted {
                project_id: project.id,
            },
        )
    }
}

fn clean_worktree_base(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

fn normalize_default_agent(value: Option<String>) -> Result<Option<String>, String> {
    let Some(value) = clean_worktree_base(value) else {
        return Ok(None);
    };
    crate::project::normalize_agent_provider(&value).map(Some)
}

fn normalize_preserve_patterns(value: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let Some(values) = value else {
        return Ok(Vec::new());
    };
    let mut patterns = Vec::new();
    for raw in values {
        let pattern = raw.trim().replace('\\', "/");

        let pattern = pattern.strip_prefix("./").unwrap_or(&pattern);
        if pattern.is_empty() {
            continue;
        }
        if pattern.starts_with('/')
            || pattern.contains(':')
            || pattern.split('/').any(|part| part == "..")
        {
            return Err(format!(
                "preserve pattern must be repository-relative: {raw}"
            ));
        }
        if !patterns.iter().any(|existing| existing == pattern) {
            patterns.push(pattern.to_owned());
        }
    }
    Ok(patterns)
}
fn normalize_lifecycle(
    value: Option<crate::api::schema::ProjectLifecycle>,
) -> Result<crate::project::ProjectLifecycle, String> {
    let Some(value) = value else {
        return Ok(crate::project::ProjectLifecycle::default());
    };
    Ok(crate::project::ProjectLifecycle {
        prepare: normalize_lifecycle_command(value.prepare, "prepare")?,
        setup: normalize_lifecycle_command(value.setup, "setup")?,
        run: normalize_lifecycle_command(value.run, "run")?,
        teardown: normalize_lifecycle_command(value.teardown, "teardown")?,
    })
}

fn normalize_lifecycle_command(
    value: Option<String>,
    step: &str,
) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 8192 || value.chars().any(char::is_control) {
        return Err(format!(
            "lifecycle {step} command must be printable and at most 8192 bytes"
        ));
    }
    Ok(Some(value))
}

fn project_not_found(id: String, project_id: &str) -> String {
    encode_error(
        id,
        "project_not_found",
        format!("project {project_id} not found"),
    )
}
