//! Shared launch boundary; authorization and filesystem checks remain in consumers.
use crate::{Error, Result, SessionId};
use camino::{Utf8Path, Utf8PathBuf};
use std::fmt;
use uuid::{Uuid, Variant};

/// Canonical managed profile identifier, not an arbitrary file path.
/// ```
/// let profile = qn_core::ProfileRef::try_from("qn/claude-code")?;
/// assert_eq!(profile.as_str(), "qn/claude-code");
/// # Ok::<(), qn_core::Error>(())
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ProfileRef(String);
impl ProfileRef {
    /// Borrow the identifier for registry lookup; this does not authorize its use.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<&str> for ProfileRef {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        fn component(value: &str) -> bool {
            value.len() <= 64
                && value
                    .bytes()
                    .next()
                    .is_some_and(|b| b.is_ascii_alphanumeric())
                && value
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        }
        let (namespace, name) = value.split_once('/').ok_or(Error::InvalidProfileRef)?;
        if !component(namespace) || !component(name) {
            return Err(Error::InvalidProfileRef);
        }
        Ok(Self(value.into()))
    }
}
impl fmt::Display for ProfileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl fmt::Debug for ProfileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProfileRef(..)")
    }
}

/// Opaque grant record identity; the broker must authenticate and validate it.
/// ```
/// let handle = qn_core::GrantHandle::new();
/// assert_eq!(qn_core::GrantHandle::try_from(handle.to_string().as_str())?, handle);
/// # Ok::<(), qn_core::Error>(())
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrantHandle(Uuid);
impl GrantHandle {
    /// Allocate an identity; this does not create or authorize a grant.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}
impl Default for GrantHandle {
    fn default() -> Self {
        Self::new()
    }
}
impl TryFrom<&str> for GrantHandle {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        let raw = value
            .strip_prefix("grt-")
            .ok_or(Error::InvalidGrantHandle)?;
        let uuid = Uuid::parse_str(raw).map_err(|_| Error::InvalidGrantHandle)?;
        if uuid.get_version_num() != 7
            || uuid.get_variant() != Variant::RFC4122
            || uuid.to_string() != raw
        {
            return Err(Error::InvalidGrantHandle);
        }
        Ok(Self(uuid))
    }
}
impl fmt::Display for GrantHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "grt-{}", self.0)
    }
}
impl fmt::Debug for GrantHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GrantHandle(..)")
    }
}

/// Structurally validated launch request; contains no environment/credential-value API.
///
/// The adapter still checks profile provenance, directory ownership and live grant scope.
/// ```
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let cwd = camino::Utf8PathBuf::from_path_buf(std::env::current_dir()?).map_err(|_| "UTF-8 cwd required")?;
/// let spec = qn_core::RunSpec::new(qn_core::SessionId::new(),
///     qn_core::ProfileRef::try_from("qn/codex")?, vec!["codex".into()], cwd)?;
/// assert!(spec.grant().is_none());
/// # Ok(()) } example().unwrap();
/// ```
#[derive(Clone)]
pub struct RunSpec {
    id: SessionId,
    profile: ProfileRef,
    command: Vec<String>,
    workdir: Utf8PathBuf,
    grant: Option<GrantHandle>,
}
impl RunSpec {
    /// Validate launch structure without filesystem access or process execution.
    /// # Errors
    /// Rejects relative/NUL workdirs, an empty program and NUL in any argument.
    pub fn new(
        id: SessionId,
        profile: ProfileRef,
        command: Vec<String>,
        workdir: Utf8PathBuf,
    ) -> Result<Self> {
        if !workdir.is_absolute() || workdir.as_str().contains('\0') {
            return Err(Error::InvalidRunInput { field: "workdir" });
        }
        if command.first().is_none_or(String::is_empty)
            || command.iter().any(|arg| arg.contains('\0'))
        {
            return Err(Error::InvalidRunInput { field: "command" });
        }
        Ok(Self {
            id,
            profile,
            command,
            workdir,
            grant: None,
        })
    }
    /// Attach an opaque reference; consumers must validate the actual grant.
    #[must_use]
    pub fn with_grant(mut self, grant: GrantHandle) -> Self {
        self.grant = Some(grant);
        self
    }
    /// Identity allocated before grant creation/startup.
    #[must_use]
    pub fn id(&self) -> SessionId {
        self.id
    }
    /// Managed profile reference for registry lookup.
    #[must_use]
    pub fn profile(&self) -> &ProfileRef {
        &self.profile
    }
    /// Literal executable and arguments; never shell-interpolated by core.
    #[must_use]
    pub fn command(&self) -> &[String] {
        &self.command
    }
    /// Absolute working directory; physical-path checks belong to the adapter.
    #[must_use]
    pub fn workdir(&self) -> &Utf8Path {
        &self.workdir
    }
    /// Optional grant identity, never the underlying credential.
    #[must_use]
    pub fn grant(&self) -> Option<GrantHandle> {
        self.grant
    }
}
impl fmt::Debug for RunSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunSpec")
            .field("id", &self.id)
            .field("argument_count", &self.command.len())
            .field("has_grant", &self.grant.is_some())
            .finish_non_exhaustive()
    }
}
