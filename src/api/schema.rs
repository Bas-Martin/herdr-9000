use serde::{Deserialize, Serialize};

pub mod agents;
pub mod automations;
pub mod checks;
pub mod commands;
pub mod common;
pub mod diffs;
pub mod events;
pub mod external;
pub mod github;
pub mod integrations;
pub(crate) mod operations;
pub mod panes;
pub mod plugins;
pub mod projects;
pub mod resources;
pub mod response;
pub mod server;
pub mod session;
pub mod tabs;
pub mod tasks;
pub mod tmux;
pub mod workspaces;
pub mod worktrees;

pub use agents::*;
pub use automations::*;
pub use checks::*;
pub use commands::*;
pub use common::*;
pub use diffs::*;
pub use events::*;
pub use external::*;
pub use github::*;
pub use integrations::*;
pub use panes::*;
pub use plugins::*;
pub use projects::*;
pub use resources::*;
pub use response::*;
pub use server::*;
pub use session::*;
pub use tabs::*;
pub use tasks::*;
pub use tmux::*;
pub use workspaces::*;
pub use worktrees::*;

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Request {
    pub id: String,
    #[serde(flatten)]
    pub method: Method,
}

// Variant order and parameter types preserve the existing wire contract.
// The two flags are independent: presentation impact, then client-shell admission.
operations::define_operations! {
    Ping(PingParams) => "ping", false, false;
    ServerStop(EmptyParams) => "server.stop", false, false;
    ServerLiveHandoff(ServerLiveHandoffParams) => "server.live_handoff", false, false;
    ServerReloadConfig(EmptyParams) => "server.reload_config", true, true;
    ServerSshAgentRegister(ServerSshAgentRegisterParams) => "server.ssh_agent.register", false, false;
    ServerAgentManifests(EmptyParams) => "server.agent_manifests", false, false;
    ServerReloadAgentManifests(EmptyParams) => "server.reload_agent_manifests", true, false;
    NotificationShow(NotificationShowParams) => "notification.show", true, false;
    ProductAnnouncementDismiss(ProductAnnouncementDismissParams) => "product_announcement.dismiss", true, true;
    ReleaseNotesDismiss(ReleaseNotesDismissParams) => "release_notes.dismiss", true, true;
    CommandInvoke(CommandInvokeParams) => "command.invoke", true, true;
    ClientWindowTitleSet(ClientWindowTitleSetParams) => "client.window_title.set", false, false;
    ClientWindowTitleClear(EmptyParams) => "client.window_title.clear", false, false;
    ClientShellSurfaceSet(ClientShellSurfaceSetParams) => "client_shell.surface.set", false, true;
    SessionSnapshot(EmptyParams) => "session.snapshot", false, false;
    WorkspaceCreate(WorkspaceCreateParams) => "workspace.create", true, true;
    WorkspaceList(EmptyParams) => "workspace.list", false, false;
    WorkspaceGet(WorkspaceTarget) => "workspace.get", false, false;
    WorkspaceFocus(WorkspaceTarget) => "workspace.focus", true, true;
    WorkspaceRename(WorkspaceRenameParams) => "workspace.rename", true, true;
    WorkspaceMove(WorkspaceMoveParams) => "workspace.move", true, true;
    WorkspaceMoveBlock(WorkspaceMoveBlockParams) => "workspace.move_block", true, true;
    WorkspaceReportMetadata(WorkspaceReportMetadataParams) => "workspace.report_metadata", true, false;
    WorkspaceClose(WorkspaceCloseParams) => "workspace.close", true, true;
    ProjectCreate(ProjectCreateParams) => "project.create", true, true;
    ProjectList(EmptyParams) => "project.list", false, true;
    ProjectGet(ProjectTarget) => "project.get", false, false;
    ProjectOpen(ProjectOpenParams) => "project.open", true, true;
    ProjectRename(ProjectRenameParams) => "project.rename", true, true;
    ProjectUpdate(ProjectUpdateParams) => "project.update", true, true;
    ProjectDelete(ProjectTarget) => "project.delete", true, true;
    TaskCreate(TaskCreateParams) => "task.create", true, false;
    TaskList(TaskListParams) => "task.list", false, false;
    TaskOpen(TaskOpenParams) => "task.open", true, false;
    TaskRename(TaskRenameParams) => "task.rename", true, false;
    TaskResources(TaskResourcesParams) => "task.resources", true, false;
    TaskClose(TaskTarget) => "task.close", true, false;
    TaskDiff(TaskDiffParams) => "task.diff", false, false;
    TaskFileRead(TaskFileReadParams) => "task.file_read", false, false;
    TaskFileWrite(TaskFileWriteParams) => "task.file_write", true, false;
    TaskGitAction(TaskGitActionParams) => "task.git_action", true, false;
    TaskGitHubSearch(GitHubIssueSearchParams) => "task.github_search", false, false;
    TaskGitHubCreate(GitHubIssueTaskCreateParams) => "task.github_create", true, false;
    TaskChecks(TaskChecksParams) => "task.checks", false, false;
    AutomationCreate(AutomationCreateParams) => "automation.create", true, false;
    AutomationList(AutomationListParams) => "automation.list", false, false;
    AutomationUpdate(AutomationUpdateParams) => "automation.update", true, false;
    AutomationDelete(AutomationTarget) => "automation.delete", true, false;
    AutomationRunNow(AutomationRunNowParams) => "automation.run_now", true, false;
    ExternalTrackerConfigure(ExternalTrackerConfigureParams) => "external.configure", true, false;
    ExternalIssueSearch(ExternalIssueSearchParams) => "external.search", false, false;
    ExternalIssueTaskCreate(ExternalIssueTaskCreateParams) => "external.task_create", true, false;
    ResourceCreate(ResourceCreateParams) => "resource.create", true, false;
    ResourceList(ResourceListParams) => "resource.list", false, false;
    ResourceUpdate(ResourceUpdateParams) => "resource.update", true, false;
    ResourceDelete(ResourceTarget) => "resource.delete", true, false;
    WorktreeList(WorktreeListParams) => "worktree.list", false, true;
    WorktreeCreate(WorktreeCreateParams) => "worktree.create", true, true;
    WorktreeOpen(WorktreeOpenParams) => "worktree.open", true, true;
    WorktreeRemove(WorktreeRemoveParams) => "worktree.remove", true, true;
    TabCreate(TabCreateParams) => "tab.create", true, true;
    TabList(TabListParams) => "tab.list", false, false;
    TabGet(TabTarget) => "tab.get", false, false;
    TabFocus(TabTarget) => "tab.focus", true, true;
    TabRename(TabRenameParams) => "tab.rename", true, true;
    TabMove(TabMoveParams) => "tab.move", true, true;
    TabClose(TabTarget) => "tab.close", true, true;
    AgentList(EmptyParams) => "agent.list", false, false;
    AgentGet(AgentTarget) => "agent.get", false, false;
    AgentRead(AgentReadParams) => "agent.read", false, false;
    AgentExplain(AgentTarget) => "agent.explain", false, false;
    AgentSendKeys(AgentSendKeysParams) => "agent.send_keys", true, false;
    AgentRename(AgentRenameParams) => "agent.rename", true, false;
    AgentViewSet(AgentViewSetParams) => "agent.view.set", true, false;
    AgentViewClear(AgentViewClearParams) => "agent.view.clear", true, false;
    AgentFocus(AgentTarget) => "agent.focus", true, false;
    AgentStartTmux(TmuxAgentStartParams) => "agent.start_tmux", false, false;
    AgentStart(AgentStartParams) => "agent.start", true, false;
    AgentPrompt(AgentPromptParams) => "agent.prompt", true, false;
    TmuxPaneList(EmptyParams) => "tmux.list", false, false;
    TmuxPaneCapture(TmuxPaneCaptureParams) => "tmux.capture", false, false;
    TmuxPaneSendKeys(TmuxPaneSendKeysParams) => "tmux.send_keys", false, false;
    TmuxPaneKill(TmuxPaneTarget) => "tmux.kill", false, false;
    TmuxPaneFocus(TmuxPaneTarget) => "tmux.focus", false, false;
    AgentWait(AgentWaitParams) => "agent.wait", false, false;
    PaneSplit(PaneSplitParams) => "pane.split", true, true;
    PaneSwap(PaneSwapParams) => "pane.swap", true, true;
    PaneMove(PaneMoveParams) => "pane.move", true, false;
    PaneZoom(PaneZoomParams) => "pane.zoom", true, true;
    PaneLayout(PaneLayoutParams) => "pane.layout", false, false;
    PaneProcessInfo(PaneProcessInfoParams) => "pane.process_info", false, false;
    LayoutExport(LayoutExportParams) => "layout.export", false, false;
    LayoutApply(LayoutApplyParams) => "layout.apply", true, false;
    LayoutSetSplitRatio(LayoutSetSplitRatioParams) => "layout.set_split_ratio", true, true;
    PaneNeighbor(PaneNeighborParams) => "pane.neighbor", false, false;
    PaneEdges(PaneEdgesParams) => "pane.edges", false, false;
    PaneFocusDirection(PaneFocusDirectionParams) => "pane.focus_direction", true, true;
    PaneResize(PaneResizeParams) => "pane.resize", true, true;
    PaneScroll(PaneScrollParams) => "pane.scroll", true, true;
    PaneClear(PaneTarget) => "pane.clear", true, true;
    PaneEditScrollback(PaneTarget) => "pane.edit_scrollback", true, true;
    PaneSelectionRead(PaneSelectionReadParams) => "pane.selection.read", false, true;
    PaneCopyMotion(PaneCopyMotionParams) => "pane.copy_motion", false, true;
    PaneCopySearch(PaneCopySearchParams) => "pane.copy_search", false, true;
    PaneList(PaneListParams) => "pane.list", false, false;
    PaneCurrent(PaneCurrentParams) => "pane.current", false, false;
    PaneGet(PaneTarget) => "pane.get", false, false;
    PaneFocus(PaneTarget) => "pane.focus", true, true;
    PaneInputSet(PaneInputSetParams) => "pane.input.set", true, true;
    PaneLinkActivate(PaneLinkActivateParams) => "pane.link.activate", false, true;
    PaneLinkResolve(PaneLinkActivateParams) => "pane.link.resolve", false, true;
    PaneRename(PaneRenameParams) => "pane.rename", true, true;
    PaneSendText(PaneSendTextParams) => "pane.send_text", false, false;
    PaneSendKeys(PaneSendKeysParams) => "pane.send_keys", false, false;
    PaneSendInput(PaneSendInputParams) => "pane.send_input", false, false;
    PaneRead(PaneReadParams) => "pane.read", false, false;
    PaneReportAgent(PaneReportAgentParams) => "pane.report_agent", true, false;
    PaneReportAgentSession(PaneReportAgentSessionParams) => "pane.report_agent_session", true, false;
    PaneReportMetadata(PaneReportMetadataParams) => "pane.report_metadata", true, false;
    PaneClearAgentAuthority(PaneClearAgentAuthorityParams) => "pane.clear_agent_authority", true, false;
    PaneReleaseAgent(PaneReleaseAgentParams) => "pane.release_agent", true, false;
    PaneClose(PaneTarget) => "pane.close", true, true;
    PopupClose(EmptyParams) => "popup.close", true, false;
    EventsSubscribe(EventsSubscribeParams) => "events.subscribe", false, false;
    EventsWait(EventsWaitParams) => "events.wait", false, false;
    PaneWaitForOutput(PaneWaitForOutputParams) => "pane.wait_for_output", false, false;
    IntegrationList(EmptyParams) => "integration.list", false, true;
    IntegrationInstall(IntegrationInstallParams) => "integration.install", false, true;
    IntegrationUninstall(IntegrationUninstallParams) => "integration.uninstall", false, false;
    PluginLink(PluginLinkParams) => "plugin.link", false, false;
    PluginList(PluginListParams) => "plugin.list", false, false;
    PluginUnlink(PluginUnlinkParams) => "plugin.unlink", true, false;
    PluginEnable(PluginSetEnabledParams) => "plugin.enable", false, false;
    PluginDisable(PluginSetEnabledParams) => "plugin.disable", true, false;
    PluginActionList(PluginActionListParams) => "plugin.action.list", false, false;
    PluginActionInvoke(PluginActionInvokeParams) => "plugin.action.invoke", true, false;
    PluginLogList(PluginLogListParams) => "plugin.log.list", false, false;
    PluginPaneOpen(PluginPaneOpenParams) => "plugin.pane.open", true, false;
    PluginPaneFocus(PluginPaneFocusParams) => "plugin.pane.focus", true, false;
    PluginPaneClose(PluginPaneCloseParams) => "plugin.pane.close", true, false;
}

#[cfg(test)]
mod tests;
