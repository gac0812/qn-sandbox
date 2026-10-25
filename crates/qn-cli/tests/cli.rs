//! End-to-end tests for the `qn` executable.

#![allow(clippy::unwrap_used, clippy::expect_used)] // spec 02 §7: tests may panic with context

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

fn qn() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_qn"));
    command.env_remove("QN_DEFAULT_TIMEOUT_MS");
    command
}

fn doctor(home: &TestDir) -> Output {
    let mut command = qn();
    command
        .args(["doctor", "--config-only"])
        .env("QN_HOME", home.path());
    command.output().expect("qn doctor should execute")
}

fn assert_config_error_hides(home: &TestDir, reason: &str, hidden: &[&str]) {
    let output = doctor(home);
    assert_eq!(
        output.status.code(),
        Some(1),
        "doctor should fail through its normal error path: {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse config"), "{stderr}");
    assert!(
        stderr.contains(&home.config_file().display().to_string()),
        "error should identify the config file: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "doctor must not panic: {stderr}"
    );
    for token in hidden {
        assert!(
            !stderr.contains(token),
            "error must not echo `{token}`: {stderr}"
        );
    }
    assert!(
        stderr.contains(reason),
        "error should explain why: {stderr}"
    );
}

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(label: &str) -> Self {
        Self::from_component(label, "")
    }

    fn from_component(label: &str, component: &str) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "qn-cli-{label}-{}-{sequence}-{component}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("test directory should be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn config_file(&self) -> PathBuf {
        self.path.join("config.toml")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn it_reports_the_package_version() {
    let output = qn()
        .arg("--version")
        .output()
        .expect("qn --version should execute");

    assert!(
        output.status.success(),
        "qn --version should succeed: {output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        concat!("qn ", env!("CARGO_PKG_VERSION")),
        "version output should identify qn and the package version"
    );
}

#[test]
fn it_lists_doctor_in_help() {
    let output = qn()
        .arg("--help")
        .output()
        .expect("qn --help should execute");

    assert!(
        output.status.success(),
        "qn --help should succeed: {output:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("doctor"),
        "help should advertise the doctor command"
    );
}

#[test]
fn it_doctors_an_empty_qn_home_with_defaults() {
    let home = TestDir::new("defaults");
    let output = doctor(&home);

    assert!(
        output.status.success(),
        "doctor should accept a missing config file: {output:?}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("home: {}", home.path().display())),
        "doctor should report the resolved qn home: {stdout}"
    );
    assert!(
        stdout.contains("config source: defaults (file not found)"),
        "doctor should distinguish built-in defaults from a loaded file: {stdout}"
    );
    assert!(
        stdout.contains("command timeout: 120000 ms"),
        "doctor should report the built-in timeout: {stdout}"
    );
    assert!(
        stdout.contains("sandbox engine: not checked (configuration-only mode)"),
        "doctor must not imply that the sandbox engine is ready: {stdout}"
    );
}

#[test]
fn it_applies_environment_overrides_in_doctor() {
    let home = TestDir::new("override");
    std::fs::write(home.config_file(), "default_timeout_ms = 5000\n")
        .expect("config file should be written");

    let output = qn()
        .args(["doctor", "--config-only"])
        .env("QN_HOME", home.path())
        .env("QN_DEFAULT_TIMEOUT_MS", "7000")
        .output()
        .expect("qn doctor should execute");

    assert!(
        output.status.success(),
        "doctor should accept a valid config: {output:?}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("config source: loaded"),
        "doctor should report that the config file was loaded: {stdout}"
    );
    assert!(
        stdout.contains("command timeout: 7000 ms"),
        "environment must override the config file: {stdout}"
    );
}

#[test]
fn it_loads_the_config_file_without_environment_override() {
    let home = TestDir::new("file");
    std::fs::write(home.config_file(), "default_timeout_ms = 5000\n")
        .expect("config file should be written");

    let output = doctor(&home);

    assert!(
        output.status.success(),
        "doctor should accept a valid config: {output:?}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("config source: loaded"),
        "doctor should report that the config file was loaded: {stdout}"
    );
    assert!(
        stdout.contains("command timeout: 5000 ms"),
        "doctor should apply config-file values without an env override: {stdout}"
    );
}

#[test]
fn it_hides_unknown_config_keys_from_doctor_errors() {
    let home = TestDir::new("unknown-secret");
    let secret = "sentinel-cli-secret-9f3a";
    std::fs::write(home.config_file(), format!("api_key = \"{secret}\"\n"))
        .expect("config file should be written");

    assert_config_error_hides(&home, "unknown config key", &["api_key", secret]);
}

#[test]
fn it_hides_config_values_from_type_errors() {
    let home = TestDir::new("type-error");
    let secret = "sentinel-type-error-secret-4c2e";
    std::fs::write(
        home.config_file(),
        format!("default_timeout_ms = \"{secret}\"\n"),
    )
    .expect("config file should be written");

    assert_config_error_hides(&home, "invalid config value", &[secret]);
}

#[cfg(unix)]
#[test]
fn it_rejects_non_utf8_qn_home_without_echoing_invalid_bytes() {
    let output = qn()
        .args(["doctor", "--config-only"])
        .env(
            "QN_HOME",
            OsString::from_vec(vec![b'/', b't', b'm', b'p', 0xff]),
        )
        .output()
        .expect("qn doctor should execute");

    assert_safe_non_unicode_environment_error(output, "QN_HOME");
}

#[cfg(unix)]
#[test]
fn it_rejects_non_utf8_timeout_override_without_echoing_invalid_bytes() {
    let home = TestDir::new("non-utf8-timeout");
    let output = qn()
        .args(["doctor", "--config-only"])
        .env("QN_HOME", home.path())
        .env(
            "QN_DEFAULT_TIMEOUT_MS",
            OsString::from_vec(vec![b'7', 0xff, b'0']),
        )
        .output()
        .expect("qn doctor should execute");

    assert_safe_non_unicode_environment_error(output, "QN_DEFAULT_TIMEOUT_MS");
}

#[cfg(unix)]
fn assert_safe_non_unicode_environment_error(output: Output, variable: &str) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "non-UTF-8 environment input must fail closed: {output:?}"
    );
    let stderr = String::from_utf8(output.stderr)
        .expect("configuration errors must be valid UTF-8, not lossy replacements");
    assert!(
        stderr.contains(variable),
        "error must identify the rejected environment variable: {stderr}"
    );
    assert!(
        stderr.contains("not valid UTF-8"),
        "error must explain the encoding problem: {stderr}"
    );
    assert!(
        stderr.contains("set it to a valid UTF-8 value"),
        "error must include actionable remediation: {stderr}"
    );
    assert!(
        !stderr.contains('\u{fffd}'),
        "error must not echo invalid bytes via lossy conversion: {stderr}"
    );
}

