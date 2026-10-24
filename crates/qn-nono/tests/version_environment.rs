//! Harness-free fixture so `verify_version` can invoke this executable with `--version`.
//! A separate parent carries a synthetic canary; no host environment is mutated.
use std::{error::Error, time::Duration};

use camino::Utf8PathBuf;
use qn_infra::process::{self, CmdSpec};
use qn_nono::{LOCKED_NONO_VERSION, verify_version};

const MARKER: &str = "QN_TEST_VERSION_SECRET";
const SYNTHETIC_VALUE: &str = "synthetic-version-canary";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let executable = Utf8PathBuf::from_path_buf(std::env::current_exe()?)
        .map_err(|_| "test executable path must be UTF-8")?;
    match std::env::args().nth(1).as_deref() {
        Some("--version") => {
            if std::env::var_os(MARKER).is_some() {
                return Err("inherited marker reached the version subprocess".into());
            }
            println!("nono {LOCKED_NONO_VERSION}");
        }
        Some("--probe-parent") => {
            if std::env::var(MARKER)? != SYNTHETIC_VALUE {
                return Err("parent must receive the synthetic canary".into());
            }
            verify_version(&executable).await?;
        }
        _ => {
            let spec = CmdSpec::new(executable.as_str())
                .arg("--probe-parent")
                .env(MARKER, SYNTHETIC_VALUE)
                .timeout(Duration::from_secs(15))
                .label("version environment regression");
            let output = process::run(&spec).await?;
            if !output.success || output.stdout_truncated || output.stderr_truncated {
                return Err("version environment boundary regression failed".into());
            }
        }
    }
    Ok(())
}
