//! Trusted host command capture, matching nono 0.80.0 text-output providers.
//! Source: nono v0.80.0 docs/profile-authoring-guide.md, `credential_capture`.
use camino::Utf8PathBuf;
use qn_infra::process::{CmdSpec, ProcessEnd, ProcessPolicy, spawn_managed};
use std::{fmt, time::Duration};
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;
const MAX_OUTPUT: usize = 16 * 1024;
/// Host command failures never include command arguments or captured output.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Invalid trusted configuration.
    #[error(
        "invalid provider configuration; use absolute host paths, an allowed environment key and a 1–300 second timeout"
    )]
    Configuration,
    /// Managed executor failure with redacted lifecycle context.
    #[error("host provider execution failed; inspect its installation and lifecycle: {0}")]
    Process(#[from] qn_infra::Error),
    /// Cancellation was requested before output admission.
    #[error("host provider cancelled; check the session grant before retrying")]
    Cancelled,
    /// The trusted provider exceeded its configured deadline.
    #[error("host provider timed out; check the host credential backend")]
    Timeout,
    /// Nonzero exit without exposing stderr.
    #[error("host provider failed; authenticate with the trusted host credential backend")]
    Failed,
    /// Incomplete or invalid secret output.
    #[error(
        "host provider output rejected; return one complete nonempty UTF-8 secret without control characters"
    )]
    Output,
}
/// An owned host-only secret. Never place it in sandbox configuration or logs.
pub struct ProviderSecret(Zeroizing<String>);
impl ProviderSecret {
    /// Borrow for encrypted host delivery to nono; this does not authorize a grant.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for ProviderSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProviderSecret([REDACTED])")
    }
}
/// Reusable trusted host command, with no cache and no inherited environment.
/// Program, arguments and environment must originate in trusted host configuration,
/// never sandbox metadata. The caller authorizes before IO and again before delivery.
pub struct CommandProvider {
    spec: CmdSpec,
}
impl fmt::Debug for CommandProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CommandProvider([REDACTED])")
    }
}
impl CommandProvider {
    /// Configure an absolute host executable/home and a 1–300 second timeout.
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = qn_broker::provider::CommandProvider::new(
    ///     std::env::current_exe()?.try_into()?, std::env::current_dir()?.try_into()?,
    ///     std::time::Duration::from_secs(5))?;
    /// # Ok(()) }
    /// ```
    /// # Errors
    /// Rejects relative paths, NUL and invalid timeout; paths remain host authority.
    pub fn new(
        program: Utf8PathBuf,
        home: Utf8PathBuf,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        if !program.is_absolute()
            || !home.is_absolute()
            || program.as_str().contains('\0')
            || home.as_str().contains('\0')
            || !(Duration::from_secs(1)..=Duration::from_secs(300)).contains(&timeout)
        {
            return Err(ProviderError::Configuration);
        }
        Ok(Self {
            spec: CmdSpec::new(program.into_string())
                .env_clear()
                .env("HOME", home.as_str())
                .cwd(home)
                .timeout(timeout)
                .max_output_bytes(MAX_OUTPUT),
        })
    }
    /// Add one trusted nonsecret argument (for example `auth` or a credential reference).
    /// Never pass a credential value or sandbox-supplied argument here.
    #[must_use]
    pub fn arg(mut self, argument: impl Into<String>) -> Self {
        self.spec = self.spec.arg(argument);
        self
    }
    /// Add an explicit nonsecret host setting; no credential/proxy inheritance.
    /// # Errors
    /// Only PATH, LANG, `LC_ALL` and Windows `SystemRoot` are accepted; NUL is rejected.
    pub fn environment(mut self, name: &str, value: &str) -> Result<Self, ProviderError> {
        if !matches!(name, "PATH" | "LANG" | "LC_ALL" | "SystemRoot") || value.contains('\0') {
            return Err(ProviderError::Configuration);
        }
        self.spec = self.spec.env(name, value);
        Ok(self)
    }
    /// Read one raw secret from the host command; stdin closes immediately.
    /// Uses the existing managed executor, which never logs argv/env/output.
    /// # Errors
    /// Rejects cancellation, timeout, failure, truncation and invalid text.
    /// #93 lifecycle limits apply; completion does not prove a sandbox stopped.
    pub async fn capture(
        &self,
        policy: &dyn ProcessPolicy,
        cancel: CancellationToken,
    ) -> Result<ProviderSecret, ProviderError> {
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        let output = spawn_managed(&self.spec, policy)?
            .wait(cancel.clone())
            .await?;
        if cancel.is_cancelled() || output.end == ProcessEnd::Cancelled {
            return Err(ProviderError::Cancelled);
        }
        if output.end == ProcessEnd::TimedOut {
            return Err(ProviderError::Timeout);
        }
        let outcome = output.output();
        if !outcome.success {
            return Err(ProviderError::Failed);
        }
        if outcome.stdout_truncated || outcome.stderr_truncated {
            return Err(ProviderError::Output);
        }
        let value = std::str::from_utf8(&outcome.stdout_raw).map_err(|_| ProviderError::Output)?;
        let value = value.trim();
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(ProviderError::Output);
        }
        Ok(ProviderSecret(Zeroizing::new(value.to_owned())))
    }
}
