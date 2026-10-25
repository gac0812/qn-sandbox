//! Foreground session CLI contracts; failure paths need no engine.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    path::Path,
    process::{Command, Output, Stdio},
    time::Duration,
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(args)
        .env("QN_HOME", root)
        .env_remove("QN_DEFAULT_TIMEOUT_MS")
        .output()
        .unwrap()
}

fn run_in(root: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(args)
        .current_dir(cwd)
        .env("QN_HOME", root)
        .env_remove("QN_DEFAULT_TIMEOUT_MS")
        .output()
        .unwrap()
}

fn session() -> String {
    qn_core::SessionId::new().to_string()
}

#[test]
fn it_lists_run_status_and_stop_in_help() {
    let root = tempfile::tempdir().unwrap();
    let output = run(root.path(), &["--help"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for command in ["run", "status", "stop"] {
        assert!(text.contains(command), "help must list {command}: {text}");
    }
}

#[test]
fn it_rejects_unknown_profiles_and_missing_engines_before_creating_runtime_state() {
    let root = tempfile::tempdir().unwrap();
    let unknown = run(
        root.path(),
        &["run", "--profile", "qn/unknown", "--", "/bin/true"],
    );
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown profile"));

    let project = tempfile::tempdir().unwrap();
    let missing = run_in(
        root.path(),
        project.path(),
        &["run", "--profile", "qn/claude-code", "--", "/bin/true"],
    );
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("qn install"));
    assert!(
        !root.path().join("runtime").exists(),
        "engine inspection must fail before runtime session creation"
    );
}

#[test]
fn it_requires_session_records_for_status_and_stop() {
    let root = tempfile::tempdir().unwrap();
    let id = session();
    let status = run(root.path(), &["status", &id]);
    assert_eq!(status.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&status.stderr).contains("session is unknown"));

    let stop = run(root.path(), &["stop", &id]);
    assert_eq!(stop.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&stop.stderr).contains("session is unknown"));

    let invalid = run(root.path(), &["stop", &id, "--timeout", "0"]);
    assert_eq!(invalid.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid stop timeout"));
}

#[test]
fn it_requires_the_child_delimiter() {
    let root = tempfile::tempdir().unwrap();
    let output = run(root.path(), &["run", "--profile", "qn/claude-code"]);
    assert_eq!(output.status.code(), Some(2));
}

fn shared_temp(prefix: &str) -> tempfile::TempDir {
    // The official macOS profile grants system_read_macos (/private/var), which
    // overlaps a QN_HOME created under the default temp directory. Use a private
    // root outside that grant, as the other native fixtures do.
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in("/Users/Shared")
        .unwrap()
}

fn installed_home() -> Option<tempfile::TempDir> {
    let archive = std::env::var("QN_TEST_NONO_ARCHIVE").ok()?;
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => return None,
    };
    let version = qn_config::NONO_PINNED_VERSION;
    let root = shared_temp("qn-session-home-");
    let managed = root.path().join("cache/nono").join(target).join(version);
    std::fs::create_dir_all(&managed).unwrap();
    std::fs::copy(
        archive,
        managed.join(format!("nono-v{version}-{target}.tar.gz")),
    )
    .unwrap();
    let install = run(root.path(), &["install"]);
    assert!(install.status.success(), "{install:?}");
    Some(root)
}

fn wait_for_running_record(root: &Path) -> String {
    let runtime = root.join("runtime");
    for _ in 0..100 {
        if let Ok(entries) = std::fs::read_dir(&runtime) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if text.contains("\"phase\":\"running\"") {
                        return path
                            .file_stem()
                            .and_then(|value| value.to_str())
                            .unwrap()
                            .to_owned();
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "running session record not found under {}",
        runtime.display()
    );
}

#[test]
#[ignore = "requires QN_TEST_NONO_ARCHIVE pointing at the pinned arm64 macOS archive"]
fn it_runs_echo_under_the_official_profile_and_reports_unconfirmed_cleanup() {
    let Some(root) = installed_home() else {
        return;
    };
    let project = shared_temp("qn-session-project-");
    let output = run_in(
        root.path(),
        project.path(),
        &[
            "run",
            "--profile",
            "qn/claude-code",
            "--",
            "/bin/echo",
            "qn-session-ok",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("qn-session-ok"),
        "run failed\nstdout={stdout}\nstderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("cleanup: unconfirmed"), "{stdout}");
    let id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("session: "))
        .expect("session id printed");

    let status = run(root.path(), &["status", id]);
    assert!(status.status.success(), "{status:?}");
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("phase: exited_observed"), "{text}");
    assert!(text.contains("cleanup: unconfirmed"), "{text}");
    assert!(text.contains("native: exited"), "{text}");

    let stop = run(root.path(), &["stop", id]);
    assert_eq!(stop.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&stop.stderr).contains("already observed exited"));
}

#[test]
#[ignore = "requires QN_TEST_NONO_ARCHIVE pointing at the pinned arm64 macOS archive"]
fn it_stops_a_running_session_without_claiming_cleanup() {
    let Some(root) = installed_home() else {
        return;
    };
    let project = shared_temp("qn-session-project-");
    // Null streams: a descendant may survive the supervisor (#93) and would
    // otherwise keep an inherited pipe open past the stop request.
    let mut child = Command::new(env!("CARGO_BIN_EXE_qn"))
        .args([
            "run",
            "--profile",
            "qn/claude-code",
            "--",
            "/bin/sleep",
            "30",
        ])
        .current_dir(project.path())
        .env("QN_HOME", root.path())
        .env_remove("QN_DEFAULT_TIMEOUT_MS")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let id = wait_for_running_record(root.path());

    let started = std::time::Instant::now();
    let stop = run(root.path(), &["stop", &id, "--force"]);
    assert!(stop.status.success(), "stop failed: {stop:?}");
    let text = String::from_utf8_lossy(&stop.stdout);
    assert!(text.contains("stop requested: true"), "{text}");
    assert!(text.contains("cleanup: unconfirmed"), "{text}");

    let exit = child.wait().unwrap();
    assert!(exit.success(), "run process should exit cleanly: {exit:?}");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "stop must terminate the supervisor before the 30s command ends; elapsed {:?}",
        started.elapsed()
    );
    let status = run(root.path(), &["status", &id]);
    let text = String::from_utf8_lossy(&status.stdout);
    assert!(text.contains("phase: exited_observed"), "{text}");
    assert!(text.contains("cleanup: unconfirmed"), "{text}");
}
