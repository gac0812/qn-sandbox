#![allow(clippy::cast_possible_wrap, clippy::expect_used, clippy::unwrap_used)]

use super::*;
use camino::Utf8PathBuf;
use qn_core::runtime::CleanupState;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

struct Session {
    _dir: TempDir,
    root: Utf8PathBuf,
    markers: Vec<String>,
}

impl Session {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("session tempdir");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 root");
        for name in ["home", "config", "state", "cache", "tmp"] {
            fs::create_dir(root.join(name)).expect("session subdir");
        }
        let markers = qn_nono::profile::cleanup_markers(&root);
        Self {
            _dir: dir,
            root,
            markers,
        }
    }

    fn python(&self, script: &str, pidfile: &str) -> Command {
        let home = self.root.join("home");
        let config = self.root.join("config");
        let state = self.root.join("state");
        let cache = self.root.join("cache");
        let tmp = self.root.join("tmp");
        let mut command = Command::new("/usr/bin/python3");
        command
            .arg("-c")
            .arg(script)
            .arg(pidfile)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home.as_str())
            .env("TMPDIR", tmp.as_str())
            .env("XDG_CONFIG_HOME", config.as_str())
            .env("XDG_STATE_HOME", state.as_str())
            .env("XDG_CACHE_HOME", cache.as_str())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
}

struct Killer(Option<u32>);

impl Killer {
    fn new(pid: u32) -> Self {
        Self(Some(pid))
    }

    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for Killer {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            let _ = kill(NixPid::from_raw(pid as i32), Signal::SIGKILL);
        }
    }
}

fn alive(pid: u32) -> bool {
    kill(NixPid::from_raw(pid as i32), None).is_ok()
}

fn wait_pid(path: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if let Ok(text) = fs::read_to_string(path) {
            if let Ok(pid) = text.trim().parse() {
                return pid;
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("witness pid file was not written: {}", path.display());
}

fn fork_and_detach() -> &'static str {
    r"
import os, signal, sys, time, pathlib
base = pathlib.Path(sys.argv[1])
for s in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
    signal.signal(s, signal.SIG_IGN)
pid = os.fork()
if pid > 0:
    (base.parent / (base.name + '.parent')).write_text(str(os.getpid()))
    os._exit(0)
os.setsid()
(base.parent / (base.name + '.child')).write_text(str(os.getpid()))
with (base.parent / (base.name + '.heartbeat')).open('w') as fh:
    for _ in range(300):
        fh.write('x')
        fh.flush()
        time.sleep(0.1)
"
}

fn fork_and_keep_parent() -> &'static str {
    r"
import os, signal, sys, time, pathlib
base = pathlib.Path(sys.argv[1])
pid = os.fork()
if pid == 0:
    os.setsid()
    for s in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(s, signal.SIG_IGN)
    (base.parent / (base.name + '.child')).write_text(str(os.getpid()))
    while True:
        time.sleep(0.1)
(base.parent / (base.name + '.parent')).write_text(str(os.getpid()))
time.sleep(30)
"
}

/// The survivor is visible as a descendant first, then its intermediate parent
/// exits and it replaces its environment via `execve(..., env={})`.
fn fork_detach_and_clear_environment() -> &'static str {
    r"
import os, signal, sys, time, pathlib
base = pathlib.Path(sys.argv[1])
pid = os.fork()
if pid == 0:
    os.setsid()
    inner = os.fork()
    if inner > 0:
        time.sleep(0.5)
        os._exit(0)
    time.sleep(0.6)
    (base.parent / (base.name + '.child')).write_text(str(os.getpid()))
    os.execve('/bin/sleep', ['sleep', '30'], {})
    os._exit(1)
(base.parent / (base.name + '.parent')).write_text(str(os.getpid()))
time.sleep(30)
"
}

#[test]
fn it_reaps_a_setsid_descendant_after_the_parent_exited() {
    let session = Session::new();
    let pidfile = session.root.join("run");
    let mut parent = session
        .python(fork_and_detach(), pidfile.as_str())
        .spawn()
        .expect("spawn detached witness");
    parent.wait().expect("reap the short-lived parent");
    let witness = wait_pid(Path::new(&format!("{pidfile}.child")));
    let mut guard = Killer::new(witness);
    assert!(alive(witness), "witness must be running before teardown");

    let outcome = reap_session(&session.markers);
    assert_eq!(
        outcome.state(),
        CleanupState::Confirmed,
        "reaper did not prove cleanup"
    );
    assert!(
        outcome.terminated() >= 1,
        "the detached witness should be terminated"
    );
    assert!(!alive(witness), "setsid witness survived cleanup");
    guard.disarm();
}

#[test]
fn it_tracks_a_setsid_child_while_the_parent_is_alive() {
    let session = Session::new();
    let pidfile = session.root.join("run");
    let mut parent = session
        .python(fork_and_keep_parent(), pidfile.as_str())
        .spawn()
        .expect("spawn tracked witness");
    let parent_pid = parent.id();
    let mut parent_guard = Killer::new(parent_pid);
    let witness = wait_pid(Path::new(&format!("{pidfile}.child")));
    let mut witness_guard = Killer::new(witness);

    let tracker = SessionCleanup::start(parent_pid, session.markers.clone());
    thread::sleep(Duration::from_millis(300));
    let outcome = tracker.finish();
    assert_eq!(
        outcome.state(),
        CleanupState::Confirmed,
        "tracked tree did not quiesce"
    );
    assert!(!alive(witness), "setsid child survived");
    witness_guard.disarm();
    let status = parent.wait().expect("reap the killed parent");
    assert!(!status.success(), "reaper must terminate the parent");
    assert!(!alive(parent_pid), "managed parent survived");
    parent_guard.disarm();
}

#[test]
fn it_reaps_a_tracked_child_that_reparented_and_cleared_its_environment() {
    let session = Session::new();
    let pidfile = session.root.join("run");
    let mut parent = session
        .python(fork_detach_and_clear_environment(), pidfile.as_str())
        .spawn()
        .expect("spawn env-clearing witness");
    let parent_pid = parent.id();
    let mut parent_guard = Killer::new(parent_pid);
    let tracker = SessionCleanup::start(parent_pid, session.markers.clone());
    let witness = wait_pid(Path::new(&format!("{pidfile}.child")));
    let mut witness_guard = Killer::new(witness);
    // The marker is gone after `execve(..., env={})`; only the tracker's
    // recorded identity still links this process to the session.
    assert!(alive(witness), "witness must be running before teardown");

    let outcome = tracker.finish();
    assert_eq!(
        outcome.state(),
        CleanupState::Confirmed,
        "env-clearing witness was not accounted for"
    );
    assert!(!alive(witness), "env-clearing witness survived cleanup");
    witness_guard.disarm();
    let _ = parent.kill();
    let _ = parent.wait();
    parent_guard.disarm();
}

#[test]
fn it_does_not_signal_an_unrelated_process() {
    let session = Session::new();
    let other = tempfile::tempdir().expect("unrelated tempdir");
    let mut unrelated = Command::new("/bin/sleep")
        .arg("30")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", other.path())
        .env("TMPDIR", other.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn unrelated process");
    let pid = unrelated.id();
    let mut guard = Killer::new(pid);

    let outcome = reap_session(&session.markers);
    assert_eq!(outcome.state(), CleanupState::Confirmed);
    assert_eq!(outcome.terminated(), 0);
    assert!(alive(pid), "unrelated process was signalled");
    guard.disarm();
    let _ = unrelated.kill();
    let _ = unrelated.wait();
}
