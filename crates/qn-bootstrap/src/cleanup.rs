//! Session-owned execution-tree reaping (#93).
//!
//! macOS has no child subreaper or cgroup equivalent. This module therefore
//! combines three pieces of evidence instead of a numeric group kill:
//!
//! 1. every sandboxed process inherits a session-private `HOME`/`TMPDIR`/XDG
//!    environment marker (the values live under `QN_HOME/runtime/<session>`);
//! 2. a tracker records descendants observed while the managed parent owns
//!    them, including `setsid` children before reparenting;
//! 3. teardown freezes candidates, re-checks the `(pid, start_time)` identity,
//!    sends `SIGKILL`, and requires several marker-free scans before reporting
//!    [`CleanupState::Confirmed`].
//!
//! The proof is deliberately scoped: a process that clears the inherited
//! session environment and reparents before the first observation is not
//! visible without a kernel boundary (VM or a privileged supervisor). Such a
//! case leaves the baseline fallback in place rather than a false success.
//!
//! Identity is re-checked immediately before every signal, but the check and
//! `kill(2)` are separate syscalls and macOS has no `pidfd`. A PID recycled
//! inside that one-syscall window is not detectable; every PID recycled after
//! it is rejected by the recorded start time.

#[cfg(unix)]
mod unix {
    use nix::{
        errno::Errno,
        sys::signal::{Signal, kill},
        unistd::Pid as NixPid,
    };
    use qn_core::runtime::CleanupOutcome;
    use std::{
        collections::{BTreeSet, HashMap},
        os::unix::ffi::OsStrExt,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };
    use sysinfo::{Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};

    const TRACK_INTERVAL: Duration = Duration::from_millis(100);
    const MARKER_SCAN_EVERY: u32 = 10;
    const ROUND_INTERVAL: Duration = Duration::from_millis(50);
    const QUIET_ROUNDS: u32 = 3;
    const TEARDOWN_BUDGET: Duration = Duration::from_secs(4);
    const MAX_ROUNDS: u32 = 40;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct Identity {
        pid: u32,
        start_time: u64,
    }

    impl Identity {
        fn sysinfo_pid(self) -> Pid {
            Pid::from_u32(self.pid)
        }

        fn signal(self, signal: Signal) -> Result<(), Errno> {
            let pid = i32::try_from(self.pid).map_err(|_| Errno::EINVAL)?;
            kill(NixPid::from_raw(pid), signal)
        }
    }

    fn scan(include_environment: bool) -> System {
        let mut system = System::new();
        let kind = if include_environment {
            ProcessRefreshKind::nothing().with_environ(UpdateKind::Always)
        } else {
            ProcessRefreshKind::nothing()
        };
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        system
    }

    fn identity(system: &System, pid: Pid) -> Option<Identity> {
        system.process(pid).and_then(|process| {
            (!matches!(process.status(), ProcessStatus::Zombie)).then(|| Identity {
                pid: pid.as_u32(),
                start_time: process.start_time(),
            })
        })
    }

    fn descendants(system: &System, root: Identity) -> BTreeSet<Identity> {
        if identity(system, root.sysinfo_pid()) != Some(root) {
            return BTreeSet::new();
        }
        let mut children: HashMap<u32, Vec<Identity>> = HashMap::new();
        for (pid, process) in system.processes() {
            let Some(parent) = process.parent() else {
                continue;
            };
            if matches!(process.status(), ProcessStatus::Zombie) {
                continue;
            }
            children.entry(parent.as_u32()).or_default().push(Identity {
                pid: pid.as_u32(),
                start_time: process.start_time(),
            });
        }
        let mut found = BTreeSet::from([root]);
        let mut pending = vec![root.pid];
        while let Some(pid) = pending.pop() {
            let Some(kids) = children.get(&pid) else {
                continue;
            };
            for kid in kids {
                if found.insert(*kid) {
                    pending.push(kid.pid);
                }
            }
        }
        found
    }

