//! Versioned official-profile catalog and installed-registry contracts (#75/#59).
//!
//! A profile asset is plain nono profile JSON. Core carries it as an opaque,
//! bounded string: core never parses adapter policy syntax and never sees a
//! credential value. Installed versions are addressed by the canonical
//! `namespace/name` identity plus a strict `MAJOR.MINOR.PATCH` version.

use crate::ProfileRef;
use std::{fmt, num::ParseIntError};

/// Maximum accepted size of one nono profile asset.
pub const MAX_PROFILE_BYTES: usize = 64 * 1024;

/// Where a profile version was resolved from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileSource {
    /// Compiled into this qn build.
    Builtin,
    /// Installed under the host profile registry.
    Installed,
}

/// Strict `MAJOR.MINOR.PATCH` profile version without pre-release metadata.
///
/// ```
/// assert!(qn_core::profile::ProfileVersion::try_from("1.0.0").is_ok());
/// assert!(qn_core::profile::ProfileVersion::try_from("01.0.0").is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileVersion(String);

impl ProfileVersion {
    /// Borrow the canonical version text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for ProfileVersion {
    type Error = ProfileError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut parts = value.split('.');
        let mut parsed = [0_u32; 3];
        for slot in &mut parsed {
            let part = parts.next().ok_or(ProfileError::InvalidVersion)?;
            if part.is_empty() || part.len() > 10 || (part.len() > 1 && part.starts_with('0')) {
                return Err(ProfileError::InvalidVersion);
            }
            *slot = part
                .parse()
                .map_err(|_: ParseIntError| ProfileError::InvalidVersion)?;
        }
        if parts.next().is_some() {
            return Err(ProfileError::InvalidVersion);
        }
        // Reject non-canonical spellings such as "1.0.0+" or unicode digits.
        let canonical = format!("{}.{}.{}", parsed[0], parsed[1], parsed[2]);
        if canonical != value {
            return Err(ProfileError::InvalidVersion);
        }
        Ok(Self(canonical))
    }
}

impl fmt::Display for ProfileVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Bounded profile identity; contains no host path or credential material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileSummary {
    id: ProfileRef,
    version: ProfileVersion,
    source: ProfileSource,
}

impl ProfileSummary {
    /// Build a summary from already validated identity and version parts.
    #[must_use]
    pub fn new(id: ProfileRef, version: ProfileVersion, source: ProfileSource) -> Self {
        Self {
            id,
            version,
            source,
        }
    }

    /// Canonical `namespace/name` profile identity.
    #[must_use]
    pub fn id(&self) -> &ProfileRef {
        &self.id
    }

    /// Exact installed or compiled version.
    #[must_use]
    pub fn version(&self) -> &ProfileVersion {
        &self.version
    }

    /// Where this version came from.
    #[must_use]
    pub fn source(&self) -> ProfileSource {
        self.source
    }
}

impl fmt::Display for ProfileSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.id, self.version)
    }
}

/// One resolved profile asset plus its closed publication metadata.
///
/// `Debug` deliberately omits the asset body and all paths.
pub struct ResolvedProfile {
    summary: ProfileSummary,
    nono_profile: String,
    network_hosts: Vec<String>,
    state_dir: String,
}

impl ResolvedProfile {
    /// Validate bounded asset data before it crosses the adapter boundary.
    ///
    /// # Errors
    ///
    /// Rejects an empty or oversized asset, empty/duplicate hosts and a state
    /// directory that is not one relative path component.
    pub fn new(
        summary: ProfileSummary,
        nono_profile: String,
        network_hosts: Vec<String>,
        state_dir: String,
    ) -> Result<Self, ProfileError> {
        if nono_profile.is_empty() || nono_profile.len() > MAX_PROFILE_BYTES {
            return Err(ProfileError::InvalidCatalog);
        }
        if state_dir.is_empty()
            || state_dir.contains(['/', '\\', '\0'])
            || state_dir == "."
            || state_dir == ".."
        {
            return Err(ProfileError::InvalidCatalog);
        }
        let mut hosts = Vec::with_capacity(network_hosts.len());
        for host in network_hosts {
            if host.is_empty()
                || host.len() > 253
                || host.bytes().any(|byte| {
                    !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'*'))
                })
                || hosts.contains(&host)
            {
                return Err(ProfileError::InvalidCatalog);
            }
            hosts.push(host);
        }
        Ok(Self {
            summary,
            nono_profile,
            network_hosts: hosts,
            state_dir,
        })
    }

    /// Identity, version and source.
    #[must_use]
    pub fn summary(&self) -> &ProfileSummary {
        &self.summary
    }

    /// Exact nono profile JSON; never contains a credential value.
    #[must_use]
    pub fn nono_profile(&self) -> &str {
        &self.nono_profile
    }

    /// Proxy allowlist hosts declared by the asset.
    #[must_use]
    pub fn network_hosts(&self) -> &[String] {
        &self.network_hosts
    }

    /// Application state directory relative to the managed session home.
    #[must_use]
    pub fn state_dir(&self) -> &str {
        &self.state_dir
    }
}

