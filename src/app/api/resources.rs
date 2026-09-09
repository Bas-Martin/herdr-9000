use crate::api::schema::{
    ResourceCreateParams, ResourceInfo, ResourceListParams, ResourceTarget, ResourceUpdateParams,
    ResponseResult,
};
use crate::app::App;
use crate::resource::Resource;
use crate::resource::ResourceScope;

use super::responses::{encode_error, encode_success};

impl App {
    fn resource_info(&self, resource: &Resource) -> ResourceInfo {
        ResourceInfo {
            resource_id: resource.id.clone(),
            kind: resource.kind,
            name: resource.name.clone(),
            description: resource.description.clone(),
            scope: resource.scope,
            project_id: resource.project_id.clone(),
            task_id: resource.task_id.clone(),
            provider: resource.provider.clone(),
            content: resource.content.clone(),
            enabled: resource.enabled,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }

    pub(super) fn handle_resource_create(
        &mut self,
        id: String,
        params: ResourceCreateParams,
    ) -> String {
        let name = params.name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return encode_error(id, "invalid_params", "resource name must be printable");
        }
        if params.content.len() > 256 * 1024 {
            return encode_error(id, "invalid_params", "resource content is too large");
        }
        if let Some(provider) = params.provider.as_deref() {
            if crate::detect::parse_agent_label(provider).is_none() {
                return encode_error(id, "invalid_params", "resource provider is unknown");
            }
        }
        match params.scope {
            ResourceScope::Global => {
                if params.project_id.is_some() || params.task_id.is_some() {
                    return encode_error(
                        id,
                        "invalid_params",
                        "global resources cannot have an owner",
                    );
                }
            }
            ResourceScope::Project => {
                let Some(project_id) = params.project_id.as_deref() else {
                    return encode_error(
                        id,
                        "invalid_params",
                        "project resources require project_id",
                    );
                };
                if self.state.projects.find(project_id).is_none() {
                    return encode_error(
                        id,
                        "project_not_found",
                        "resource project does not exist",
                    );
                }
                if params.task_id.is_some() {
                    return encode_error(
                        id,
                        "invalid_params",
                        "project resources cannot have task_id",
                    );
                }
            }
            ResourceScope::Task => {
                let Some(task_id) = params.task_id.as_deref() else {
                    return encode_error(id, "invalid_params", "task resources require task_id");
                };
                if self.state.tasks.find(task_id).is_none() {
                    return encode_error(id, "task_not_found", "resource task does not exist");
                }
                if params.project_id.is_some() {
                    return encode_error(
                        id,
                        "invalid_params",
                        "task resources cannot have project_id",
                    );
                }
            }
        }
        let resource = Resource::new(
            params.kind,
            name.to_owned(),
            params.description.trim().to_owned(),
            params.scope,
            params.project_id,
            params.task_id,
            params.provider,
            params.content,
        );
        let previous = self.state.resources.clone();
        self.state.resources.insert(resource.clone());
        if let Err(err) = crate::persist::save_resources(&self.state.resources) {
            self.state.resources = previous;
            return encode_error(id, "resource_save_failed", err.to_string());
        }
        encode_success(
            id,
            ResponseResult::ResourceInfo {
                resource: self.resource_info(&resource),
            },
        )
    }

    pub(super) fn handle_resource_list(
        &mut self,
        id: String,
        params: ResourceListParams,
    ) -> String {
        if let Some(project_id) = params.project_id.as_deref() {
            if self.state.projects.find(project_id).is_none() {
                return encode_error(id, "project_not_found", "resource project does not exist");
            }
        }
        if let Some(task_id) = params.task_id.as_deref() {
            if self.state.tasks.find(task_id).is_none() {
                return encode_error(id, "task_not_found", "resource task does not exist");
            }
        }
        let resources = self
            .state
            .resources
            .resources
            .iter()
            .filter(|resource| params.include_disabled || resource.enabled)
            .filter(|resource| {
                params.project_id.as_deref().is_none_or(|project_id| {
                    resource.scope == ResourceScope::Global
                        || resource.project_id.as_deref() == Some(project_id)
                })
            })
            .filter(|resource| {
                params.task_id.as_deref().is_none_or(|task_id| {
                    resource.scope != ResourceScope::Task
                        || resource.task_id.as_deref() == Some(task_id)
                })
            })
            .map(|resource| self.resource_info(resource))
            .collect();
        encode_success(id, ResponseResult::ResourceList { resources })
    }

    pub(super) fn handle_resource_update(
        &mut self,
        id: String,
        params: ResourceUpdateParams,
    ) -> String {
        let previous = self.state.resources.clone();
        let Some(resource) = self.state.resources.find_mut(&params.resource_id) else {
            return encode_error(id, "resource_not_found", "resource does not exist");
        };
        if let Some(name) = params.name {
            let name = name.trim();
            if name.is_empty() || name.chars().any(char::is_control) {
                return encode_error(id, "invalid_params", "resource name must be printable");
            }
            resource.name = name.to_owned();
        }
        if let Some(description) = params.description {
            resource.description = description.trim().to_owned();
        }
        if let Some(content) = params.content {
            if content.len() > 256 * 1024 {
                return encode_error(id, "invalid_params", "resource content is too large");
            }
            resource.content = content;
        }
        if let Some(enabled) = params.enabled {
            resource.enabled = enabled;
        }
        resource.updated_at = crate::task::current_unix_ms();
        let updated = resource.clone();
        if let Err(err) = crate::persist::save_resources(&self.state.resources) {
            self.state.resources = previous;
            return encode_error(id, "resource_save_failed", err.to_string());
        }
        encode_success(
            id,
            ResponseResult::ResourceInfo {
                resource: self.resource_info(&updated),
            },
        )
    }

    pub(super) fn handle_resource_delete(&mut self, id: String, target: ResourceTarget) -> String {
        let Some(index) = self
            .state
            .resources
            .resources
            .iter()
            .position(|resource| resource.id == target.resource_id)
        else {
            return encode_error(id, "resource_not_found", "resource does not exist");
        };
        let previous = self.state.resources.clone();
        self.state.resources.resources.remove(index);
        if let Err(err) = crate::persist::save_resources(&self.state.resources) {
            self.state.resources = previous;
            return encode_error(id, "resource_save_failed", err.to_string());
        }
        encode_success(
            id,
            ResponseResult::ResourceDeleted {
                resource_id: target.resource_id,
            },
        )
    }
}
