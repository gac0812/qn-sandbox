#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;

use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use tar::{Builder, Header};

use super::{extract_binary, verify_checksum};
use crate::error::InstallerError;
use crate::manifest::{self, ReleaseArtifact};
use crate::target::NonoTarget;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qn-nono-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture_artifact() -> ReleaseArtifact {
    *manifest::artifact(NonoTarget::Aarch64AppleDarwin).unwrap()
}

#[test]
fn it_rejects_a_corrupt_cache_even_when_a_binary_already_exists() {
    let root = temp_dir("unverified-existing");
    let version = root.join(manifest::LOCKED_NONO_VERSION);
    fs::create_dir_all(&version).unwrap();
    fs::write(version.join("nono"), b"untrusted executable").unwrap();
    fs::write(
        version.join(fixture_artifact().archive_name),
        b"corrupt archive",
    )
    .unwrap();
    let options = super::InstallOptions::new(&root).with_target(NonoTarget::Aarch64AppleDarwin);
    assert!(matches!(
        super::install(&options),
        Err(InstallerError::ChecksumMismatch { .. })
    ));
}

#[test]
fn checksum_accepts_matching_archive() {
    let dir = temp_dir("verify-ok");
    let path = dir.join("archive.tar.gz");
    fs::write(&path, b"payload").unwrap();
    let mut artifact = fixture_artifact();
    artifact.archive_sha256 = Box::leak(hex::encode(Sha256::digest(b"payload")).into_boxed_str());
    verify_checksum(&path, &artifact).unwrap();
}

#[test]
fn checksum_rejects_tampered_archive() {
    let dir = temp_dir("verify-bad");
    let path = dir.join("archive.tar.gz");
    fs::write(&path, b"tampered payload").unwrap();
    let error = verify_checksum(&path, &fixture_artifact()).unwrap_err();
    match error {
        InstallerError::ChecksumMismatch { expected, actual } => {
            assert_eq!(expected, fixture_artifact().archive_sha256);
            assert_eq!(actual.len(), 64);
        }
        other => panic!("expected ChecksumMismatch, got {other:?}"),
    }
}

/// Builds an archive with the upstream layout (root `nono` binary plus an
/// extra file) and returns the gzipped bytes.
fn fixture_archive() -> Vec<u8> {
    let mut gz = GzEncoder::new(Vec::new(), flate2::Compression::fast());
    {
        let mut builder = Builder::new(&mut gz);
        for (name, mode, payload) in [
            ("nono", 0o755, b"binary-bytes".as_slice()),
            ("docs/README", 0o644, b"readme".as_slice()),
        ] {
            let mut header = Header::new_gnu();
            header.set_size(payload.len() as u64);
            header.set_mode(mode);
            header.set_cksum();
            builder.append_data(&mut header, name, payload).unwrap();
        }
        builder.finish().unwrap();
    }
    gz.finish().unwrap()
}

#[test]
fn extract_installs_only_the_nono_binary() {
    let dir = temp_dir("extract");
    let archive_path = dir.join("archive.tar.gz");
    fs::write(&archive_path, fixture_archive()).unwrap();
    let destination = dir.join("out").join("nono");

    extract_binary(&archive_path, &destination).unwrap();

    assert_eq!(fs::read(&destination).unwrap(), b"binary-bytes");
    assert!(!dir.join("out").join("docs").join("README").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&destination).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "binary must stay executable");
    }
}

#[test]
fn extract_fails_on_layout_drift() {
    let dir = temp_dir("extract-missing");
    let archive_path = dir.join("archive.tar.gz");
    // Archive without the `nono` entry simulates upstream layout drift.
    let mut gz = GzEncoder::new(Vec::new(), flate2::Compression::fast());
    {
        let mut builder = Builder::new(&mut gz);
        let mut header = Header::new_gnu();
        header.set_size(6);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "docs/README", "readme".as_bytes())
            .unwrap();
        builder.finish().unwrap();
    }
    fs::write(&archive_path, gz.finish().unwrap()).unwrap();

    let error = extract_binary(&archive_path, &dir.join("out").join("nono")).unwrap_err();
    assert!(matches!(error, InstallerError::ArchiveMissingBinary));
}

/// Manual smoke: installs the real pinned release (network required).
#[test]
#[ignore = "downloads the real pinned release from GitHub; run manually"]
fn installs_real_release() {
    let root = temp_dir("smoke");
    let options = super::InstallOptions::new(&root);
    let installed = super::install(&options).unwrap();
    assert_eq!(installed.version, manifest::LOCKED_NONO_VERSION);
    assert!(installed.path.is_file());
    assert_eq!(
        installed.path,
        root.join(manifest::LOCKED_NONO_VERSION).join("nono")
    );
}
