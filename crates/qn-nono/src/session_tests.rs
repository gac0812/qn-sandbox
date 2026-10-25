#![allow(clippy::unwrap_used, clippy::expect_used)]
use super::*;
use serde_json::json;
const ID: &str = "0123456789abcdef";
fn record(session: SessionId, id: &str, state: &str, code: Option<i32>) -> wire::SessionRecord {
    serde_json::from_value(json!({"session_id":id,"name":session.to_string(),
        "status":state,"exit_code":code}))
    .unwrap()
}
#[test]
fn it_correlates_only_an_exact_unique_name() {
    let session = SessionId::new();
    let found = select(&[record(session, ID, "running", None)], session)
        .unwrap()
        .unwrap();
    assert_eq!(found.session(), session, "retain qn identity");
    assert_eq!(
        found.native_id(),
        &NonoSessionId::try_from(ID).unwrap(),
        "full native identity"
    );
    assert_eq!(
        found.state(),
        NativeSessionState::Running,
        "native running observation"
    );
    assert_eq!(found.exit_code(), None, "do not invent exit code");
    for name in [
        None,
        Some("sxn-".into()),
        Some(format!("{session}-extra")),
        Some(SessionId::new().to_string()),
    ] {
        let mut foreign = record(session, ID, "running", None);
        foreign.name = name;
        assert!(
            select(&[foreign], session).unwrap().is_none(),
            "never bind similar/foreign names"
        );
    }
    assert!(
        select(&[], session).unwrap().is_none(),
        "no match is not exit evidence"
    );
    let debug = format!("{found:?}");
    assert!(
        !debug.contains(ID) && !debug.contains(&session.to_string()),
        "redact identifiers"
    );
}
#[test]
fn it_refuses_duplicates_aliases_and_malformed_identity() {
    let session = SessionId::new();
    for other in [
        record(session, "fedcba9876543210", "running", None),
        record(SessionId::new(), ID, "exited", Some(0)),
    ] {
        assert!(
            select(&[record(session, ID, "running", None), other], session).is_err(),
            "reject ambiguous mapping"
        );
    }
    assert!(
        matches!(
            select(
                &[record(session, "SYNTHETIC_SECRET", "running", None)],
                session
            ),
            Err(NonoError::InvalidNonoSessionId)
        ),
        "reject malformed native ID"
    );
}
#[test]
fn it_rechecks_both_identifiers_and_preserves_native_states() {
    let session = SessionId::new();
    let expected = observation(&record(session, ID, "running", None), session).unwrap();
    for (state, code, want) in [
        ("running", None, NativeSessionState::Running),
        ("paused", None, NativeSessionState::Paused),
        ("exited", None, NativeSessionState::Exited),
        ("exited", Some(7), NativeSessionState::Exited),
    ] {
        let current = verify(&record(session, ID, state, code), &expected).unwrap();
        assert_eq!(
            (current.state(), current.exit_code()),
            (want, code),
            "preserve observations"
        );
    }
    for wrong in [
        record(SessionId::new(), ID, "running", None),
        record(session, "fedcba9876543210", "running", None),
        record(session, ID, "paused", Some(0)),
        record(session, ID, "running", Some(7)),
    ] {
        let error = verify(&wrong, &expected)
            .expect_err("refuse changed identity or inconsistent observation");
        assert!(
            !format!("{error:?} {error}").contains(ID),
            "redact rejected values"
        );
    }
    assert!(
        serde_json::from_value::<wire::SessionRecord>(
            json!({"session_id":ID,"name":session.to_string(),"status":"future","exit_code":null})
        )
        .is_err(),
        "unknown native state must not become running"
    );
}
