//! Verify the CLI version contract observed with nono release 0.80.0 (#41).

use std::time::Duration;

use camino::Utf8Path;
use qn_infra::process::{self, CmdOutcome, CmdSpec};

use crate::{LOCKED_NONO_VERSION, NonoError};

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_OUTPUT_LIMIT: usize = 1024;

/// Check that a managed executable reports exactly the workspace's pinned version.
///
/// The caller supplies an absolute path: this never searches PATH or installs a
/// binary. This read-only probe neither launches a sandbox nor verifies file
/// integrity; successful version output is not a substitute for SHA-256 checks.
///
/// # Errors
///
/// Returns a typed error for relative paths, process failures, unsuccessful or
/// truncated output, malformed responses, and versions differing from the pin.
///
/// # Example
///
/// ```no_run
/// # async fn probe() -> Result<(), qn_nono::NonoError> {
/// use camino::Utf8Path;
/// qn_nono::verify_version(Utf8Path::new("/opt/qn/0.80.0/nono")).await?;
/// # Ok(())
/// # }
/// ```
pub async fn verify_version(binary: &Utf8Path) -> Result<(), NonoError> {
    if !binary.is_absolute() {
        return Err(NonoError::RelativeExecutable);
    }
    let command = CmdSpec::new(binary.as_str())
        .arg("--version")
        .timeout(PROBE_TIMEOUT)
        .max_output_bytes(PROBE_OUTPUT_LIMIT)
        .label("nono version probe");
    validate_output(&process::run(&command).await?)
}

fn validate_output(output: &CmdOutcome) -> Result<(), NonoError> {
    if !output.success || output.exit_code != Some(0) {
        return Err(NonoError::VersionProbeFailed {
            code: output.exit_code,
        });
    }
    if output.stdout_truncated || output.stderr_truncated {
        return Err(NonoError::InvalidVersionOutput);
    }
    let version = output
        .stdout
        .trim_end_matches(['\r', '\n'])
        .strip_prefix("nono ")
        .filter(|version| !version.is_empty() && !version.contains(char::is_whitespace))
        .ok_or(NonoError::InvalidVersionOutput)?;
    if version != LOCKED_NONO_VERSION {
        return Err(NonoError::VersionMismatch {
            expected: LOCKED_NONO_VERSION,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_output, verify_version};
    use crate::{LOCKED_NONO_VERSION, NonoError};
    use camino::Utf8Path;
    use qn_infra::process::CmdOutcome;
    use std::time::Duration;

    fn output(stdout: &str) -> CmdOutcome {
        CmdOutcome {
            exit_code: Some(0),
            success: true,
            stdout: stdout.to_owned(),
            stderr: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
            duration: Duration::ZERO,
        }
    }

    #[test]
    fn it_accepts_the_pinned_version_with_cli_line_endings() {
        for suffix in ["", "\n", "\r\n"] {
            let value = format!("nono {LOCKED_NONO_VERSION}{suffix}");
            assert!(validate_output(&output(&value)).is_ok(), "valid CLI output");
        }
    }

    #[test]
    fn it_rejects_other_versions_without_echoing_untrusted_output() {
        for version in ["0.79.0", "0.80.0-dev", "synthetic-sensitive-value"] {
            let result = validate_output(&output(&format!("nono {version}\n")));
            assert!(matches!(result, Err(NonoError::VersionMismatch { .. })));
            if let Err(error) = result {
                assert!(!format!("{error} {error:?}").contains(version));
            }
        }
    }

    #[test]
    fn it_rejects_malformed_or_mixed_output() {
        for value in [
            "",
            "nono ",
            "0.80.0",
            "other 0.80.0",
            "nono 0.80.0 extra",
            "warning\nnono 0.80.0",
            "nono 0.80.0\nextra",
        ] {
            assert!(
                matches!(
                    validate_output(&output(value)),
                    Err(NonoError::InvalidVersionOutput)
                ),
                "reject {value:?}"
            );
        }
    }

    #[test]
    fn it_rejects_nonzero_and_signal_exit_even_with_valid_stdout() {
        for code in [Some(7), None] {
            let mut value = output(&format!("nono {LOCKED_NONO_VERSION}"));
            value.success = false;
            value.exit_code = code;
            assert!(matches!(validate_output(&value),
                Err(NonoError::VersionProbeFailed { code: actual }) if actual == code));
        }
    }

    #[test]
    fn it_rejects_truncated_stdout_or_stderr() {
        for (stdout, stderr) in [(true, false), (false, true)] {
            let mut value = output(&format!("nono {LOCKED_NONO_VERSION}"));
            value.stdout_truncated = stdout;
            value.stderr_truncated = stderr;
            assert!(matches!(
                validate_output(&value),
                Err(NonoError::InvalidVersionOutput)
            ));
        }
    }

    #[tokio::test]
    async fn it_rejects_relative_paths_before_spawning() {
        assert!(matches!(
            verify_version(Utf8Path::new("nono")).await,
            Err(NonoError::RelativeExecutable)
        ));
    }

    #[tokio::test]
    async fn it_preserves_the_missing_executable_error() {
        let path = std::env::temp_dir()
            .join(format!("qn-missing-{}", std::process::id()))
            .join("missing-nono");
        let Some(path) = Utf8Path::from_path(&path) else {
            panic!("UTF-8 temp path required")
        };
        assert!(matches!(verify_version(path).await,
            Err(NonoError::Process { source: qn_infra::Error::Spawn { source, .. } })
                if source.kind() == std::io::ErrorKind::NotFound));
    }
}