impl fmt::Debug for ResolvedProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedProfile")
            .field("summary", &self.summary)
            .field("asset_bytes", &self.nono_profile.len())
            .field("host_count", &self.network_hosts.len())
            .finish_non_exhaustive()
    }
}

/// Read-only and publication operations on the host profile registry.
///
/// Implementations perform bounded local filesystem work. They never launch an
/// engine, resolve a credential or widen a sandbox grant.
pub trait ProfileRegistry: Send + Sync {
    /// List compiled profiles and every installed version, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError`] when the registry cannot be read safely.
    fn list(&self) -> Result<Vec<ProfileSummary>, ProfileError>;

    /// Resolve the current version, falling back to the compiled catalog.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError`] for unknown identities, missing installs and
    /// integrity failures.
    fn resolve(&self, id: ProfileRef) -> Result<ResolvedProfile, ProfileError>;

    /// Install the compiled version into the host registry.
    ///
    /// Re-installing identical bytes is idempotent; different bytes for an
    /// existing version are refused.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError`] for unknown identities, conflicts and IO errors.
    fn install(&self, id: ProfileRef) -> Result<ProfileSummary, ProfileError>;

    /// Select the previous verified version as current.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError`] when no earlier version exists or integrity
    /// verification fails.
    fn rollback(&self, id: ProfileRef) -> Result<ProfileSummary, ProfileError>;
}

/// Profile-catalog and registry failures; no variant echoes untrusted payload.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    /// A version is not canonical `MAJOR.MINOR.PATCH`.
    #[error("invalid profile version; use MAJOR.MINOR.PATCH without leading zeros")]
    InvalidVersion,
    /// The compiled catalog or one asset failed validation.
    #[error("official profile catalog is invalid; reinstall qn and run 'qn profile list'")]
    InvalidCatalog,
    /// No compiled or installed profile has this identity.
    #[error("unknown profile; run 'qn profile list' to see available profiles")]
    UnknownProfile,
    /// The registry root is not an absolute path.
    #[error("profile registry root must be absolute; set QN_HOME to a private directory")]
    InvalidRoot,
    /// A supplied asset path is not absolute.
    #[error("profile asset path must be absolute; use a path from the trusted host location")]
    InvalidAssetPath,
    /// An existing version has different bytes.
    #[error("profile version already exists with different content; publish a new version")]
    Conflict,
    /// The profile has not been installed into the host registry.
    #[error("profile is not installed; run 'qn profile install <id>' first")]
    NotInstalled,
    /// No earlier version is available.
    #[error("no earlier profile version is available; publish a new version first")]
    NoRollback,
    /// Stored bytes do not match the recorded digest.
    #[error("installed profile failed its integrity check; reinstall it")]
    Integrity,
    /// Bounded local storage could not complete the operation.
    #[error("profile registry storage failed; check qn home permissions and retry")]
    Io(#[source] std::io::Error),
    /// The managed engine is missing or failed its integrity/version inspection.
    #[error("managed engine is unavailable; run 'qn install' first")]
    Engine(#[source] crate::setup::SetupError),
    /// The pinned engine rejected the profile or could not be executed.
    #[error("native profile validation failed; inspect the pinned engine and profile")]
    Validation(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// The native validation command could not be constructed from the asset.
    #[error("native profile validation is unavailable; check the managed profile")]
    ValidationUnavailable,
}
