// Tests may panic under coding standard 02 §7; production validation is fallible.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::fmt::Write as _;

use super::*;
use serde_json::{Value, json};
use std::{error::Error, fs};

fn setup() -> (tempfile::TempDir, ManagedConfigBinding, Value) {
    let root = tempfile::tempdir().expect("private test root");
    let session = SessionId::new();
    let grant = GrantHandle::new();
    let port = NonZeroU16::new(8080).expect("nonzero test port");
    let binding = ManagedConfigBinding::new(session, grant, port);
    let json = json!({"version":1,"session":session.to_string(),
        "grant":grant.to_string(),"proxy_port":port.get()});
    (root, binding, json)
}

fn path(root: &tempfile::TempDir) -> &Utf8Path {
    Utf8Path::from_path(root.path()).expect("UTF-8 temporary root")
}

fn entries(root: &tempfile::TempDir) -> usize {
    fs::read_dir(root.path())
        .expect("inspect test root")
        .count()
}

#[test]
fn it_publishes_only_canonical_routing_fields_and_replaces_old_config() {
    let (root, binding, json) = setup();
    let target = path(&root).join(FILE_NAME);
    let input = serde_json::to_vec_pretty(&json).expect("serialize fixture");
    write_managed_config(path(&root), &binding, &input).expect("publish routing");
    let bytes = fs::read(&target).expect("read published config");
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json,
        "preserve routing"
    );
    assert!(
        !bytes.contains(&b'\n'),
        "canonical output removes input whitespace"
    );
    assert_eq!(
        entries(&root),
        1,
        "no leftover temporary files after publication"
    );
    fs::write(&target, "old manifest").expect("seed previous config");
    write_managed_config(path(&root), &binding, &input).expect("replace old manifest");
    assert_eq!(
        fs::read(&target).unwrap(),
        bytes,
        "replacement is deterministic"
    );
    assert_eq!(
        entries(&root),
        1,
        "replacement does not retain temporary files"
    );
}

#[test]
fn it_rejects_unsafe_json_before_creating_or_modifying_files() {
    let (root, binding, json) = setup();
    let mut attacks = Vec::new();
    for field in [
        "api_key",
        "env",
        "headers",
        "url",
        "path",
        "SYNTHETIC_SECRET",
    ] {
        let mut value = json.clone();
        value[field] = json!("SYNTHETIC_SECRET");
        attacks.push(serde_json::to_vec(&value).unwrap());
    }
    for (field, value) in [
        ("version", json!(2)),
        ("session", json!(SessionId::new().to_string())),
        ("grant", json!(GrantHandle::new().to_string())),
        ("proxy_port", json!(0)),
        ("proxy_port", json!(8081)),
        ("proxy_port", json!(65536)),
        ("proxy_port", json!("SYNTHETIC_SECRET")),
        ("grant", json!("SYNTHETIC_SECRET")),
    ] {
        let mut changed = json.clone();
        changed[field] = value;
        attacks.push(serde_json::to_vec(&changed).unwrap());
    }
    for field in ["version", "session", "grant", "proxy_port"] {
        let mut missing = json.clone();
        missing.as_object_mut().unwrap().remove(field);
        attacks.push(serde_json::to_vec(&missing).unwrap());
        let mut duplicate = serde_json::to_string(&json).unwrap();
        duplicate.pop();
        write!(duplicate, ",{field:?}:{} }}", json[field]).unwrap();
        attacks.push(duplicate.into_bytes());
    }
    attacks.extend([
        b"{SYNTHETIC_SECRET".to_vec(),
        b"null".to_vec(),
        vec![0xff],
        format!("{json} trailing").into_bytes(),
        vec![b' '; MAX_BYTES + 1],
    ]);
    let target = path(&root).join(FILE_NAME);
    for input in attacks {
        let failure = write_managed_config(path(&root), &binding, &input)
            .expect_err("unsafe input must be refused");
        assert_eq!(
            entries(&root),
            0,
            "rejection must not create even a temporary file"
        );
        let mut error: &dyn Error = &failure;
        loop {
            assert!(
                !format!("{error:?} {error}").contains("SYNTHETIC_SECRET"),
                "redact full error chain"
            );
            let Some(source) = error.source() else {
                break;
            };
            error = source;
        }
        fs::write(&target, "known-good").unwrap();
        assert!(
            write_managed_config(path(&root), &binding, &input).is_err(),
            "refuse unsafe replacement"
        );
        assert_eq!(
            fs::read(&target).unwrap(),
            b"known-good",
            "refusal preserves previous config"
        );
        assert_eq!(entries(&root), 1, "refusal must leave only previous config");
        fs::remove_file(&target).unwrap();
    }
}

#[test]
fn it_validates_before_filesystem_access_and_rejects_relative_roots() {
    let (root, binding, json) = setup();
    let absent = path(&root).join("absent");
    assert!(
        matches!(
            write_managed_config(&absent, &binding, b"{}"),
            Err(ManagedConfigError::Json(_))
        ),
        "schema refusal precedes IO"
    );
    let input = serde_json::to_vec(&json).unwrap();
    assert!(
        matches!(
            write_managed_config(Utf8Path::new("relative"), &binding, &input),
            Err(ManagedConfigError::InvalidRoot)
        ),
        "never resolve ambient paths"
    );
    let failure = write_managed_config(&absent, &binding, &input).expect_err("absent root");
    assert!(
        matches!(failure, ManagedConfigError::Io(_)),
        "retain IO category"
    );
    assert!(failure.source().is_some(), "preserve underlying OS failure");
    assert_eq!(entries(&root), 0, "do not silently create a directory");
}

#[test]
fn it_cleans_failed_publication_without_damaging_an_existing_target() {
    let (root, binding, json) = setup();
    let target = path(&root).join(FILE_NAME);
    fs::create_dir(&target).unwrap();
    fs::write(target.join("sentinel"), b"untouched").unwrap();
    assert!(
        matches!(
            write_managed_config(path(&root), &binding, &serde_json::to_vec(&json).unwrap()),
            Err(ManagedConfigError::Io(_))
        ),
        "cannot replace a directory"
    );
    assert_eq!(
        fs::read(target.join("sentinel")).unwrap(),
        b"untouched",
        "preserve target on persist failure"
    );
    assert_eq!(
        entries(&root),
        1,
        "failed persist cleans unpublished temporary file"
    );
}

#[test]
fn it_redacts_trusted_binding_debug() {
    let (_, binding, _) = setup();
    let debug = format!("{binding:?}");
    assert!(
        !debug.contains(&binding.session.to_string()),
        "session omitted"
    );
    assert!(!debug.contains(&binding.grant.to_string()), "grant omitted");
    assert!(!debug.contains("8080"), "listener omitted");
}
