use std::path::PathBuf;

use crate::api::schema::{
    AgentStatus, ResponseResult, TmuxAgentStartParams, TmuxPaneCaptureParams, TmuxPaneInfo,
    TmuxPaneOrigin, TmuxPaneSendKeysParams, TmuxPaneTarget,
};
use crate::app::App;

use super::responses::{encode_error, encode_success};

const MAX_CAPTURE_LINES: u32 = 500;

impl App {
    pub(super) fn handle_agent_start_tmux(
        &mut self,
        id: String,
        params: TmuxAgentStartParams,
    ) -> String {
        if !super::super::agents::valid_agent_name(&params.name) {
            return encode_error(
                id,
                "invalid_agent_name",
                "agent name must start with a lowercase letter and contain only lowercase letters, digits, '-' or '_' (1-32 characters)",
            );
        }
        let Some(kind) = crate::detect::parse_agent_label(&params.kind) else {
            return encode_error(
                id,
                "unsupported_agent_kind",
                format!("unsupported interactive agent kind {}", params.kind),
            );
        };
        if params
            .args
            .iter()
            .any(|arg| arg.chars().any(char::is_control))
            || !super::super::agents::valid_agent_environment(&params.environment)
        {
            return encode_error(
                id,
                "invalid_agent_argument",
                "tmux agent arguments and environment must be safely encodable",
            );
        }
        let panes = match crate::platform::list_tmux_panes() {
            Ok(panes) => panes,
            Err(err) => return tmux_error(id, err),
        };
        if panes.iter().any(|pane| {
            pane.managed && pane.agent_name.as_deref() == Some(params.name.as_str()) && !pane.dead
        }) {
            return encode_error(
                id,
                "agent_name_taken",
                format!("tmux agent name {} is already in use", params.name),
            );
        }
        let target = params.target.clone().or_else(|| {
            std::env::var_os("TMUX_PANE").and_then(|value| value.to_str().map(str::to_owned))
        });
        let cwd = match params.cwd {
            Some(path) => PathBuf::from(path),
            None => target
                .as_deref()
                .and_then(|target| {
                    panes
                        .iter()
                        .find(|pane| pane.pane_id == target)
                        .and_then(|pane| pane.current_path.clone())
                })
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from(".")),
        };
        if !cwd.is_dir() {
            return encode_error(
                id,
                "invalid_cwd",
                format!(
                    "tmux agent working directory is not a directory: {}",
                    cwd.display()
                ),
            );
        }
        let provider = crate::detect::agent_label(kind).to_owned();
        let mut argv = vec![crate::detect::interactive_agent_executable(kind).to_owned()];
        argv.extend(params.args);
        let spawned = match crate::platform::spawn_tmux_pane(
            target.as_deref(),
            &cwd,
            &argv,
            &params.environment,
            &params.name,
            &provider,
        ) {
            Ok(spawned) => spawned,
            Err(err) => return tmux_error(id, err),
        };
        let pane = match crate::platform::list_tmux_panes().ok().and_then(|panes| {
            panes
                .into_iter()
                .find(|pane| pane.pane_id == spawned.pane_id)
        }) {
            Some(pane) => pane,
            None => {
                return encode_error(
                    id,
                    "tmux_pane_unavailable",
                    format!(
                        "tmux spawned pane {} but it could not be read back",
                        spawned.pane_id
                    ),
                )
            }
        };
        encode_success(
            id,
            ResponseResult::AgentTmuxStarted {
                pane: tmux_pane_info(pane),
                argv,
            },
        )
    }

    pub(super) fn handle_tmux_list(&mut self, id: String) -> String {
        let panes = match crate::platform::list_tmux_panes() {
            Ok(panes) => panes.into_iter().map(tmux_pane_info).collect(),
            Err(err) => return tmux_error(id, err),
        };
        encode_success(id, ResponseResult::TmuxPaneList { panes })
    }

    pub(super) fn handle_tmux_capture(
        &mut self,
        id: String,
        params: TmuxPaneCaptureParams,
    ) -> String {
        let lines = params.lines.clamp(1, MAX_CAPTURE_LINES) as usize;
        match crate::platform::capture_tmux_pane(&params.pane_id, lines) {
            Ok(output) => encode_success(
                id,
                ResponseResult::TmuxPaneCaptured {
                    pane_id: params.pane_id,
                    output,
                },
            ),
            Err(err) => tmux_error(id, err),
        }
    }

    pub(super) fn handle_tmux_send_keys(
        &mut self,
        id: String,
        params: TmuxPaneSendKeysParams,
    ) -> String {
        match crate::platform::send_tmux_keys(&params.pane_id, &params.keys) {
            Ok(()) => encode_success(
                id,
                ResponseResult::TmuxPaneAction {
                    pane_id: params.pane_id,
                },
            ),
            Err(err) => tmux_error(id, err),
        }
    }

    pub(super) fn handle_tmux_kill(&mut self, id: String, target: TmuxPaneTarget) -> String {
        let pane = match crate::platform::list_tmux_panes().and_then(|panes| {
            panes
                .into_iter()
                .find(|pane| pane.pane_id == target.pane_id)
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "tmux pane was not found")
                })
        }) {
            Ok(pane) => pane,
            Err(err) => return tmux_error(id, err),
        };
        if !pane.managed {
            return encode_error(
                id,
                "tmux_pane_unmanaged",
                "only Herdr-created tmux subagent panes can be closed from Herdr",
            );
        }
        match crate::platform::kill_tmux_pane(&target.pane_id) {
            Ok(()) => encode_success(
                id,
                ResponseResult::TmuxPaneAction {
                    pane_id: target.pane_id,
                },
            ),
            Err(err) => tmux_error(id, err),
        }
    }

    pub(super) fn handle_tmux_focus(&mut self, id: String, target: TmuxPaneTarget) -> String {
        match crate::platform::focus_tmux_pane(&target.pane_id) {
            Ok(()) => encode_success(
                id,
                ResponseResult::TmuxPaneAction {
                    pane_id: target.pane_id,
                },
            ),
            Err(err) => tmux_error(id, err),
        }
    }
}

