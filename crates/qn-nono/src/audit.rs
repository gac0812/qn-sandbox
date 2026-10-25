//! Isolated native audit queries, based on the measured 0.80.0 schema (#46/#68).
use crate::{
    NonoArgs, NonoError,
    args::{NonoSessionId, validate_path},
    verify_version, wire,
};
use camino::Utf8PathBuf;
use qn_infra::process::{CmdSpec, ManagedOutput, ProcessEnd, ProcessPolicy, spawn_managed};
use std::{fmt, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

pub use crate::wire::{AuditRecord, AuditSummary, NetworkEvent};

/// Reads native audit records under a caller-provisioned managed root.
///
/// The binary must already be checksum-verified by the installer. Queries check
/// its version but do not prove integrity or authenticity of historical events.
/// Create home/config/state/cache/tmp subdirectories outside the agent workspace.
///
/// ```no_run
/// # async fn example(policy: std::sync::Arc<dyn qn_infra::process::ProcessPolicy>) -> Result<(), qn_nono::NonoError> {
/// use qn_nono::audit::AuditReader;
/// let reader = AuditReader::new("/opt/qn/nono".into(), "/opt/qn/managed".into(), policy)?;
/// let records = reader.list().await?;
/// assert!(records.iter().all(|record| record.session_id.len() == 16));
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct AuditReader {
    binary: Utf8PathBuf,
    root: Utf8PathBuf,
    policy: Arc<dyn ProcessPolicy>,
}
impl fmt::Debug for AuditReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditReader").finish_non_exhaustive()
    }
}

impl AuditReader {
    /// Bind a managed binary/root and injected process policy without executing it.
    /// # Errors
    /// Rejects relative paths and embedded NUL before any child is spawned.
    pub fn new(
        binary: Utf8PathBuf,
        root: Utf8PathBuf,
        policy: Arc<dyn ProcessPolicy>,
    ) -> Result<Self, NonoError> {
        validate_path(&binary, "audit executable path")?;
        validate_path(&root, "managed state root")?;
        Ok(Self {
            binary,
            root,
            policy,
        })
    }

    /// Read native audit metadata; no state/denial inference is performed.
    /// # Errors
    /// Rejects unsuccessful, truncated or malformed responses and version drift.
    pub async fn list(&self) -> Result<Vec<AuditSummary>, NonoError> {
        let output = self.query(NonoArgs::audit_list(), "audit list").await?;
        let records = wire::audit_list(output.output())?;
        for record in &records {
            NonoSessionId::try_from(record.session_id.as_str())?;
        }
        Ok(records)
    }

    /// Read one full native ID and verify that the returned record matches it.
    /// # Errors
    /// Returns query/protocol errors or refuses a mismatched record identity.
    pub async fn show(&self, id: &NonoSessionId) -> Result<AuditRecord, NonoError> {
        let output = self.query(NonoArgs::audit_show(id), "audit show").await?;
        let record = wire::audit(output.output())?;
        if &NonoSessionId::try_from(record.session_id.as_str())? != id {
            return Err(NonoError::Protocol {
                operation: "audit show",
                reason: "record identity mismatch",
            });
        }
        Ok(record)
    }

    pub(crate) async fn query(
        &self,
        args: NonoArgs,
        operation: &'static str,
    ) -> Result<ManagedOutput, NonoError> {
        verify_version(&self.binary).await?;
        let mut spec = CmdSpec::new(self.binary.as_str())
            .args(args.as_slice().iter().cloned())
            .env_clear()
            .cwd(self.root.clone())
            .timeout(Duration::from_secs(5))
            .max_output_bytes(1 << 20);
        for (key, directory) in [
            ("HOME", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_CACHE_HOME", "cache"),
            ("TMPDIR", "tmp"),
        ] {
            spec = spec.env(key, self.root.join(directory).as_str());
        }
        let output = spawn_managed(&spec, self.policy.as_ref())?
            .wait(CancellationToken::new())
            .await?;
        if output.end == ProcessEnd::TimedOut {
            return Err(NonoError::CommandTimedOut { operation });
        }
        if output.end != ProcessEnd::Exited || !output.output().success {
            return Err(NonoError::CommandFailed {
                operation,
                code: output.output().exit_code,
            });
        }
        Ok(output)
    }
}
