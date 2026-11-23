use std::path::PathBuf;
use std::sync::atomic::Ordering;

use super::PaneRuntime;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct CwdSnapshot {
    pub cwd: Option<PathBuf>,
    pub foreground_cwd: Option<PathBuf>,
    pub follow_cwd: Option<PathBuf>,
}

impl PaneRuntime {
    pub(crate) fn cwd_probe(&self) -> impl FnOnce() -> CwdSnapshot + Send + 'static {
        let pid = self.child_pid.load(Ordering::Acquire);
        let reported_cwd = self.reported_cwd.clone();
        #[cfg(unix)]
        let foreground_actor = match &self.io {
            super::PaneRuntimeIo::Actor(actor) => Some(actor.clone()),
            #[cfg(test)]
            super::PaneRuntimeIo::TestChannel { .. } => None,
        };
        move || {
            let cwd = reported_cwd
                .lock()
                .ok()
                .and_then(|cwd| cwd.clone())
                .or_else(|| crate::platform::pane_process_cwd(pid));
            #[cfg(unix)]
            let (foreground_cwd, follow_cwd) = {
                let foreground_pgid = foreground_actor
                    .as_ref()
                    .and_then(|actor| actor.foreground_process_group_id());
                let shell_cwd = super::absolute_process_cwd(pid);
                let leader_cwd = foreground_pgid
                    .or_else(|| crate::platform::foreground_process_group_id(pid))
                    .and_then(super::absolute_process_cwd);
                let foreground_cwd = leader_cwd.or_else(|| {
                    super::foreground_member_cwd_different_from_shell(pid, shell_cwd.as_ref())
                });
                let follow_cwd = foreground_pgid
                    .and_then(super::usable_process_cwd)
                    .or_else(|| cwd.clone());
                (foreground_cwd, follow_cwd)
            };
            #[cfg(not(unix))]
            let (foreground_cwd, follow_cwd) = (None, cwd.clone());
            CwdSnapshot {
                cwd,
                foreground_cwd,
                follow_cwd,
            }
        }
    }
}
