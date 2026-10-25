#![allow(clippy::unwrap_used)]
use super::*;
use std::{
    fs,
    sync::{Arc, Barrier},
};

#[test]
fn it_preserves_attempt_identity_after_drop_and_rejects_drift() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let id = SessionId::new();
    let binding = LaunchBinding::reserve(root, id).unwrap();
    let path = root.join(format!("{id}.launch"));
    let bytes = fs::read(&path).unwrap();
    let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(record["session_id"], id.to_string());
    assert_eq!(record["native_id"], binding.native_id().as_str());
    assert!(!format!("{binding:?}").contains(binding.native_id().as_str()));
    binding.verify(id, binding.native_id()).unwrap();
    let other = LaunchBinding::reserve(root, SessionId::new()).unwrap();
    assert!(binding.verify(id, other.native_id()).is_err());
    assert!(
        binding
            .verify(other.session(), binding.native_id())
            .is_err()
    );
    drop(binding);
    assert!(matches!(
        LaunchBinding::reserve(root, id),
        Err(NonoError::SessionReserved)
    ));
    assert_eq!(
        fs::read(path).unwrap(),
        bytes,
        "existing association cannot be overwritten"
    );
}

#[test]
fn it_reserves_exactly_once_under_concurrent_callers() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let id = SessionId::new();
    let barrier = Arc::new(Barrier::new(8));
    let results = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    LaunchBinding::reserve(root, id)
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(NonoError::SessionReserved)))
            .count(),
        7
    );
}

#[test]
fn it_refuses_unreserved_names_detached_launches_and_relative_roots() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let binding = LaunchBinding::reserve(root, SessionId::new()).unwrap();
    let args = RunArgs {
        profile: root.join("profile.json"),
        name: binding.session(),
        workdir: root.to_owned(),
        mode: RunMode::Foreground,
        command: vec!["agent".into()],
    };
    let _ = binding.command(&root.join("nono"), args.clone()).unwrap();
    let mut wrong = args.clone();
    wrong.name = SessionId::new();
    assert!(binding.command(&root.join("nono"), wrong).is_err());
    let mut detached = args;
    detached.mode = RunMode::Detached;
    assert!(binding.command(&root.join("nono"), detached).is_err());
    assert!(LaunchBinding::reserve(Utf8Path::new("relative"), SessionId::new()).is_err());
    assert!(LaunchBinding::reserve(&root.join("missing"), SessionId::new()).is_err());
}
