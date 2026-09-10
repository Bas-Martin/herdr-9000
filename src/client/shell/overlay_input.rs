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
            Some(ClientShellOverlay::TaskFileEditor(editor)) => {
                let inserted = if let Some(replace) = editor.find_mode {
                    let value = text
                        .chars()
                        .filter(|character| !character.is_control())
                        .collect::<String>();
                    let target = if replace {
                        &mut editor.replace_text
                    } else {
                        &mut editor.find_query
                    };
                    target.push_str(&value);
                    !value.is_empty()
                } else {
                    match editor.field {
                        ClientTaskFileEditorField::Path => {
                            let value = text
                                .chars()
                                .filter(|character| !character.is_control())
                                .collect::<String>();
                            editor.path.push_str(&value);
                            !value.is_empty()
                        }
                        ClientTaskFileEditorField::Content => {
                            let value = text
                                .chars()
                                .filter(|character| {
                                    !character.is_control() || matches!(character, '\n' | '\t')
                                })
                                .collect::<String>();
                            if value.is_empty() {
                                false
                            } else {
                                editor.content.insert_str(editor.cursor, &value);
                                editor.cursor += value.len();
                                editor.dirty = true;
                                true
                            }
                        }
                    }
                };
                editor.error = None;
                inserted
            }
            Some(ClientShellOverlay::TaskWebBrowser(browser)) => {
                let value = text
                    .chars()
                    .filter(|character| !character.is_control())
                    .collect::<String>();
                if value.is_empty() {
                    false
                } else {
                    match browser.field {
                        ClientTaskBrowserField::Url => browser.url.push_str(&value),
                        ClientTaskBrowserField::ProfileName => {
                            browser.profile_name.push_str(&value)
                        }
                    }
                    browser.error = None;
                    true
                }
            }
            Some(ClientShellOverlay::ResourceEditor(editor)) => {
                let value = text
                    .chars()
                    .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
                    .collect::<String>();
                if value.is_empty() {
                    false
                } else {
                    let target = match editor.field {
                        ClientResourceEditorField::Name => &mut editor.name,
                        ClientResourceEditorField::Kind => &mut editor.kind,
                        ClientResourceEditorField::Scope => &mut editor.scope,
                        ClientResourceEditorField::Provider => &mut editor.provider,
                        ClientResourceEditorField::Content => &mut editor.content,
                    };
                    target.push_str(&value);
                    editor.error = None;
                    true
                }
            }
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
                environment: None,
                remote_endpoint_id: None,
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
                remote_endpoint_id: None,
                environment: None,
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
    pub(super) fn open_task_browser(&mut self, outcome: &mut ClientShellInput) {
        self.overlay = Some(ClientShellOverlay::TaskBrowser(ClientTaskBrowserOverlay {
            tasks: Vec::new(),
            task_endpoints: Vec::new(),
            task_endpoint_labels: Vec::new(),
            selected: 0,
            loading: false,
            pending_requests: 0,
            opening: false,
            error: None,
        }));
        self.request_task_browser(outcome);
    }
    fn open_selected_task_web_browser(&mut self) {
        let Some(task_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TaskBrowser(browser) if !browser.loading && !browser.opening => {
                browser
                    .tasks
                    .get(browser.selected)
                    .map(|task| task.task_id.clone())
            }
            _ => None,
        }) else {
            return;
        };
        let profiles = self
            .config
            .preferences
            .browser_profiles
            .get(&task_id)
            .cloned()
            .unwrap_or_default();
        let selected_profile = self
            .config
            .preferences
            .browser_last_profiles
            .get(&task_id)
            .and_then(|name| profiles.iter().position(|profile| &profile.name == name))
            .unwrap_or(0);
        let (url, profile_name) = profiles
            .get(selected_profile)
            .map(|profile| (profile.url.clone(), profile.name.clone()))
            .unwrap_or_else(|| (String::new(), "local".to_owned()));
        self.overlay = Some(ClientShellOverlay::TaskWebBrowser(
            ClientTaskWebBrowserOverlay {
                task_id,
                profiles,
                selected_profile,
                url,
                profile_name,
                field: ClientTaskBrowserField::Url,
                preview_url: None,
                preview_body: Vec::new(),
                error: None,
            },
        ));
    }

    pub(super) fn open_resource_library(&mut self, outcome: &mut ClientShellInput) {
        self.overlay = Some(ClientShellOverlay::ResourceLibrary(
            ClientResourceLibraryOverlay {
                resources: Vec::new(),
                selected: 0,
                loading: false,
                pending: false,
                error: None,
                task_id: None,
                project_id: None,
                endpoint_id: None,
                assigned_resource_ids: Vec::new(),
            },
        ));
        self.request_resource_list(outcome);
    }
    pub(super) fn open_project_resource_library(&mut self, outcome: &mut ClientShellInput) {
        let Some(project_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::ProjectSettings(settings) => settings
                .project
                .as_ref()
                .map(|project| project.project_id.clone()),
            _ => None,
        }) else {
            return;
        };
        let endpoint_id = self.active_endpoint_id.clone();
        self.overlay = Some(ClientShellOverlay::ResourceLibrary(
            ClientResourceLibraryOverlay {
                resources: Vec::new(),
                selected: 0,
                loading: false,
                pending: false,
                error: None,
                task_id: None,
                project_id: Some(project_id),
                endpoint_id: Some(endpoint_id),
                assigned_resource_ids: Vec::new(),
            },
        ));
        self.request_resource_list(outcome);
    }

    pub(super) fn open_task_resource_library(&mut self, outcome: &mut ClientShellInput) {
        let Some((task_id, endpoint_id, assigned_resource_ids, project_id)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskBrowser(browser)
                    if !browser.loading && !browser.opening =>
                {
                    let index = browser.selected;
                    Some((
                        browser.tasks.get(index)?.task_id.clone(),
                        browser.task_endpoints.get(index)?.clone(),
                        browser.tasks.get(index)?.resource_ids.clone(),
                        browser.tasks.get(index)?.project_id.clone(),
                    ))
                }
                _ => None,
            })
        else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::ResourceLibrary(
            ClientResourceLibraryOverlay {
                resources: Vec::new(),
                selected: 0,
                loading: false,
                pending: false,
                error: None,
                task_id: Some(task_id),
                project_id: Some(project_id),
                endpoint_id: Some(endpoint_id),
                assigned_resource_ids,
            },
        ));
        self.request_resource_list(outcome);
    }

    fn open_resource_editor(&mut self, resource: Option<crate::api::schema::ResourceInfo>) {
        self.overlay = Some(ClientShellOverlay::ResourceEditor(
            ClientResourceEditorOverlay {
                resource_id: resource
                    .as_ref()
                    .map(|resource| resource.resource_id.clone()),
                project_id: resource
                    .as_ref()
                    .and_then(|resource| resource.project_id.clone()),
                task_id: resource
                    .as_ref()
                    .and_then(|resource| resource.task_id.clone()),
                name: resource
                    .as_ref()
                    .map(|resource| resource.name.clone())
                    .unwrap_or_default(),
                kind: resource
                    .as_ref()
                    .map(|resource| format!("{:?}", resource.kind).to_ascii_lowercase())
                    .unwrap_or_else(|| "prompt".to_owned()),
                scope: resource
                    .as_ref()
                    .map(|resource| format!("{:?}", resource.scope).to_ascii_lowercase())
                    .unwrap_or_else(|| "global".to_owned()),
                provider: resource
                    .as_ref()
                    .and_then(|resource| resource.provider.clone())
                    .unwrap_or_default(),
                content: resource
                    .as_ref()
                    .map(|resource| resource.content.clone())
                    .unwrap_or_default(),
                field: ClientResourceEditorField::Name,
                endpoint_id: None,
                saving: false,
                error: None,
            },
        ));
    }

    fn open_new_resource_editor(&mut self) {
        self.open_resource_editor(None);
    }
    fn open_new_task_resource_editor(&mut self) {
        let Some((task_id, project_id, endpoint_id)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::ResourceLibrary(library) => Some((
                    library.task_id.clone()?,
                    library.project_id.clone()?,
                    library.endpoint_id.clone()?,
                )),
                _ => None,
            })
        else {
            return;
        };
        self.open_resource_editor(None);
        if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
            editor.project_id = Some(project_id);
            editor.task_id = Some(task_id);
            editor.scope = "task".to_owned();
            editor.endpoint_id = Some(endpoint_id);
        }
    }
    fn open_new_project_resource_editor(&mut self) {
        let Some((project_id, endpoint_id)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::ResourceLibrary(library) if library.task_id.is_none() => {
                    Some((library.project_id.clone()?, library.endpoint_id.clone()?))
                }
                _ => None,
            })
        else {
            return;
        };
        self.open_resource_editor(None);
        if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
            editor.project_id = Some(project_id);
            editor.scope = "project".to_owned();
            editor.endpoint_id = Some(endpoint_id);
        }
    }

    fn request_resource_list(&mut self, outcome: &mut ClientShellInput) {
        let Some((project_id, task_id, endpoint_id)) =
            self.overlay.as_mut().and_then(|overlay| match overlay {
                ClientShellOverlay::ResourceLibrary(library) if !library.pending => {
                    library.loading = true;
                    library.pending = true;
                    library.error = None;
                    Some((
                        library.project_id.clone(),
                        library.task_id.clone(),
                        library.endpoint_id.clone(),
                    ))
                }
                _ => None,
            })
        else {
            return;
        };
        let method =
            crate::api::schema::Method::ResourceList(crate::api::schema::ResourceListParams {
                project_id,
                task_id,
                include_disabled: true,
            });
        let sent = if let Some(endpoint_id) = endpoint_id {
            self.push_endpoint_method_to_endpoint(
                endpoint_id,
                method,
                PendingEndpointKind::ResourceList,
                outcome,
            )
        } else {
            self.push_endpoint_method_with_kind(method, PendingEndpointKind::ResourceList, outcome)
        };
        if !sent {
            if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
                library.loading = false;
                library.pending = false;
                library.error = Some("The resource endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }
    fn save_resource_editor(&mut self, outcome: &mut ClientShellInput) {
        let (
            resource_id,
            project_id,
            task_id,
            endpoint_id,
            name,
            kind_name,
            scope_name,
            provider,
            content,
        ) = {
            let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() else {
                return;
            };
            if editor.saving {
                return;
            }
            if editor.name.trim().is_empty() || editor.content.trim().is_empty() {
                editor.error = Some("name and content are required".to_owned());
                return;
            }
            (
                editor.resource_id.clone(),
                editor.project_id.clone(),
                editor.task_id.clone(),
                editor.endpoint_id.clone(),
                editor.name.trim().to_owned(),
                editor.kind.trim().to_owned(),
                editor.scope.trim().to_owned(),
                if editor.provider.trim().is_empty() {
                    editor.resource_id.as_ref().map(|_| String::new())
                } else {
                    Some(editor.provider.trim().to_owned())
                },
                editor.content.clone(),
            )
        };
        let kind = match kind_name.as_str() {
            "prompt" => crate::resource::ResourceKind::Prompt,
            "skill" => crate::resource::ResourceKind::Skill,
            "mcp" => crate::resource::ResourceKind::Mcp,
            _ => {
                if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                    editor.error = Some("kind must be prompt, skill, or mcp".to_owned());
                }
                return;
            }
        };
        let scope = match scope_name.as_str() {
            "global" => crate::resource::ResourceScope::Global,
            "project" => crate::resource::ResourceScope::Project,
            "task" => crate::resource::ResourceScope::Task,
            _ => {
                if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                    editor.error = Some("scope must be global, project, or task".to_owned());
                }
                return;
            }
        };
        if matches!(scope, crate::resource::ResourceScope::Project) && project_id.is_none() {
            if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                editor.error = Some("project scope requires a project context".to_owned());
            }
            return;
        }
        if matches!(scope, crate::resource::ResourceScope::Task) && task_id.is_none() {
            if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                editor.error = Some("task scope requires a task context".to_owned());
            }
            return;
        }
        let (project_id, task_id) = match scope {
            crate::resource::ResourceScope::Global => (None, None),
            crate::resource::ResourceScope::Project => (project_id, None),
            crate::resource::ResourceScope::Task => (None, task_id),
        };
        let method = if let Some(resource_id) = resource_id {
            crate::api::schema::Method::ResourceUpdate(crate::api::schema::ResourceUpdateParams {
                resource_id,
                name: Some(name),
                description: None,
                content: Some(content),
                enabled: None,
                provider,
            })
        } else {
            crate::api::schema::Method::ResourceCreate(crate::api::schema::ResourceCreateParams {
                kind,
                name,
                description: String::new(),
                scope,
                project_id,
                task_id,
                provider,
                content,
            })
        };
        if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
            editor.saving = true;
            editor.error = None;
        }
        let sent = if let Some(endpoint_id) = endpoint_id {
            self.push_endpoint_method_to_endpoint(
                endpoint_id,
                method,
                PendingEndpointKind::ResourceAction,
                outcome,
            )
        } else {
            self.push_endpoint_method_with_kind(
                method,
                PendingEndpointKind::ResourceAction,
                outcome,
            )
        };
        if !sent {
            if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                editor.saving = false;
                editor.error = Some("The resource endpoint is unavailable.".to_owned());
            }
        }
    }

    fn route_resource_editor_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if !matches!(self.overlay, Some(ClientShellOverlay::ResourceEditor(_))) {
            return false;
        }
        let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
        match code {
            crossterm::event::KeyCode::Esc if modifiers.is_empty() => self.overlay = None,
            crossterm::event::KeyCode::Tab if modifiers.is_empty() => {
                if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                    editor.field = match editor.field {
                        ClientResourceEditorField::Name => ClientResourceEditorField::Kind,
                        ClientResourceEditorField::Kind => ClientResourceEditorField::Scope,
                        ClientResourceEditorField::Scope => ClientResourceEditorField::Provider,
                        ClientResourceEditorField::Provider => ClientResourceEditorField::Content,
                        ClientResourceEditorField::Content => ClientResourceEditorField::Name,
                    };
                }
            }
            crossterm::event::KeyCode::BackTab if modifiers.is_empty() => {
                if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                    editor.field = match editor.field {
                        ClientResourceEditorField::Name => ClientResourceEditorField::Content,
                        ClientResourceEditorField::Kind => ClientResourceEditorField::Name,
                        ClientResourceEditorField::Scope => ClientResourceEditorField::Kind,
                        ClientResourceEditorField::Provider => ClientResourceEditorField::Scope,
                        ClientResourceEditorField::Content => ClientResourceEditorField::Provider,
                    };
                }
            }
            crossterm::event::KeyCode::Enter if modifiers.is_empty() => {
                self.save_resource_editor(outcome);
            }
            crossterm::event::KeyCode::Backspace if modifiers.is_empty() => {
                if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                    match editor.field {
                        ClientResourceEditorField::Name => {
                            editor.name.pop();
                        }
                        ClientResourceEditorField::Kind => {
                            editor.kind.pop();
                        }
                        ClientResourceEditorField::Scope => {
                            editor.scope.pop();
                        }
                        ClientResourceEditorField::Provider => {
                            editor.provider.pop();
                        }
                        ClientResourceEditorField::Content => {
                            editor.content.pop();
                        }
                    }
                    editor.error = None;
                }
            }
            crossterm::event::KeyCode::Char('u')
                if modifiers == crossterm::event::KeyModifiers::CONTROL =>
            {
                if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                    match editor.field {
                        ClientResourceEditorField::Name => editor.name.clear(),
                        ClientResourceEditorField::Kind => editor.kind.clear(),
                        ClientResourceEditorField::Scope => editor.scope.clear(),
                        ClientResourceEditorField::Provider => editor.provider.clear(),
                        ClientResourceEditorField::Content => editor.content.clear(),
                    }
                    editor.error = None;
                }
            }
            crossterm::event::KeyCode::Char(character)
                if modifiers
                    .difference(crossterm::event::KeyModifiers::SHIFT)
                    .is_empty() =>
            {
                let text = key
                    .generated_text
                    .clone()
                    .unwrap_or_else(|| character.to_string());
                self.insert_overlay_text(&text);
            }
            _ => {}
        }
        outcome.repaint = true;
        true
    }

    pub(super) fn move_resource_selection(&mut self, delta: isize) {
        if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
            let last = library.resources.len().saturating_sub(1);
            library.selected = (library.selected as isize + delta).clamp(0, last as isize) as usize;
        }
    }
    fn resource_library_is_task_context(&self) -> bool {
        matches!(
            self.overlay,
            Some(ClientShellOverlay::ResourceLibrary(
                ClientResourceLibraryOverlay {
                    task_id: Some(_),
                    ..
                }
            ))
        )
    }
    fn resource_library_is_project_context(&self) -> bool {
        matches!(
            self.overlay,
            Some(ClientShellOverlay::ResourceLibrary(
                ClientResourceLibraryOverlay {
                    task_id: None,
                    project_id: Some(_),
                    ..
                }
            ))
        )
    }

    fn update_selected_task_resources(&mut self, outcome: &mut ClientShellInput) {
        let Some((task_id, endpoint_id, resource_id, assigned)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::ResourceLibrary(library)
                    if !library.pending && !library.loading =>
                {
                    let resource = library.resources.get(library.selected)?;
                    Some((
                        library.task_id.clone()?,
                        library.endpoint_id.clone()?,
                        resource.resource_id.clone(),
                        library
                            .assigned_resource_ids
                            .contains(&resource.resource_id),
                    ))
                }
                _ => None,
            })
        else {
            return;
        };
        let mut resource_ids = self
            .overlay
            .as_ref()
            .and_then(|overlay| match overlay {
                ClientShellOverlay::ResourceLibrary(library) => {
                    Some(library.assigned_resource_ids.clone())
                }
                _ => None,
            })
            .unwrap_or_default();
        if assigned {
            resource_ids.retain(|id| id != &resource_id);
        } else {
            resource_ids.push(resource_id);
        }
        if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
            library.pending = true;
            library.error = None;
        }
        let sent = self.push_endpoint_method_to_endpoint(
            endpoint_id,
            crate::api::schema::Method::TaskResources(crate::api::schema::TaskResourcesParams {
                task_id,
                resource_ids,
            }),
            PendingEndpointKind::ResourceAction,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
                library.pending = false;
                library.error = Some("The task endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }

    pub(super) fn update_selected_resource(&mut self, outcome: &mut ClientShellInput) {
        let Some((resource_id, enabled)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::ResourceLibrary(library)
                    if !library.pending && !library.loading =>
                {
                    let resource = library.resources.get(library.selected)?;
                    Some((resource.resource_id.clone(), resource.enabled))
                }
                _ => None,
            })
        else {
            return;
        };
        if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
            library.pending = true;
            library.error = None;
        }
        let sent = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::ResourceUpdate(crate::api::schema::ResourceUpdateParams {
                resource_id,
                name: None,
                description: None,
                content: None,
                enabled: Some(!enabled),
                provider: None,
            }),
            PendingEndpointKind::ResourceAction,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
                library.pending = false;
                library.error = Some("The resource endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }

    fn delete_selected_resource(&mut self, outcome: &mut ClientShellInput) {
        let Some(resource_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::ResourceLibrary(library)
                if !library.pending && !library.loading =>
            {
                library
                    .resources
                    .get(library.selected)
                    .map(|resource| resource.resource_id.clone())
            }
            _ => None,
        }) else {
            return;
        };
        if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
            library.pending = true;
            library.error = None;
        }
        let sent = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::ResourceDelete(crate::api::schema::ResourceTarget {
                resource_id,
            }),
            PendingEndpointKind::ResourceAction,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
                library.pending = false;
                library.error = Some("The resource endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }

    pub(super) fn open_tmux_panes(&mut self, outcome: &mut ClientShellInput) {
        self.overlay = Some(ClientShellOverlay::TmuxPanes(ClientTmuxPaneOverlay {
            panes: Vec::new(),
            selected: 0,
            watching: false,
            output: String::new(),
            loading: false,
            pending: false,
            error: None,
        }));
        self.tmux_next_refresh =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(1));
        self.request_tmux_list(outcome);
    }

    fn request_tmux_list(&mut self, outcome: &mut ClientShellInput) {
        if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
            if overlay.pending {
                return;
            }
            overlay.loading = true;
            overlay.pending = true;
            overlay.error = None;
        } else {
            return;
        }
        let sent = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::TmuxPaneList(crate::api::schema::EmptyParams::default()),
            PendingEndpointKind::TmuxList,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                overlay.loading = false;
                overlay.pending = false;
                overlay.error = Some("tmux is unavailable on this endpoint".to_owned());
            }
        }
        outcome.repaint = true;
    }

    fn request_tmux_capture(&mut self, outcome: &mut ClientShellInput) {
        let Some(pane_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TmuxPanes(overlay) if overlay.watching && !overlay.pending => {
                overlay
                    .panes
                    .get(overlay.selected)
                    .map(|pane| pane.pane_id.clone())
            }
            _ => None,
        }) else {
            return;
        };
        if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
            overlay.pending = true;
            overlay.error = None;
        }
        let sent = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::TmuxPaneCapture(
                crate::api::schema::TmuxPaneCaptureParams {
                    pane_id,
                    lines: 200,
                },
            ),
            PendingEndpointKind::TmuxCapture,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                overlay.pending = false;
                overlay.error = Some("tmux capture is unavailable".to_owned());
            }
        }
        outcome.repaint = true;
    }

    pub(super) fn open_selected_tmux_pane(&mut self, outcome: &mut ClientShellInput) {
        let Some(pane_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TmuxPanes(overlay)
                if !overlay.loading && !overlay.panes.is_empty() =>
            {
                overlay
                    .panes
                    .get(overlay.selected)
                    .map(|pane| pane.pane_id.clone())
            }
            _ => None,
        }) else {
            return;
        };
        if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
            overlay.watching = true;
            overlay.output.clear();
            overlay.error = None;
            overlay.pending = false;
        }
        self.request_tmux_capture_for(pane_id, outcome);
    }

    fn request_tmux_capture_for(&mut self, pane_id: String, outcome: &mut ClientShellInput) {
        if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
            if overlay.pending {
                return;
            }
            overlay.pending = true;
        } else {
            return;
        }
        let sent = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::TmuxPaneCapture(
                crate::api::schema::TmuxPaneCaptureParams {
                    pane_id,
                    lines: 200,
                },
            ),
            PendingEndpointKind::TmuxCapture,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                overlay.pending = false;
                overlay.error = Some("tmux capture is unavailable".to_owned());
            }
        }
        outcome.repaint = true;
    }

    fn send_tmux_interrupt(&mut self, outcome: &mut ClientShellInput) {
        let Some(pane_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TmuxPanes(overlay) if !overlay.panes.is_empty() => overlay
                .panes
                .get(overlay.selected)
                .map(|pane| pane.pane_id.clone()),
            _ => None,
        }) else {
            return;
        };
        let _ = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::TmuxPaneSendKeys(
                crate::api::schema::TmuxPaneSendKeysParams {
                    pane_id,
                    keys: vec!["C-c".to_owned()],
                },
            ),
            PendingEndpointKind::TmuxAction,
            outcome,
        );
    }

    fn focus_selected_tmux_pane(&mut self, outcome: &mut ClientShellInput) {
        let Some(pane_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TmuxPanes(overlay) => overlay
                .panes
                .get(overlay.selected)
                .map(|pane| pane.pane_id.clone()),
            _ => None,
        }) else {
            return;
        };
        let _ = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::TmuxPaneFocus(crate::api::schema::TmuxPaneTarget {
                pane_id,
            }),
            PendingEndpointKind::TmuxAction,
            outcome,
        );
    }

    fn close_selected_tmux_pane(&mut self, outcome: &mut ClientShellInput) {
        let Some(pane) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TmuxPanes(overlay) => overlay.panes.get(overlay.selected),
            _ => None,
        }) else {
            return;
        };
        if !matches!(
            pane.origin,
            crate::api::schema::TmuxPaneOrigin::HerdrSubagent
        ) {
            if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                overlay.error = Some("external tmux panes cannot be closed from Herdr".to_owned());
            }
            outcome.repaint = true;
            return;
        }
        let pane_id = pane.pane_id.clone();
        let _ = self.push_endpoint_method_with_kind(
            crate::api::schema::Method::TmuxPaneKill(crate::api::schema::TmuxPaneTarget {
                pane_id,
            }),
            PendingEndpointKind::TmuxAction,
            outcome,
        );
    }

    pub(crate) fn tick_tmux_panes(
        &mut self,
        now: std::time::Instant,
        outcome: &mut ClientShellInput,
    ) {
        if !matches!(self.overlay, Some(ClientShellOverlay::TmuxPanes(_))) {
            self.tmux_next_refresh = None;
            return;
        }
        let Some(deadline) = self.tmux_next_refresh else {
            return;
        };
        if now < deadline {
            return;
        }
        self.tmux_next_refresh = Some(now + std::time::Duration::from_secs(1));
        let watching = matches!(
            self.overlay,
            Some(ClientShellOverlay::TmuxPanes(ClientTmuxPaneOverlay {
                watching: true,
                pending: false,
                ..
            }))
        );
        if watching {
            self.request_tmux_capture(outcome);
        } else if matches!(
            self.overlay,
            Some(ClientShellOverlay::TmuxPanes(ClientTmuxPaneOverlay {
                pending: false,
                ..
            }))
        ) {
            self.request_tmux_list(outcome);
        }
    }

    pub(super) fn handle_tmux_list_result(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        let (panes, error) = match result {
            Ok(crate::api::schema::ResponseResult::TmuxPaneList { panes }) => (panes, None),
            Ok(_) => (Vec::new(), Some("unexpected tmux list response".to_owned())),
            Err(error) => (Vec::new(), Some(error.message)),
        };
        if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
            let selected_id = overlay
                .panes
                .get(overlay.selected)
                .map(|pane| pane.pane_id.clone());
            overlay.panes = panes;
            overlay.selected = selected_id
                .and_then(|id| overlay.panes.iter().position(|pane| pane.pane_id == id))
                .unwrap_or_else(|| overlay.selected.min(overlay.panes.len().saturating_sub(1)));
            overlay.loading = false;
            overlay.pending = false;
            overlay.error = error;
        }
        (true, Vec::new())
    }

    pub(super) fn handle_tmux_capture_result(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        match result {
            Ok(crate::api::schema::ResponseResult::TmuxPaneCaptured { pane_id, output }) => {
                if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                    if overlay
                        .panes
                        .get(overlay.selected)
                        .is_some_and(|pane| pane.pane_id == pane_id)
                    {
                        overlay.output = output;
                    }
                    overlay.pending = false;
                    overlay.error = None;
                }
            }
            Ok(_) => {
                if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                    overlay.pending = false;
                    overlay.error = Some("unexpected tmux capture response".to_owned());
                }
            }
            Err(error) => {
                if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                    overlay.pending = false;
                    overlay.error = Some(error.message);
                }
            }
        }
        (true, Vec::new())
    }

    pub(super) fn handle_tmux_action_result(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        match result {
            Ok(crate::api::schema::ResponseResult::TmuxPaneAction { .. }) => {
                if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                    overlay.pending = false;
                    overlay.error = None;
                }
            }
            Ok(_) => {
                if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                    overlay.pending = false;
                    overlay.error = Some("unexpected tmux action response".to_owned());
                }
            }
            Err(error) => {
                if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                    overlay.pending = false;
                    overlay.error = Some(error.message);
                }
            }
        }
        (true, Vec::new())
    }

    fn request_task_browser(&mut self, outcome: &mut ClientShellInput) {
        let endpoint_ids = self
            .endpoints
            .iter()
            .filter(|endpoint| self.endpoint_is_online(&endpoint.endpoint_id))
            .map(|endpoint| endpoint.endpoint_id.clone())
            .collect::<Vec<_>>();
        if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
            browser.loading = !endpoint_ids.is_empty();
            browser.pending_requests = endpoint_ids.len();
            browser.tasks.clear();
            browser.task_endpoints.clear();
            browser.task_endpoint_labels.clear();
            browser.error = None;
        }
        if endpoint_ids.is_empty() {
            if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
                browser.error = Some("no task endpoints are online".to_owned());
            }
            outcome.repaint = true;
            return;
        }
        for endpoint_id in endpoint_ids {
            let sent = self.push_endpoint_method_to_endpoint(
                endpoint_id.clone(),
                crate::api::schema::Method::TaskList(crate::api::schema::TaskListParams {
                    project_id: None,
                    include_closed: true,
                }),
                PendingEndpointKind::TaskList { endpoint_id },
                outcome,
            );
            if !sent {
                if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
                    browser.pending_requests = browser.pending_requests.saturating_sub(1);
                    browser.error.get_or_insert_with(|| {
                        "one or more task endpoints rejected the request".to_owned()
                    });
                }
            }
        }
        if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
            browser.loading = browser.pending_requests != 0;
        }
        outcome.repaint = true;
    }

    pub(super) fn move_task_browser_selection(&mut self, delta: isize) {
        let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() else {
            return;
        };
        let last = browser.tasks.len().saturating_sub(1);
        browser.selected = (browser.selected as isize + delta).clamp(0, last as isize) as usize;
    }

    pub(super) fn open_selected_task(&mut self, outcome: &mut ClientShellInput) {
        let Some((task_id, endpoint_id)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskBrowser(browser)
                    if !browser.loading && !browser.opening =>
                {
                    let index = browser.selected;
                    Some((
                        browser.tasks.get(index)?.task_id.clone(),
                        browser.task_endpoints.get(index)?.clone(),
                    ))
                }
                _ => None,
            })
        else {
            return;
        };
        if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
            browser.opening = true;
            browser.error = None;
        }
        let sent = self.push_endpoint_method_to_endpoint(
            endpoint_id.clone(),
            crate::api::schema::Method::TaskOpen(crate::api::schema::TaskOpenParams {
                task_id,
                focus: true,
            }),
            PendingEndpointKind::TaskOpen { endpoint_id },
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
                browser.opening = false;
                browser.error = Some("The task endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }

    fn retry_selected_task(&mut self, outcome: &mut ClientShellInput) {
        let Some((task_id, endpoint_id)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskBrowser(browser)
                    if !browser.loading && !browser.opening =>
                {
                    let index = browser.selected;
                    Some((
                        browser.tasks.get(index)?.task_id.clone(),
                        browser.task_endpoints.get(index)?.clone(),
                    ))
                }
                _ => None,
            })
        else {
            return;
        };
        let sent = self.push_endpoint_method_to_endpoint(
            endpoint_id.clone(),
            crate::api::schema::Method::TaskRetry(crate::api::schema::TaskTarget { task_id }),
            PendingEndpointKind::Generic,
            outcome,
        );
        if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
            browser.error = Some(if sent {
                "retry requested; press R to refresh".to_owned()
            } else {
                "The task endpoint is unavailable.".to_owned()
            });
        }
    }

    fn open_selected_task_editor(&mut self, outcome: &mut ClientShellInput) {
        let Some((task_id, endpoint_id)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskBrowser(browser)
                    if !browser.loading && !browser.opening =>
                {
                    let index = browser.selected;
                    Some((
                        browser.tasks.get(index)?.task_id.clone(),
                        browser.task_endpoints.get(index)?.clone(),
                    ))
                }
                _ => None,
            })
        else {
            return;
        };
        self.overlay = Some(ClientShellOverlay::TaskFileEditor(
            ClientTaskFileEditorOverlay {
                endpoint_id,
                task_id,
                path: String::new(),
                content: String::new(),
                find_query: String::new(),
                replace_text: String::new(),
                find_mode: None,
                field: ClientTaskFileEditorField::Path,
                cursor: 0,
                loading: true,
                files: Vec::new(),
                selected_file: 0,
                tabs: Vec::new(),
                selected_tab: 0,
                dirty: false,
                saving: false,
                error: None,
            },
        ));
        self.request_task_file_list(outcome);
    }

    fn refresh_task_file_list(&mut self, outcome: &mut ClientShellInput) {
        if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
            if editor.loading || editor.saving {
                return;
            }
            editor.loading = true;
            editor.error = None;
        }
        self.request_task_file_list(outcome);
        outcome.repaint = true;
    }
    fn request_task_file_list(&mut self, outcome: &mut ClientShellInput) {
        let Some((endpoint_id, task_id)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskFileEditor(editor) if editor.loading => {
                    Some((editor.endpoint_id.clone(), editor.task_id.clone()))
                }
                _ => None,
            })
        else {
            return;
        };
        let sent = self.push_endpoint_method_to_endpoint(
            endpoint_id,
            crate::api::schema::Method::TaskFileList(crate::api::schema::TaskFileListParams {
                task_id,
            }),
            PendingEndpointKind::TaskFileList,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.error = Some("The task file endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }

    fn cache_task_editor_tab(&mut self) {
        let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() else {
            return;
        };
        if editor.loading {
            return;
        }
        let active_path = editor
            .tabs
            .get(editor.selected_tab)
            .map(|tab| tab.path.clone())
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| editor.path.clone());
        if active_path.is_empty() {
            return;
        }
        if let Some(tab) = editor.tabs.iter_mut().find(|tab| tab.path == active_path) {
            tab.content = editor.content.clone();
            tab.cursor = editor.cursor;
            tab.dirty = editor.dirty;
            return;
        }
        editor.tabs.push(ClientTaskFileEditorTab {
            path: active_path,
            content: editor.content.clone(),
            cursor: editor.cursor,
            dirty: editor.dirty,
            deleted: false,
        });
        editor.selected_tab = editor.tabs.len().saturating_sub(1);
    }

    fn restore_task_editor_tab(&mut self, path: &str) -> bool {
        let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() else {
            return false;
        };
        let Some(index) = editor.tabs.iter().position(|tab| tab.path == path) else {
            return false;
        };
        let Some(tab) = editor.tabs.get(index) else {
            return false;
        };
        editor.path = tab.path.clone();
        editor.content = tab.content.clone();
        editor.cursor = tab.cursor.min(editor.content.len());
        editor.selected_tab = index;
        editor.dirty = tab.dirty;
        editor.loading = false;
        editor.error = tab
            .deleted
            .then(|| "file was deleted from the task workspace".to_owned());
        true
    }

    fn switch_task_editor_tab(&mut self, delta: isize) {
        self.cache_task_editor_tab();
        let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() else {
            return;
        };
        if editor.tabs.is_empty() {
            return;
        }
        let next =
            (editor.selected_tab as isize + delta).rem_euclid(editor.tabs.len() as isize) as usize;
        let tab = editor.tabs[next].path.clone();
        let _ = self.restore_task_editor_tab(&tab);
        if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
            editor.field = ClientTaskFileEditorField::Content;
        }
    }

    fn close_task_editor_tab(&mut self) {
        self.cache_task_editor_tab();
        let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() else {
            return;
        };
        let Some(tab) = editor.tabs.get(editor.selected_tab) else {
            return;
        };
        if tab.dirty {
            editor.error = Some("save the tab before closing it".to_owned());
            return;
        }
        editor.tabs.remove(editor.selected_tab);
        if editor.tabs.is_empty() {
            editor.selected_tab = 0;
            editor.path.clear();
            editor.content.clear();
            editor.cursor = 0;
            editor.dirty = false;
            editor.field = ClientTaskFileEditorField::Path;
            return;
        }
        editor.selected_tab = editor.selected_tab.min(editor.tabs.len() - 1);
        let path = editor.tabs[editor.selected_tab].path.clone();
        let _ = self.restore_task_editor_tab(&path);
    }

    fn request_task_file_read(&mut self, outcome: &mut ClientShellInput) {
        let Some(path) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TaskFileEditor(editor) if !editor.path.trim().is_empty() => {
                Some(editor.path.clone())
            }
            _ => None,
        }) else {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.error = Some("enter a file path before reading".to_owned());
            }
            outcome.repaint = true;
            return;
        };
        self.cache_task_editor_tab();
        if self.overlay.as_ref().is_some_and(|overlay| {
            matches!(
                overlay,
                ClientShellOverlay::TaskFileEditor(editor)
                    if editor.files.iter().any(|file| file.path == path && file.is_dir)
            )
        }) {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.error = Some("select a file, not a directory".to_owned());
            }
            outcome.repaint = true;
            return;
        }
        if self.restore_task_editor_tab(&path) {
            outcome.repaint = true;
            return;
        }
        let Some(task_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TaskFileEditor(editor) => Some(editor.task_id.clone()),
            _ => None,
        }) else {
            return;
        };
        if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
            editor.loading = true;
            editor.error = None;
        }
        let Some(endpoint_id) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TaskFileEditor(editor) => Some(editor.endpoint_id.clone()),
            _ => None,
        }) else {
            return;
        };
        let sent = self.push_endpoint_method_to_endpoint(
            endpoint_id,
            crate::api::schema::Method::TaskFileRead(crate::api::schema::TaskFileReadParams {
                task_id,
                path,
            }),
            PendingEndpointKind::TaskFileRead,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.loading = false;
                editor.error = Some("The task file endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }

    pub(super) fn save_task_file(&mut self, outcome: &mut ClientShellInput) {
        if self.overlay.as_ref().is_some_and(|overlay| {
            matches!(
                overlay,
                ClientShellOverlay::TaskFileEditor(editor)
                    if editor.tabs.iter().any(|tab| tab.path == editor.path && tab.deleted)
            )
        }) {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.error =
                    Some("cannot save a file that was deleted from the workspace".to_owned());
            }
            outcome.repaint = true;
            return;
        }
        let Some((endpoint_id, task_id, path, content)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskFileEditor(editor)
                    if !editor.path.trim().is_empty() && !editor.loading && !editor.saving =>
                {
                    Some((
                        editor.endpoint_id.clone(),
                        editor.task_id.clone(),
                        editor.path.clone(),
                        editor.content.clone(),
                    ))
                }
                _ => None,
            })
        else {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                if editor.path.trim().is_empty() {
                    editor.error = Some("enter a file path before saving".to_owned());
                }
            }
            outcome.repaint = true;
            return;
        };
        if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
            editor.saving = true;
            editor.error = None;
        }
        let sent = self.push_endpoint_method_to_endpoint(
            endpoint_id,
            crate::api::schema::Method::TaskFileWrite(crate::api::schema::TaskFileWriteParams {
                task_id,
                path,
                content,
            }),
            PendingEndpointKind::TaskFileWrite,
            outcome,
        );
        if !sent {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.saving = false;
                editor.error = Some("The task file endpoint is unavailable.".to_owned());
            }
        }
        outcome.repaint = true;
    }
    fn move_task_editor_cursor(&mut self, right: bool) {
        let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() else {
            return;
        };
        if editor.field != ClientTaskFileEditorField::Content {
            return;
        }
        if right {
            editor.cursor = editor
                .content
                .get(editor.cursor..)
                .and_then(|value| value.chars().next())
                .map_or(editor.content.len(), |character| {
                    editor.cursor + character.len_utf8()
                });
        } else {
            editor.cursor = editor
                .content
                .get(..editor.cursor)
                .and_then(|value| value.char_indices().next_back())
                .map_or(0, |(index, _)| index);
        }
    }

    fn backspace_task_editor(&mut self) {
        let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() else {
            return;
        };
        if let Some(replace) = editor.find_mode {
            let target = if replace {
                &mut editor.replace_text
            } else {
                &mut editor.find_query
            };
            target.pop();
        } else {
            match editor.field {
                ClientTaskFileEditorField::Path => {
                    editor.path.pop();
                }
                ClientTaskFileEditorField::Content => {
                    let previous = editor
                        .content
                        .get(..editor.cursor)
                        .and_then(|value| value.char_indices().next_back())
                        .map_or(0, |(index, _)| index);
                    if previous != editor.cursor {
                        editor.content.drain(previous..editor.cursor);
                        editor.cursor = previous;
                        editor.dirty = true;
                    }
                }
            }
        }
        editor.error = None;
    }

    fn replace_task_editor_matches(&mut self) {
        let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() else {
            return;
        };
        if editor.find_query.is_empty() {
            editor.error = Some("find text must not be empty".to_owned());
            return;
        }
        let replaced = editor
            .content
            .replace(&editor.find_query, &editor.replace_text);
        if replaced == editor.content {
            editor.error = Some("find text was not found".to_owned());
            return;
        }
        editor.content = replaced;
        editor.cursor = editor.content.len();
        editor.dirty = true;
        editor.find_mode = None;
        editor.field = ClientTaskFileEditorField::Content;
        editor.error = None;
    }
    fn move_task_editor_file(&mut self, delta: isize) {
        let Some((next_index, next_path)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskFileEditor(editor) => {
                    let file_indexes = editor
                        .files
                        .iter()
                        .enumerate()
                        .filter(|(_, file)| !file.is_dir)
                        .map(|(index, _)| index)
                        .collect::<Vec<_>>();
                    if file_indexes.is_empty() {
                        return None;
                    }
                    let current = file_indexes
                        .iter()
                        .position(|index| *index == editor.selected_file)
                        .unwrap_or(0);
                    let next = (current as isize + delta).clamp(0, file_indexes.len() as isize - 1)
                        as usize;
                    let index = file_indexes[next];
                    Some((index, editor.files.get(index)?.path.clone()))
                }
                _ => None,
            })
        else {
            return;
        };
        self.cache_task_editor_tab();
        if self.restore_task_editor_tab(&next_path) {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.selected_file = next_index;
                editor.field = ClientTaskFileEditorField::Path;
                editor.error = None;
            }
            return;
        }
        if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
            editor.selected_file = next_index;
            editor.path = next_path;
            editor.content.clear();
            editor.cursor = 0;
            editor.dirty = false;
            editor.loading = false;
            editor.field = ClientTaskFileEditorField::Path;
            editor.error = None;
        }
    }
    fn route_task_file_editor_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) -> bool {
        use crossterm::event::KeyModifiers;
        if !matches!(self.overlay, Some(ClientShellOverlay::TaskFileEditor(_))) {
            return false;
        }
        let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
        if code == KeyCode::Char('f') && modifiers == KeyModifiers::CONTROL {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.find_mode = Some(false);
                editor.error = None;
            }
            outcome.repaint = true;
            return true;
        }
        if code == KeyCode::Char('r') && modifiers == KeyModifiers::CONTROL {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.find_mode = Some(true);
                editor.error = None;
            }
            outcome.repaint = true;
            return true;
        }
        if self.overlay.as_ref().is_some_and(|overlay| {
            matches!(
                overlay,
                ClientShellOverlay::TaskFileEditor(editor) if editor.find_mode.is_some()
            )
        }) {
            if code == KeyCode::Esc && modifiers.is_empty() {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.find_mode = None;
                    editor.error = None;
                }
            } else if code == KeyCode::Enter && modifiers.is_empty() {
                let replace = self.overlay.as_ref().is_some_and(|overlay| {
                    matches!(
                        overlay,
                        ClientShellOverlay::TaskFileEditor(editor) if editor.find_mode == Some(true)
                    )
                });
                if replace {
                    self.replace_task_editor_matches();
                } else if let Some(ClientShellOverlay::TaskFileEditor(editor)) =
                    self.overlay.as_mut()
                {
                    editor.find_mode = Some(true);
                }
            } else if code == KeyCode::Backspace && modifiers.is_empty() {
                self.backspace_task_editor();
            } else if code == KeyCode::Char('u') && modifiers == KeyModifiers::CONTROL {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    if editor.find_mode == Some(true) {
                        editor.replace_text.clear();
                    } else {
                        editor.find_query.clear();
                    }
                }
            } else if modifiers.difference(KeyModifiers::SHIFT).is_empty()
                && matches!(code, KeyCode::Char(_))
            {
                self.insert_overlay_text(&key.generated_text.clone().unwrap_or_else(
                    || match code {
                        KeyCode::Char(character) => character.to_string(),
                        _ => String::new(),
                    },
                ));
            }
            outcome.repaint = true;
            return true;
        }
        if matches!(
            code,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j')
        ) && modifiers.is_empty()
            && self.overlay.as_ref().is_some_and(|overlay| {
                matches!(
                    overlay,
                    ClientShellOverlay::TaskFileEditor(editor)
                        if editor.field == ClientTaskFileEditorField::Path
                            && !editor.files.is_empty()
                )
            })
        {
            self.move_task_editor_file(if matches!(code, KeyCode::Up | KeyCode::Char('k')) {
                -1
            } else {
                1
            });
            outcome.repaint = true;
            return true;
        }
        if matches!(code, KeyCode::Char('[') | KeyCode::Char(']'))
            && modifiers.is_empty()
            && self.overlay.as_ref().is_some_and(|overlay| {
                matches!(
                    overlay,
                    ClientShellOverlay::TaskFileEditor(editor) if editor.tabs.len() > 1
                )
            })
        {
            self.switch_task_editor_tab(if code == KeyCode::Char('[') { -1 } else { 1 });
            outcome.repaint = true;
            return true;
        }
        if code == KeyCode::Char('x')
            && modifiers.is_empty()
            && self.overlay.as_ref().is_some_and(|overlay| {
                matches!(
                    overlay,
                    ClientShellOverlay::TaskFileEditor(editor)
                        if editor.field == ClientTaskFileEditorField::Content
                            && !editor.path.is_empty()
                            && !editor.loading
                )
            })
        {
            self.close_task_editor_tab();
            outcome.repaint = true;
            return true;
        }
        if code == KeyCode::Esc && modifiers.is_empty() {
            let dirty = self.overlay.as_ref().is_some_and(|overlay| {
                matches!(overlay, ClientShellOverlay::TaskFileEditor(editor) if editor.dirty)
            });
            if dirty {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.error = Some("unsaved changes; press ctrl+s before closing".to_owned());
                }
            } else {
                self.overlay = None;
            }
        } else if code == KeyCode::Char('s') && modifiers == KeyModifiers::CONTROL {
            self.save_task_file(outcome);
        } else if code == KeyCode::Char('r') && modifiers.is_empty() {
            let path_field = self.overlay.as_ref().is_some_and(|overlay| {
                matches!(
                    overlay,
                    ClientShellOverlay::TaskFileEditor(editor)
                        if editor.field == ClientTaskFileEditorField::Path
                )
            });
            if path_field {
                self.refresh_task_file_list(outcome);
            }
        } else if code == KeyCode::Tab || code == KeyCode::BackTab {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                editor.field = match editor.field {
                    ClientTaskFileEditorField::Path => ClientTaskFileEditorField::Content,
                    ClientTaskFileEditorField::Content => ClientTaskFileEditorField::Path,
                };
                editor.error = None;
            }
        } else if code == KeyCode::Enter && modifiers.is_empty() {
            let find_mode = self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskFileEditor(editor) => editor.find_mode,
                _ => None,
            });
            if let Some(replace) = find_mode {
                if replace {
                    self.replace_task_editor_matches();
                } else if let Some(ClientShellOverlay::TaskFileEditor(editor)) =
                    self.overlay.as_mut()
                {
                    editor.find_mode = Some(true);
                }
            } else {
                let field = match self.overlay.as_ref() {
                    Some(ClientShellOverlay::TaskFileEditor(editor)) => editor.field,
                    _ => return true,
                };
                match field {
                    ClientTaskFileEditorField::Path => self.request_task_file_read(outcome),
                    ClientTaskFileEditorField::Content => {
                        self.insert_overlay_text("\n");
                    }
                }
            }
        } else if code == KeyCode::Backspace && modifiers.is_empty() {
            self.backspace_task_editor();
        } else if code == KeyCode::Left && modifiers.is_empty() {
            self.move_task_editor_cursor(false);
        } else if code == KeyCode::Right && modifiers.is_empty() {
            self.move_task_editor_cursor(true);
        } else if code == KeyCode::Home && modifiers.is_empty() {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                if editor.field == ClientTaskFileEditorField::Content {
                    editor.cursor = editor
                        .content
                        .get(..editor.cursor)
                        .and_then(|value| value.rfind('\n'))
                        .map_or(0, |index| index + 1);
                }
            }
        } else if code == KeyCode::End && modifiers.is_empty() {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                if editor.field == ClientTaskFileEditorField::Content {
                    editor.cursor = editor
                        .content
                        .get(editor.cursor..)
                        .and_then(|value| value.find('\n'))
                        .map_or(editor.content.len(), |offset| editor.cursor + offset);
                }
            }
        } else if code == KeyCode::Char('u') && modifiers == KeyModifiers::CONTROL {
            if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                if let Some(replace) = editor.find_mode {
                    if replace {
                        editor.replace_text.clear();
                    } else {
                        editor.find_query.clear();
                    }
                } else {
                    match editor.field {
                        ClientTaskFileEditorField::Path => editor.path.clear(),
                        ClientTaskFileEditorField::Content => {
                            editor.content.clear();
                            editor.cursor = 0;
                            editor.dirty = true;
                        }
                    }
                }
                editor.error = None;
            }
        }
        outcome.repaint = true;
        true
    }
    pub(super) fn move_tmux_selection(&mut self, delta: isize) {
        if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
            let last = overlay.panes.len().saturating_sub(1);
            overlay.selected = (overlay.selected as isize + delta).clamp(0, last as isize) as usize;
        }
    }

    fn route_tmux_panes_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if !matches!(self.overlay, Some(ClientShellOverlay::TmuxPanes(_))) {
            return false;
        }
        let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
        match code {
            KeyCode::Esc if modifiers.is_empty() => {
                let watching = matches!(
                    self.overlay,
                    Some(ClientShellOverlay::TmuxPanes(ClientTmuxPaneOverlay {
                        watching: true,
                        ..
                    }))
                );
                if watching {
                    if let Some(ClientShellOverlay::TmuxPanes(overlay)) = self.overlay.as_mut() {
                        overlay.watching = false;
                        overlay.output.clear();
                        overlay.error = None;
                    }
                } else {
                    self.overlay = None;
                    self.tmux_next_refresh = None;
                }
            }
            KeyCode::Up | KeyCode::Char('k') if modifiers.is_empty() => {
                self.move_tmux_selection(-1);
            }
            KeyCode::Down | KeyCode::Char('j') if modifiers.is_empty() => {
                self.move_tmux_selection(1);
            }
            KeyCode::Enter if modifiers.is_empty() => self.open_selected_tmux_pane(outcome),
            KeyCode::Char('r') if modifiers.is_empty() => {
                let watching = matches!(
                    self.overlay,
                    Some(ClientShellOverlay::TmuxPanes(ClientTmuxPaneOverlay {
                        watching: true,
                        ..
                    }))
                );
                if watching {
                    self.request_tmux_capture(outcome);
                } else {
                    self.request_tmux_list(outcome);
                }
            }
            KeyCode::Char('c') if modifiers == crossterm::event::KeyModifiers::CONTROL => {
                self.send_tmux_interrupt(outcome)
            }
            KeyCode::Char('f') if modifiers.is_empty() => self.focus_selected_tmux_pane(outcome),
            KeyCode::Char('s') if modifiers.is_empty() => self.send_tmux_interrupt(outcome),
            KeyCode::Char('x') if modifiers.is_empty() => self.close_selected_tmux_pane(outcome),
            _ => return true,
        }
        outcome.repaint = true;
        true
    }

    pub(super) fn handle_task_file_list_result(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        match result {
            Ok(crate::api::schema::ResponseResult::TaskFileList { files, .. }) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.files = files;
                    for tab in &mut editor.tabs {
                        tab.deleted = !editor
                            .files
                            .iter()
                            .any(|file| !file.is_dir && file.path == tab.path);
                    }
                    let selected_is_file = editor
                        .files
                        .get(editor.selected_file)
                        .is_some_and(|file| !file.is_dir);
                    if !selected_is_file {
                        editor.selected_file = editor
                            .files
                            .iter()
                            .position(|file| !file.is_dir)
                            .unwrap_or(0);
                    }
                    if editor.path.is_empty() {
                        if let Some(file) = editor.files.get(editor.selected_file) {
                            if !file.is_dir {
                                editor.path = file.path.clone();
                            }
                        }
                    }
                    editor.loading = false;
                    editor.error = None;
                }
            }
            Ok(_) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.loading = false;
                    editor.error = Some("unexpected task file list response".to_owned());
                }
            }
            Err(error) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.loading = false;
                    editor.error = Some(error.message);
                }
            }
        }
        (true, Vec::new())
    }

    pub(super) fn handle_task_file_read_result(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        match result {
            Ok(crate::api::schema::ResponseResult::TaskFileContent { file }) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.path = file.path;
                    editor.content = file.content;
                    editor.cursor = editor.content.len();
                    editor.field = ClientTaskFileEditorField::Content;
                    editor.dirty = false;
                    editor.loading = false;
                    editor.error = None;
                    let tab_index = editor.tabs.iter().position(|tab| tab.path == editor.path);
                    if let Some(tab_index) = tab_index {
                        if let Some(tab) = editor.tabs.get_mut(tab_index) {
                            tab.content = editor.content.clone();
                            tab.cursor = editor.cursor;
                            tab.dirty = false;
                            tab.deleted = false;
                        }
                        editor.selected_tab = tab_index;
                    } else {
                        editor.tabs.push(ClientTaskFileEditorTab {
                            path: editor.path.clone(),
                            content: editor.content.clone(),
                            cursor: editor.cursor,
                            dirty: false,
                            deleted: false,
                        });
                        editor.selected_tab = editor.tabs.len().saturating_sub(1);
                    }
                }
            }
            Ok(_) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.loading = false;
                    editor.error = Some("unexpected task file read response".to_owned());
                }
            }
            Err(error) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.loading = false;
                    editor.error = Some(error.message);
                }
            }
        }
        (true, Vec::new())
    }

    pub(super) fn handle_task_file_write_result(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        match result {
            Ok(crate::api::schema::ResponseResult::TaskFileWritten { .. }) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.saving = false;
                    editor.dirty = false;
                    editor.error = None;
                    if let Some(tab) = editor.tabs.iter_mut().find(|tab| tab.path == editor.path) {
                        tab.content = editor.content.clone();
                        tab.cursor = editor.cursor;
                        tab.dirty = false;
                        tab.deleted = false;
                    }
                }
            }
            Ok(_) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.saving = false;
                    editor.error = Some("unexpected task file write response".to_owned());
                }
            }
            Err(error) => {
                if let Some(ClientShellOverlay::TaskFileEditor(editor)) = self.overlay.as_mut() {
                    editor.saving = false;
                    editor.error = Some(error.message);
                }
            }
        }
        (true, Vec::new())
    }

    pub(super) fn handle_resource_list_result(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        let (resources, error) = match result {
            Ok(crate::api::schema::ResponseResult::ResourceList { resources }) => (resources, None),
            Ok(_) => (
                Vec::new(),
                Some("unexpected resource list response".to_owned()),
            ),
            Err(error) => (Vec::new(), Some(error.message)),
        };
        if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
            let selected_id = library
                .resources
                .get(library.selected)
                .map(|resource| resource.resource_id.clone());
            library.resources = resources;
            library.selected = selected_id
                .and_then(|id| {
                    library
                        .resources
                        .iter()
                        .position(|resource| resource.resource_id == id)
                })
                .unwrap_or_else(|| {
                    library
                        .selected
                        .min(library.resources.len().saturating_sub(1))
                });
            library.loading = false;
            library.pending = false;
            library.error = error;
        }
        (true, Vec::new())
    }

    pub(super) fn handle_resource_action_result(
        &mut self,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        match result {
            Ok(crate::api::schema::ResponseResult::ResourceInfo { resource }) => {
                if matches!(self.overlay, Some(ClientShellOverlay::ResourceEditor(_))) {
                    self.overlay = None;
                } else if let Some(ClientShellOverlay::ResourceLibrary(library)) =
                    self.overlay.as_mut()
                {
                    if let Some(existing) = library
                        .resources
                        .iter_mut()
                        .find(|existing| existing.resource_id == resource.resource_id)
                    {
                        *existing = resource;
                    }
                    library.pending = false;
                    library.error = None;
                }
            }
            Ok(crate::api::schema::ResponseResult::ResourceDeleted { resource_id }) => {
                if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
                    library
                        .resources
                        .retain(|resource| resource.resource_id != resource_id);
                    library.selected = library
                        .selected
                        .min(library.resources.len().saturating_sub(1));
                    library.pending = false;
                    library.error = None;
                }
            }
            Ok(crate::api::schema::ResponseResult::TaskResourcesUpdated { task }) => {
                if let Some(ClientShellOverlay::ResourceLibrary(library)) = self.overlay.as_mut() {
                    library.assigned_resource_ids = task.resource_ids;
                    library.pending = false;
                    library.error = None;
                }
            }
            Ok(_) => {
                if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                    editor.saving = false;
                    editor.error = Some("unexpected resource response".to_owned());
                } else if let Some(ClientShellOverlay::ResourceLibrary(library)) =
                    self.overlay.as_mut()
                {
                    library.pending = false;
                    library.error = Some("unexpected resource response".to_owned());
                }
            }
            Err(error) => {
                if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut() {
                    editor.saving = false;
                    editor.error = Some(error.message);
                } else if let Some(ClientShellOverlay::ResourceLibrary(library)) =
                    self.overlay.as_mut()
                {
                    library.pending = false;
                    library.error = Some(error.message);
                }
            }
        }
        (true, Vec::new())
    }

    fn route_resource_library_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if !matches!(self.overlay, Some(ClientShellOverlay::ResourceLibrary(_))) {
            return false;
        }
        let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
        match code {
            KeyCode::Esc if modifiers.is_empty() => self.overlay = None,
            KeyCode::Up | KeyCode::Char('k') if modifiers.is_empty() => {
                self.move_resource_selection(-1)
            }
            KeyCode::Down | KeyCode::Char('j') if modifiers.is_empty() => {
                self.move_resource_selection(1)
            }
            KeyCode::Char('n') if modifiers.is_empty() => {
                if self.resource_library_is_task_context() {
                    self.open_new_task_resource_editor();
                } else if self.resource_library_is_project_context() {
                    self.open_new_project_resource_editor();
                } else {
                    self.open_new_resource_editor();
                }
            }
            KeyCode::Char('e') if modifiers.is_empty() => {
                if let Some((resource, endpoint_id)) =
                    self.overlay.as_ref().and_then(|overlay| match overlay {
                        ClientShellOverlay::ResourceLibrary(library)
                            if !library.pending && !library.loading =>
                        {
                            Some((
                                library.resources.get(library.selected).cloned()?,
                                library.endpoint_id.clone(),
                            ))
                        }
                        _ => None,
                    })
                {
                    self.open_resource_editor(Some(resource));
                    if let Some(ClientShellOverlay::ResourceEditor(editor)) = self.overlay.as_mut()
                    {
                        editor.endpoint_id = endpoint_id;
                    }
                }
            }
            KeyCode::Enter if modifiers.is_empty() => {
                if self.resource_library_is_task_context() {
                    self.update_selected_task_resources(outcome);
                } else {
                    self.update_selected_resource(outcome);
                }
            }
            KeyCode::Char('d') if modifiers.is_empty() => self.delete_selected_resource(outcome),
            KeyCode::Char('r') if modifiers.is_empty() => self.request_resource_list(outcome),
            _ => {}
        }
        outcome.repaint = true;
        true
    }

    fn route_task_browser_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if !matches!(self.overlay, Some(ClientShellOverlay::TaskBrowser(_))) {
            return false;
        }
        let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
        match code {
            KeyCode::Esc if modifiers.is_empty() => self.overlay = None,
            KeyCode::Enter if modifiers.is_empty() => self.open_selected_task(outcome),
            KeyCode::Up | KeyCode::Char('k') if modifiers.is_empty() => {
                self.move_task_browser_selection(-1)
            }
            KeyCode::Down | KeyCode::Char('j') if modifiers.is_empty() => {
                self.move_task_browser_selection(1)
            }
            KeyCode::Char('b') if modifiers.is_empty() => self.open_selected_task_web_browser(),
            KeyCode::Char('e') if modifiers.is_empty() => self.open_selected_task_editor(outcome),
            KeyCode::Char('t') if modifiers.is_empty() => self.retry_selected_task(outcome),
            KeyCode::Char('r') if modifiers.is_empty() => self.open_task_resource_library(outcome),
            KeyCode::Char('R') if modifiers.is_empty() => self.request_task_browser(outcome),
            _ => {}
        }
        outcome.repaint = true;
        true
    }
    pub(super) fn handle_task_list_result(
        &mut self,
        endpoint_id: ClientEndpointId,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        let endpoint_label = self.endpoint_label(&endpoint_id).to_owned();
        let (tasks, error) = match result {
            Ok(crate::api::schema::ResponseResult::TaskList { tasks }) => (tasks, None),
            Ok(_) => (Vec::new(), Some("unexpected task list response".to_owned())),
            Err(error) => (Vec::new(), Some(error.message)),
        };
        if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
            if !tasks.is_empty() {
                browser
                    .task_endpoints
                    .extend(std::iter::repeat_n(endpoint_id.clone(), tasks.len()));
                browser
                    .task_endpoint_labels
                    .extend(std::iter::repeat_n(endpoint_label, tasks.len()));
                browser.tasks.extend(tasks);
            }
            browser.pending_requests = browser.pending_requests.saturating_sub(1);
            browser.loading = browser.pending_requests != 0;
            browser.selected = browser.selected.min(browser.tasks.len().saturating_sub(1));
            if browser.tasks.is_empty() {
                browser.error = error;
            } else {
                browser.error = None;
            }
        }
        (true, Vec::new())
    }
    fn persist_task_browser_profiles(
        &mut self,
        task_id: String,
        profiles: Vec<preferences::ClientBrowserProfile>,
        outcome: &mut ClientShellInput,
    ) {
        self.config
            .preferences
            .browser_profiles
            .insert(task_id, profiles);
        self.persist_chrome_preferences(outcome);
    }

    fn save_task_browser_profile(&mut self, outcome: &mut ClientShellInput) {
        let Some((task_id, profile_name, url, selected)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskWebBrowser(browser) => Some((
                    browser.task_id.clone(),
                    browser.profile_name.trim().to_owned(),
                    browser.url.trim().to_owned(),
                    browser.selected_profile,
                )),
                _ => None,
            })
        else {
            return;
        };
        if crate::app::actions::safe_web_url(&url).is_none() {
            if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                browser.error = Some("URL must use http:// or https://".to_owned());
            }
            return;
        }
        if profile_name.is_empty() {
            if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                browser.error = Some("profile name must not be empty".to_owned());
            }
            return;
        }
        if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_ref() {
            if browser
                .profiles
                .iter()
                .enumerate()
                .any(|(index, profile)| index != selected && profile.name == profile_name)
            {
                if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                    browser.error = Some("profile name must be unique for this task".to_owned());
                }
                return;
            }
        }
        let selected_name = profile_name.clone();
        let profiles =
            if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                let profile = preferences::ClientBrowserProfile {
                    name: profile_name,
                    url,
                };
                if let Some(existing) = browser.profiles.get_mut(selected) {
                    *existing = profile;
                } else {
                    browser.profiles.push(profile);
                    browser.selected_profile = browser.profiles.len().saturating_sub(1);
                }
                browser.error = None;
                browser.profiles.clone()
            } else {
                return;
            };
        self.persist_task_browser_profiles(task_id.clone(), profiles, outcome);
        self.persist_task_browser_selection(task_id, Some(selected_name), outcome);
    }
    fn persist_task_browser_selection(
        &mut self,
        task_id: String,
        profile_name: Option<String>,
        outcome: &mut ClientShellInput,
    ) {
        if let Some(profile_name) = profile_name {
            self.config
                .preferences
                .browser_last_profiles
                .insert(task_id, profile_name);
        } else {
            self.config
                .preferences
                .browser_last_profiles
                .remove(&task_id);
        }
        self.persist_chrome_preferences(outcome);
    }

    fn select_task_browser_profile(&mut self, delta: isize, outcome: &mut ClientShellInput) {
        let Some((task_id, profile_name)) =
            self.overlay.as_mut().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskWebBrowser(browser) => {
                    if browser.profiles.is_empty() {
                        return None;
                    }
                    browser.selected_profile = (browser.selected_profile as isize + delta)
                        .clamp(0, browser.profiles.len() as isize - 1)
                        as usize;
                    let profile = browser.profiles.get(browser.selected_profile)?;
                    browser.url = profile.url.clone();
                    browser.profile_name = profile.name.clone();
                    browser.preview_url = None;
                    browser.preview_body.clear();
                    browser.error = None;
                    Some((browser.task_id.clone(), profile.name.clone()))
                }
                _ => None,
            })
        else {
            return;
        };
        self.persist_task_browser_selection(task_id, Some(profile_name), outcome);
    }
    fn delete_task_browser_profile(&mut self, outcome: &mut ClientShellInput) {
        let Some((task_id, deleted_profile_name)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskWebBrowser(browser) => Some((
                    browser.task_id.clone(),
                    browser
                        .profiles
                        .get(browser.selected_profile)
                        .map(|profile| profile.name.clone())?,
                )),
                _ => None,
            })
        else {
            return;
        };
        let (profiles, selected_name) =
            if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                browser.profiles.remove(browser.selected_profile);
                browser.selected_profile = browser
                    .selected_profile
                    .min(browser.profiles.len().saturating_sub(1));
                if let Some(profile) = browser.profiles.get(browser.selected_profile) {
                    browser.url = profile.url.clone();
                    browser.profile_name = profile.name.clone();
                } else {
                    browser.url.clear();
                    browser.profile_name = "local".to_owned();
                }
                browser.preview_url = None;
                browser.preview_body.clear();
                browser.error = None;
                (
                    browser.profiles.clone(),
                    browser
                        .profiles
                        .get(browser.selected_profile)
                        .map(|profile| profile.name.clone()),
                )
            } else {
                return;
            };
        self.persist_task_browser_profiles(task_id.clone(), profiles, outcome);
        self.persist_task_browser_selection(task_id.clone(), selected_name, outcome);
        if let Err(error) =
            crate::app::actions::clear_web_profile_storage(&task_id, &deleted_profile_name)
        {
            if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                browser.error = Some(error);
            }
        }
    }

    fn backspace_task_browser(&mut self) {
        if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
            match browser.field {
                ClientTaskBrowserField::Url => {
                    browser.url.pop();
                }
                ClientTaskBrowserField::ProfileName => {
                    browser.profile_name.pop();
                }
            }
            browser.error = None;
            browser.preview_url = None;
            browser.preview_body.clear();
        }
    }

    fn clear_task_browser_profile(&mut self) {
        let Some((task_id, profile_name)) =
            self.overlay.as_ref().and_then(|overlay| match overlay {
                ClientShellOverlay::TaskWebBrowser(browser) => {
                    Some((browser.task_id.clone(), browser.profile_name.clone()))
                }
                _ => None,
            })
        else {
            return;
        };
        let result = crate::app::actions::clear_web_profile_storage(&task_id, &profile_name);
        if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
            browser.preview_url = None;
            browser.preview_body.clear();
            browser.error = result.err();
        }
    }

    fn preview_task_browser(&mut self, outcome: &mut ClientShellInput) {
        let Some(url) = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TaskWebBrowser(browser) => Some(browser.url.trim().to_owned()),
            _ => None,
        }) else {
            return;
        };
        self.save_task_browser_profile(outcome);
        if self.overlay.as_ref().is_some_and(|overlay| {
            matches!(
                overlay,
                ClientShellOverlay::TaskWebBrowser(browser) if browser.error.is_some()
            )
        }) {
            return;
        }
        let (task_id, profile_name) = self
            .overlay
            .as_ref()
            .and_then(|overlay| match overlay {
                ClientShellOverlay::TaskWebBrowser(browser) => {
                    Some((browser.task_id.clone(), browser.profile_name.clone()))
                }
                _ => None,
            })
            .unwrap_or_default();
        match crate::app::actions::fetch_web_preview_for_profile(&url, &task_id, &profile_name) {
            Ok(lines) => {
                if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                    browser.preview_url = Some(url);
                    browser.preview_body = lines;
                    browser.error = None;
                }
            }
            Err(error) => {
                if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                    browser.preview_url = None;
                    browser.preview_body.clear();
                    browser.error = Some(error);
                }
            }
        }
    }

    fn open_task_browser_external(&mut self, outcome: &mut ClientShellInput) {
        let url = self.overlay.as_ref().and_then(|overlay| match overlay {
            ClientShellOverlay::TaskWebBrowser(browser) => {
                browser.preview_url.clone().or_else(|| {
                    crate::app::actions::safe_web_url(browser.url.trim()).map(str::to_owned)
                })
            }
            _ => None,
        });
        let Some(url) = url else {
            if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                browser.error = Some("URL must use http:// or https://".to_owned());
            }
            outcome.repaint = true;
            return;
        };
        outcome.actions.push(ClientShellAction::OpenSafeWebUrl(url));
    }

    fn route_task_web_browser_key(
        &mut self,
        key: &crate::input::TerminalKey,
        outcome: &mut ClientShellInput,
    ) -> bool {
        if !matches!(self.overlay, Some(ClientShellOverlay::TaskWebBrowser(_))) {
            return false;
        }
        let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
        use crossterm::event::KeyModifiers;
        match code {
            KeyCode::Esc if modifiers.is_empty() => self.overlay = None,
            KeyCode::Tab if modifiers.is_empty() => {
                if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                    browser.field = match browser.field {
                        ClientTaskBrowserField::Url => ClientTaskBrowserField::ProfileName,
                        ClientTaskBrowserField::ProfileName => ClientTaskBrowserField::Url,
                    };
                }
            }
            KeyCode::Up | KeyCode::Char('k') if modifiers.is_empty() => {
                self.select_task_browser_profile(-1, outcome)
            }
            KeyCode::Down | KeyCode::Char('j') if modifiers.is_empty() => {
                self.select_task_browser_profile(1, outcome)
            }
            KeyCode::Enter if modifiers.is_empty() => {
                if matches!(
                    self.overlay,
                    Some(ClientShellOverlay::TaskWebBrowser(
                        ClientTaskWebBrowserOverlay {
                            field: ClientTaskBrowserField::ProfileName,
                            ..
                        }
                    ))
                ) {
                    self.save_task_browser_profile(outcome);
                } else {
                    self.preview_task_browser(outcome);
                }
            }
            KeyCode::Char('n') if modifiers.is_empty() => {
                if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                    browser.profile_name.clear();
                    browser.url.clear();
                    browser.selected_profile = browser.profiles.len();
                    browser.field = ClientTaskBrowserField::ProfileName;
                    browser.preview_url = None;
                    browser.preview_body.clear();
                    browser.error = None;
                }
            }
            KeyCode::Char('d') if modifiers.is_empty() => self.delete_task_browser_profile(outcome),
            KeyCode::Char('c') if modifiers.is_empty() => self.clear_task_browser_profile(),
            KeyCode::Char('x') if modifiers.is_empty() => self.open_task_browser_external(outcome),
            KeyCode::Backspace if modifiers.is_empty() => self.backspace_task_browser(),
            KeyCode::Char('u') if modifiers == KeyModifiers::CONTROL => {
                if let Some(ClientShellOverlay::TaskWebBrowser(browser)) = self.overlay.as_mut() {
                    match browser.field {
                        ClientTaskBrowserField::Url => browser.url.clear(),
                        ClientTaskBrowserField::ProfileName => browser.profile_name.clear(),
                    }
                    browser.preview_url = None;
                    browser.preview_body.clear();
                    browser.error = None;
                }
            }
            _ => {}
        }
        outcome.repaint = true;
        true
    }

    pub(super) fn handle_task_open_result(
        &mut self,
        endpoint_id: ClientEndpointId,
        result: Result<crate::api::schema::ResponseResult, ClientShellEndpointError>,
    ) -> (bool, Vec<ClientShellAction>) {
        match result {
            Ok(crate::api::schema::ResponseResult::TaskOpened { task, runtime }) => {
                self.overlay = None;
                if endpoint_id != self.active_endpoint_id {
                    let target = runtime
                        .workspace_id
                        .or(task.workspace_id)
                        .map(ClientEndpointFocusTarget::Workspace);
                    return (
                        true,
                        vec![ClientShellAction::ActivateEndpoint {
                            endpoint_id,
                            target,
                        }],
                    );
                }
            }
            Ok(_) => {
                if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
                    browser.opening = false;
                    browser.error = Some("unexpected task open response".to_owned());
                }
            }
            Err(error) => {
                if let Some(ClientShellOverlay::TaskBrowser(browser)) = self.overlay.as_mut() {
                    browser.opening = false;
                    browser.error = Some(error.message);
                }
            }
        }
        (true, Vec::new())
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

        if self.route_task_web_browser_key(key, outcome) {
            return;
        }
        if self.route_task_file_editor_key(key, outcome) {
            return;
        }
        if self.route_tmux_panes_key(key, outcome) {
            return;
        }
        if self.route_resource_editor_key(key, outcome) {
            return;
        }
        if self.route_resource_library_key(key, outcome) {
            return;
        }
        if self.route_task_browser_key(key, outcome) {
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

        if self.route_worktree_app_key(key, outcome) {
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
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    self.open_project_resource_library(outcome)
                }
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
                environment: None,
                preserve_patterns: Some(patterns.patterns.clone()),
                lifecycle: None,
                remote_endpoint_id: None,
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
                    environment: None,
                    default_agent: None,
                    preserve_patterns: None,
                    remote_endpoint_id: None,
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
                    environment: None,
                    default_agent: Some(trimmed.to_owned()),
                    preserve_patterns: None,
                    lifecycle: None,
                    remote_endpoint_id: None,
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
