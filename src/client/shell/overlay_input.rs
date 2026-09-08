use super::*;

impl ClientShellState {
    pub(super) fn dismiss_product_announcement(&mut self, outcome: &mut ClientShellInput) {
        let announcement = match self.overlay.take() {
            Some(ClientShellOverlay::ProductAnnouncement(announcement)) => announcement,
            other => {
                self.overlay = other;
                return;
            }
        };
        if self.snapshot.is_none() {
            self.overlay = Some(ClientShellOverlay::ProductAnnouncement(announcement));
            return;
        }
        self.dismissed_product_announcement =
            Some((announcement.version.clone(), announcement.id.clone()));
        self.chrome_drag = None;
        self.push_endpoint_method_with_kind(
            crate::api::schema::Method::ProductAnnouncementDismiss(
                crate::api::schema::ProductAnnouncementDismissParams {
                    version: announcement.version.clone(),
                    id: announcement.id.clone(),
                },
            ),
            PendingEndpointKind::ProductAnnouncementDismiss {
                version: announcement.version,
                id: announcement.id,
            },
            outcome,
        );
        outcome.repaint = true;
    }

    pub(super) fn scroll_product_announcement(&mut self, delta: isize) {
        let max_scroll = self.hits.product_announcement_max_scroll;
        if let Some(ClientShellOverlay::ProductAnnouncement(announcement)) = self.overlay.as_mut() {
            let current = usize::from(announcement.scroll);
            let next = if delta.is_negative() {
                current.saturating_sub(delta.unsigned_abs())
            } else {
                current.saturating_add(delta as usize)
            }
            .min(max_scroll);
            announcement.scroll = u16::try_from(next).unwrap_or(u16::MAX);
        }
    }

    pub(super) fn set_product_announcement_offset_from_bottom(
        &mut self,
        offset_from_bottom: usize,
    ) {
        let max_scroll = self.hits.product_announcement_max_scroll;
        if let Some(ClientShellOverlay::ProductAnnouncement(announcement)) = self.overlay.as_mut() {
            announcement.scroll =
                u16::try_from(max_scroll.saturating_sub(offset_from_bottom.min(max_scroll)))
                    .unwrap_or(u16::MAX);
        }
    }

