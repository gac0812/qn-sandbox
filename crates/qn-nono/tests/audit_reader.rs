//! Short-lived Rust/native query fixture; production injects bootstrap's group policy.
#![allow(clippy::expect_used)]
use camino::Utf8PathBuf;
use process_wrap::tokio::TokioCommandWrap;
use qn_infra::process::{self, CmdSpec, ProcessPolicy, TerminationSafety};
use qn_nono::session::{NativeSessionState, SessionReader};
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
        Some("audit" | "ps" | "inspect") => fake_query()?,
        Some("--parent") => verify(binary).await?,
        Some("--native-session") => {
            let reader = SessionReader::new(
                std::env::var("QN_TEST_NONO")?.into(),
                std::env::var("QN_TEST_ROOT")?.into(),
                Arc::new(QueryPolicy),
            )?;
            let id = qn_core::SessionId::try_from(std::env::var("QN_TEST_SESSION")?.as_str())?;
            let record = reader.find(id).await?.ok_or("native session missing")?;
            assert_eq!(
                record.state(),
                NativeSessionState::Running,
                "native running smoke"
            );
            reader.inspect(&record).await?;
        }
        Some("--native") => {
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

async fn verify_sessions(binary: &Utf8PathBuf, root: &Utf8PathBuf) -> Result<(), Box<dyn Error>> {
    let reader = SessionReader::new(binary.clone(), root.clone(), Arc::new(QueryPolicy))?;
    let id = qn_core::SessionId::try_from("sxn-019a1234-5678-7abc-8123-000000000001")?;
    fs::write(
        root.join("sessions.json"),
        include_str!("fixtures/nono-0.80.0/ps-running.json"),
    )?;
    fs::write(
        root.join("inspect.json"),
        include_str!("fixtures/nono-0.80.0/inspect-running.json"),
    )?;
    assert!(
        reader.find(qn_core::SessionId::new()).await?.is_none(),
        "foreign name stays unbound"
    );
    let found = reader.find(id).await?.ok_or("fixture session missing")?;
    assert_eq!(found.session(), id, "preserve expected qn name");
    assert_eq!(found.state(), NativeSessionState::Running, "native running");
    let args: Vec<String> = serde_json::from_slice(&fs::read(root.join("inspect-args.json"))?)?;
    assert_eq!(
        args,
        ["inspect", "--json", "--", ID],
        "inspect only the full native ID"
    );
    let mut inspected: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("inspect.json"))?)?;
    inspected["status"] = serde_json::json!("exited");
    inspected["exit_code"] = serde_json::json!(7);
    fs::write(root.join("inspect.json"), serde_json::to_vec(&inspected)?)?;
    let ended = reader.inspect(&found).await?;
    assert_eq!(
        (ended.state(), ended.exit_code()),
        (NativeSessionState::Exited, Some(7)),
        "observe exit without inventing cleanup"
    );
    inspected["name"] = serde_json::json!("synthetic-secret");
    fs::write(root.join("inspect.json"), serde_json::to_vec(&inspected)?)?;
    let error = reader
        .inspect(&found)
        .await
        .expect_err("refuse changed qn name");
    assert!(
        !format!("{error:?} {error}").contains("synthetic-secret"),
        "redact response"
    );
    fs::write(root.join("mode"), "fail")?;
    assert!(
        matches!(
            reader.find(id).await,
            Err(NonoError::CommandFailed { code: Some(7), .. })
        ),
        "ps nonzero is not absence"
    );
    fs::write(root.join("mode"), "ok")?;
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let response = match args.first().map(String::as_str) {
        Some("ps") => "sessions.json",
        Some("inspect") => {
            fs::write(root.join("inspect-args.json"), serde_json::to_vec(&args)?)?;
            "inspect.json"
        }
        _ => "response",
    };
    print!("{}", fs::read_to_string(root.join(response))?);
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
    verify_sessions(&binary, &root).await?;
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
