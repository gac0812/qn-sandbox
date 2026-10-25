#![allow(clippy::unwrap_used)]
use super::*;
use crate::launch::LaunchBinding;
use std::{error::Error, fs};

const ID: &str = "0123456789abcdef";

fn assert_redacted(error: &(dyn Error + 'static)) {
    let mut cause = Some(error);
    while let Some(error) = cause {
        assert!(!format!("{error} {error:?}").contains("canary"));
        cause = error.source();
    }
}

#[test]
fn it_keeps_exact_launch_association_after_owner_drop_and_refuses_replacement() {
    let temporary = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(temporary.path().canonicalize().unwrap()).unwrap();
    let id = SessionId::new();
    let binding = LaunchBinding::reserve(&root, id).unwrap();
    binding.publish_audit(&root, &root).unwrap();
    let original = fs::read(root.join(format!("{id}.native.json"))).unwrap();
    assert!(binding.publish_audit(&root, &root).is_err());
    let native = binding.native_id().as_str().to_owned();
    drop(binding);
    let loaded = association::load(&root, id).unwrap();
    assert_eq!(loaded.native.as_str(), native);
    assert_eq!(loaded.root, root);
    assert_eq!(
        fs::read(root.join(format!("{id}.native.json"))).unwrap(),
        original
    );
}

#[test]
fn it_rejects_malformed_foreign_and_oversized_associations_without_payloads() {
    let temporary = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(temporary.path().canonicalize().unwrap()).unwrap();
    let id = SessionId::new();
    let file = root.join(format!("{id}.native.json"));
    assert!(matches!(
        association::load(&root, id),
        Err(NativeAuditError::MissingAssociation)
    ));
    let valid = serde_json::json!({"session_id":id.to_string(),"native_id":ID,"runtime_root":root.as_str()});
    let mut inputs = vec![
        String::new(),
        "[]".into(),
        "canary".into(),
        " ".repeat(16 * 1024 + 1),
        format!("{{\"session_id\":\"canary\",{}", &valid.to_string()[1..]),
        serde_json::json!([id.to_string(), ID, root.as_str()]).to_string(),
    ];
    for (key, value) in [
        (
            "session_id",
            serde_json::json!(SessionId::new().to_string()),
        ),
        ("native_id", serde_json::json!("../canary")),
        ("runtime_root", serde_json::json!("relative-canary")),
        (
            "runtime_root",
            serde_json::json!(root.join("missing-canary").as_str()),
        ),
        ("unknown", serde_json::json!("canary")),
    ] {
        let mut value_copy = valid.clone();
        value_copy[key] = value;
        inputs.push(value_copy.to_string());
    }
    for input in inputs {
        fs::write(&file, input).unwrap();
        match association::load(&root, id) {
            Err(error) => assert_redacted(&error),
            Ok(_) => panic!("invalid association was accepted"),
        }
    }
    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();
    assert!(matches!(
        association::load(&root, id),
        Err(NativeAuditError::InvalidAssociation)
    ));
}

#[test]
fn it_projects_only_closed_native_fields_without_synthesizing_clock_or_cleanup() {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/nono-0.80.0/audit-show.json"
    ))
    .unwrap();
    let mut event = value["network_events"][0].clone();
    for field in ["target", "mode", "denial_category", "reason", "path"] {
        event[field] = serde_json::json!("credential-canary");
    }
    event["timestamp_unix_ms"] = serde_json::Value::Null;
    value["network_events"] = serde_json::json!([event.clone(), event.clone()]);
    value["network_events"][1]["decision"] = serde_json::json!("allow");
    value["network_events"][1]["timestamp_unix_ms"] = serde_json::json!(10);
    let report = project(
        SessionId::new(),
        serde_json::from_value(value.clone()).unwrap(),
    )
    .unwrap();
    assert_eq!(report.events.len(), 2);
    assert_eq!(report.events[0].wall_millis, None);
    assert_eq!(report.events[0].decision, NetworkDecision::Denied);
    assert_eq!(report.events[1].index, 2);
    assert_eq!(report.events[1].decision, NetworkDecision::Allowed);
    assert_eq!(report.events[1].wall_millis, Some(10));
    assert_eq!(report.reported_event_count, 3);
    assert_eq!(report.integrity, AuditIntegrity::Unverified);
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(!encoded.contains("canary") && !format!("{report:?}").contains("canary"));
    value["network_events"][0]["decision"] = serde_json::json!("unknown-canary");
    let error = project(SessionId::new(), serde_json::from_value(value).unwrap()).unwrap_err();
    assert_redacted(&error);
}

#[test]
fn it_redacts_storage_errors_through_the_full_error_chain() {
    let error = failure(NonoError::Io {
        step: "canary",
        source: std::io::Error::other("canary"),
    });
    assert_redacted(&error);
}

#[test]
fn it_preserves_approval_observations_without_claiming_request_authorization() {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/nono-0.80.0/audit-show.json"
    ))
    .unwrap();
    for (native, expected) in [
        ("approve_requested", NetworkDecision::ApprovalRequested),
        ("approve_granted", NetworkDecision::ApprovalGranted),
        ("approve_denied", NetworkDecision::ApprovalDenied),
        ("approve_timeout", NetworkDecision::ApprovalTimeout),
        ("approve_error", NetworkDecision::ApprovalError),
    ] {
        value["network_events"][0]["decision"] = serde_json::json!(native);
        let report = project(
            SessionId::new(),
            serde_json::from_value(value.clone()).unwrap(),
        )
        .unwrap();
        assert_eq!(report.events[0].decision, expected);
    }
}
