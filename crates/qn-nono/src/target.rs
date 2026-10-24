//! Release target triples known to the locked manifest.

use std::fmt;

/// Platforms that have an official prebuilt `nono` release archive.
///
/// Deliberately resolved from `std::env::consts` values instead of
/// `#[cfg]`: platform branching stays out of adapter code (R6), and the
/// mapping is pure and testable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NonoTarget {
    /// macOS, Apple Silicon.
    Aarch64AppleDarwin,
    /// macOS, Intel.
    X8664AppleDarwin,
    /// Linux, arm64 (glibc; also the WSL2 artifact used on Windows via qn-wsl).
    Aarch64UnknownLinuxGnu,
    /// Linux, `x86_64` (glibc; also the WSL2 artifact used on Windows via qn-wsl).
    X8664UnknownLinuxGnu,
    /// Linux, `x86_64` (musl; opt-in for restricted environments).
    X8664UnknownLinuxMusl,
}

impl NonoTarget {
    /// Maps `std::env::consts::OS` / `ARCH` values onto a release target.
    ///
    /// Returns `None` for hosts without an upstream artifact (e.g. Windows
    /// hosts — there, `qn-wsl` installs the Linux build inside WSL2).
    #[must_use]
    pub fn from_os_arch(os: &str, arch: &str) -> Option<Self> {
        match (os, arch) {
            ("macos", "aarch64") => Some(Self::Aarch64AppleDarwin),
            ("macos", "x86_64") => Some(Self::X8664AppleDarwin),
            ("linux", "aarch64") => Some(Self::Aarch64UnknownLinuxGnu),
            ("linux", "x86_64") => Some(Self::X8664UnknownLinuxGnu),
            _ => None,
        }
    }

    /// Resolves the host platform, or `None` when upstream ships no build for it.
    #[must_use]
    pub fn current() -> Option<Self> {
        Self::from_os_arch(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// Rust target triple of the upstream release artifact.
    #[must_use]
    pub fn triple(self) -> &'static str {
        match self {
            Self::Aarch64AppleDarwin => "aarch64-apple-darwin",
            Self::X8664AppleDarwin => "x86_64-apple-darwin",
            Self::Aarch64UnknownLinuxGnu => "aarch64-unknown-linux-gnu",
            Self::X8664UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            Self::X8664UnknownLinuxMusl => "x86_64-unknown-linux-musl",
        }
    }
}

impl fmt::Display for NonoTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.triple())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::NonoTarget;

    #[test]
    fn maps_host_os_arch_onto_release_targets() {
        assert_eq!(
            NonoTarget::from_os_arch("macos", "aarch64"),
            Some(NonoTarget::Aarch64AppleDarwin)
        );
        assert_eq!(
            NonoTarget::from_os_arch("macos", "x86_64"),
            Some(NonoTarget::X8664AppleDarwin)
        );
        assert_eq!(
            NonoTarget::from_os_arch("linux", "x86_64"),
            Some(NonoTarget::X8664UnknownLinuxGnu)
        );
        assert_eq!(
            NonoTarget::from_os_arch("linux", "aarch64"),
            Some(NonoTarget::Aarch64UnknownLinuxGnu)
        );
    }

    #[test]
    fn windows_host_has_no_direct_target() {
        // nono runs inside WSL2 on Windows; the host is never a direct target.
        assert_eq!(NonoTarget::from_os_arch("windows", "x86_64"), None);
    }

    #[test]
    fn display_matches_triple() {
        assert_eq!(
            NonoTarget::Aarch64AppleDarwin.to_string(),
            "aarch64-apple-darwin"
        );
        assert_eq!(
            NonoTarget::X8664UnknownLinuxMusl.to_string(),
            "x86_64-unknown-linux-musl"
        );
    }
}
