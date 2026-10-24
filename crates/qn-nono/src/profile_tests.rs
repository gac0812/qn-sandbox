#![allow(clippy::unwrap_used)]
use super::*;
use qn_core::{GrantHandle, ProfileRef, SessionId};

fn fixture() -> (tempfile::TempDir, RunSpec, RuntimePaths) {
    let dir = match std::env::var("QN_TEST_PROFILE_PARENT") {
        Ok(parent) => tempfile::tempdir_in(parent).unwrap(),
        Err(_) => tempfile::tempdir().unwrap(),
    };
    let root = utf8(dir.path().to_owned()).unwrap();
    for sub in ["home/project", "home/.ssh", "control", "bin"] {
        fs::create_dir_all(root.join(sub)).unwrap();
    }
    let binary = root.join("bin/nono");
    fs::write(&binary, b"fixture binary").unwrap();
    let spec = RunSpec::new(
        SessionId::new(),
        ProfileRef::try_from("qn/codex").unwrap(),
        vec!["agent".into()],
        root.join("home/project"),
    )
    .unwrap();
    let paths = RuntimePaths {
        control_root: root.join("control"),
        host_homes: vec![root.join("home")],
        engine_binary: binary,
        tool_dirs: vec![root.join("bin")],
    };
    (dir, spec, paths)
}

#[test]
fn it_emits_closed_offline_policy_and_cleans_unpersisted_state() {
    let (_dir, spec, paths) = fixture();
    let profile = PreparedProfile::prepare(spec, &paths).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(profile.profile_path()).unwrap()).unwrap();
    assert_eq!(value["network"]["block"], true);
    assert_eq!(value["security"]["capability_elevation"], false);
    for forbidden in [
        "env_credentials",
        "session_hooks",
        "hooks",
        "allow_parent_of_protected",
    ] {
        assert!(value.get(forbidden).is_none());
    }
    assert!(value["filesystem"].get("bypass_protection").is_none());
    assert_eq!(value["environment"]["set_vars"]["TMPDIR"], "$HOME/tmp");
    let root = profile.root().to_owned();
    assert!(!format!("{profile:?}").contains(root.as_str()));
    drop(profile);
    assert!(!root.exists());
}

#[test]
fn it_rejects_grants_unknown_profiles_and_overlapping_paths() {
    let (_dir, spec, paths) = fixture();
    assert!(matches!(
        PreparedProfile::prepare(spec.clone().with_grant(GrantHandle::new()), &paths),
        Err(NonoError::GrantBindingRequired)
    ));
    assert!(matches!(
        PreparedProfile::prepare(request(&spec, "other/app", spec.workdir()), &paths),
        Err(NonoError::UnsupportedProfile)
    ));
    for workdir in [
        paths.host_homes[0].clone(),
        paths.host_homes[0].join(".ssh"),
        paths.control_root.clone(),
        paths.engine_binary.parent().unwrap().to_owned(),
    ] {
        assert!(matches!(
            PreparedProfile::prepare(request(&spec, spec.profile().as_str(), &workdir), &paths),
            Err(NonoError::UnsafeWorkdir)
        ));
    }
    assert_eq!(fs::read_dir(&paths.control_root).unwrap().count(), 0);
    let alias: Utf8PathBuf = paths.host_homes[0].as_str().to_uppercase().into();
    if alias.exists() {
        assert!(matches!(
            PreparedProfile::prepare(request(&spec, spec.profile().as_str(), &alias), &paths),
            Err(NonoError::UnsafeWorkdir)
        ));
    }
}

#[test]
fn it_can_explicitly_retain_audit_state() {
    let (_dir, spec, paths) = fixture();
    let profile = PreparedProfile::prepare(spec, &paths).unwrap().persist();
    let root = profile.root().to_owned();
    drop(profile);
    assert!(root.join("profile.json").is_file());
}

#[cfg(unix)]
#[test]
fn it_rejects_a_project_containing_a_redirected_credential_directory() {
    for name in [".ssh", ".netrc", ".kube", ".docker"] {
        let (_dir, spec, paths) = fixture();
        fs::remove_dir(paths.host_homes[0].join(".ssh")).unwrap();
        let target = spec.workdir().join("keys");
        if name == ".netrc" {
            fs::write(&target, "synthetic credential").unwrap();
        } else {
            fs::create_dir(&target).unwrap();
        }
        std::os::unix::fs::symlink(target, paths.host_homes[0].join(name)).unwrap();
        assert!(
            matches!(
                PreparedProfile::prepare(spec, &paths),
                Err(NonoError::UnsafeWorkdir)
            ),
            "redirected {name} must never become project-readable"
        );
    }
}

fn request(spec: &RunSpec, profile: &str, workdir: &Utf8Path) -> RunSpec {
    RunSpec::new(
        spec.id(),
        ProfileRef::try_from(profile).unwrap(),
        spec.command().to_vec(),
        workdir.to_owned(),
    )
    .unwrap()
}
