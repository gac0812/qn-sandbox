//! Closed qn routing metadata; application renderers remain separate (#84).
use camino::Utf8Path;
use qn_core::{GrantHandle, SessionId};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fmt, io::Write, num::NonZeroU16};

const MAX_BYTES: usize = 4096;
const FILE_NAME: &str = "routing.json";

/// Trusted routing identities supplied by the authenticated host composition root.
/// Construction does not authenticate a caller, authorize a grant or probe a port.
#[derive(Clone, Copy)]
pub struct ManagedConfigBinding {
    session: SessionId,
    grant: GrantHandle,
    port: NonZeroU16,
}
impl ManagedConfigBinding {
    /// Bind already-authorized identities to an approved local proxy listener.
    #[must_use]
    pub fn new(session: SessionId, grant: GrantHandle, port: NonZeroU16) -> Self {
        Self {
            session,
            grant,
            port,
        }
    }
}
impl fmt::Debug for ManagedConfigBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ManagedConfigBinding(..)")
    }
}

/// Publication failure with no untrusted configuration values in its formatting.
#[derive(Debug, thiserror::Error)]
pub enum ManagedConfigError {
    /// The input exceeds the parser's bounded allocation budget.
    #[error("managed config exceeds 4096 bytes; provide only the approved routing fields")]
    TooLarge,
    /// Malformed or non-closed JSON; parser text is redacted at the source boundary.
    #[error("invalid managed config JSON; use the closed routing schema: {0}")]
    Json(#[from] ManagedConfigJsonError),
    /// A field does not match its trusted binding or schema version.
    #[error("managed config binding mismatch; regenerate it from the authorized session")]
    BindingMismatch,
    /// Publication must never resolve against an ambient working directory.
    #[error("managed config root must be absolute; supply a private host-controlled directory")]
    InvalidRoot,
    /// Filesystem failure; the previous target remains intact before publication.
    #[error("managed config could not be published; check the private directory: {0}")]
    Io(#[from] std::io::Error),
}

/// Retains the parser cause without exposing field names, values or parser text.
/// Source traversal deliberately terminates here because serde errors echo input.
pub struct ManagedConfigJsonError(serde_json::Error);
impl fmt::Display for ManagedConfigJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at line {}, column {}",
            self.0.classify(),
            self.0.line(),
            self.0.column()
        )
    }
}
impl fmt::Debug for ManagedConfigJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for ManagedConfigJsonError {}
impl From<serde_json::Error> for ManagedConfigError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json(ManagedConfigJsonError(source))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u8,
    session: String,
    grant: String,
    proxy_port: u16,
}

/// Validate and atomically replace routing.json inside a private host directory.
///
/// Input contains exactly version (1), session, grant and `proxy_port`. The port
/// denotes a numeric loopback listener; no URL, path, env or headers are accepted.
/// All parsing/binding checks finish before any file is created. The caller must
/// keep this directory inaccessible to the sandbox and supply a live authorized
/// binding. Grant revocation is enforced per request by the broker, not this file.
/// This internal manifest is not an Agent/MCP application's configuration format.
///
/// This is blocking filesystem IO; asynchronous callers must use `spawn_blocking`.
/// Atomic replacement does not promise power-loss durability of the directory.
/// # Errors
/// Rejects oversized, malformed, unknown/duplicate fields and mismatched bindings.
/// IO failures before replacement leave any existing target unchanged.
/// tempfile cleans up unpublished temporary files on ordinary error paths.
pub fn write_managed_config(
    root: &Utf8Path,
    binding: &ManagedConfigBinding,
    input: &[u8],
) -> Result<(), ManagedConfigError> {
    let manifest: Manifest = decode(input)?;
    if manifest.version != 1
        || manifest.session != binding.session.to_string()
        || manifest.grant != binding.grant.to_string()
        || manifest.proxy_port != binding.port.get()
    {
        return Err(ManagedConfigError::BindingMismatch);
    }
    let serialized = serde_json::to_vec(&manifest)?;
    publish(root, FILE_NAME, &serialized, Publication::Replace)
}

pub(crate) fn decode<T: DeserializeOwned>(input: &[u8]) -> Result<T, ManagedConfigError> {
    if input.len() > MAX_BYTES {
        return Err(ManagedConfigError::TooLarge);
    }
    Ok(serde_json::from_slice(input)?)
}

#[derive(Clone, Copy)]
pub(crate) enum Publication {
    Replace,
    Create,
}

pub(crate) fn publish(
    root: &Utf8Path,
    name: &str,
    serialized: &[u8],
    mode: Publication,
) -> Result<(), ManagedConfigError> {
    if !root.is_absolute() {
        return Err(ManagedConfigError::InvalidRoot);
    }
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    file.write_all(serialized)?;
    file.as_file().sync_all()?;
    match mode {
        Publication::Replace => file.persist(root.join(name)),
        Publication::Create => file.persist_noclobber(root.join(name)),
    }
    .map_err(|error| ManagedConfigError::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
#[path = "managed_config_tests.rs"]
mod tests;
