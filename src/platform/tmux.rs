use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaneSnapshot {
    pub(crate) pane_id: String,
    pub(crate) session_id: String,
    pub(crate) session_name: String,
    pub(crate) window_id: String,
    pub(crate) window_index: u32,
    pub(crate) window_name: String,
    pub(crate) pane_index: u32,
    pub(crate) title: String,
    pub(crate) current_command: String,
    pub(crate) current_path: Option<PathBuf>,
    pub(crate) active: bool,
    pub(crate) dead: bool,
    pub(crate) managed: bool,
    pub(crate) agent_name: Option<String>,
    pub(crate) agent_kind: Option<String>,
    pub(crate) parent_pane_id: Option<String>,
    pub(crate) last_output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpawnedPane {
    pub(crate) pane_id: String,
}

#[cfg(not(windows))]
const FIELD_SEPARATOR: char = '\u{1f}';
#[cfg(not(windows))]
const LIST_FORMAT: &str = "#{pane_id}\u{1f}#{session_id}\u{1f}#{session_name}\u{1f}#{window_id}\u{1f}#{window_index}\u{1f}#{window_name}\u{1f}#{pane_index}\u{1f}#{pane_title}\u{1f}#{pane_current_command}\u{1f}#{pane_current_path}\u{1f}#{pane_active}\u{1f}#{pane_dead}\u{1f}#{@herdr_managed}\u{1f}#{@herdr_agent_name}\u{1f}#{@herdr_agent_kind}\u{1f}#{@herdr_parent_pane_id}";

#[cfg(not(windows))]
pub(crate) fn list_panes() -> std::io::Result<Vec<PaneSnapshot>> {
    let output = run_tmux(&["list-panes", "-a", "-F", LIST_FORMAT])?;
    let mut panes = Vec::new();
    for line in output.lines().filter(|line| !line.is_empty()) {
        let fields = line.split(FIELD_SEPARATOR).collect::<Vec<_>>();
        if fields.len() != 16 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "tmux returned an unexpected pane record",
            ));
        }
        let pane = PaneSnapshot {
            pane_id: fields[0].to_owned(),
            session_id: fields[1].to_owned(),
            session_name: fields[2].to_owned(),
            window_id: fields[3].to_owned(),
            window_index: parse_u32(fields[4], "window index")?,
            window_name: fields[5].to_owned(),
            pane_index: parse_u32(fields[6], "pane index")?,
            title: fields[7].to_owned(),
            current_command: fields[8].to_owned(),
            current_path: (!fields[9].is_empty()).then(|| PathBuf::from(fields[9])),
            active: parse_bool(fields[10], "pane active")?,
            dead: parse_bool(fields[11], "pane dead")?,
            managed: fields[12] == "1",
            agent_name: non_empty(fields[13]),
            agent_kind: non_empty(fields[14]),
            parent_pane_id: non_empty(fields[15]),
            last_output: capture_pane(fields[0], 80)?,
        };
        panes.push(pane);
    }
    Ok(panes)
}

#[cfg(not(windows))]
pub(crate) fn capture_pane(pane_id: &str, lines: usize) -> std::io::Result<String> {
    validate_target(pane_id)?;
    let start = format!("-{}", lines.max(1));
    run_tmux(&["capture-pane", "-p", "-J", "-S", &start, "-t", pane_id])
}

#[cfg(not(windows))]
pub(crate) fn spawn_pane(
    target: Option<&str>,
    cwd: &Path,
    command: &[String],
    environment: &BTreeMap<String, String>,
    agent_name: &str,
    agent_kind: &str,
) -> std::io::Result<SpawnedPane> {
    if command.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "tmux agent command must not be empty",
        ));
    }
    let cwd = cwd.to_str().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "tmux pane working directory must be valid UTF-8",
        )
    })?;
    let target = target
        .map(str::to_owned)
        .or_else(|| {
            std::env::var_os("TMUX_PANE").and_then(|value| value.to_str().map(str::to_owned))
        })
        .map(Ok)
        .unwrap_or_else(|| ensure_agent_session(cwd))?;
    validate_target(&target)?;
    let mut args = vec![
        "split-window".to_owned(),
        "-d".to_owned(),
        "-P".to_owned(),
        "-F".to_owned(),
        "#{pane_id}".to_owned(),
        "-t".to_owned(),
        target.clone(),
        "-c".to_owned(),
        cwd.to_owned(),
        "-e".to_owned(),
        "HERDR_TMUX_MANAGED=1".to_owned(),
        "-e".to_owned(),
        format!("HERDR_TMUX_AGENT_NAME={agent_name}"),
        "-e".to_owned(),
        format!("HERDR_TMUX_AGENT_KIND={agent_kind}"),
        "-e".to_owned(),
        format!("HERDR_TMUX_PARENT_PANE_ID={target}"),
    ];
    for (key, value) in environment {
        args.push("-e".to_owned());
        args.push(format!("{key}={value}"));
    }
    args.push("--".to_owned());
    args.extend(command.iter().cloned());
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let pane_id = run_tmux(&arg_refs)?.trim().to_owned();
    if pane_id.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "tmux did not return the spawned pane id",
        ));
    }
    for (option, value) in [
        ("@herdr_managed", "1"),
        ("@herdr_agent_name", agent_name),
        ("@herdr_agent_kind", agent_kind),
        ("@herdr_parent_pane_id", target),
    ] {
        let option_args = ["set-option", "-p", "-t", &pane_id, option, value];
        run_tmux(&option_args)?;
    }
    Ok(SpawnedPane { pane_id })
}

