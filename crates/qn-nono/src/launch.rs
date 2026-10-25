//! Host-owned launch association, fixed before native or capture activity (#108).
use crate::{NonoArgs, NonoError, RunArgs, RunMode, args::NonoSessionId};
use camino::Utf8Path;
use qn_core::SessionId;
use qn_infra::process::CmdSpec;
use std::{fmt, fs::OpenOptions, io::Write};

// Private pinned hook: nono v0.80.0 launch_runtime.rs uses this for foreground
// identity; proxy_runtime.rs binds capture metadata; exec_strategy.rs blocks it
// from the sandbox environment. Native regressions must pass on every upgrade.
const NATIVE_ID_ENV: &str = "NONO_DETACHED_SESSION_ID";

/// Immutable host association, independent of request data or observed native names.
/// The owner must retain one launch handle and never reuse its command for another run.
/// Reservations are permanent attempt markers, not proof of a live process or cleanup.
pub struct LaunchBinding {
    session: SessionId,
    native: NonoSessionId,
}

impl LaunchBinding {
    /// Publish a no-replace query association before spawn in a trusted audit directory.
    /// Call through a blocking task. Roots must remain outside all sandbox grants.
    /// The host owns directory durability/retention; this grants no lifecycle authority.
    /// # Errors
    /// Refuses duplicate records, invalid roots and write/sync failures.
    pub fn publish_audit(
        &self,
        audit_root: &Utf8Path,
        runtime_root: &Utf8Path,
    ) -> Result<(), NonoError> {
        crate::native_audit::association::publish(audit_root, runtime_root, self)
    }

    /// Reserve in an existing trusted, private control directory outside sandbox grants.
    /// Call through a blocking task. All launches must use the same control namespace.
    /// The no-replace file survives Drop and all later launch failures; never delete
    /// it to retry the same ID. Crash-durable directory publication is an owner duty.
    /// # Errors
    /// Rejects invalid roots, duplicate IDs, unavailable randomness and write/sync errors.
    pub fn reserve(root: &Utf8Path, session: SessionId) -> Result<Self, NonoError> {
        crate::args::validate_path(root, "launch control root")?;
        let mut bytes = [0; 8];
        getrandom::getrandom(&mut bytes).map_err(|source| NonoError::IdentityRandom { source })?;
        let native = NonoSessionId::try_from(hex::encode(bytes).as_str())?;
        let path = root.join(format!("{session}.launch"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::AlreadyExists {
                    NonoError::SessionReserved
                } else {
                    io_error(source)
                }
            })?;
        // A partial write also consumes the ID: unknown attempts are never reused.
        let record =
            serde_json::json!({"session_id": session.to_string(), "native_id": native.as_str()});
        file.write_all(record.to_string().as_bytes())
            .map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        Ok(Self { session, native })
    }

    /// qn identity reserved before native startup.
    #[must_use]
    pub fn session(&self) -> SessionId {
        self.session
    }

    /// Predetermined full native ID, suitable for host-side capture route binding.
    #[must_use]
    pub fn native_id(&self) -> &NonoSessionId {
        &self.native
    }

    /// Construct one foreground launch with empty parent environment and fixed identity.
    /// Caller verifies binary integrity/version and profile policy, adds only trusted
    /// HOME/XDG/PATH values, and executes through the owned infra process lifecycle.
    /// # Errors
    /// Rejects another qn name, detached launches and invalid binary/profile/argv paths.
    pub fn command(&self, binary: &Utf8Path, args: RunArgs) -> Result<CmdSpec, NonoError> {
        self.bound_command(binary, args, None)
    }

    /// Construct a bound launch using an approved fixed native credential proxy port.
    /// The host must use the same port in broker-validated Agent configuration;
    /// this is not port ownership proof. Native bind failure must stop startup.
    ///
    /// ```no_run
    /// # fn example(binding: &qn_nono::launch::LaunchBinding, binary: &camino::Utf8Path,
    /// # args: qn_nono::RunArgs, port: std::num::NonZeroU16) -> Result<(), qn_nono::NonoError> {
    /// let command = binding.command_with_proxy_port(binary, args, port)?;
    /// assert!(command.arg_values().iter().any(|arg| arg == "--proxy-port"));
    /// # Ok(()) }
    /// ```
    /// # Errors
    /// Preserves every identity, foreground and path check of command.
    pub fn command_with_proxy_port(
        &self,
        binary: &Utf8Path,
        args: RunArgs,
        port: std::num::NonZeroU16,
    ) -> Result<CmdSpec, NonoError> {
        self.bound_command(binary, args, Some(port))
    }

    fn bound_command(
        &self,
        binary: &Utf8Path,
        args: RunArgs,
        port: Option<std::num::NonZeroU16>,
    ) -> Result<CmdSpec, NonoError> {
        crate::args::validate_path(binary, "launch binary")?;
        if args.name != self.session || args.mode != RunMode::Foreground {
            return Err(NonoError::InvalidArgument {
                field: "launch binding",
            });
        }
        let args = match port {
            Some(port) => NonoArgs::run_with_proxy_port(args, port)?,
            None => NonoArgs::run(args)?,
        };
        Ok(CmdSpec::new(binary.as_str())
            .args(args.as_slice().iter().cloned())
            .env_clear()
            .env(NATIVE_ID_ENV, self.native.as_str()))
    }

    /// Check an observed association; a match alone grants no lifecycle authority.
    /// # Errors
    /// Rejects qn/native drift without adopting the observed identifier.
    pub fn verify(&self, session: SessionId, native: &NonoSessionId) -> Result<(), NonoError> {
        if session != self.session || native != &self.native {
            return Err(NonoError::Protocol {
                operation: "launch identity",
                reason: "prebound identity mismatch",
            });
        }
        Ok(())
    }
}
impl fmt::Debug for LaunchBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchBinding").finish_non_exhaustive()
    }
}
fn io_error(source: std::io::Error) -> NonoError {
    NonoError::Io {
        step: "reserving launch identity",
        source,
    }
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
