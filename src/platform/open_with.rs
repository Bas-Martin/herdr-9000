use std::path::{Path, PathBuf};

use crate::open_with::Application;

pub(crate) fn is_executable(path: &Path) -> bool {
    if !path.is_absolute() || !path.is_file() {
        return false;
    }
    executable_file(path)
}

#[cfg(windows)]
fn executable_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

#[cfg(unix)]
fn executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(any(unix, windows)))]
fn executable_file(_path: &Path) -> bool {
    false
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|path| is_executable(path))
    })
}

pub(crate) fn ensure_supported(application: Application) -> Result<(), String> {
    if cfg!(windows) || application == Application::VsCode {
        Ok(())
    } else {
        Err(format!("{} is only supported on Windows. Use Visual Studio Code on this platform (Settings → Open With).", application.name()))
    }
}

#[cfg(windows)]
pub(crate) fn detect(application: Application) -> Option<PathBuf> {
    match application {
        Application::VsCode => {
            if let Some(path) = on_path("Code.exe") {
                return Some(path);
            }
            // The official installer adds its bin directory, not Code.exe, to PATH.
            if let Some(path) = std::env::var_os("PATH").and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|directory| directory.join("..\\Code.exe"))
                    .find(|path| is_executable(path))
            }) {
                return Some(path);
            }
            [
                ("LOCALAPPDATA", "Programs\\Microsoft VS Code\\Code.exe"),
                ("ProgramFiles", "Microsoft VS Code\\Code.exe"),
                ("ProgramFiles(x86)", "Microsoft VS Code\\Code.exe"),
            ]
            .into_iter()
            .filter_map(|(variable, suffix)| {
                std::env::var_os(variable).map(|base| PathBuf::from(base).join(suffix))
            })
            .find(|path| is_executable(path))
        }
        Application::VisualStudio => {
            let vswhere = on_path("vswhere.exe").or_else(|| {
                ["ProgramFiles(x86)", "ProgramFiles"]
                    .into_iter()
                    .filter_map(|variable| {
                        std::env::var_os(variable).map(|base| {
                            PathBuf::from(base)
                                .join("Microsoft Visual Studio\\Installer\\vswhere.exe")
                        })
                    })
                    .find(|path| is_executable(path))
            })?;
            let mut command = crate::noninteractive_process::command(vswhere);
            command.args([
                "-all",
                "-products",
                "*",
                "-prerelease",
                "-utf8",
                "-find",
                "Common7\\IDE\\devenv.exe",
            ]);
            let output = detection_output(command)?;
            String::from_utf8(output)
                .ok()?
                .lines()
                .map(|line| PathBuf::from(line.trim().trim_start_matches('\u{feff}')))
                .find(|path| is_executable(path))
        }
        Application::Explorer => None,
    }
}

#[cfg(windows)]
fn detection_output(mut command: std::process::Command) -> Option<Vec<u8>> {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return status
                    .success()
                    .then(|| child.wait_with_output().ok())
                    .flatten()
                    .map(|output| output.stdout)
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn detect(application: Application) -> Option<PathBuf> {
    if application != Application::VsCode {
        return None;
    }
    on_path("code").or_else(|| {
        let path =
            PathBuf::from("/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code");
        is_executable(&path).then_some(path)
    })
}

#[cfg(windows)]
pub(crate) fn launch(
    application: Application,
    executable: Option<&Path>,
    directory: &Path,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{GetExitCodeProcess, WaitForSingleObject},
        UI::Shell::{
            ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
            SHELLEXECUTEINFOW,
        },
    };
    let file = executable
        .unwrap_or(directory)
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let parameters = executable.map(|_| {
        let directory = super::quote_windows_command_line_arg(&directory.to_string_lossy());
        if application == Application::VsCode {
            format!("--new-window {directory}")
        } else {
            directory
        }
    });
    let parameters = parameters
        .as_ref()
        .map(|value| value.encode_utf16().chain(Some(0)).collect::<Vec<_>>());
    let verb = windows::core::w!("open");
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = parameters
        .as_ref()
        .map_or(std::ptr::null(), |value| value.as_ptr());
    info.nShow = 1;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(format!(
            "Cannot launch {}: {}",
            application.name(),
            std::io::Error::last_os_error()
        ));
    }
    // Shell launches can reuse an existing GUI process and return no process handle.
    // Windows reaps processes itself; always release any returned handle.
    if !info.hProcess.is_null() {
        let mut exit_code = 0;
        let exited = unsafe { WaitForSingleObject(info.hProcess, 0) } == 0;
        let has_code = exited && unsafe { GetExitCodeProcess(info.hProcess, &mut exit_code) } != 0;
        unsafe {
            CloseHandle(info.hProcess);
        }
        if has_code && exit_code != 0 {
            return Err(format!(
                "{} exited with code {exit_code}",
                application.name()
            ));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn launch(
    application: Application,
    executable: Option<&Path>,
    directory: &Path,
) -> Result<(), String> {
    use std::process::Stdio;
    ensure_supported(application)?;
    let executable = executable.ok_or_else(|| "Missing application executable".to_string())?;
    let runtime = tokio::runtime::Handle::try_current().map_err(|error| {
        format!(
            "Cannot launch {} without the application runtime: {error}",
            application.name()
        )
    })?;
    let mut command = tokio::process::Command::new(executable);
    command
        .arg("--new-window")
        .arg(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Cannot launch {}: {error}", application.name()))?;
    if let Some(status) = child
        .try_wait()
        .map_err(|error| format!("Cannot read {} launch result: {error}", application.name()))?
    {
        return if status.success() {
            Ok(())
        } else {
            Err(format!("{} exited with {status}", application.name()))
        };
    }
    // Tokio owns the wait and reaps the child without blocking the TUI or a worker thread.
    runtime.spawn(async move {
        match child.wait().await {
            Ok(status) if !status.success() => {
                tracing::warn!("{} exited with {status}", application.name())
            }
            Err(error) => tracing::warn!("Cannot reap {}: {error}", application.name()),
            _ => {}
        }
    });
    Ok(())
}
