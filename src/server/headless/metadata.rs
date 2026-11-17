use super::*;

struct CwdObservation {
    terminal_id: crate::terminal::TerminalId,
    pid: Option<u32>,
    reported_cwd: PathBuf,
    cwd: crate::pane::CwdSnapshot,
}

#[derive(Default)]
pub(super) struct MetadataRefresh {
    pending: Option<std::thread::JoinHandle<Vec<CwdObservation>>>,
    next_refresh: Option<Instant>,
}

impl MetadataRefresh {
    pub(super) fn apply(&mut self, app: &mut app::App) -> bool {
        let mut changed = false;
        if self.pending.as_ref().is_some_and(|job| job.is_finished()) {
            if let Some(job) = self.pending.take() {
                match job.join() {
                    Ok(observations) => {
                        for observation in observations {
                            let Some(runtime) = app.terminal_runtimes.get(&observation.terminal_id)
                            else {
                                continue;
                            };
                            let Some(terminal) =
                                app.state.terminals.get_mut(&observation.terminal_id)
                            else {
                                continue;
                            };
                            // Reject observations spanning a respawn or a newer OSC cwd report.
                            if runtime.child_pid() == observation.pid
                                && terminal.cwd == observation.reported_cwd
                                && terminal.observed_cwd != observation.cwd
                            {
                                terminal.observed_cwd = observation.cwd;
                                changed = true;
                            }
                        }
                    }
                    Err(_) => warn!("presentation metadata worker panicked"),
                }
            }
        }
        changed
    }

    pub(super) fn request(&mut self, app: &app::App, now: Instant) {
        if self.pending.is_some() || self.next_refresh.is_some_and(|deadline| now < deadline) {
            return;
        }
        self.next_refresh = Some(now + Duration::from_millis(250));
        let probes = app
            .state
            .terminals
            .iter()
            .filter_map(|(id, terminal)| {
                let runtime = app.terminal_runtimes.get(id)?;
                Some((
                    id.clone(),
                    runtime.child_pid(),
                    terminal.cwd.clone(),
                    runtime.cwd_probe(),
                ))
            })
            .collect::<Vec<_>>();
        if probes.is_empty() {
            return;
        }
        let notify = app.render_notify.clone();
        match std::thread::Builder::new()
            .name("herdr-presentation-metadata".into())
            .spawn(move || {
                let observations = probes
                    .into_iter()
                    .map(|(terminal_id, pid, reported_cwd, probe)| CwdObservation {
                        terminal_id,
                        pid,
                        reported_cwd,
                        cwd: probe(),
                    })
                    .collect();
                notify.notify_one();
                observations
            }) {
            Ok(job) => self.pending = Some(job),
            Err(error) => warn!(%error, "failed to spawn presentation metadata worker"),
        }
    }
}
