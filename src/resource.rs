use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_RESOURCE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResourceKind {
    Prompt,
    Skill,
    Mcp,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResourceScope {
    Global,
    Project,
    Task,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Resource {
    pub(crate) id: String,
    pub(crate) kind: ResourceKind,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) scope: ResourceScope,
    pub(crate) project_id: Option<String>,
    pub(crate) task_id: Option<String>,
    pub(crate) provider: Option<String>,
    pub(crate) content: String,
    pub(crate) enabled: bool,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ResourceRegistry {
    #[serde(default = "default_registry_version")]
    pub(crate) version: u32,
    #[serde(default)]
    pub(crate) resources: Vec<Resource>,
}

fn default_registry_version() -> u32 {
    1
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self {
            version: default_registry_version(),
            resources: Vec::new(),
        }
    }
}

impl ResourceRegistry {
    pub(crate) fn find(&self, id: &str) -> Option<&Resource> {
        self.resources.iter().find(|resource| resource.id == id)
    }

    pub(crate) fn find_mut(&mut self, id: &str) -> Option<&mut Resource> {
        self.resources.iter_mut().find(|resource| resource.id == id)
    }

    pub(crate) fn insert(&mut self, resource: Resource) {
        self.resources.push(resource);
    }
}

impl Resource {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        kind: ResourceKind,
        name: String,
        description: String,
        scope: ResourceScope,
        project_id: Option<String>,
        task_id: Option<String>,
        provider: Option<String>,
        content: String,
    ) -> Self {
        let now = crate::task::current_unix_ms();
        Self {
            id: format!("r{}", NEXT_RESOURCE_ID.fetch_add(1, Ordering::Relaxed)),
            kind,
            name,
            description,
            scope,
            project_id,
            task_id,
            provider,
            content,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    pub(crate) fn capability_diagnostic(&self) -> Option<String> {
        self.capability_diagnostic_for_provider(self.provider.as_deref())
    }

    pub(crate) fn capability_diagnostic_for_provider(
        &self,
        provider: Option<&str>,
    ) -> Option<String> {
        if self.kind == ResourceKind::Mcp
            && serde_json::from_str::<serde_json::Value>(&self.content).is_err()
        {
            return Some(format!("MCP resource {} contains invalid JSON", self.id));
        }
        let Some(provider) = provider else {
            return None;
        };
        let Some(agent) = crate::detect::parse_agent_label(provider) else {
            return Some(format!(
                "{} resources are not configured for provider {provider}",
                self.kind.label()
            ));
        };
        if self.kind == ResourceKind::Mcp && agent != crate::detect::Agent::Claude {
            return Some(format!(
                "MCP resources are currently supported for Claude only (provider {provider})"
            ));
        }
        None
    }

    pub(crate) fn is_available_for_provider(&self, provider: Option<&str>) -> bool {
        self.provider.as_deref().is_none_or(|required| {
            provider.is_some_and(|actual| {
                required.eq_ignore_ascii_case(actual)
                    && self
                        .capability_diagnostic_for_provider(Some(actual))
                        .is_none()
            })
        }) && self.capability_diagnostic_for_provider(provider).is_none()
    }
}

impl ResourceKind {
    fn label(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Skill => "skill",
            Self::Mcp => "MCP",
        }
    }
}

pub(crate) fn reserve_resource_ids(registry: &ResourceRegistry) {
    let next_id = registry
        .resources
        .iter()
        .filter_map(|resource| resource.id.strip_prefix('r'))
        .filter_map(|value| value.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    NEXT_RESOURCE_ID.fetch_max(next_id, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(provider: Option<&str>) -> Resource {
        Resource::new(
            ResourceKind::Skill,
            "demo".to_owned(),
            String::new(),
            ResourceScope::Global,
            None,
            None,
            provider.map(str::to_owned),
            "instructions".to_owned(),
        )
    }

    #[test]
    fn known_provider_is_available_without_diagnostic() {
        let resource = resource(Some("codex"));
        assert!(resource.capability_diagnostic().is_none());
        assert!(resource.is_available_for_provider(Some("codex")));
    }

    #[test]
    fn unknown_provider_exposes_capability_diagnostic() {
        let resource = resource(Some("unsupported-agent"));
        let diagnostic = resource
            .capability_diagnostic()
            .expect("unknown providers should be diagnosed");
        assert!(diagnostic.contains("unsupported-agent"));
        assert!(!resource.is_available_for_provider(Some("unsupported-agent")));
    }
}
