#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use std::{error::Error, time::Duration};
const PROFILE: &str = include_str!("../tests/fixtures/nono-0.80.0/profile-valid.json");
const SESSION: &str = include_str!("../tests/fixtures/nono-0.80.0/inspect-running.json");
const AUDIT: &str = include_str!("../tests/fixtures/nono-0.80.0/audit-show.json");
const DENIED: &str = include_str!("../tests/fixtures/nono-0.80.0/diagnostics-denied.txt");
const EMPTY: &str = include_str!("../tests/fixtures/nono-0.80.0/diagnostics-empty.txt");
fn output(text: &str) -> CmdOutcome {
    CmdOutcome {
        exit_code: Some(0),
        success: true,
        stdout: text.into(),
        stderr: text.into(),
        stdout_raw: text.as_bytes().to_vec(),
        stderr_raw: text.as_bytes().to_vec(),
        stdout_truncated: false,
        stderr_truncated: false,
        duration: Duration::ZERO,
    }
}
#[test]
fn it_reads_all_measured_contracts() {
    assert!(profile(&output(PROFILE)).unwrap().valid);
    let bad = include_str!("../tests/fixtures/nono-0.80.0/profile-invalid.json");
    assert!(!profile(&output(bad)).unwrap().valid);
    let ps = include_str!("../tests/fixtures/nono-0.80.0/ps-running.json");
    assert_eq!(sessions(&output(ps)).unwrap()[0].status, Status::Running);
    assert_eq!(inspect(&output(SESSION)).unwrap().exit_code, None);
    let list = include_str!("../tests/fixtures/nono-0.80.0/audit-list.json");
    assert!(!audit_list(&output(list)).unwrap()[0].is_alive);
    let events = audit(&output(AUDIT)).unwrap().network_events;
    assert_eq!(events[0].decision, "deny");
    let session = diagnostics(&output(DENIED)).unwrap().session;
    assert_eq!(session.denials.len(), 1);
}
#[test]
fn it_accepts_unknown_fields_and_keeps_nullable_keys_required() {
    let mut value: Value = serde_json::from_str(SESSION).unwrap();
    value["future"] = serde_json::json!({"nested": true});
    assert!(inspect(&output(&value.to_string())).is_ok());
    for key in ["session_id", "name", "status", "exit_code"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(
            inspect(&output(&missing.to_string())).is_err(),
            "missing {key}"
        );
    }
    value["status"] = Value::String("paused".into());
    let paused = inspect(&output(&value.to_string())).unwrap();
    assert_eq!(paused.status, Status::Paused);
    value["status"] = Value::String("exited".into());
    value["exit_code"] = Value::from(137);
    assert_eq!(
        inspect(&output(&value.to_string())).unwrap().exit_code,
        Some(137)
    );
}
#[test]
fn it_rejects_missing_or_wrongly_typed_required_fields() {
    for (text, keys, op) in [
        (
            PROFILE,
            &["file", "valid", "errors", "warnings", "deprecated_keys"][..],
            "profile",
        ),
        (
            AUDIT,
            &[
                "session_id",
                "network_events",
                "audit_event_count",
                "exit_code",
            ][..],
            "audit",
        ),
    ] {
        for key in keys {
            for remove in [true, false] {
                let mut value: Value = serde_json::from_str(text).unwrap();
                if remove {
                    value.as_object_mut().unwrap().remove(*key);
                } else {
                    value[*key] = serde_json::json!({"bad": true});
                }
                let out = output(&value.to_string());
                assert!(if op == "profile" {
                    profile(&out).is_err()
                } else {
                    audit(&out).is_err()
                });
            }
        }
    }
    assert!(sessions(&output("{}")).is_err());
    assert!(audit_list(&output(r#"[{"session_id":"id"}]"#)).is_err());
}
#[test]
fn it_rejects_broken_json_duplicate_keys_and_truncated_streams() {
    for text in [
        "",
        "{",
        "[] trailing",
        r#"{"session_id":"one","session_id":"two"}"#,
    ] {
        assert!(inspect(&output(text)).is_err());
    }
    for (stdout, stderr) in [(true, false), (false, true)] {
        let mut out = output(PROFILE);
        out.stdout_truncated = stdout;
        out.stderr_truncated = stderr;
        assert!(profile(&out).is_err());
        out.stderr = DENIED.into();
        assert!(diagnostics(&out).is_err());
    }
    assert!(diagnostics(&output(&" ".repeat((1 << 20) + 1))).is_err());
}
#[test]
fn it_handles_mixed_text_without_inventing_denials() {
    let text =
        format!("[nono] warning\nlog {{ braces }}\n{{not json}}\n{{\"unrelated\":true}}\n{EMPTY}");
    let parsed = diagnostics(&output(&text)).unwrap();
    assert_eq!(parsed.session.exit_code, 1);
    assert!(parsed.session.denials.is_empty());
    assert!(!parsed.session.diagnostics.is_empty());
    assert!(!format!("{parsed:?}").contains("session"));
}
#[test]
fn it_rejects_ambiguous_missing_nested_or_incomplete_diagnostics() {
    for text in [
        format!("{DENIED}\n{EMPTY}"),
        format!("[\n{EMPTY}"),
        format!("[0,\n{EMPTY}"),
        format!("[true,\n{EMPTY}"),
        "no report".into(),
        format!("{EMPTY}\n{{\"session\":"),
        "{\"session\":null}".into(),
        "{\"session\":{\"exit_code\":1}}".into(),
        "{\"outer\":{\"session\":{}}}".into(),
        "[]\n".repeat(65),
    ] {
        assert!(diagnostics(&output(&text)).is_err(), "reject {text:?}");
    }
}
#[test]
fn it_redacts_data_errors_and_every_exposed_source() {
    let sensitive = "synthetic-secret-marker";
    let text = SESSION.replace("running", sensitive);
    let err = inspect(&output(&text)).unwrap_err();
    let mut current: &dyn Error = &err;
    loop {
        assert!(!format!("{current} {current:?}").contains(sensitive));
        match current.source() {
            Some(source) => current = source,
            None => break,
        }
    }
    let mut value: Value = serde_json::from_str(SESSION).unwrap();
    value["session_id"] = Value::String(sensitive.into());
    assert!(!format!("{:?}", inspect(&output(&value.to_string())).unwrap()).contains(sensitive));
}
#[test]
fn it_rejects_nested_schema_drift_and_preserves_string_braces() {
    let parsed = DENIED[DENIED.find("{\n  \"session\"").unwrap()..].to_owned();
    let value = serde_json::Deserializer::from_str(&parsed)
        .into_iter::<Value>()
        .next()
        .unwrap()
        .unwrap();
    for key in ["denials", "violations", "diagnostics"] {
        let mut bad = value.clone();
        bad["session"][key] = serde_json::json!([{}]);
        assert!(diagnostics(&output(&bad.to_string())).is_err());
    }
    let mut braces = value;
    braces["session"]["denials"][0]["path"] = Value::String("/fixture/{quoted}\"file".into());
    assert_eq!(
        diagnostics(&output(&braces.to_string()))
            .unwrap()
            .session
            .denials
            .len(),
        1
    );
}
