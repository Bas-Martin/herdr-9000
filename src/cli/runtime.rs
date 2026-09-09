use crate::api::schema::{
    EmptyParams, GitHubIssueSearchParams, GitHubIssueTaskCreateParams, Method,
    PaneFocusDirectionParams, PaneInputSetParams, PaneMoveParams, PaneRenameParams,
    PaneResizeParams, PaneSplitParams, PaneSwapParams, PaneTarget, PaneZoomParams,
    ProjectCreateParams, ProjectOpenParams, ProjectRenameParams, ProjectTarget, Request,
    ResourceCreateParams, ResourceListParams, ResourceTarget, ResourceUpdateParams,
    TabCreateParams, TabListParams, TabRenameParams, TabTarget, TaskChecksParams, TaskCreateParams,
    TaskDiffParams, TaskFileWriteParams, TaskGitActionParams, TaskListParams, TaskOpenParams,
    TaskRenameParams, TaskResourcesParams, TaskTarget, WorkspaceCloseParams, WorkspaceCreateParams,
    WorkspaceRenameParams, WorkspaceTarget, WorktreeCreateParams, WorktreeListParams,
    WorktreeOpenParams, WorktreeRemoveParams,
};

fn print_method_response(id: &'static str, method: Method) -> std::io::Result<i32> {
    super::print_response(&super::send_request(&Request {
        id: id.into(),
        method,
    })?)
}

pub(super) fn workspace_list() -> std::io::Result<i32> {
    print_method_response(
        "cli:workspace:list",
        Method::WorkspaceList(EmptyParams::default()),
    )
}

pub(super) fn workspace_create(params: WorkspaceCreateParams) -> std::io::Result<i32> {
    print_method_response("cli:workspace:create", Method::WorkspaceCreate(params))
}

pub(super) fn workspace_get(workspace_id: String) -> std::io::Result<i32> {
    print_method_response(
        "cli:workspace:get",
        Method::WorkspaceGet(WorkspaceTarget { workspace_id }),
    )
}

pub(super) fn workspace_focus(workspace_id: String) -> std::io::Result<i32> {
    print_method_response(
        "cli:workspace:focus",
        Method::WorkspaceFocus(WorkspaceTarget { workspace_id }),
    )
}

pub(super) fn workspace_rename(params: WorkspaceRenameParams) -> std::io::Result<i32> {
    print_method_response("cli:workspace:rename", Method::WorkspaceRename(params))
}

pub(super) fn workspace_close(params: WorkspaceCloseParams) -> std::io::Result<i32> {
    print_method_response("cli:workspace:close", Method::WorkspaceClose(params))
}

pub(super) fn project_list() -> std::io::Result<i32> {
    print_method_response(
        "cli:project:list",
        Method::ProjectList(EmptyParams::default()),
    )
}

pub(super) fn project_create(params: ProjectCreateParams) -> std::io::Result<i32> {
    print_method_response("cli:project:create", Method::ProjectCreate(params))
}

pub(super) fn project_get(target: ProjectTarget) -> std::io::Result<i32> {
    print_method_response("cli:project:get", Method::ProjectGet(target))
}

pub(super) fn project_open(params: ProjectOpenParams) -> std::io::Result<i32> {
    print_method_response("cli:project:open", Method::ProjectOpen(params))
}

pub(super) fn project_rename(params: ProjectRenameParams) -> std::io::Result<i32> {
    print_method_response("cli:project:rename", Method::ProjectRename(params))
}

pub(super) fn project_delete(target: ProjectTarget) -> std::io::Result<i32> {
    print_method_response("cli:project:delete", Method::ProjectDelete(target))
}

pub(super) fn task_list(params: TaskListParams) -> std::io::Result<i32> {
    print_method_response("cli:task:list", Method::TaskList(params))
}

pub(super) fn task_create(params: TaskCreateParams) -> std::io::Result<i32> {
    print_method_response("cli:task:create", Method::TaskCreate(params))
}

pub(super) fn task_open(params: TaskOpenParams) -> std::io::Result<i32> {
    print_method_response("cli:task:open", Method::TaskOpen(params))
}

pub(super) fn task_rename(params: TaskRenameParams) -> std::io::Result<i32> {
    print_method_response("cli:task:rename", Method::TaskRename(params))
}

pub(super) fn task_close(target: TaskTarget) -> std::io::Result<i32> {
    print_method_response("cli:task:close", Method::TaskClose(target))
}

pub(super) fn task_resources(params: TaskResourcesParams) -> std::io::Result<i32> {
    print_method_response("cli:task:resources", Method::TaskResources(params))
}

pub(super) fn resource_list(params: ResourceListParams) -> std::io::Result<i32> {
    print_method_response("cli:resource:list", Method::ResourceList(params))
}

pub(super) fn resource_create(params: ResourceCreateParams) -> std::io::Result<i32> {
    print_method_response("cli:resource:create", Method::ResourceCreate(params))
}

