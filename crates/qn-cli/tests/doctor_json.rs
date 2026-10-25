//! Binary contracts for configuration-only JSON diagnostics.

#![allow(clippy::expect_used)] // spec 02 §7: test assertions

use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};

fn doctor(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_qn"));
    command.args(["doctor", "--json"]).env("QN_HOME", home);
    command.env_remove("QN_DEFAULT_TIMEOUT_MS");
    command
}

fn report(output: &Output) -> Value {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("stdout must be exactly one JSON document")
}

#[test]
fn it_reports_defaults_with_explicit_unchecked_engine() {
    let home = tempfile::tempdir().expect("temporary home");
    let output = doctor(home.path()).output().expect("qn should run");
    assert_eq!(
        report(&output),
        json!({
            "schema_version": 1,
            "home": home.path().to_str().expect("UTF-8 home"),
            "config_file": home.path().join("config.toml").to_str().expect("UTF-8 config"),
            "config_source": "defaults",
            "command_timeout_ms": 120_000,
            "sandbox_engine": "not_checked",
            "configuration": "ok"
        })
    );
}

#[test]
fn it_preserves_file_then_environment_precedence() {
    let home = tempfile::tempdir().expect("temporary home");
    std::fs::write(
        home.path().join("config.toml"),
        "default_timeout_ms = 5000\n",
    )
    .expect("write config");
    let file = doctor(home.path()).output().expect("qn should run");
    assert_eq!(report(&file)["config_source"], "file");
    assert_eq!(report(&file)["command_timeout_ms"], 5000);
    let env = doctor(home.path())
        .env("QN_DEFAULT_TIMEOUT_MS", "7000")
        .output()
        .expect("qn should run");
    assert_eq!(report(&env)["command_timeout_ms"], 7000);
}

#[test]
fn it_rejects_invalid_config_without_partial_json_or_secret_echo() {
    let home = tempfile::tempdir().expect("temporary home");
    let secret = "synthetic-json-secret";
    for config in [
        format!("api_key = \"{secret}\"\n"),
        format!("default_timeout_ms = \"{secret}\"\n"),
        format!("default_timeout_ms = \"{secret}\n"),
    ] {
        std::fs::write(home.path().join("config.toml"), config).expect("write config");
        let output = doctor(home.path()).output().expect("qn should run");
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "no partial successful report");
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
        assert!(stderr.contains("failed to parse config"), "{stderr}");
        assert!(!stderr.contains(secret) && !stderr.contains("api_key"));
    }
}

#[test]
fn it_rejects_invalid_environment_without_partial_json() {
    let home = tempfile::tempdir().expect("temporary home");
    let secret = "synthetic-invalid-timeout";
    let output = doctor(home.path())
        .env("QN_DEFAULT_TIMEOUT_MS", secret)
        .output()
        .expect("qn should run");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    assert!(stderr.contains("QN_DEFAULT_TIMEOUT_MS"));
    assert!(!stderr.contains(secret));
}

#[cfg(unix)]
#[test]
fn it_uses_terminal_safe_display_paths_even_after_json_decoding() {
    let parent = tempfile::tempdir().expect("temporary parent");
    let home = parent.path().join("home\nforged\u{1b}[2J\u{202e}");
    std::fs::create_dir(&home).expect("create unsafe display path");
    let output = doctor(&home).output().expect("qn should run");
    let value = report(&output);
    for field in ["home", "config_file"] {
        let path = value[field].as_str().expect("string display path");
        assert!(
            path.contains("home\\nforged\\u{1b}[2J\\u{202e}"),
            "{path:?}"
        );
        assert!(!path.chars().any(char::is_control));
        assert!(!path.contains('\u{202e}'));
    }
}

#[test]
fn it_accepts_config_only_for_text_and_json_diagnostics() {
    let home = tempfile::tempdir().expect("temporary home");
    for arguments in [
        vec!["doctor", "--config-only"],
        vec!["doctor", "--config-only", "--json"],
        vec!["doctor", "--json", "--config-only"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_qn"))
            .args(&arguments)
            .env("QN_HOME", home.path())
            .output()
            .expect("qn should run");
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        if arguments.contains(&"--json") {
            let report: Value = serde_json::from_slice(&output.stdout).expect("JSON report");
            assert_eq!(report["sandbox_engine"], "not_checked");
        } else {
            let stdout = String::from_utf8(output.stdout).expect("UTF-8 text");
            assert!(stdout.contains("sandbox engine: not checked"));
        }
    }
}

#[test]
fn it_shows_config_only_in_doctor_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(["doctor", "--help"])
        .output()
        .expect("qn should run");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("--config-only"));
    assert!(stdout.contains("--json"));
}

#[test]
fn it_preserves_the_full_unsigned_timeout_range() {
    let home = tempfile::tempdir().expect("temporary home");
    let output = doctor(home.path())
        .env("QN_DEFAULT_TIMEOUT_MS", u64::MAX.to_string())
        .output()
        .expect("qn should run");
    assert_eq!(
        report(&output)["command_timeout_ms"].as_u64(),
        Some(u64::MAX)
    );
}

#[test]
fn it_advertises_json_in_doctor_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(["doctor", "--help"])
        .output()
        .expect("qn should run");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("--json"));
}

#[cfg(unix)]
#[test]
fn it_rejects_non_unicode_environment_without_partial_json() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let home = tempfile::tempdir().expect("temporary home");
    for variable in ["QN_HOME", "QN_DEFAULT_TIMEOUT_MS"] {
        let output = doctor(home.path())
            .env(variable, OsString::from_vec(vec![0xff]))
            .output()
            .expect("qn should run");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
        assert!(stderr.contains(variable) && stderr.contains("not valid UTF-8"));
        assert!(!stderr.contains('\u{fffd}'));
    }
}
