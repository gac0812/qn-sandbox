//! Locked release manifest for the pinned `nono` version.
//!
//! The version constant and the per-target SHA-256 digests form the version
//! lock required by 04 §6: the workspace drives exactly this release, and an
//! upgrade is a dedicated `deps:` PR that regenerates this file against the
//! matching upstream `SHA256SUMS.txt` with regression evidence.
//!
//! Source of truth: upstream `SHA256SUMS.txt` for v0.80.0
//! (<https://github.com/nolabs-ai/nono/releases/download/v0.80.0/SHA256SUMS.txt>),
//! observed 2026-10-24. Upstream ships no Windows artifact — WSL2 installs
//! use the Linux artifacts via `qn-wsl` (M2).

use crate::target::NonoTarget;

/// The only `nono` version this workspace is allowed to drive.
pub const LOCKED_NONO_VERSION: &str = "0.80.0";

/// Base URL of upstream GitHub release downloads.
const RELEASE_BASE_URL: &str = "https://github.com/nolabs-ai/nono/releases/download";

/// One prebuilt archive of the locked release.
#[derive(Clone, Copy, Debug)]
pub struct ReleaseArtifact {
    /// Platform the archive is built for.
    pub target: NonoTarget,
    /// Asset file name within the GitHub release.
    pub archive_name: &'static str,
    /// SHA-256 of the archive, from the upstream `SHA256SUMS.txt`.
    pub archive_sha256: &'static str,
}

impl ReleaseArtifact {
    /// Download URL of this artifact within the locked release.
    #[must_use]
    pub fn download_url(&self) -> String {
        format!(
            "{RELEASE_BASE_URL}/v{LOCKED_NONO_VERSION}/{}",
            self.archive_name
        )
    }
}

/// Every artifact of the locked release, transcribed from upstream
/// `SHA256SUMS.txt` (v0.80.0, observed 2026-10-24).
pub const MANIFEST: &[ReleaseArtifact] = &[
    ReleaseArtifact {
        target: NonoTarget::Aarch64AppleDarwin,
        archive_name: "nono-v0.80.0-aarch64-apple-darwin.tar.gz",
        archive_sha256: "ae824167b3d90531b830429ba92b7781379b1c2d79f9b11fdd88b0fd267e4e32",
    },
    ReleaseArtifact {
        target: NonoTarget::X8664AppleDarwin,
        archive_name: "nono-v0.80.0-x86_64-apple-darwin.tar.gz",
        archive_sha256: "b354e674e6b4643ad5c572d148ae367bb0c5c103410fbc22923c5593a6f7d0b2",
    },
    ReleaseArtifact {
        target: NonoTarget::Aarch64UnknownLinuxGnu,
        archive_name: "nono-v0.80.0-aarch64-unknown-linux-gnu.tar.gz",
        archive_sha256: "d2a24c8c37714f85c829b5586ffd81fc2acf336c95372d881ba62140884f22cb",
    },
    ReleaseArtifact {
        target: NonoTarget::X8664UnknownLinuxGnu,
        archive_name: "nono-v0.80.0-x86_64-unknown-linux-gnu.tar.gz",
        archive_sha256: "6da665b61999570c10dffc7da78828fe1fb7f3cac53319e505cebecaef17037f",
    },
    ReleaseArtifact {
        target: NonoTarget::X8664UnknownLinuxMusl,
        archive_name: "nono-v0.80.0-x86_64-unknown-linux-musl.tar.gz",
        archive_sha256: "cae6755f1188b561ea02f62995feb8ad87d457f6d5255956ec54f41bbefca31e",
    },
];

/// Looks up the locked artifact for `target`; `None` when upstream ships no
/// build for that platform.
#[must_use]
pub fn artifact(target: NonoTarget) -> Option<&'static ReleaseArtifact> {
    MANIFEST.iter().find(|entry| entry.target == target)
}

/// Comma-separated list of targets that have locked artifacts, for error messages.
#[must_use]
pub fn supported_targets() -> String {
    MANIFEST
        .iter()
        .map(|entry| entry.target.triple())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{LOCKED_NONO_VERSION, MANIFEST, artifact, supported_targets};
    use crate::target::NonoTarget;

    #[test]
    fn locks_the_expected_version() {
        assert_eq!(LOCKED_NONO_VERSION, "0.80.0");
    }

    #[test]
    fn every_target_has_an_artifact_with_wellformed_digest() {
        for target in [
            NonoTarget::Aarch64AppleDarwin,
            NonoTarget::X8664AppleDarwin,
            NonoTarget::Aarch64UnknownLinuxGnu,
            NonoTarget::X8664UnknownLinuxGnu,
            NonoTarget::X8664UnknownLinuxMusl,
        ] {
            let artifact = artifact(target).unwrap();
            assert!(artifact.archive_name.starts_with("nono-v0.80.0-"));
            assert_eq!(artifact.archive_sha256.len(), 64);
            assert!(
                artifact
                    .archive_sha256
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                "digest must be lowercase hex"
            );
        }
    }

    #[test]
    fn manifest_covers_each_target_exactly_once() {
        for (index, lhs) in MANIFEST.iter().enumerate() {
            assert!(
                MANIFEST
                    .iter()
                    .skip(index + 1)
                    .all(|rhs| rhs.target != lhs.target),
                "duplicate target in manifest"
            );
        }
    }

    #[test]
    fn supported_targets_lists_every_manifest_entry() {
        let supported = supported_targets();
        assert_eq!(supported.split(", ").count(), MANIFEST.len());
        assert!(supported.contains("aarch64-apple-darwin"));
    }

    #[test]
    fn download_urls_are_pinned_release_urls() {
        let artifact = artifact(NonoTarget::Aarch64AppleDarwin).unwrap();
        assert_eq!(
            artifact.download_url(),
            "https://github.com/nolabs-ai/nono/releases/download/v0.80.0/nono-v0.80.0-aarch64-apple-darwin.tar.gz"
        );
    }
}
