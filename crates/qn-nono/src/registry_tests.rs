#![allow(clippy::unwrap_used)]
use super::*;
use qn_core::profile::ProfileRegistry;

fn root() -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(directory.path().to_owned()).unwrap();
    (directory, path)
}

fn identity(text: &str) -> ProfileRef {
    ProfileRef::try_from(text).unwrap()
}

#[test]
fn it_loads_two_strict_embedded_profiles() {
    let catalog = OfficialCatalog::embedded().unwrap();
    let summaries = catalog.summaries();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].id().as_str(), "qn/claude-code");
    assert_eq!(summaries[0].version().as_str(), "1.0.0");
    assert_eq!(summaries[0].source(), ProfileSource::Builtin);
    let claude = catalog.resolve(&identity("qn/claude-code")).unwrap();
    assert_eq!(claude.state_dir(), ".claude");
    assert!(
        claude
            .network_hosts()
            .contains(&"api.anthropic.com".to_owned())
    );
    assert!(!claude.network_hosts().contains(&"*".to_owned()));
    assert!(
        claude.nono_profile().contains("\"deny_credentials\"")
            || claude.nono_profile().contains("\"extends\": \"default\"")
    );
    assert!(!format!("{claude:?}").contains("api.anthropic.com"));
    let codex = catalog.resolve(&identity("qn/codex")).unwrap();
    assert_eq!(codex.state_dir(), ".codex");
    assert!(codex.network_hosts().contains(&"api.openai.com".to_owned()));
}

#[test]
fn it_rejects_unknown_profiles_and_relative_roots() {
    let catalog = OfficialCatalog::embedded().unwrap();
    assert!(matches!(
        catalog.resolve(&identity("qn/other")),
        Err(ProfileError::UnknownProfile)
    ));
    assert!(matches!(
        HostProfileRegistry::new("relative".into()),
        Err(ProfileError::InvalidRoot)
    ));
    let (_directory, path) = root();
    let registry = HostProfileRegistry::new(path).unwrap();
    assert!(matches!(
        registry.install(identity("qn/other")),
        Err(ProfileError::UnknownProfile)
    ));
}

#[test]
fn it_installs_resolves_and_lists_an_official_profile() {
    let (_directory, path) = root();
    let registry = HostProfileRegistry::new(path).unwrap();
    let catalog = OfficialCatalog::embedded().unwrap();
    let id = identity("qn/claude-code");
    let installed = registry.install(id.clone()).unwrap();
    assert_eq!(installed.source(), ProfileSource::Installed);
    let resolved = registry.resolve(id.clone()).unwrap();
    let builtin = catalog.resolve(&id).unwrap();
    assert_eq!(resolved.nono_profile(), builtin.nono_profile());
    assert_eq!(resolved.summary().source(), ProfileSource::Installed);
    let listed = registry.list().unwrap();
    assert!(listed.iter().any(|summary| {
        summary.id() == &id
            && summary.version().as_str() == "1.0.0"
            && summary.source() == ProfileSource::Installed
    }));
    // Re-installing identical bytes is idempotent.
    registry.install(id).unwrap();
}

#[test]
fn it_detects_tampered_assets_and_conflicting_versions() {
    let (_directory, path) = root();
    let registry = HostProfileRegistry::new(path).unwrap();
    let id = identity("qn/codex");
    registry.install(id.clone()).unwrap();
    let profile_path = registry
        .id_dir(&id)
        .join("versions/1.0.0")
        .join(PROFILE_FILE);
    let original = fs::read(&profile_path).unwrap();
    fs::write(&profile_path, b"{}").unwrap();
    assert!(matches!(
        registry.resolve(id.clone()),
        Err(ProfileError::Integrity)
    ));
    fs::write(&profile_path, original).unwrap();
    registry.resolve(id.clone()).unwrap();

    let manifest_path = registry
        .id_dir(&id)
        .join("versions/1.0.0")
        .join(VERSION_MANIFEST);
    let mut stored: StoredManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    stored.sha256 = "0".repeat(64);
    fs::write(&manifest_path, serde_json::to_vec(&stored).unwrap()).unwrap();
    assert!(matches!(registry.install(id), Err(ProfileError::Conflict)));
}

#[test]
fn it_rolls_back_to_the_previous_verified_version() {
    let (_directory, path) = root();
    let registry = HostProfileRegistry::new(path).unwrap();
    let id = identity("qn/claude-code");
    registry.install(id.clone()).unwrap();
    let base = OfficialCatalog::embedded().unwrap().resolve(&id).unwrap();
    let previous = ProfileVersion::try_from("0.9.0").unwrap();
    let previous_asset = base
        .nono_profile()
        .replace("\"version\": \"1.0.0\"", "\"version\": \"0.9.0\"");
    registry
        .publish(&id, &previous, &previous_asset, base.state_dir())
        .unwrap();
    let selected = registry.rollback(id.clone()).unwrap();
    assert_eq!(selected.version().as_str(), "0.9.0");
    assert_eq!(
        registry
            .resolve(id.clone())
            .unwrap()
            .summary()
            .version()
            .as_str(),
        "0.9.0"
    );
    assert!(matches!(
        registry.rollback(id),
        Err(ProfileError::NoRollback)
    ));
}

