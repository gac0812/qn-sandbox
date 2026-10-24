//! Command boundary tests; no nono process or host configuration is required.
#![allow(clippy::unwrap_used)]
use qn_nono::{
    NonoArgs, NonoError,
    args::{NonoSessionId, NonoStopMode},
};
use std::time::Duration;

const ID: &str = "0123456789abcdef";

#[test]
fn it_builds_local_json_queries_with_fixed_option_boundaries() {
    let id = NonoSessionId::try_from(ID).unwrap();
    assert_eq!(NonoArgs::sessions().as_slice(), ["ps", "--all", "--json"]);
    assert_eq!(
        NonoArgs::inspect(&id).as_slice(),
        ["inspect", "--json", "--", ID]
    );
    assert_eq!(
        NonoArgs::audit_list().as_slice(),
        ["audit", "list", "--json"]
    );
    assert_eq!(
        NonoArgs::audit_show(&id).as_slice(),
        ["audit", "show", "--json", "--", ID]
    );
}

#[test]
fn it_rejects_untrusted_selectors_without_echoing_them() {
    for raw in [
        "",
        "0123",
        "0123456789abcdef0",
        "0123456789abcdeg",
        "0123456789ABCDEf",
        "--force",
        "../../etc/passwd",
        "0123456789abcde\0",
        "0123456789abcde ",
        "0123456789abcde\n",
        "sxn-019a1234-5678-7abc-8123-000000000001",
        "$(touch marker)",
    ] {
        let error = NonoSessionId::try_from(raw).unwrap_err();
        assert!(matches!(error, NonoError::InvalidNonoSessionId));
        if !raw.is_empty() {
            assert!(
                !format!("{error} {error:?}").contains(raw),
                "must redact rejected ID"
            );
        }
    }
}

#[test]
fn it_keeps_force_and_graceful_stop_options_distinct() {
    let id = NonoSessionId::try_from(ID).unwrap();
    assert_eq!(
        NonoArgs::stop(&id, NonoStopMode::Force).unwrap().as_slice(),
        ["stop", "--force", "--", ID]
    );
    for secs in [1, 3, 300] {
        let mode = NonoStopMode::Graceful {
            timeout: Duration::from_secs(secs),
        };
        let args = NonoArgs::stop(&id, mode).unwrap();
        assert_eq!(
            args.as_slice(),
            ["stop", "--timeout", &secs.to_string(), "--", ID]
        );
    }
    for timeout in [
        Duration::ZERO,
        Duration::from_millis(1500),
        Duration::from_secs(301),
        Duration::MAX,
    ] {
        let result = NonoArgs::stop(&id, NonoStopMode::Graceful { timeout });
        assert!(matches!(result, Err(NonoError::InvalidStopTimeout)));
    }
}

#[test]
fn it_hides_native_identifiers_from_debug() {
    let id = NonoSessionId::try_from(ID).unwrap();
    let args = NonoArgs::inspect(&id);
    assert!(!format!("{id:?} {args:?}").contains(ID));
}
