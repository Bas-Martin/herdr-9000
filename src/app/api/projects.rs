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

        let previous = self.state.projects.clone();
        let project = crate::project::Project::new(name.to_owned(), root_path, worktree_root);
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
        self.update_project(id, params.project_id, params.name, None, None)
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
        )
    }

    fn update_project(
        &mut self,
        id: String,
        project_id: String,
        name: String,
        root_path_param: Option<String>,
        worktree_root_param: Option<String>,
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
        let previous = self.state.projects.clone();
        let Some(project) = self.state.projects.find_mut(&project_id) else {
            return project_not_found(id, &project_id);
        };
        let old_root_path = project.root_path.clone();
        project.name = name.to_owned();
        project.root_path = root_path;
        project.worktree_root = worktree_root;
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

fn project_not_found(id: String, project_id: &str) -> String {
    encode_error(
        id,
        "project_not_found",
        format!("project {project_id} not found"),
    )
}