fn tmux_pane_info(pane: crate::platform::TmuxPaneSnapshot) -> TmuxPaneInfo {
    let status = if pane.dead {
        AgentStatus::Done
    } else if pane.managed || !is_shell_command(&pane.current_command) {
        AgentStatus::Working
    } else {
        AgentStatus::Idle
    };
    TmuxPaneInfo {
        pane_id: pane.pane_id,
        session_id: pane.session_id,
        session_name: pane.session_name,
        window_id: pane.window_id,
        window_index: pane.window_index,
        window_name: pane.window_name,
        pane_index: pane.pane_index,
        title: pane.title,
        current_command: pane.current_command,
        current_path: pane
            .current_path
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned()),
        active: pane.active,
        dead: pane.dead,
        origin: if pane.managed {
            TmuxPaneOrigin::HerdrSubagent
        } else {
            TmuxPaneOrigin::External
        },
        agent_name: pane.agent_name,
        agent_kind: pane.agent_kind,
        parent_pane_id: pane.parent_pane_id,
        status,
        last_output: pane.last_output,
    }
}

fn is_shell_command(command: &str) -> bool {
    let command = command.rsplit(['/', '\\']).next().unwrap_or(command);
    matches!(
        command.to_ascii_lowercase().as_str(),
        "bash" | "dash" | "fish" | "ksh" | "pwsh" | "sh" | "tcsh" | "zsh"
    )
}

fn tmux_error(id: String, err: std::io::Error) -> String {
    let code = match err.kind() {
        std::io::ErrorKind::InvalidInput => "invalid_params",
        std::io::ErrorKind::NotFound
        | std::io::ErrorKind::Unsupported
        | std::io::ErrorKind::PermissionDenied => "tmux_unavailable",
        _ => "tmux_command_failed",
    };
    encode_error(id, code, err.to_string())
}