    pub(super) fn open_release_notes(&mut self) {
        let Some(notes) = self
            .snapshot
            .as_deref()
            .and_then(|snapshot| snapshot.release_notes.as_ref())
        else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::ReleaseNotes(release_notes_state(notes)));
        self.chrome_drag = None;
    }

    pub(super) fn dismiss_release_notes(&mut self, outcome: &mut ClientShellInput) {
        let notes = match self.overlay.take() {
            Some(ClientShellOverlay::ReleaseNotes(notes)) => notes,
            other => {
                self.overlay = other;
                return;
            }
        };
        self.chrome_drag = None;
        self.mode = if self
            .snapshot
            .as_deref()
            .and_then(|snapshot| snapshot.focused_workspace_id.as_deref())
            .is_some()
        {
            ClientShellMode::Terminal
        } else {
            ClientShellMode::Navigate
        };
        self.push_endpoint_method_with_kind(
            crate::api::schema::Method::ReleaseNotesDismiss(
                crate::api::schema::ReleaseNotesDismissParams {
                    version: notes.version.clone(),
                },
            ),
            PendingEndpointKind::ReleaseNotesDismiss,
            outcome,
        );
        outcome.repaint = true;
    }

    pub(super) fn current_release_notes_input_geometry(
        &self,
    ) -> Option<(Rect, Option<Rect>, crate::pane::ScrollMetrics)> {
        let notes = match self.overlay.as_ref()? {
            ClientShellOverlay::ReleaseNotes(notes) => notes,
            _ => return None,
        };
        let (cols, rows) = self.last_composed_size?;
        let outer = crate::ui::centered_popup_rect(
            Rect::new(0, 0, cols, rows),
            crate::ui::RELEASE_NOTES_MODAL_SIZE.0,
            crate::ui::RELEASE_NOTES_MODAL_SIZE.1,
        )?;
        let inner = Rect::new(
            outer.x.saturating_add(1),
            outer.y.saturating_add(1),
            outer.width.saturating_sub(2),
            outer.height.saturating_sub(2),
        );
        if inner.height < 8 || inner.width < 20 {
            return None;
        }
        let stack = crate::ui::modal_stack_areas(inner, 2, 1, 0, 1);
        let close = crate::ui::release_notes_close_button_rect(Rect::new(
            stack.header.x,
            stack.header.y,
            stack.header.width,
            1,
        ));
        let install_command = self
            .snapshot
            .as_deref()
            .map(|snapshot| snapshot.update_install_command.as_str())
            .unwrap_or_default();
        let metrics = crate::ui::release_notes_scroll_metrics(
            notes,
            install_command,
            stack.content,
            &self.config.palette,
        );
        let track = crate::ui::release_notes_scrollbar_rect(stack.content, metrics);
        Some((close, track, metrics))
    }

    fn current_release_notes_max_scroll(&self) -> usize {
        self.current_release_notes_input_geometry()
            .map(|(_, _, metrics)| metrics.max_offset_from_bottom)
            .unwrap_or(self.hits.release_notes_max_scroll)
    }

    pub(super) fn scroll_release_notes(&mut self, delta: isize) {
        let max_scroll = self.current_release_notes_max_scroll();
        if let Some(ClientShellOverlay::ReleaseNotes(notes)) = self.overlay.as_mut() {
            let current = usize::from(notes.scroll);
            let next = if delta.is_negative() {
                current.saturating_sub(delta.unsigned_abs())
            } else {
                current.saturating_add(delta as usize)
            }
            .min(max_scroll);
            notes.scroll = u16::try_from(next).unwrap_or(u16::MAX);
        }
    }

    pub(super) fn set_release_notes_offset_from_bottom(&mut self, offset_from_bottom: usize) {
        let max_scroll = self.current_release_notes_max_scroll();
        if let Some(ClientShellOverlay::ReleaseNotes(notes)) = self.overlay.as_mut() {
            notes.scroll =
                u16::try_from(max_scroll.saturating_sub(offset_from_bottom.min(max_scroll)))
                    .unwrap_or(u16::MAX);
        }
    }

    pub(super) fn complete_onboarding(&mut self, outcome: &mut ClientShellInput) {
        if self.snapshot.is_none() {
            return;
        }
        if let Err(error) = crate::config::update_file_at(
            &self.config.local_config_path,
            "onboarding setting",
            |content| crate::config::upsert_top_level_bool(content, "onboarding", false),
        ) {
            self.set_local_config_diagnostic(Some(error));
        }
        self.config.startup_onboarding = false;
        self.open_settings_overlay();
        self.select_settings_section(ClientSettingsSection::Integrations, outcome);
    }

    pub(super) fn open_navigator_overlay(&mut self) {
        let expanded_workspaces =
            super::aggregate_navigation::cached_endpoint_snapshots(&self.endpoints)
                .flat_map(|endpoint| {
                    endpoint.snapshot.workspaces.iter().map(move |workspace| {
                        (endpoint.endpoint_id.clone(), workspace.workspace_id.clone())
                    })
                })
                .collect();
        let mut navigator = ClientNavigatorOverlay {
            query: String::new(),
            search_focused: false,
            selected: None,
            scroll: 0,
            filter: None,
            expanded_workspaces,
        };
        let rows =
            render::client_navigator_rows(&self.endpoints, &self.active_endpoint_id, &navigator);
        navigator.selected = rows
            .iter()
            .find(|row| row.current)
            .map(|row| row.target.clone());
        self.overlay = Some(ClientShellOverlay::Navigator(navigator));
    }

    pub(super) fn move_navigator_selection(&mut self, delta: isize) {
        let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() else {
            return;
        };
        let rows =
            render::client_navigator_rows(&self.endpoints, &self.active_endpoint_id, navigator);
        if rows.is_empty() {
            navigator.selected = None;
            return;
        }
        let selected =
            super::aggregate_navigation::navigator_selected_index(&rows, navigator).unwrap_or(0);
        let next =
            (selected as isize + delta).clamp(0, rows.len().saturating_sub(1) as isize) as usize;
        navigator.selected = Some(rows[next].target.clone());
    }

    pub(super) fn accept_navigator_selection(&mut self, outcome: &mut ClientShellInput) {
        let target = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::Navigator(navigator) => {
                let rows = render::client_navigator_rows(
                    &self.endpoints,
                    &self.active_endpoint_id,
                    navigator,
                );
                super::aggregate_navigation::selected_navigator_target(&rows, navigator)
            }
            _ => None,
        });
        let Some(target) = target else {
            return;
        };
        let activated = match target {
            ClientNavigatorTarget::Machine { endpoint_id } => {
                self.activate_endpoint(endpoint_id, outcome)
            }
            ClientNavigatorTarget::Workspace {
                endpoint_id,
                workspace_id,
            } => self.focus_or_activate(
                endpoint_id,
                ClientEndpointFocusTarget::Workspace(workspace_id),
                outcome,
            ),
            ClientNavigatorTarget::Tab {
                endpoint_id,
                tab_id,
            } => {
                self.focus_or_activate(endpoint_id, ClientEndpointFocusTarget::Tab(tab_id), outcome)
            }
            ClientNavigatorTarget::Pane {
                endpoint_id,
                pane_id,
            } => self.focus_or_activate(
                endpoint_id,
                ClientEndpointFocusTarget::Pane(pane_id),
                outcome,
            ),
        };
        if activated {
            self.overlay = None;
        }
        outcome.repaint = true;
    }

    pub(super) fn toggle_selected_navigator_workspace(&mut self) {
        let workspace_key = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::Navigator(navigator) => {
                let rows = render::client_navigator_rows(
                    &self.endpoints,
                    &self.active_endpoint_id,
                    navigator,
                );
                super::aggregate_navigation::selected_navigator_target(&rows, navigator).and_then(
                    |target| match target {
                        ClientNavigatorTarget::Workspace {
                            endpoint_id,
                            workspace_id,
                        } => Some((endpoint_id, workspace_id)),
                        _ => None,
                    },
                )
            }
            _ => None,
        });
        if let (Some(workspace_key), Some(ClientShellOverlay::Navigator(navigator))) =
            (workspace_key, self.overlay.as_mut())
        {
            if !navigator.expanded_workspaces.remove(&workspace_key) {
                navigator.expanded_workspaces.insert(workspace_key);
            }
            navigator.selected = None;
            navigator.scroll = 0;
        }
    }

    pub(super) fn workspace_action_id(&self) -> Option<String> {
        self.navigate_workspace_id.clone().or_else(|| {
            self.snapshot
                .as_deref()
                .and_then(|snapshot| snapshot.focused_workspace_id.clone())
        })
    }

    pub(super) fn open_new_workspace_overlay(&mut self) {
        let name = String::new();
        let root_path = {
            #[cfg(windows)]
            {
                "C:\\Repository\\".to_owned()
            }
            #[cfg(not(windows))]
            {
                String::new()
            }
        };
        self.overlay = Some(ClientShellOverlay::ProjectCreate(
            ClientProjectCreateOverlay {
                project_id: None,
                name,
                root_path,
                worktree_root: String::new(),
                field: ClientProjectCreateField::Name,
                error: None,
                submitting: false,
            },
        ));
    }

    pub(super) fn open_project_settings_overlay(
        &mut self,
        workspace_id: String,
        outcome: &mut ClientShellInput,
    ) {
        self.overlay = Some(ClientShellOverlay::ProjectSettings(
            ClientProjectSettingsOverlay {
                workspace_id: workspace_id.clone(),
                project: None,
                loading: true,
                error: None,
            },
        ));
        let sent = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::ProjectList(crate::api::schema::EmptyParams::default()),
            PendingEndpointKind::ProjectList { workspace_id },
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::ProjectSettings(settings)) = self.overlay.as_mut() {
                settings.loading = false;
                settings.error = Some("project list is unavailable".to_owned());
            }
        }
    }

    pub(super) fn open_project_rename_overlay(&mut self) {
        let Some(project) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::ProjectSettings(settings) => settings.project.clone(),
            _ => None,
        }) else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "rename project",
            input: project.name.clone(),
            replace_on_type: false,
            target: ClientRenameTarget::Project {
                project_id: project.project_id.clone(),
            },
        }));
    }

    pub(super) fn open_project_base_overlay(&mut self) {
        let Some(project) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::ProjectSettings(settings) => settings.project.clone(),
            _ => None,
        }) else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "worktree base",
            input: project
                .worktree_base
                .unwrap_or_else(|| crate::project::DEFAULT_WORKTREE_BASE.to_owned()),
            replace_on_type: false,
            target: ClientRenameTarget::ProjectBase {
                project_id: project.project_id,
                name: project.name,
                root_path: project.root_path,
                worktree_root: project.worktree_root,
            },
        }));
    }
    pub(super) fn open_project_default_agent_overlay(&mut self) {
        let Some(project) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::ProjectSettings(settings) => settings.project.clone(),
            _ => None,
        }) else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "default agent",
            input: project.default_agent.unwrap_or_default(),
            replace_on_type: false,
            target: ClientRenameTarget::ProjectDefaultAgent {
                project_id: project.project_id,
                name: project.name,
                root_path: project.root_path,
                worktree_root: project.worktree_root,
            },
        }));
    }
    pub(super) fn open_project_patterns_overlay(&mut self) {
        let Some(project) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::ProjectSettings(settings) => settings.project.clone(),
            _ => None,
        }) else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::ProjectPatterns(
            ClientProjectPatternsOverlay {
                project_id: project.project_id,
                name: project.name,
                root_path: project.root_path,
                worktree_root: project.worktree_root,
                worktree_base: project.worktree_base,
                patterns: project.preserve_patterns,
                selected: 0,
                input: String::new(),
                editing: false,
                error: None,
            },
        ));
    }

    pub(super) fn open_project_edit_overlay(&mut self) {
        let Some(project) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::ProjectSettings(settings) => settings.project.clone(),
            _ => None,
        }) else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::ProjectCreate(
            ClientProjectCreateOverlay {
                project_id: Some(project.project_id),
                name: project.name,
                root_path: project.root_path,
                worktree_root: project.worktree_root.unwrap_or_default(),
                field: ClientProjectCreateField::RootPath,
                error: None,
                submitting: false,
            },
        ));
    }

    pub(super) fn open_rename_workspace_overlay(&mut self) {
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        let Some(workspace_id) = self.workspace_action_id() else {
            return;
        };
        let Some(workspace) = snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.workspace_id == workspace_id)
        else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "rename workspace",
            input: workspace.label.clone(),
            replace_on_type: false,
            target: ClientRenameTarget::Workspace { workspace_id },
        }));
    }

    pub(super) fn open_new_tab_overlay(&mut self) {
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        let Some(workspace_id) = snapshot.focused_workspace_id.clone() else {
            return;
        };
        let default_name = (snapshot
            .tabs
            .iter()
            .filter(|tab| tab.workspace_id == workspace_id)
            .count()
            + 1)
        .to_string();
        self.overlay = Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "new tab",
            input: default_name.clone(),
            replace_on_type: true,
            target: ClientRenameTarget::NewTab {
                workspace_id,
                default_name,
            },
        }));
    }

    pub(super) fn open_rename_tab_overlay(&mut self) {
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        let Some(tab_id) = snapshot.focused_tab_id.as_deref() else {
            return;
        };
        let Some(tab) = snapshot.tabs.iter().find(|tab| tab.tab_id == tab_id) else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "rename tab",
            input: tab.label.clone(),
            replace_on_type: false,
            target: ClientRenameTarget::Tab {
                tab_id: tab.tab_id.clone(),
                auto_name: !tab.custom_label,
                original_name: tab.label.clone(),
            },
        }));
    }

    pub(super) fn open_rename_pane_overlay(&mut self) {
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        let Some(pane_id) = snapshot.focused_pane_id.as_deref() else {
            return;
        };
        let Some(pane) = snapshot.panes.iter().find(|pane| pane.pane_id == pane_id) else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "rename pane",
            input: pane.label.clone().unwrap_or_default(),
            replace_on_type: pane.label.is_none(),
            target: ClientRenameTarget::Pane {
                pane_id: pane.pane_id.clone(),
            },
        }));
    }

    pub(super) fn insert_overlay_text(&mut self, text: &str) -> bool {
        if self.insert_worktree_overlay_text(text) {
            return true;
        }
        match self.overlay.as_mut() {
            Some(ClientShellOverlay::ProjectCreate(project)) => {
                let value = match project.field {
                    ClientProjectCreateField::Name => &mut project.name,
                    ClientProjectCreateField::RootPath => &mut project.root_path,
                    ClientProjectCreateField::WorktreeRoot => &mut project.worktree_root,
                };
                value.push_str(text);
                project.error = None;
                true
            }
            Some(ClientShellOverlay::Rename(rename)) => {
                if rename.replace_on_type {
                    rename.input.clear();
                    rename.replace_on_type = false;
                }
                rename.input.push_str(text);
                true
            }
            Some(ClientShellOverlay::Help(help)) if help.search_focused => {
                help.query
                    .extend(text.chars().filter(|character| !character.is_control()));
                help.scroll = 0;
                true
            }
            Some(ClientShellOverlay::Navigator(navigator)) if navigator.search_focused => {
                navigator.query.push_str(text);
                navigator.filter = None;
                navigator.selected = None;

                true
            }
            _ => false,
        }
    }

    fn route_project_create_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        use crossterm::event::{KeyCode, KeyModifiers};
        if key.code == KeyCode::Esc {
            self.overlay = None;
            outcome.repaint = true;
            return;
        }
        if key.code == KeyCode::Tab
            || key.code == KeyCode::BackTab
            || key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            if let Some(ClientShellOverlay::ProjectCreate(project)) = self.overlay.as_mut() {
                project.field = match project.field {
                    ClientProjectCreateField::Name => ClientProjectCreateField::RootPath,
                    ClientProjectCreateField::RootPath => ClientProjectCreateField::WorktreeRoot,
                    ClientProjectCreateField::WorktreeRoot => ClientProjectCreateField::Name,
                };
                outcome.repaint = true;
            }
            return;
        }
        if key.code == KeyCode::Enter {
            let field = match self.overlay.as_ref() {
                Some(ClientShellOverlay::ProjectCreate(project)) => project.field,
                _ => return,
            };
            match field {
                ClientProjectCreateField::Name => {
                    if let Some(ClientShellOverlay::ProjectCreate(project)) = self.overlay.as_mut()
                    {
                        project.field = ClientProjectCreateField::RootPath;
                        outcome.repaint = true;
                    }
                }
                ClientProjectCreateField::RootPath => {
                    if let Some(ClientShellOverlay::ProjectCreate(project)) = self.overlay.as_mut()
                    {
                        project.field = ClientProjectCreateField::WorktreeRoot;
                        outcome.repaint = true;
                    }
                }
                ClientProjectCreateField::WorktreeRoot => self.submit_project_create(outcome),
            }
            return;
        }
        if key.code == KeyCode::Backspace {
            if let Some(ClientShellOverlay::ProjectCreate(project)) = self.overlay.as_mut() {
                let value = match project.field {
                    ClientProjectCreateField::Name => &mut project.name,
                    ClientProjectCreateField::RootPath => &mut project.root_path,
                    ClientProjectCreateField::WorktreeRoot => &mut project.worktree_root,
                };
                value.pop();
                project.error = None;
                outcome.repaint = true;
            }
            return;
        }
        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let Some(ClientShellOverlay::ProjectCreate(project)) = self.overlay.as_mut() {
                match project.field {
                    ClientProjectCreateField::Name => project.name.clear(),
                    ClientProjectCreateField::RootPath => project.root_path.clear(),
                    ClientProjectCreateField::WorktreeRoot => project.worktree_root.clear(),
                }
                project.error = None;
                outcome.repaint = true;
            }
            return;
        }
        if let KeyCode::Char(character) = key.code {
            if key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
                if let Some(text) = key.generated_text.as_deref() {
                    self.insert_overlay_text(text);
                } else {
                    let text = character.to_string();
                    self.insert_overlay_text(&text);
                }
                outcome.repaint = true;
            }
        }
    }

    pub(super) fn submit_project_create(&mut self, outcome: &mut ClientShellInput) {
        let Some(ClientShellOverlay::ProjectCreate(project)) = self.overlay.as_mut() else {
            return;
        };
        if project.submitting {
            return;
        }
        let project_id = project.project_id.clone();
        let name = project.name.trim().to_owned();
        let root_path = project.root_path.trim().to_owned();
        if name.is_empty() {
            project.error = Some("Project name is required.".to_owned());
            outcome.repaint = true;
            return;
        }
        if root_path.is_empty() {
            project.error = Some("Repository or folder path is required.".to_owned());
            project.field = ClientProjectCreateField::RootPath;
            outcome.repaint = true;
            return;
        }
        let worktree_root = project.worktree_root.trim().to_owned();
        let editing = project_id.is_some();
        let method = if let Some(project_id) = project_id {
            crate::api::schema::Method::ProjectUpdate(crate::api::schema::ProjectUpdateParams {
                project_id,
                name,
                root_path,
                worktree_root: Some(worktree_root),
                worktree_base: None,
                default_agent: None,
                preserve_patterns: None,
                lifecycle: None,
            })
        } else {
            crate::api::schema::Method::ProjectCreate(crate::api::schema::ProjectCreateParams {
                name,
                root_path,
                open: true,
                focus: true,
                worktree_root: (!worktree_root.is_empty()).then_some(worktree_root),
                worktree_base: None,
                default_agent: None,
                preserve_patterns: None,
                lifecycle: None,
            })
        };
        project.submitting = true;
        project.error = None;
        let sent = self.push_endpoint_method_with_kind(
            method,
            if editing {
                PendingEndpointKind::ProjectUpdate
            } else {
                PendingEndpointKind::ProjectCreate
            },
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::ProjectCreate(project)) = self.overlay.as_mut() {
                project.submitting = false;
                project.error = Some("The project endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }
    pub(super) fn route_overlay_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        use crossterm::event::KeyModifiers;

        if matches!(self.overlay, Some(ClientShellOverlay::Onboarding)) {
            if matches!(
                key.code,
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l')
            ) {
                self.complete_onboarding(outcome);
            }
            return;
        }

        if matches!(
            self.overlay,
            Some(ClientShellOverlay::ProductAnnouncement(_))
        ) {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => self.dismiss_product_announcement(outcome),
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll_product_announcement(-1);
                    outcome.repaint = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll_product_announcement(1);
                    outcome.repaint = true;
                }
                KeyCode::PageUp => {
                    self.scroll_product_announcement(-8);
                    outcome.repaint = true;
                }
                KeyCode::PageDown => {
                    self.scroll_product_announcement(8);
                    outcome.repaint = true;
                }
                KeyCode::Home => {
                    if let Some(ClientShellOverlay::ProductAnnouncement(announcement)) =
                        self.overlay.as_mut()
                    {
                        announcement.scroll = 0;
                    }
                    outcome.repaint = true;
                }
                KeyCode::End => {
                    if let Some(ClientShellOverlay::ProductAnnouncement(announcement)) =
                        self.overlay.as_mut()
                    {
                        announcement.scroll =
                            u16::try_from(self.hits.product_announcement_max_scroll)
                                .unwrap_or(u16::MAX);
                    }
                    outcome.repaint = true;
                }
                _ => {}
            }
            return;
        }

        if matches!(self.overlay, Some(ClientShellOverlay::ReleaseNotes(_))) {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => self.dismiss_release_notes(outcome),
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll_release_notes(-1);
                    outcome.repaint = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll_release_notes(1);
                    outcome.repaint = true;
                }
                KeyCode::PageUp => {
                    self.scroll_release_notes(-8);
                    outcome.repaint = true;
                }
                KeyCode::PageDown => {
                    self.scroll_release_notes(8);
                    outcome.repaint = true;
                }
                KeyCode::Home => {
                    if let Some(ClientShellOverlay::ReleaseNotes(notes)) = self.overlay.as_mut() {
                        notes.scroll = 0;
                    }
                    outcome.repaint = true;
                }
                KeyCode::End => {
                    let max_scroll = self.current_release_notes_max_scroll();
                    if let Some(ClientShellOverlay::ReleaseNotes(notes)) = self.overlay.as_mut() {
                        notes.scroll = u16::try_from(max_scroll).unwrap_or(u16::MAX);
                    }
                    outcome.repaint = true;
                }
                _ => {}
            }
            return;
        }

        if matches!(self.overlay, Some(ClientShellOverlay::GlobalMenu(_))) {
            match key.code {
                KeyCode::Esc => {
                    self.overlay = None;
                    outcome.repaint = true;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.move_global_menu_selection(-1);
                    outcome.repaint = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.move_global_menu_selection(1);
                    outcome.repaint = true;
                }
                KeyCode::Enter => {
                    let highlighted = match self.overlay.as_ref() {
                        Some(ClientShellOverlay::GlobalMenu(menu)) => menu.highlighted,
                        _ => return,
                    };
                    self.activate_global_menu_item(highlighted, outcome);
                }
                _ => {}
            }
            return;
        }

        if self.route_settings_key(key, outcome) {
            return;
        }

        if matches!(self.overlay, Some(ClientShellOverlay::ContextMenu(_))) {
            match key.code {
                KeyCode::Esc => {
                    self.overlay = None;
                    outcome.repaint = true;
                }
                KeyCode::Up => {
                    self.move_context_menu_selection(-1);
                    outcome.repaint = true;
                }
                KeyCode::Down => {
                    self.move_context_menu_selection(1);
                    outcome.repaint = true;
                }
                KeyCode::Enter => {
                    let highlighted = match self.overlay.as_ref() {
                        Some(ClientShellOverlay::ContextMenu(menu)) => menu.highlighted,
                        _ => return,
                    };
                    self.activate_context_menu_item(highlighted, outcome);
                }
                _ => {}
            }
            return;
        }

        if self.route_worktree_overlay_key(key, outcome) {
            return;
        }
        if matches!(self.overlay, Some(ClientShellOverlay::Navigator(_))) {
            let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
            let search_focused = matches!(
                self.overlay,
                Some(ClientShellOverlay::Navigator(ClientNavigatorOverlay {
                    search_focused: true,
                    ..
                }))
            );
            if code == KeyCode::Esc {
                if search_focused {
                    if let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() {
                        navigator.search_focused = false;
                    }
                } else {
                    self.overlay = None;
                }
                outcome.repaint = true;
                return;
            }
            if code == KeyCode::Enter {
                self.accept_navigator_selection(outcome);
                return;
            }
            if search_focused {
                if code == KeyCode::Up
                    || code == KeyCode::Char('p') && modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.move_navigator_selection(-1);
                    outcome.repaint = true;
                    return;
                }
                if code == KeyCode::Down
                    || code == KeyCode::Char('n') && modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.move_navigator_selection(1);
                    outcome.repaint = true;
                    return;
                }
                if let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() {
                    if code == KeyCode::Char('u') && modifiers.contains(KeyModifiers::CONTROL) {
                        navigator.query.clear();
                        navigator.filter = None;
                        navigator.selected = None;
                    } else if code == KeyCode::Backspace {
                        navigator.query.pop();
                        navigator.filter = None;
                        navigator.selected = None;
                    } else if let KeyCode::Char(character) = code {
                        if modifiers.difference(KeyModifiers::SHIFT).is_empty() {
                            navigator.filter = None;
                            if let Some(text) = key.generated_text.as_deref() {
                                navigator.query.push_str(text);
                            } else {
                                navigator.query.push(character);
                            }
                            navigator.selected = None;
                        }
                    }
                    outcome.repaint = true;
                }
                return;
            }
            if code == KeyCode::Backspace && modifiers.is_empty() {
                if let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() {
                    if navigator.filter.take().is_some() {
                        navigator.selected = None;
                    }
                }
                outcome.repaint = true;
                return;
            }
            if code == KeyCode::Home && modifiers.is_empty() {
                if let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() {
                    navigator.selected = None;
                    navigator.scroll = 0;
                }
                outcome.repaint = true;
                return;
            }
            if matches!(code, KeyCode::End | KeyCode::Char('G')) && modifiers.is_empty() {
                let last = self.overlay.as_ref().and_then(|overlay| match overlay {
                    ClientShellOverlay::Navigator(navigator) => render::client_navigator_rows(
                        &self.endpoints,
                        &self.active_endpoint_id,
                        navigator,
                    )
                    .last()
                    .map(|row| row.target.clone()),
                    _ => None,
                });
                if let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() {
                    navigator.selected = last;
                }
                outcome.repaint = true;
                return;
            }
            if code == KeyCode::Char('/') && modifiers.is_empty() {
                if let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() {
                    navigator.search_focused = true;
                    navigator.filter = None;
                }
                outcome.repaint = true;
                return;
            }
            if matches!(code, KeyCode::Down | KeyCode::Char('j')) && modifiers.is_empty() {
                self.move_navigator_selection(1);
                outcome.repaint = true;
                return;
            }
            if matches!(code, KeyCode::Up | KeyCode::Char('k')) && modifiers.is_empty() {
                self.move_navigator_selection(-1);
                outcome.repaint = true;
                return;
            }
            if code == KeyCode::Char('d') && modifiers.contains(KeyModifiers::CONTROL) {
                self.move_navigator_selection(8);
                outcome.repaint = true;
                return;
            }
            if code == KeyCode::Char('u') && modifiers.contains(KeyModifiers::CONTROL) {
                self.move_navigator_selection(-8);
                outcome.repaint = true;
                return;
            }
            if let Some(filter) = match code {
                KeyCode::Char('b') if modifiers.is_empty() => Some(ClientNavigatorFilter::Blocked),
                KeyCode::Char('w') if modifiers.is_empty() => Some(ClientNavigatorFilter::Working),
                KeyCode::Char('i') if modifiers.is_empty() => Some(ClientNavigatorFilter::Idle),
                KeyCode::Char('d') if modifiers.is_empty() => Some(ClientNavigatorFilter::Done),
                _ => None,
            } {
                if let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() {
                    navigator.query.clear();
                    navigator.filter = Some(filter);
                    navigator.selected = None;
                }
                outcome.repaint = true;
                return;
            }
            if code == KeyCode::Char('a') && modifiers.is_empty() {
                if let Some(ClientShellOverlay::Navigator(navigator)) = self.overlay.as_mut() {
                    navigator.query.clear();
                    navigator.filter = None;
                    navigator.selected = None;
                }
                outcome.repaint = true;
                return;
            }
            if code == KeyCode::Char(' ') && modifiers.is_empty() {
                self.toggle_selected_navigator_workspace();
                outcome.repaint = true;
                return;
            }
            return;
        }

        if matches!(self.overlay, Some(ClientShellOverlay::Help(_))) {
            let text_character = crate::input::keybind_help_text_char(key);
            let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
            let search_focused = matches!(
                self.overlay,
                Some(ClientShellOverlay::Help(ClientHelpOverlay {
                    search_focused: true,
                    ..
                }))
            );
            if search_focused {
                match code {
                    KeyCode::Esc => {
                        if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                            help.search_focused = false;
                            help.query.clear();
                            help.scroll = 0;
                        }
                    }
                    KeyCode::Enter => self.overlay = None,
                    KeyCode::Home => {
                        if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                            help.scroll = 0;
                        }
                    }
                    KeyCode::End => {
                        if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                            help.scroll = self.hits.help_max_scroll;
                        }
                    }
                    KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                        let delta = match code {
                            KeyCode::Up => -1,
                            KeyCode::Down => 1,
                            KeyCode::PageUp => -8,
                            KeyCode::PageDown => 8,
                            _ => unreachable!(),
                        };
                        if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                            help.scroll = help
                                .scroll
                                .saturating_add_signed(delta)
                                .min(self.hits.help_max_scroll);
                        }
                    }
                    KeyCode::Backspace => {
                        if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                            help.query.pop();
                            help.scroll = 0;
                        }
                    }
                    KeyCode::Char('u') if modifiers == KeyModifiers::CONTROL => {
                        if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                            help.query.clear();
                            help.scroll = 0;
                        }
                    }
                    _ => {
                        if let Some(character) = text_character {
                            if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                                help.query.push(character);
                                help.scroll = 0;
                            }
                        }
                    }
                }
                outcome.repaint = true;
                return;
            }

            match code {
                KeyCode::Esc | KeyCode::Enter => self.overlay = None,
                KeyCode::Home => {
                    if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                        help.scroll = 0;
                    }
                }
                KeyCode::End => {
                    if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                        help.scroll = self.hits.help_max_scroll;
                    }
                }
                KeyCode::Up
                | KeyCode::Char('k')
                | KeyCode::Down
                | KeyCode::Char('j')
                | KeyCode::PageUp
                | KeyCode::PageDown => {
                    let delta = match code {
                        KeyCode::Up | KeyCode::Char('k') => -1,
                        KeyCode::Down | KeyCode::Char('j') => 1,
                        KeyCode::PageUp => -8,
                        KeyCode::PageDown => 8,
                        _ => unreachable!(),
                    };
                    if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                        help.scroll = help
                            .scroll
                            .saturating_add_signed(delta)
                            .min(self.hits.help_max_scroll);
                    }
                }
                _ if text_character == Some('/') => {
                    if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
                        help.search_focused = true;
                        help.scroll = 0;
                    }
                }
                _ if text_character == Some('?') => self.overlay = None,
                _ => {}
            }
            outcome.repaint = true;
            return;
        }

        if matches!(self.overlay, Some(ClientShellOverlay::ConfirmClose(_))) {
            if key.code == KeyCode::Enter {
                let Some(ClientShellOverlay::ConfirmClose(confirm)) = self.overlay.take() else {
                    return;
                };
                self.push_endpoint_method(
                    crate::api::schema::Method::WorkspaceClose(
                        crate::api::schema::WorkspaceCloseParams {
                            workspace_id: confirm.workspace_id,
                            close_group: true,
                        },
                    ),
                    outcome,
                );
                outcome.repaint = true;
            } else if key.code == KeyCode::Esc {
                self.overlay = None;
                self.mode = ClientShellMode::Navigate;
                self.navigate_workspace_id = self
                    .snapshot
                    .as_deref()
                    .and_then(|snapshot| snapshot.focused_workspace_id.clone());
                outcome.repaint = true;
            }
            return;
        }
        if matches!(self.overlay, Some(ClientShellOverlay::ProjectCreate(_))) {
            self.route_project_create_key(key, outcome);
            return;
        }

        if matches!(self.overlay, Some(ClientShellOverlay::ProjectPatterns(_))) {
            self.route_project_patterns_key(key, outcome);
            return;
        }

        if matches!(self.overlay, Some(ClientShellOverlay::ProjectSettings(_))) {
            match key.code {
                KeyCode::Esc => self.overlay = None,
                KeyCode::Char('e') | KeyCode::Char('E') => self.open_project_edit_overlay(),
                KeyCode::Char('b') | KeyCode::Char('B') => self.open_project_base_overlay(),
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.open_project_default_agent_overlay()
                }
                KeyCode::Char('p') | KeyCode::Char('P') => self.open_project_patterns_overlay(),
                KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.open_project_rename_overlay()
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    let project_id = self.overlay.as_ref().and_then(|overlay| match overlay {
                        ClientShellOverlay::ProjectSettings(settings) => settings
                            .project
                            .as_ref()
                            .map(|project| project.project_id.clone()),
                        _ => None,
                    });
                    if let Some(project_id) = project_id {
                        self.overlay = None;
                        self.push_endpoint_method(
                            crate::api::schema::Method::ProjectOpen(
                                crate::api::schema::ProjectOpenParams {
                                    project_id,
                                    focus: true,
                                },
                            ),
                            outcome,
                        );
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    let project_id = self.overlay.as_ref().and_then(|overlay| match overlay {
                        ClientShellOverlay::ProjectSettings(settings) => settings
                            .project
                            .as_ref()
                            .map(|project| project.project_id.clone()),
                        _ => None,
                    });
                    if let Some(project_id) = project_id {
                        self.overlay = None;
                        self.push_endpoint_method(
                            crate::api::schema::Method::ProjectDelete(
                                crate::api::schema::ProjectTarget { project_id },
                            ),
                            outcome,
                        );
                    }
                }
                _ => {}
            }
            outcome.repaint = true;
            return;
        }

        let Some(ClientShellOverlay::Rename(rename)) = self.overlay.as_mut() else {
            return;
        };
        if key.code == KeyCode::Enter {
            self.save_rename_overlay(outcome);
            return;
        }
        if key.code == KeyCode::Esc {
            self.overlay = None;
            outcome.repaint = true;
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            rename.input.clear();
            rename.replace_on_type = false;
            outcome.repaint = true;
            return;
        }
        if (key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL))
            || (key.code == KeyCode::Backspace && key.modifiers.contains(KeyModifiers::SUPER))
        {
            rename.input.clear();
            rename.replace_on_type = false;
            outcome.repaint = true;
            return;
        }
        if key.code == KeyCode::Backspace
            && (key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT))
            || matches!(key.code, KeyCode::Char('h' | 'w'))
                && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            delete_overlay_word(rename);
            outcome.repaint = true;
            return;
        }
        if key.code == KeyCode::Backspace {
            if rename.replace_on_type {
                rename.input.clear();
                rename.replace_on_type = false;
            } else {
                rename.input.pop();
            }
            outcome.repaint = true;
            return;
        }
        if let KeyCode::Char(character) = key.code {
            if key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
                if rename.replace_on_type {
                    rename.input.clear();
                    rename.replace_on_type = false;
                }
                if let Some(text) = key.generated_text.as_deref() {
                    rename.input.push_str(text);
                } else {
                    rename.input.push(character);
                }
                outcome.repaint = true;
            }
        }
    }

    fn route_project_patterns_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) {
        use crossterm::event::{KeyCode, KeyModifiers};
        let editing = matches!(
            self.overlay,
            Some(ClientShellOverlay::ProjectPatterns(
                ClientProjectPatternsOverlay { editing: true, .. }
            ))
        );
        if editing {
            match key.code {
                KeyCode::Esc => {
                    if let Some(ClientShellOverlay::ProjectPatterns(patterns)) =
                        self.overlay.as_mut()
                    {
                        patterns.editing = false;
                        patterns.input.clear();
                        patterns.error = None;
                    }
                }
                KeyCode::Enter => self.commit_project_pattern_edit(),
                KeyCode::Backspace => {
                    if let Some(ClientShellOverlay::ProjectPatterns(patterns)) =
                        self.overlay.as_mut()
                    {
                        patterns.input.pop();
                    }
                }
                KeyCode::Char(character)
                    if key.modifiers.difference(KeyModifiers::SHIFT).is_empty() =>
                {
                    if let Some(ClientShellOverlay::ProjectPatterns(patterns)) =
                        self.overlay.as_mut()
                    {
                        if let Some(text) = key.generated_text.as_deref() {
                            patterns.input.push_str(text);
                        } else {
                            patterns.input.push(character);
                        }
                    }
                }
                _ => {}
            }
            outcome.repaint = true;
            return;
        }

        match key.code {
            KeyCode::Esc => self.overlay = None,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                if let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_mut() {
                    patterns.selected = patterns.selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                if let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_mut() {
                    if !patterns.patterns.is_empty() {
                        patterns.selected =
                            (patterns.selected + 1).min(patterns.patterns.len() - 1);
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                if let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_mut() {
                    patterns.selected = patterns.patterns.len();
                    patterns.input.clear();
                    patterns.editing = true;
                    patterns.error = None;
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_mut() {
                    if let Some(pattern) = patterns.patterns.get(patterns.selected).cloned() {
                        patterns.input = pattern;
                        patterns.editing = true;
                        patterns.error = None;
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_mut() {
                    if patterns.selected < patterns.patterns.len() {
                        patterns.patterns.remove(patterns.selected);
                        patterns.selected = patterns
                            .selected
                            .min(patterns.patterns.len().saturating_sub(1));
                        patterns.error = None;
                    }
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                if let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_mut() {
                    patterns.patterns.clear();
                    patterns.selected = 0;
                    patterns.error = None;
                }
            }
            KeyCode::Enter | KeyCode::Char('s') | KeyCode::Char('S') => {
                self.submit_project_patterns(outcome);
                return;
            }
            _ => {}
        }
        outcome.repaint = true;
    }

    fn commit_project_pattern_edit(&mut self) {
        let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_mut() else {
            return;
        };
        let pattern = patterns.input.trim().replace('\\', "/");
        if pattern.is_empty() {
            patterns.error = Some("pattern must not be empty".to_owned());
            return;
        }
        if patterns.selected == patterns.patterns.len() {
            patterns.patterns.push(pattern);
        } else {
            patterns.patterns[patterns.selected] = pattern;
        }
        patterns.input.clear();
        patterns.editing = false;
        patterns.error = None;
    }

    pub(super) fn submit_project_patterns(&mut self, outcome: &mut ClientShellInput) {
        let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_ref() else {
            return;
        };
        let method =
            crate::api::schema::Method::ProjectUpdate(crate::api::schema::ProjectUpdateParams {
                project_id: patterns.project_id.clone(),
                name: patterns.name.clone(),
                root_path: patterns.root_path.clone(),
                worktree_root: patterns.worktree_root.clone(),
                worktree_base: patterns.worktree_base.clone(),
                default_agent: None,
                preserve_patterns: Some(patterns.patterns.clone()),
                lifecycle: None,
            });
        if !self.push_endpoint_method_with_kind(method, PendingEndpointKind::ProjectUpdate, outcome)
        {
            if let Some(ClientShellOverlay::ProjectPatterns(patterns)) = self.overlay.as_mut() {
                patterns.error = Some("could not submit project settings".to_owned());
            }
        }
    }

    pub(super) fn save_rename_overlay(&mut self, outcome: &mut ClientShellInput) {
        let Some(ClientShellOverlay::Rename(rename)) = self.overlay.take() else {
            return;
        };
        let trimmed = rename.input.trim();
        let method = match rename.target {
            ClientRenameTarget::Project { project_id } => (!trimmed.is_empty()).then(|| {
                crate::api::schema::Method::ProjectRename(crate::api::schema::ProjectRenameParams {
                    project_id,
                    name: trimmed.to_owned(),
                })
            }),
            ClientRenameTarget::ProjectBase {
                project_id,
                name,
                root_path,
                worktree_root,
            } => Some(crate::api::schema::Method::ProjectUpdate(
                crate::api::schema::ProjectUpdateParams {
                    project_id,
                    name,
                    root_path,
                    worktree_root,
                    worktree_base: Some(trimmed.to_owned()),
                    default_agent: None,
                    preserve_patterns: None,
                    lifecycle: None,
                },
            )),
            ClientRenameTarget::ProjectDefaultAgent {
                project_id,
                name,
                root_path,
                worktree_root,
            } => Some(crate::api::schema::Method::ProjectUpdate(
                crate::api::schema::ProjectUpdateParams {
                    project_id,
                    name,
                    root_path,
                    worktree_root,
                    worktree_base: None,
                    default_agent: Some(trimmed.to_owned()),
                    preserve_patterns: None,
                    lifecycle: None,
                },
            )),
            ClientRenameTarget::Workspace { workspace_id } => (!trimmed.is_empty()).then(|| {
                crate::api::schema::Method::WorkspaceRename(
                    crate::api::schema::WorkspaceRenameParams {
                        workspace_id,
                        label: trimmed.to_owned(),
                    },
                )
            }),
            ClientRenameTarget::NewTab {
                workspace_id,
                default_name,
            } => Some(crate::api::schema::Method::TabCreate(
                crate::api::schema::TabCreateParams {
                    workspace_id: Some(workspace_id),
                    cwd: None,
                    focus: true,
                    label: (!trimmed.is_empty() && trimmed != default_name)
                        .then(|| trimmed.to_owned()),
                    env: Default::default(),
                },
            )),
            ClientRenameTarget::Tab {
                tab_id,
                auto_name,
                original_name,
            } => (!(trimmed.is_empty() || auto_name && trimmed == original_name)).then(|| {
                crate::api::schema::Method::TabRename(crate::api::schema::TabRenameParams {
                    tab_id,
                    label: trimmed.to_owned(),
                })
            }),
            ClientRenameTarget::Pane { pane_id } => Some(crate::api::schema::Method::PaneRename(
                crate::api::schema::PaneRenameParams {
                    pane_id,
                    label: Some(trimmed.to_owned()),
                },
            )),
        };
        if let Some(method) = method {
            self.push_endpoint_method(method, outcome);
        }
        outcome.repaint = true;
    }

    pub(super) fn open_confirm_close_overlay(&mut self, workspace_id: String) {
        let Some(snapshot) = self.snapshot.as_deref() else {
            return;
        };
        let Some(workspace) = snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.workspace_id == workspace_id)
        else {
            return;
        };
        let group_key = workspace
            .worktree
            .as_ref()
            .filter(|worktree| !worktree.is_linked_worktree)
            .map(|worktree| worktree.key.as_str());
        let group = group_key
            .map(|key| {
                snapshot
                    .workspaces
                    .iter()
                    .filter(|member| {
                        member
                            .worktree
                            .as_ref()
                            .is_some_and(|worktree| worktree.key == key)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![workspace]);
        let closes_group = group.len() > 1;
        let pane_count = group
            .iter()
            .map(|member| {
                snapshot
                    .panes
                    .iter()
                    .filter(|pane| pane.workspace_id == member.workspace_id)
                    .count()
            })
            .sum::<usize>();
        let panes = if pane_count == 1 {
            "1 pane".to_owned()
        } else {
            format!("{pane_count} panes")
        };
        let scope = if closes_group {
            format!("{} workspaces, {panes}", group.len())
        } else {
            panes
        };
        self.overlay = Some(ClientShellOverlay::ConfirmClose(
            ClientConfirmCloseOverlay {
                workspace_id,
                title: if closes_group {
                    "Close worktree group?".to_owned()
                } else {
                    "Close workspace?".to_owned()
                },
                detail: format!("{} — {scope}", workspace.label),
            },
        ));
    }
}