#[cfg(unix)]
const UNSAFE_PATH_COMPONENT: &str = "home\nforged: pwned\u{1b}[31m\u{202e}gnp.exe";

#[cfg(unix)]
const ESCAPED_PATH_COMPONENT: &str = "home\\nforged: pwned\\u{1b}[31m\\u{202e}gnp.exe";

#[cfg(unix)]
#[test]
fn it_escapes_control_characters_in_successful_doctor_output() {
    let home = TestDir::from_component("control-success", UNSAFE_PATH_COMPONENT);
    let output = doctor(&home);

    assert!(
        output.status.success(),
        "doctor should accept a valid path containing controls: {output:?}"
    );
    let stdout =
        String::from_utf8(output.stdout).expect("doctor output containing controls must be UTF-8");
    assert!(
        stdout.contains(ESCAPED_PATH_COMPONENT),
        "path controls must be rendered visibly: {stdout}"
    );
    assert!(
        !stdout.contains('\u{1b}'),
        "raw terminal escape characters must never reach stdout: {stdout}"
    );
    assert!(
        !stdout.contains('\u{202e}'),
        "raw bidi controls must never reach stdout: {stdout}"
    );
    assert!(
        !stdout.contains("\nforged: pwned"),
        "path controls must not forge a new doctor field: {stdout}"
    );
}

#[cfg(unix)]
#[test]
fn it_escapes_control_characters_in_doctor_error_output() {
    let home = TestDir::from_component("control-error", UNSAFE_PATH_COMPONENT);
    std::fs::write(home.config_file(), "default_timeout_ms = [broken\n")
        .expect("config file should be written");

    let output = doctor(&home);

    assert_eq!(
        output.status.code(),
        Some(1),
        "malformed config should use the normal error path: {output:?}"
    );
    let stderr =
        String::from_utf8(output.stderr).expect("doctor errors containing controls must be UTF-8");
    assert!(
        stderr.contains(ESCAPED_PATH_COMPONENT),
        "path controls must be rendered visibly in errors: {stderr}"
    );
    assert!(
        !stderr.contains('\u{1b}'),
        "raw terminal escape characters must never reach stderr: {stderr}"
    );
    assert!(
        !stderr.contains('\u{202e}'),
        "raw bidi controls must never reach stderr: {stderr}"
    );
    assert!(
        !stderr.contains("\nforged: pwned"),
        "path controls must not forge a new error line: {stderr}"
    );
}
