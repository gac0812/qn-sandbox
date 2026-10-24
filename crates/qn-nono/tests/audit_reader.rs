//! Short-lived Rust/native query fixture; production injects bootstrap's group policy.
#![allow(clippy::expect_used)]
use camino::Utf8PathBuf;
use process_wrap::tokio::TokioCommandWrap;
use qn_infra::process::{self, CmdSpec, ProcessPolicy, TerminationSafety};
use qn_nono::{NonoError, args::NonoSessionId, audit::AuditReader};
use std::{error::Error, fs, sync::Arc};

struct QueryPolicy;
impl ProcessPolicy for QueryPolicy {
    fn configure(&self, _: &mut TokioCommandWrap) -> TerminationSafety {
        TerminationSafety::WhileParentOwned
    }
}
const ID: &str = "0123456789abcdef";
const LIST: &str = include_str!("fixtures/nono-0.80.0/audit-list.json");
const SHOW: &str = include_str!("fixtures/nono-0.80.0/audit-show.json");

fn path(path: std::path::PathBuf) -> Result<Utf8PathBuf, Box<dyn Error>> {
    Utf8PathBuf::from_path_buf(path).map_err(|_| "UTF-8 test path required".into())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let binary = path(std::env::current_exe()?)?;
    match std::env::args().nth(1).as_deref() {
        Some("--version") => {
            assert!(std::env::var_os("QN_AUDIT_CANARY").is_none());
            let version = if binary.file_stem() == Some("bad-version") {
                "0.79.0"
            } else {
                qn_nono::LOCKED_NONO_VERSION
            };
            println!("nono {version}");
        }
        Some("audit") => fake_query()?,
        Some("--parent") => verify(binary).await?,
        _ if std::env::var_os("QN_TEST_NONO").is_some() => {
            let reader = AuditReader::new(
                std::env::var("QN_TEST_NONO")?.into(),
                std::env::var("QN_TEST_ROOT")?.into(),
                Arc::new(QueryPolicy),
            )?;
            let records = reader.list().await?;
            assert!(
                !records.is_empty(),
                "isolated smoke requires existing synthetic audit records"
            );
            reader
                .show(&NonoSessionId::try_from(records[0].session_id.as_str())?)
                .await?;
        }
        _ => {
            let out = process::run(
                &CmdSpec::new(binary.as_str())
                    .arg("--parent")
                    .env("QN_AUDIT_CANARY", "synthetic-canary"),
            )
            .await?;
            assert!(out.success, "audit fixture failed: {}", out.stderr);
        }
    }
    Ok(())
}

fn fake_query() -> Result<(), Box<dyn Error>> {
    assert!(std::env::var_os("QN_AUDIT_CANARY").is_none());
    let root = path(std::env::current_dir()?.canonicalize()?)?;
    for (key, dir) in [
        ("HOME", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
        ("TMPDIR", "tmp"),
    ] {
        assert_eq!(std::env::var(key)?, root.join(dir).as_str());
    }
    let mode = fs::read_to_string(root.join("mode"))?;
    if mode == "hang" {
        loop {
            std::thread::park();
        }
    }
    print!("{}", fs::read_to_string(root.join("response"))?);
    if mode == "fail" {
        std::process::exit(7);
    }
    Ok(())
}

async fn verify(binary: Utf8PathBuf) -> Result<(), Box<dyn Error>> {
    assert!(std::env::var_os("QN_AUDIT_CANARY").is_some());
    let directory = tempfile::tempdir()?;
    let root = path(directory.path().canonicalize()?)?;
    fs::write(root.join("mode"), "ok")?;
    let reader = AuditReader::new(binary.clone(), root.clone(), Arc::new(QueryPolicy))?;
    fs::write(root.join("response"), LIST)?;
    assert_eq!(reader.list().await?.len(), 3);
    fs::write(root.join("response"), SHOW)?;
    let record = reader.show(&NonoSessionId::try_from(ID)?).await?;
    assert_eq!(record.network_events[0].decision, "deny");
    assert_eq!(
        record.network_events[0].denial_category.as_deref(),
        Some("host_denied")
    );
    assert!(record.started.is_some() && record.network_events[0].timestamp_unix_ms.is_some());
    assert!(!format!("{record:?}").contains("blocked.invalid"));
    assert!(
        reader
            .show(&NonoSessionId::try_from("fedcba9876543210")?)
            .await
            .is_err()
    );
    fs::write(root.join("mode"), "fail")?;
    assert!(matches!(
        reader.list().await,
        Err(NonoError::CommandFailed { code: Some(7), .. })
    ));
    fs::write(root.join("mode"), "ok")?;
    let secret = "synthetic-sensitive-audit";
    fs::write(
        root.join("response"),
        format!(r#"[{{"session_id":"{ID}","is_alive":"{secret}"}}]"#),
    )?;
    let error = reader.list().await.expect_err("invalid schema must fail");
    assert!(!format!("{error} {error:?}").contains(secret));
    fs::write(root.join("response"), " ".repeat((1 << 20) + 1))?;
    assert!(reader.list().await.is_err());
    fs::write(root.join("mode"), "hang")?;
    assert!(matches!(
        reader.list().await,
        Err(NonoError::CommandTimedOut { .. })
    ));
    assert!(AuditReader::new(binary.clone(), "relative".into(), Arc::new(QueryPolicy)).is_err());
    assert!(
        AuditReader::new(
            binary.clone(),
            (root.as_str().to_owned() + "\0").into(),
            Arc::new(QueryPolicy)
        )
        .is_err()
    );
    let mut bad = root.join("bad-version");
    if let Some(extension) = binary.extension() {
        bad.set_extension(extension);
    }
    fs::copy(&binary, &bad)?;
    let reader = AuditReader::new(bad, root, Arc::new(QueryPolicy))?;
    assert!(matches!(
        reader.list().await,
        Err(NonoError::VersionMismatch { .. })
    ));
    Ok(())
}
