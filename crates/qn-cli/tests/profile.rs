//! Executable profile catalog and registry behavior; no engine or credential access.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(args)
        .env("QN_HOME", root)
        .env_remove("QN_DEFAULT_TIMEOUT_MS")
        .output()
        .unwrap()
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be versioned JSON")
}

#[test]
fn it_lists_only_compiled_profiles_before_install() {
    let root = tempfile::tempdir().unwrap();
    let output = run(root.path(), &["profile", "list", "--json"]);
    assert!(output.status.success(), "{output:?}");
    let value = json(&output);
    assert_eq!(value["schema_version"], 1);
    let profiles = value["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["id"], "qn/claude-code");
    assert_eq!(profiles[0]["source"], "builtin");
    assert_eq!(profiles[1]["id"], "qn/codex");
}

#[test]
fn it_shows_the_builtin_asset_and_switches_to_installed_after_install() {
    let root = tempfile::tempdir().unwrap();
    let builtin = run(
        root.path(),
        &["profile", "show", "qn/claude-code", "--json"],
    );
    assert!(builtin.status.success(), "{builtin:?}");
    let builtin = json(&builtin);
    assert_eq!(builtin["schema_version"], 1);
    assert_eq!(builtin["profile"]["id"], "qn/claude-code");
    assert_eq!(builtin["profile"]["source"], "builtin");
    assert_eq!(builtin["profile"]["state_dir"], ".claude");
    assert!(
        builtin["profile"]["network_hosts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|host| host == "api.anthropic.com")
    );
    let asset = builtin["profile"]["nono_profile"].as_str().unwrap();
    assert!(asset.contains("\"extends\": \"default\""));
    assert!(!asset.contains("claude_code_macos"));
    assert!(!asset.contains("Library/Keychains"));

    let installed = run(root.path(), &["profile", "install", "qn/claude-code"]);
    assert!(installed.status.success(), "{installed:?}");
    assert!(String::from_utf8_lossy(&installed.stdout).contains("installed qn/claude-code@1.0.0"));
    let shown = run(
        root.path(),
        &["profile", "show", "qn/claude-code", "--json"],
    );
    let shown = json(&shown);
    assert_eq!(shown["profile"]["source"], "installed");
    assert_eq!(shown["profile"]["nono_profile"], asset);

    // Installing identical bytes again is idempotent.
    let again = run(root.path(), &["profile", "install", "qn/claude-code"]);
    assert!(again.status.success(), "{again:?}");
}

#[test]
fn it_reports_unknown_and_invalid_identities_without_echoing_input() {
    let root = tempfile::tempdir().unwrap();
    let unknown = run(root.path(), &["profile", "show", "qn/unknown"]);
    assert_eq!(unknown.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(stderr.contains("unknown profile"), "{stderr}");
    assert!(!stderr.contains("panicked"));

    let invalid = run(root.path(), &["profile", "install", "qn/UPPER"]);
    assert_eq!(invalid.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(stderr.contains("invalid profile reference"), "{stderr}");
    assert!(!stderr.contains("UPPER"), "{stderr}");
}

#[test]
fn it_refuses_rollback_without_an_earlier_verified_version() {
    let root = tempfile::tempdir().unwrap();
    let installed = run(root.path(), &["profile", "install", "qn/codex"]);
    assert!(installed.status.success(), "{installed:?}");
    let rollback = run(root.path(), &["profile", "rollback", "qn/codex"]);
    assert_eq!(rollback.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&rollback.stderr);
    assert!(stderr.contains("no earlier profile version"), "{stderr}");

    let missing = run(root.path(), &["profile", "rollback", "qn/claude-code"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("not installed"));
}

#[test]
fn it_refuses_validation_without_a_verified_engine() {
    let root = tempfile::tempdir().unwrap();
    let output = run(root.path(), &["profile", "validate", "qn/claude-code"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("qn install"), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "failures must not emit partial JSON"
    );
    assert!(
        !root.path().join("cache").exists(),
        "inspection must not create an engine cache"
    );
}

#[test]
fn it_rejects_an_unknown_profile_before_engine_validation() {
    let root = tempfile::tempdir().unwrap();
    let output = run(root.path(), &["profile", "validate", "qn/unknown"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown profile"), "{stderr}");
    assert!(!stderr.contains("qn install"), "{stderr}");
}

#[test]
fn it_renders_human_readable_catalog_and_asset() {
    let root = tempfile::tempdir().unwrap();
    let list = run(root.path(), &["profile", "list"]);
    assert!(list.status.success());
    let text = String::from_utf8_lossy(&list.stdout);
    assert!(text.contains("qn/claude-code@1.0.0 (builtin)"), "{text}");
    let show = run(root.path(), &["profile", "show", "qn/codex"]);
    assert!(show.status.success());
    let text = String::from_utf8_lossy(&show.stdout);
    assert!(text.contains("state: .codex"), "{text}");
    assert!(text.contains("network allowlist: api.openai.com"), "{text}");
    assert!(text.contains("nono profile:"), "{text}");
}

#[test]
fn it_advertises_all_profile_operations_in_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(["profile", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for operation in ["list", "show", "install", "rollback"] {
        assert!(text.contains(operation), "missing {operation}: {text}");
    }
}

#[test]
fn it_keeps_profile_help_stream_and_exit_contracts() {
    for operation in ["list", "show", "install", "rollback"] {
        let output = Command::new(env!("CARGO_BIN_EXE_qn"))
            .args(["profile", operation, "--help"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{operation}: {output:?}");
        assert!(output.stderr.is_empty(), "{operation}: {output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Usage:"), "{operation}: {text}");
    }
}

#[test]
fn it_sanitizes_untrusted_profile_parser_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(["profile", "show", "bad\nprofile\u{202e}"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("\nprofile"));
    assert!(!stderr.contains('\u{202e}'));
    assert!(stderr.contains("invalid profile reference"), "{stderr}");
}