#[cfg(not(windows))]
pub(crate) fn send_text(pane_id: &str, text: &str) -> std::io::Result<()> {
    validate_target(pane_id)?;
    if text.is_empty()
        || text
            .chars()
            .any(|character| character.is_control() && character != '\n')
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "tmux text must be non-empty and must not contain control characters",
        ));
    }
    run_tmux(&["send-keys", "-l", "-t", pane_id, text])?;
    run_tmux(&["send-keys", "-t", pane_id, "Enter"]).map(|_| ())
}

#[cfg(windows)]
pub(crate) fn send_text(_pane_id: &str, _text: &str) -> std::io::Result<()> {
    Err(unavailable())
}

#[cfg(not(windows))]
pub(crate) fn send_keys(pane_id: &str, keys: &[String]) -> std::io::Result<()> {
    validate_target(pane_id)?;
    if keys.is_empty()
        || keys
            .iter()
            .any(|key| key.is_empty() || key.chars().any(char::is_control))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "tmux keys must be printable and must not be empty",
        ));
    }
    let mut args = vec!["send-keys".to_owned(), "-t".to_owned(), pane_id.to_owned()];
    args.extend(keys.iter().cloned());
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_tmux(&arg_refs).map(|_| ())
}
#[cfg(not(windows))]
pub(crate) fn focus_pane(pane_id: &str) -> std::io::Result<()> {
    validate_target(pane_id)?;
    run_tmux(&["select-pane", "-t", pane_id]).map(|_| ())
}

#[cfg(not(windows))]
pub(crate) fn kill_pane(pane_id: &str) -> std::io::Result<()> {
    validate_target(pane_id)?;
    run_tmux(&["kill-pane", "-t", pane_id]).map(|_| ())
}

#[cfg(not(windows))]
const DEFAULT_AGENT_SESSION: &str = "herdr-agents";

#[cfg(not(windows))]
fn ensure_agent_session(cwd: &str) -> std::io::Result<String> {
    if run_tmux(&["has-session", "-t", DEFAULT_AGENT_SESSION]).is_err() {
        match run_tmux(&["new-session", "-d", "-s", DEFAULT_AGENT_SESSION, "-c", cwd]) {
            Ok(_) => {}
            Err(error) if run_tmux(&["has-session", "-t", DEFAULT_AGENT_SESSION]).is_ok() => {}
            Err(error) => return Err(error),
        }
    }
    Ok(format!("{DEFAULT_AGENT_SESSION}:0"))
}

#[cfg(not(windows))]
fn run_tmux(args: &[&str]) -> std::io::Result<String> {
    let output = std::process::Command::new("tmux")
        .args(args)
        .output()
        .map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("tmux is unavailable or could not be started: {error}"),
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("tmux exited with status {}", output.status)
        } else {
            stderr
        };
        return Err(std::io::Error::other(message));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(windows))]
fn validate_target(target: &str) -> std::io::Result<()> {
    if target.is_empty() || target.chars().any(char::is_control) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "tmux pane target must be printable and must not be empty",
        ));
    }
    Ok(())
}
#[cfg(not(windows))]

fn parse_u32(value: &str, field: &str) -> std::io::Result<u32> {
    value.parse().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("tmux returned an invalid {field}"),
        )
    })
}

#[cfg(not(windows))]
fn parse_bool(value: &str, field: &str) -> std::io::Result<bool> {
    match value {
        "1" => Ok(true),
        "0" => Ok(false),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("tmux returned an invalid {field}"),
        )),
    }
}

#[cfg(not(windows))]
fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(windows)]
pub(crate) fn list_panes() -> std::io::Result<Vec<PaneSnapshot>> {
    Err(unavailable())
}

#[cfg(windows)]
pub(crate) fn capture_pane(_pane_id: &str, _lines: usize) -> std::io::Result<String> {
    Err(unavailable())
}

#[cfg(windows)]
pub(crate) fn spawn_pane(
    _target: Option<&str>,
    _cwd: &Path,
    _command: &[String],
    _environment: &BTreeMap<String, String>,
    _agent_name: &str,
    _agent_kind: &str,
) -> std::io::Result<SpawnedPane> {
    Err(unavailable())
}

#[cfg(windows)]
pub(crate) fn send_keys(_pane_id: &str, _keys: &[String]) -> std::io::Result<()> {
    Err(unavailable())
}

#[cfg(windows)]
pub(crate) fn kill_pane(_pane_id: &str) -> std::io::Result<()> {
    Err(unavailable())
}

#[cfg(windows)]
pub(crate) fn focus_pane(_pane_id: &str) -> std::io::Result<()> {
    Err(unavailable())
}
#[cfg(windows)]
fn unavailable() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "tmux panes are unavailable on Windows; install and run tmux on a Unix host",
    )
}
