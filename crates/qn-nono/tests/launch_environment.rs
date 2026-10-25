//! Real-child environment regression for prebound launch commands (#108).
use camino::Utf8Path;
use qn_core::SessionId;
use qn_infra::process::{self, CmdSpec};
use qn_nono::{RunArgs, RunMode, launch::LaunchBinding};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let executable = std::env::current_exe()?;
    let executable = Utf8Path::from_path(&executable).ok_or("UTF-8 path required")?;
    match std::env::args().nth(1).as_deref() {
        Some("run") => {
            assert!(std::env::var_os("QN_LAUNCH_CANARY").is_none());
            let native = std::env::var("NONO_DETACHED_SESSION_ID")?;
            assert_ne!(native, "inherited-identity");
            assert!(qn_nono::args::NonoSessionId::try_from(native.as_str()).is_ok());
            println!("{native}");
        }
        Some("--parent") => {
            assert_eq!(std::env::var("QN_LAUNCH_CANARY")?, "synthetic-parent-value");
            let directory = tempfile::tempdir()?;
            let root = Utf8Path::from_path(directory.path()).ok_or("UTF-8 root required")?;
            let binding = LaunchBinding::reserve(root, SessionId::new())?;
            let command = binding.command(
                executable,
                RunArgs {
                    name: binding.session(),
                    profile: root.join("profile.json"),
                    workdir: root.to_owned(),
                    mode: RunMode::Foreground,
                    command: vec!["synthetic-agent".into()],
                },
            )?;
            let output = process::run(&command).await?;
            assert!(output.success && !output.stdout_truncated && !output.stderr_truncated);
            assert_eq!(output.stdout.trim(), binding.native_id().as_str());
        }
        _ => {
            let output = process::run(
                &CmdSpec::new(executable.as_str())
                    .arg("--parent")
                    .env("QN_LAUNCH_CANARY", "synthetic-parent-value")
                    .env("NONO_DETACHED_SESSION_ID", "inherited-identity"),
            )
            .await?;
            assert!(
                output.success,
                "launch environment fixture failed; output redacted"
            );
        }
    }
    Ok(())
}
