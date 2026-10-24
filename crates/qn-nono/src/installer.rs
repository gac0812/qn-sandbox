//! Download, verify, and install the locked `nono` release.
//!
//! Flow: resolve target → snapshot cached archive or download the pinned
//! GitHub release archive → verify SHA-256 against the locked manifest →
//! unpack the single root `nono` binary into `<install_root>/<version>/nono`.
//!
//! Blocking implementation: callers inside async contexts must wrap `install`
//! in `spawn_blocking` (AGENTS §4.6). Callers can then explicitly await
//! [`crate::verify_version`] through the shared process runner (#41).

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::error::InstallerError;
use crate::manifest::{self, ReleaseArtifact};
use crate::target::NonoTarget;

/// Where and how to install the locked `nono` binary.
#[derive(Clone, Debug)]
pub struct InstallOptions {
    /// Caller-provided cache root; the versioned binary lands under
    /// `<install_root>/<version>/nono`. The default root comes from
    /// `qn-config` once that crate lands its path constants.
    pub install_root: PathBuf,
    /// Explicit target override; `None` resolves the current host. Used by
    /// `qn-wsl` to stage the Linux artifact for WSL2 installs.
    pub target: Option<NonoTarget>,
}

impl InstallOptions {
    /// Options installing the host build under `install_root`.
    pub fn new(install_root: impl Into<PathBuf>) -> Self {
        Self {
            install_root: install_root.into(),
            target: None,
        }
    }

    /// Overrides host detection with an explicit release target.
    #[must_use]
    pub fn with_target(mut self, target: NonoTarget) -> Self {
        self.target = Some(target);
        self
    }
}

/// Result of a successful install.
#[derive(Clone, Debug)]
pub struct InstalledBinary {
    /// Path of the installed executable.
    pub path: PathBuf,
    /// Locked version that was installed.
    pub version: &'static str,
}

/// Installs the locked `nono` release into `<install_root>/<version>/nono`.
///
/// Every call verifies a snapshot of the cached archive (or a fresh download)
/// against the manifest and republishes its executable. A cached archive permits
/// offline repair; an existing executable alone is never trusted. Use distinct
/// install roots when staging different targets. Verification does not prevent
/// a privileged host process from changing files after this function returns.
///
/// # Errors
///
/// Returns [`InstallerError::UnsupportedTarget`] when no locked artifact
/// matches the (host) target, [`InstallerError::DownloadFailed`] when the
/// pinned release cannot be fetched, [`InstallerError::ChecksumMismatch`]
/// when the archive digest deviates from the manifest, and
/// [`InstallerError::ArchiveMissingBinary`] / [`InstallerError::Io`] when
/// unpacking or staging fails.
// TODO(#14): wire verify_version into the future adapter startup after the
// shared engine contract is agreed; install stays synchronous and install-only.
pub fn install(options: &InstallOptions) -> Result<InstalledBinary, InstallerError> {
    let artifact = resolve_artifact(options)?;
    install_artifact(&options.install_root, artifact)
}

fn install_artifact(
    root: &Path,
    artifact: &ReleaseArtifact,
) -> Result<InstalledBinary, InstallerError> {
    let version_dir = root.join(manifest::LOCKED_NONO_VERSION);
    let binary_path = version_dir.join("nono");
    fs::create_dir_all(&version_dir).map_err(|source| InstallerError::Io {
        step: "creating install directory",
        source,
    })?;
    let staging = tempfile::tempdir_in(&version_dir).map_err(|source| InstallerError::Io {
        step: "creating staging directory",
        source,
    })?;

    let cache_path = version_dir.join(artifact.archive_name);
    let is_cached = cache_path.is_file();
    let archive_path = if is_cached {
        let snapshot = staging.path().join(artifact.archive_name);
        fs::copy(&cache_path, &snapshot).map_err(|source| InstallerError::Io {
            step: "snapshotting cached archive",
            source,
        })?;
        snapshot
    } else {
        download_to(artifact, staging.path())?
    };
    verify_checksum(&archive_path, artifact)?;
    extract_binary(&archive_path, &staging.path().join("nono"))?;
    if !is_cached {
        fs::rename(&archive_path, &cache_path).map_err(|source| InstallerError::Io {
            step: "publishing verified archive cache",
            source,
        })?;
    }
    // Re-extraction also restores archive permissions, not just executable bytes.
    fs::rename(staging.path().join("nono"), &binary_path).map_err(|source| InstallerError::Io {
        step: "moving binary into place",
        source,
    })?;

    Ok(InstalledBinary {
        path: binary_path,
        version: manifest::LOCKED_NONO_VERSION,
    })
}

