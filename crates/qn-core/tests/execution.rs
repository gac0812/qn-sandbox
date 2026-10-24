//! Launch-contract tests without adapters, filesystem policy or credentials.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use camino::Utf8PathBuf;
use qn_core::{Error, GrantHandle, ProfileRef, RunSpec, SessionId};

fn root() -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(std::env::temp_dir()).unwrap()
}
fn spec(command: Vec<String>, workdir: Utf8PathBuf) -> qn_core::Result<RunSpec> {
    RunSpec::new(
        SessionId::new(),
        ProfileRef::try_from("qn/codex")?,
        command,
        workdir,
    )
}

#[test]
fn it_accepts_only_canonical_managed_profile_names() {
    for name in ["qn/codex", "team-a/agent_2", "a/0"] {
        assert_eq!(ProfileRef::try_from(name).unwrap().as_str(), name);
    }
    for name in [
        "",
        "codex",
        "/codex",
        "qn/",
        "qn/../codex",
        "qn/Codex",
        "qn/--flag",
        "qn/a b",
        "qn/a\0",
        "../name",
        "a/b/c",
    ] {
        assert_eq!(ProfileRef::try_from(name), Err(Error::InvalidProfileRef));
    }
    assert!(ProfileRef::try_from(format!("qn/{}", "a".repeat(65)).as_str()).is_err());
}

#[test]
fn it_round_trips_grants_without_mixing_session_ids() {
    let grant = GrantHandle::new();
    assert_eq!(
        GrantHandle::try_from(grant.to_string().as_str()).unwrap(),
        grant
    );
    for raw in [
        SessionId::new().to_string(),
        grant.to_string().to_uppercase(),
        "grt-0190f2f8-2b0a-4f5e-9a3c-4b5d6e7f8091".into(),
        "grt-invalid".into(),
    ] {
        assert_eq!(
            GrantHandle::try_from(raw.as_str()),
            Err(Error::InvalidGrantHandle)
        );
    }
    assert!(!format!("{grant:?}").contains(&grant.to_string()));
}

#[test]
fn it_preserves_argv_and_redacts_launch_debug() {
    let argv = vec![
        "agent".into(),
        "a b".into(),
        "$(not-executed)".into(),
        String::new(),
    ];
    let grant = GrantHandle::new();
    let launch = spec(argv.clone(), root().join("private-workdir"))
        .unwrap()
        .with_grant(grant);
    assert_eq!(launch.command(), argv);
    assert_eq!(launch.profile().as_str(), "qn/codex");
    assert_eq!(launch.grant(), Some(grant));
    let debug = format!("{launch:?}");
    for hidden in ["private-workdir", "$(not-executed)", &grant.to_string()] {
        assert!(!debug.contains(hidden));
    }
}

#[test]
fn it_rejects_invalid_launch_structure_without_echoing_values() {
    for argv in [
        vec![],
        vec![String::new()],
        vec!["agent\0".into()],
        vec!["agent".into(), "private\0value".into()],
    ] {
        let error = spec(argv, root()).unwrap_err();
        assert_eq!(error, Error::InvalidRunInput { field: "command" });
        assert!(!error.to_string().contains("private"));
    }
    for directory in [
        Utf8PathBuf::from("relative"),
        Utf8PathBuf::from(root().into_string() + "\0"),
    ] {
        assert!(matches!(
            spec(vec!["agent".into()], directory),
            Err(Error::InvalidRunInput { field: "workdir" })
        ));
    }
}
