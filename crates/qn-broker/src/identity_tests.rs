#![allow(clippy::unwrap_used)]
use super::{CaptureIdentity, IdentityError, MAX_PAYLOAD};
use serde_json::{Value, json};
use std::error::Error;
const NATIVE: &str = "0123456789abcdef";
const MARKER: &str = "synthetic-sensitive-marker";
fn valid() -> Value {
    json!({"protocol":"nono.credential-provider.v1","session_id":NATIVE,
        "credential_name":"qnprobe","route_id":"qnprobe","request_host":"qnprobe",
        "request_path":"/probe?q=a%2Fb","request_method":"GET","cache_scope":"qnprobe",
        "config":{},"interaction":{}})
}
fn identity() -> CaptureIdentity {
    CaptureIdentity::new(NATIVE, "qnprobe").unwrap()
}
fn rejected(payload: &[u8]) {
    let error = identity().validate(payload).err().unwrap();
    let mut cause: Option<&dyn Error> = Some(&error);
    while let Some(error) = cause {
        assert!(
            !format!("{error:?} {error}").contains(MARKER),
            "errors must redact every source"
        );
        cause = error.source();
    }
}
#[test]
fn it_preserves_exact_scope_without_echoing_it() {
    let input = valid();
    let metadata = identity()
        .validate(&serde_json::to_vec(&input).unwrap())
        .unwrap();
    assert_eq!(metadata.method(), "GET");
    assert_eq!(metadata.path(), "/probe?q=a%2Fb");
    assert!(!format!("{metadata:?}").contains("/probe"));
    assert!(!format!("{:?}", identity()).contains(NATIVE));
}
#[test]
fn it_rejects_every_changed_identity_component() {
    for field in [
        "protocol",
        "session_id",
        "credential_name",
        "route_id",
        "request_host",
        "cache_scope",
    ] {
        let mut input = valid();
        input[field] = json!(MARKER);
        rejected(&serde_json::to_vec(&input).unwrap());
    }
    let input = serde_json::to_vec(&valid()).unwrap();
    assert!(
        CaptureIdentity::new("abcdef0123456789", "qnprobe")
            .unwrap()
            .validate(&input)
            .is_err()
    );
    assert!(
        CaptureIdentity::new(NATIVE, "another")
            .unwrap()
            .validate(&input)
            .is_err()
    );
}
#[test]
fn it_requires_every_field_and_rejects_extensions_and_duplicates() {
    rejected(br#"["nono.credential-provider.v1","0123456789abcdef","qnprobe","qnprobe","qnprobe","/probe","GET","qnprobe",{},{}]"#);
    for field in valid().as_object().unwrap().keys() {
        let mut input = valid();
        input.as_object_mut().unwrap().remove(field);
        rejected(&serde_json::to_vec(&input).unwrap());
        let duplicate = format!(
            "{{{}:{},{}",
            serde_json::to_string(field).unwrap(),
            valid()[field],
            &valid().to_string()[1..]
        );
        rejected(duplicate.as_bytes());
    }
    let mut input = valid();
    input[MARKER] = json!(MARKER);
    rejected(&serde_json::to_vec(&input).unwrap());
    for field in ["config", "interaction"] {
        for value in [
            Value::Null,
            json!([]),
            json!({MARKER:MARKER}),
            json!(MARKER),
        ] {
            let mut input = valid();
            input[field] = value;
            rejected(&serde_json::to_vec(&input).unwrap());
        }
    }
}
#[test]
fn it_rejects_bad_methods_paths_and_payload_bounds() {
    for method in ["", "get", "G ET", "*", "GET\n", "ABCDEFGHIJKLMNOPQ"] {
        let mut input = valid();
        input["request_method"] = json!(method);
        rejected(&serde_json::to_vec(&input).unwrap());
    }
    for path in [
        "",
        "https://host/path",
        "//host/path",
        "/*",
        "/a#b",
        "/a\nb",
        "/a\\b",
    ] {
        let mut input = valid();
        input["request_path"] = json!(path);
        rejected(&serde_json::to_vec(&input).unwrap());
    }
    let mut input = valid();
    input["request_path"] = json!(format!("/{}", "a".repeat(4096)));
    rejected(&serde_json::to_vec(&input).unwrap());
    for payload in [
        Vec::new(),
        vec![b' '; MAX_PAYLOAD + 1],
        vec![255],
        b"{} trailing".to_vec(),
    ] {
        rejected(&payload);
    }
}
#[test]
fn it_rejects_invalid_host_bindings() {
    for native in ["", "0123", "0123456789abcdeF", "0123456789abcdeg"] {
        assert!(matches!(
            CaptureIdentity::new(native, "qnprobe"),
            Err(IdentityError::Configuration)
        ));
    }
    for reference in ["", "UPPER", "a/b", "a:b", "*", "a b"] {
        assert!(CaptureIdentity::new(NATIVE, reference).is_err());
    }
    assert!(CaptureIdentity::new(NATIVE, &"a".repeat(65)).is_err());
}