/// Resolves the manifest artifact for the requested (or host) target.
fn resolve_artifact(options: &InstallOptions) -> Result<&'static ReleaseArtifact, InstallerError> {
    let resolved = match options.target {
        Some(target) => target,
        None => NonoTarget::current().ok_or_else(|| InstallerError::UnsupportedTarget {
            target: format!("host {}-{}", std::env::consts::OS, std::env::consts::ARCH),
            version: manifest::LOCKED_NONO_VERSION,
            supported: manifest::supported_targets(),
        })?,
    };
    manifest::artifact(resolved).ok_or_else(|| InstallerError::UnsupportedTarget {
        target: resolved.to_string(),
        version: manifest::LOCKED_NONO_VERSION,
        supported: manifest::supported_targets(),
    })
}

/// Streams the release archive into the staging directory.
fn download_to(artifact: &ReleaseArtifact, staging_dir: &Path) -> Result<PathBuf, InstallerError> {
    let url = artifact.download_url();
    let response = ureq::get(&url)
        .timeout(Duration::from_secs(120))
        .call()
        .map_err(|source| InstallerError::DownloadFailed {
            url: url.clone(),
            version: manifest::LOCKED_NONO_VERSION,
            source: Box::new(source),
        })?;
    let archive_path = staging_dir.join(artifact.archive_name);
    let mut reader = response.into_reader();
    let mut file = File::create(&archive_path).map_err(|source| InstallerError::Io {
        step: "creating download staging file",
        source,
    })?;
    io::copy(&mut reader, &mut file).map_err(|source| InstallerError::Io {
        step: "writing downloaded archive",
        source,
    })?;
    Ok(archive_path)
}

/// Hashes the archive snapshot and refuses any mismatch with the manifest.
fn verify_checksum(archive_path: &Path, artifact: &ReleaseArtifact) -> Result<(), InstallerError> {
    let mut file = File::open(archive_path).map_err(|source| InstallerError::Io {
        step: "opening archive for integrity validation",
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| InstallerError::Io {
                step: "hashing archive for integrity validation",
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(artifact.archive_sha256) {
        return Err(InstallerError::ChecksumMismatch {
            expected: artifact.archive_sha256,
            actual,
        });
    }
    Ok(())
}

/// Extracts the single root `nono` binary from the verified archive.
fn extract_binary(archive_path: &Path, destination: &Path) -> Result<(), InstallerError> {
    let archive_file = File::open(archive_path).map_err(|source| InstallerError::Io {
        step: "opening archive for extraction",
        source,
    })?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(archive_file));
    archive.set_preserve_permissions(true);
    let entries = archive.entries().map_err(|source| InstallerError::Io {
        step: "reading archive entries",
        source,
    })?;
    for entry in entries {
        let mut entry = entry.map_err(|source| InstallerError::Io {
            step: "reading archive entry",
            source,
        })?;
        let is_binary = entry.header().entry_type().is_file()
            && entry.path().is_ok_and(|path| path == Path::new("nono"));
        if is_binary {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|source| InstallerError::Io {
                    step: "creating extraction directory",
                    source,
                })?;
            }
            entry
                .unpack(destination)
                .map_err(|source| InstallerError::Io {
                    step: "extracting nono binary",
                    source,
                })?;
            return Ok(());
        }
    }
    Err(InstallerError::ArchiveMissingBinary)
}

#[cfg(test)]
#[path = "installer_tests.rs"]
mod tests;
