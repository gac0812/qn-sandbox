//! Binary contracts for the qn audit command surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

fn qn(home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(args)
        .env("QN_HOME", home)
        .env_remove("QN_DEFAULT_TIMEOUT_MS")
        .output()
        .expect("qn should execute")
}

#[test]
fn it_advertises_all_audit_operations() {
    let home = tempfile::tempdir().unwrap();
    let output = qn(home.path(), &["audit", "--help"]);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for operation in ["show", "list", "follow", "native"] {
        assert!(text.contains(operation), "missing {operation}: {text}");
    }
}

#[test]
fn it_keeps_audit_subcommand_help_on_stdout() {
    let home = tempfile::tempdir().unwrap();
    for operation in ["show", "list", "follow", "native"] {
        let output = qn(home.path(), &["audit", operation, "--help"]);
        assert!(output.status.success(), "{operation}: {output:?}");
        assert!(output.stderr.is_empty(), "{operation}: {output:?}");
        assert!(String::from_utf8(output.stdout).unwrap().contains("Usage:"));
    }
}

#[test]
fn it_accepts_json_and_filter_arguments_before_querying() {
    let home = tempfile::tempdir().unwrap();
    let session = qn_core::SessionId::new().to_string();
    for args in [
        vec![
            "audit", "show", &session, "--json", "--type", "denial", "--since", "0",
        ],
        vec![
            "audit",
            "list",
            "--session",
            &session,
            "--json",
            "--type",
            "grant",
        ],
        vec![
            "audit",
            "follow",
            "--session",
            &session,
            "--json",
            "--since",
            "0",
        ],
        vec!["audit", "native", &session, "--json"],
    ] {
        let output = qn(home.path(), &args);
        assert_eq!(
            output.status.code(),
            Some(1),
            "query should fail at missing source, not parse: {output:?}"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(!stderr.contains("unexpected argument"), "{stderr}");
        assert!(!stderr.contains(home.path().to_str().unwrap()), "{stderr}");
    }
}
