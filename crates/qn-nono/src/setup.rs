//! Compose the existing pinned installer and version probe for real setup consumers.
use crate::{InstallOptions, NonoError, NonoTarget};
use camino::{Utf8Path, Utf8PathBuf};
use qn_core::setup::{EngineInstallation, EngineSetup, SetupError, SetupFuture, SetupOperation};
use std::fmt;

/// Host-specific managed installation; never resolves an executable through PATH.
pub struct NonoSetup {
    options: InstallOptions,
}
impl NonoSetup {
    /// Bind the host target under a trusted cache namespace, without filesystem writes.
    /// # Errors
    /// Rejects relative/NUL roots and hosts requiring a separate installation route.
    pub fn new(cache: &Utf8Path) -> Result<Self, SetupError> {
        if !cache.is_absolute() || cache.as_str().contains('\0') {
            return Err(SetupError::Configuration);
        }
        let target = NonoTarget::current().ok_or(SetupError::Unsupported)?;
        Ok(Self {
            options: InstallOptions::new(cache.join(target.triple())).with_target(target),
        })
    }
}
impl EngineSetup for NonoSetup {
    fn run(&self, operation: SetupOperation) -> SetupFuture<'_> {
        let options = self.options.clone();
        Box::pin(async move {
            let worker = tokio::task::spawn_blocking(move || match operation {
                SetupOperation::Install => crate::install(&options),
                SetupOperation::Inspect => crate::installer::verify_installation(&options),
            });
            let installed = worker
                .await
                .map_err(|error| SetupError::Failed(Box::new(SetupWorkerCause(error))))?
                .map_err(failure)?;
            let path = Utf8PathBuf::from_path_buf(installed.path)
                .map_err(|_| SetupError::Configuration)?;
            crate::verify_version(&path).await.map_err(failure)?;
            Ok(EngineInstallation {
                engine: "nono",
                version: installed.version,
                path,
            })
        })
    }
}
// A panicking worker can carry arbitrary text; retain the cause privately.
struct SetupWorkerCause(tokio::task::JoinError);
impl fmt::Display for SetupWorkerCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_cancelled() {
            "installation worker cancelled"
        } else {
            "installation worker did not complete"
        })
    }
}
impl fmt::Debug for SetupWorkerCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for SetupWorkerCause {}

// HTTP/provider libraries may put proxy authentication or child text in errors.
// Keep the original cause private; expose only a closed operational category.
struct SetupCause(NonoError);
impl fmt::Display for SetupCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match &self.0 {
            NonoError::InstallationIntegrity | NonoError::ChecksumMismatch { .. } => {
                "installation integrity check failed"
            }
            NonoError::DownloadFailed { .. } => {
                "release download failed; check host network or proxy configuration"
            }
            NonoError::Io { .. } => "managed installation IO failed; check directory access",
            NonoError::VersionMismatch { .. }
            | NonoError::InvalidVersionOutput
            | NonoError::VersionProbeFailed { .. } => {
                "pinned version probe rejected the executable"
            }
            NonoError::Process { .. } => {
                "engine process probe failed; check executable permissions and host support"
            }
            _ => "pinned release setup rejected",
        })
    }
}
impl fmt::Debug for SetupCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for SetupCause {}
fn failure(error: NonoError) -> SetupError {
    SetupError::Failed(Box::new(SetupCause(error)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    #[test]
    fn it_redacts_all_error_formats_and_sources() {
        let error = failure(NonoError::Io {
            step: "sensitive-canary",
            source: std::io::Error::other("sensitive-canary"),
        });
        let mut source: Option<&dyn Error> = Some(&error);
        while let Some(error) = source {
            assert!(!format!("{error} {error:?}").contains("canary"));
            source = error.source();
        }
        assert!(matches!(
            NonoSetup::new(Utf8Path::new("relative")),
            Err(SetupError::Configuration)
        ));
    }
}
