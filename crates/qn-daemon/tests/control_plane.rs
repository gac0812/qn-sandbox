//! Public contract tests for the shared daemon composition root.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use qn_config::paths::QnPaths;
use qn_daemon::ControlPlane;

#[test]
fn it_rejects_relative_roots_before_constructing_adapters() {
    let result = ControlPlane::new(QnPaths::from_root("relative/qn"));
    assert!(result.is_err());
}

#[test]
fn it_exposes_shared_paths_and_all_core_service_surfaces() {
    let root = tempfile::tempdir().unwrap();
    let home = camino::Utf8PathBuf::from_path_buf(root.path().to_owned()).unwrap();
    let plane = ControlPlane::new(QnPaths::from_root(home.clone())).unwrap();

    assert_eq!(plane.paths().root, home);
    assert!(!plane.profiles().list().unwrap().is_empty());
    let _ = plane.engine();
    let _ = plane.audit();
    let _ = plane.native_audit();
}