pub(super) fn resource_update(params: ResourceUpdateParams) -> std::io::Result<i32> {
    print_method_response("cli:resource:update", Method::ResourceUpdate(params))
}

pub(super) fn resource_delete(target: ResourceTarget) -> std::io::Result<i32> {
    print_method_response("cli:resource:delete", Method::ResourceDelete(target))
}
pub(super) fn task_diff(params: TaskDiffParams) -> std::io::Result<i32> {
    print_method_response("cli:task:diff", Method::TaskDiff(params))
}
pub(super) fn task_checks(params: TaskChecksParams) -> std::io::Result<i32> {
    print_method_response("cli:task:checks", Method::TaskChecks(params))
}

pub(super) fn task_file_write(params: TaskFileWriteParams) -> std::io::Result<i32> {
    print_method_response("cli:task:file-write", Method::TaskFileWrite(params))
}
pub(super) fn task_git_action(params: TaskGitActionParams) -> std::io::Result<i32> {
    print_method_response("cli:task:git-action", Method::TaskGitAction(params))
}
pub(super) fn github_issue_search(params: GitHubIssueSearchParams) -> std::io::Result<i32> {
    print_method_response("cli:task:github-search", Method::TaskGitHubSearch(params))
}

pub(super) fn github_issue_create(params: GitHubIssueTaskCreateParams) -> std::io::Result<i32> {
    print_method_response("cli:task:github-create", Method::TaskGitHubCreate(params))
}

pub(super) fn tab_list(params: TabListParams) -> std::io::Result<i32> {
    print_method_response("cli:tab:list", Method::TabList(params))
}

pub(super) fn tab_create(params: TabCreateParams) -> std::io::Result<i32> {
    print_method_response("cli:tab:create", Method::TabCreate(params))
}

pub(super) fn tab_get(tab_id: String) -> std::io::Result<i32> {
    print_method_response("cli:tab:get", Method::TabGet(TabTarget { tab_id }))
}

pub(super) fn tab_focus(tab_id: String) -> std::io::Result<i32> {
    print_method_response("cli:tab:focus", Method::TabFocus(TabTarget { tab_id }))
}

pub(super) fn tab_rename(params: TabRenameParams) -> std::io::Result<i32> {
    print_method_response("cli:tab:rename", Method::TabRename(params))
}

pub(super) fn tab_close(tab_id: String) -> std::io::Result<i32> {
    print_method_response("cli:tab:close", Method::TabClose(TabTarget { tab_id }))
}

pub(super) fn worktree_list(params: WorktreeListParams) -> std::io::Result<i32> {
    print_method_response("cli:worktree:list", Method::WorktreeList(params))
}

pub(super) fn worktree_create(params: WorktreeCreateParams) -> std::io::Result<i32> {
    print_method_response("cli:worktree:create", Method::WorktreeCreate(params))
}

pub(super) fn worktree_open(params: WorktreeOpenParams) -> std::io::Result<i32> {
    print_method_response("cli:worktree:open", Method::WorktreeOpen(params))
}

pub(super) fn worktree_remove(params: WorktreeRemoveParams) -> std::io::Result<i32> {
    print_method_response("cli:worktree:remove", Method::WorktreeRemove(params))
}

pub(super) fn pane_focus(params: PaneFocusDirectionParams) -> std::io::Result<i32> {
    print_method_response("cli:pane:focus", Method::PaneFocusDirection(params))
}

pub(super) fn pane_resize(params: PaneResizeParams) -> std::io::Result<i32> {
    print_method_response("cli:pane:resize", Method::PaneResize(params))
}

pub(super) fn pane_zoom(params: PaneZoomParams) -> std::io::Result<i32> {
    print_method_response("cli:pane:zoom", Method::PaneZoom(params))
}

pub(super) fn pane_rename(params: PaneRenameParams) -> std::io::Result<i32> {
    print_method_response("cli:pane:rename", Method::PaneRename(params))
}

pub(super) fn pane_input_set(params: PaneInputSetParams) -> std::io::Result<i32> {
    print_method_response("cli:pane:input:set", Method::PaneInputSet(params))
}

pub(super) fn pane_split(params: PaneSplitParams) -> std::io::Result<i32> {
    print_method_response("cli:pane:split", Method::PaneSplit(params))
}

pub(super) fn pane_swap(params: PaneSwapParams) -> std::io::Result<i32> {
    print_method_response("cli:pane:swap", Method::PaneSwap(params))
}

pub(super) fn pane_move(params: PaneMoveParams) -> std::io::Result<i32> {
    print_method_response("cli:pane:move", Method::PaneMove(params))
}

pub(super) fn pane_close(pane_id: String) -> std::io::Result<i32> {
    print_method_response("cli:pane:close", Method::PaneClose(PaneTarget { pane_id }))
}