    fn matches_marker(process: &sysinfo::Process, markers: &[String]) -> bool {
        process.environ().iter().any(|entry| {
            let bytes = entry.as_bytes();
            markers.iter().any(|marker| bytes == marker.as_bytes())
        })
    }

    fn marked(system: &System, markers: &[String], exclude: Identity) -> BTreeSet<Identity> {
        system
            .processes()
            .iter()
            .filter(|(pid, process)| {
                pid.as_u32() != exclude.pid
                    && !matches!(process.status(), ProcessStatus::Zombie)
                    && matches_marker(process, markers)
            })
            .map(|(pid, process)| Identity {
                pid: pid.as_u32(),
                start_time: process.start_time(),
            })
            .collect()
    }

    fn own_identity() -> Identity {
        let pid = std::process::id();
        let system = scan(false);
        identity(&system, Pid::from_u32(pid)).unwrap_or(Identity { pid, start_time: 0 })
    }

    fn live(system: &System, candidate: Identity) -> bool {
        identity(system, candidate.sysinfo_pid()) == Some(candidate)
    }

    /// A running session reaper. Drop stops tracking without claiming cleanup.
    #[derive(Debug)]
    pub(crate) struct SessionCleanup {
        root: Identity,
        markers: Arc<[String]>,
        seen: Arc<Mutex<BTreeSet<Identity>>>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl SessionCleanup {
        /// Start tracking the managed parent and its descendants.
        pub(crate) fn start(root_pid: u32, markers: Vec<String>) -> Self {
            let system = scan(false);
            let root = identity(&system, Pid::from_u32(root_pid)).unwrap_or(Identity {
                pid: root_pid,
                start_time: 0,
            });
            let seen = Arc::new(Mutex::new(BTreeSet::new()));
            let stop = Arc::new(AtomicBool::new(false));
            let markers: Arc<[String]> = markers.into();
            let me = own_identity();
            let thread = {
                let seen = Arc::clone(&seen);
                let stop = Arc::clone(&stop);
                let markers = Arc::clone(&markers);
                thread::spawn(move || track(root, me, &markers, &seen, &stop))
            };
            Self {
                root,
                markers,
                seen,
                stop,
                thread: Some(thread),
            }
        }

        /// Stop tracking and run the bounded freeze/kill/quiescence proof.
        pub(crate) fn finish(mut self) -> CleanupOutcome {
            self.halt();
            let seen = self.seen.lock().map_or_else(
                |poisoned| poisoned.into_inner().clone(),
                |seen| seen.clone(),
            );
            reap(Some(self.root), &self.markers, &seen)
        }

        fn halt(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    impl Drop for SessionCleanup {
        fn drop(&mut self) {
            self.halt();
        }
    }

    fn track(
        root: Identity,
        me: Identity,
        markers: &[String],
        seen: &Mutex<BTreeSet<Identity>>,
        stop: &AtomicBool,
    ) {
        let mut system = System::new();
        let mut tick = 0_u32;
        while !stop.load(Ordering::Acquire) {
            system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing(),
            );
            let mut batch = descendants(&system, root);
            if tick % MARKER_SCAN_EVERY == 0 {
                system.refresh_processes_specifics(
                    ProcessesToUpdate::All,
                    true,
                    ProcessRefreshKind::nothing().with_environ(UpdateKind::Always),
                );
                batch.extend(marked(&system, markers, me));
            }
            if let Ok(mut seen) = seen.lock() {
                seen.extend(batch);
            }
            tick = tick.wrapping_add(1);
            thread::sleep(TRACK_INTERVAL);
        }
    }

    /// Reap a session from its private environment markers alone.
    ///
    /// Used by `qn stop`, which owns no live tracker. A bounded scan still
    /// proves that no marked process remains; it cannot see a process that
    /// already cleared the marker.
    pub(crate) fn reap_session(markers: &[String]) -> CleanupOutcome {
        reap(None, markers, &BTreeSet::new())
    }

    fn reap(
        root: Option<Identity>,
        markers: &[String],
        seen: &BTreeSet<Identity>,
    ) -> CleanupOutcome {
        let me = own_identity();
        let probe = scan(true);
        let environment_readable = probe
            .process(Pid::from_u32(me.pid))
            .is_some_and(|process| !process.environ().is_empty());
        if !environment_readable {
            return CleanupOutcome::unconfirmed();
        }

        let mut candidates = seen.clone();
        candidates.extend(marked(&probe, markers, me));
        if let Some(root) = root {
            candidates.extend(descendants(&probe, root));
        }
        let mut frozen = BTreeSet::new();
        let mut killed = BTreeSet::new();
        let mut quiet = 0_u32;
        let deadline = Instant::now() + TEARDOWN_BUDGET;

        for _ in 0..MAX_ROUNDS {
            let system = scan(false);
            for candidate in candidates.clone() {
                if candidate.pid <= 1 || candidate == me || frozen.contains(&candidate) {
                    continue;
                }
                if !live(&system, candidate) {
                    continue;
                }
                match candidate.signal(Signal::SIGSTOP) {
                    Ok(()) => {
                        frozen.insert(candidate);
                    }
                    Err(Errno::ESRCH) => {}
                    Err(_) => return CleanupOutcome::unconfirmed(),
                }
            }

            let system = scan(false);
            for candidate in frozen.clone() {
                if !live(&system, candidate) {
                    // The original target exited; a reused PID is a different
                    // process. Drop the candidate and never signal it again:
                    // SIGCONT would target an unverified owner, and the stopped
                    // original no longer exists.
                    frozen.remove(&candidate);
                    continue;
                }
                match candidate.signal(Signal::SIGKILL) {
                    Ok(()) => {
                        killed.insert(candidate);
                    }
                    Err(Errno::ESRCH) => {}
                    Err(_) => return CleanupOutcome::unconfirmed(),
                }
            }

            thread::sleep(ROUND_INTERVAL);
            let system = scan(true);
            let mut remaining = marked(&system, markers, me);
            if let Some(root) = root {
                remaining.extend(descendants(&system, root));
            }
            // A tracked process that cleared its inherited environment is no
            // longer discoverable by marker, so quiescence must also require
            // every observed candidate to be gone.
            remaining.extend(
                seen.iter()
                    .copied()
                    .filter(|candidate| live(&system, *candidate)),
            );
            remaining.retain(|candidate| live(&system, *candidate) && *candidate != me);
            if remaining.is_empty() {
                quiet += 1;
                if quiet >= QUIET_ROUNDS {
                    return CleanupOutcome::confirmed(
                        u32::try_from(killed.len()).unwrap_or(u32::MAX),
                    );
                }
            } else {
                quiet = 0;
                candidates.extend(remaining);
            }
            if Instant::now() >= deadline {
                break;
            }
        }
        CleanupOutcome::unconfirmed()
    }

    #[cfg(test)]
    mod tests;
}

#[cfg(unix)]
pub(crate) use unix::SessionCleanup;
#[cfg(unix)]
pub(crate) use unix::reap_session;

#[cfg(not(unix))]
mod other {
    use qn_core::runtime::CleanupOutcome;

    #[derive(Debug)]
    pub(crate) struct SessionCleanup;

    impl SessionCleanup {
        pub(crate) fn start(_root_pid: u32, _markers: Vec<String>) -> Self {
            Self
        }

        pub(crate) fn finish(self) -> CleanupOutcome {
            CleanupOutcome::unconfirmed()
        }
    }

    pub(crate) fn reap_session(_markers: &[String]) -> CleanupOutcome {
        CleanupOutcome::unconfirmed()
    }
}

#[cfg(not(unix))]
pub(crate) use other::{SessionCleanup, reap_session};