#[test]
fn it_publishes_a_matching_official_asset_and_refuses_mismatches() {
    let (_directory, path) = root();
    let registry = HostProfileRegistry::new(path).unwrap();
    let id = identity("qn/claude-code");
    registry.install(id.clone()).unwrap();
    let base = OfficialCatalog::embedded().unwrap().resolve(&id).unwrap();
    let version = ProfileVersion::try_from("1.1.0").unwrap();
    let asset = base
        .nono_profile()
        .replace("\"version\": \"1.0.0\"", "\"version\": \"1.1.0\"");
    let summary = registry
        .publish_asset(id.clone(), version.clone(), &asset, base.state_dir())
        .unwrap();
    assert_eq!(summary.version().as_str(), "1.1.0");

    assert!(matches!(
        registry.publish_asset(
            id.clone(),
            ProfileVersion::try_from("1.2.0").unwrap(),
            &asset.replace("1.1.0", "1.2.0"),
            ".other"
        ),
        Err(ProfileError::InvalidCatalog)
    ));
    assert!(matches!(
        registry.publish_asset(
            id.clone(),
            ProfileVersion::try_from("1.2.0").unwrap(),
            &asset,
            base.state_dir()
        ),
        Err(ProfileError::InvalidVersion)
    ));
    let conflicting = asset.replace("api.anthropic.com", "api.example.com");
    assert!(matches!(
        registry.publish_asset(id.clone(), version, &conflicting, base.state_dir()),
        Err(ProfileError::Conflict)
    ));
    let rolled = registry.rollback(id).unwrap();
    assert_eq!(rolled.version().as_str(), "1.0.0");
}

#[test]
fn it_requires_an_install_before_a_rollback() {
    let (_directory, path) = root();
    let registry = HostProfileRegistry::new(path).unwrap();
    assert!(matches!(
        registry.rollback(identity("qn/codex")),
        Err(ProfileError::NotInstalled)
    ));
}

#[test]
fn it_builds_the_strict_native_validation_command() {
    let (_directory, path) = root();
    let registry = HostProfileRegistry::new(path).unwrap();
    // 夹具路径须是**当前平台**的绝对路径（validate_path 按 host 语义校验）；
    // 断言与夹具同源，避免 Unix/Windows 形态漂移。
    let (asset, expected_last): (&Utf8Path, &str) = if cfg!(windows) {
        (
            Utf8Path::new(r"C:\private\profile.json"),
            r"C:\private\profile.json",
        )
    } else {
        (
            Utf8Path::new("/private/profile.json"),
            "/private/profile.json",
        )
    };
    let args = registry
        .validation_args(&identity("qn/codex"), asset)
        .unwrap();
    assert_eq!(
        args.as_slice(),
        [
            "profile",
            "validate",
            "--json",
            "--strict",
            "--",
            expected_last
        ]
    );
    assert!(
        registry
            .validation_args(&identity("qn/other"), asset)
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn it_rejects_symlinked_registry_storage() {
    let (_directory, path) = root();
    let registry = HostProfileRegistry::new(path.clone()).unwrap();
    let versions = registry
        .id_dir(&identity("qn/claude-code"))
        .join("versions");
    fs::create_dir_all(&versions).unwrap();
    let target = path.join("elsewhere");
    fs::create_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&target, versions.join("1.0.0")).unwrap();
    assert!(matches!(registry.list(), Err(ProfileError::Integrity)));
}

#[test]
#[ignore = "requires QN_TEST_NONO pointing at the checksum-verified 0.80.0 binary"]
fn it_validates_embedded_assets_with_the_pinned_engine() {
    let Some(binary) = std::env::var_os("QN_TEST_NONO") else {
        return;
    };
    let catalog = OfficialCatalog::embedded().unwrap();
    let directory = tempfile::tempdir().unwrap();
    for name in ["qn/claude-code", "qn/codex"] {
        let resolved = catalog.resolve(&identity(name)).unwrap();
        let path = directory
            .path()
            .join(format!("{}.json", name.replace('/', "-")));
        fs::write(&path, resolved.nono_profile()).unwrap();
        let output = std::process::Command::new(&binary)
            .args(["profile", "validate", "--json", "--strict", "--"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} failed native validation: {}",
            name,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
