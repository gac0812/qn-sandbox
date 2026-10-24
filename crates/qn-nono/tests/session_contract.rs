//! Opt-in macOS smoke; launch only inside a fresh synthetic environment (#52).
#![allow(clippy::unwrap_used, clippy::expect_used)]
use camino::Utf8PathBuf;
use qn_infra::process::{self, CmdSpec};
use qn_nono::{
    NonoArgs, RunArgs, RunMode,
    args::{NonoSessionId, NonoStopMode},
};
use serde_json::{Value, json};
use std::{fs, time::Duration};

async fn invoke(binary: &str, args: NonoArgs) -> String {
    let spec = CmdSpec::new(binary)
        .args(args.as_slice().iter().cloned())
        .timeout(Duration::from_secs(15))
        .max_output_bytes(1 << 20)
        .label("isolated session argv contract");
    let out = process::run(&spec)
        .await
        .expect("probe transport must complete");
    assert!(
        out.success,
        "{} contract failed (exit {:?}); output redacted",
        args.as_slice()[0],
        out.exit_code
    );
    assert!(!out.stdout_truncated && !out.stderr_truncated);
    out.stdout
}

#[tokio::test]
#[ignore = "requires verified QN_TEST_NONO and fresh QN_TEST_ROOT; see qn-nono README"]
async fn it_drives_disposable_sessions_with_the_pinned_argv_contract() {
    let root = Utf8PathBuf::from(std::env::var("QN_TEST_ROOT").unwrap());
    let binary = std::env::var("QN_TEST_NONO").unwrap();
    assert!(root.is_absolute() && camino::Utf8Path::new(&binary).is_absolute());
    // Refuse to inspect the user's normal state even if the opt-in flag is set.
    for (key, dir) in [
        ("HOME", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        assert_eq!(std::env::var(key).unwrap(), root.join(dir).as_str());
    }
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    let profile = root.join("profile.json");
    fs::write(
        &profile,
        json!({"meta":{"name":"qn-session-contract","version":"0.1.0"},
            "extends":"default", "filesystem":{"allow":[project.as_str()]},
            "network":{"block":true}, "environment":{"allow_vars":["PATH","HOME","LANG"]}
        })
        .to_string(),
    )
    .unwrap();
    for query in [NonoArgs::sessions(), NonoArgs::audit_list()] {
        let records: Value = serde_json::from_str(&invoke(&binary, query).await).unwrap();
        assert_eq!(records, json!([]), "contract requires fresh state");
    }
    let mut cleanup = Vec::new();
    for mode in [
        NonoStopMode::Graceful {
            timeout: Duration::from_secs(3),
        },
        NonoStopMode::Force,
    ] {
        let name = qn_core::SessionId::new();
        invoke(
            &binary,
            NonoArgs::run(RunArgs {
                profile: profile.clone(),
                name,
                workdir: project.clone(),
                mode: RunMode::Detached,
                command: vec!["/bin/sleep".into(), "30".into()],
            })
            .unwrap(),
        )
        .await;
        let records: Vec<Value> =
            serde_json::from_str(&invoke(&binary, NonoArgs::sessions()).await).unwrap();
        let record = records
            .iter()
            .find(|r| r["name"] == name.to_string())
            .unwrap();
        let raw_id = record["session_id"].as_str().unwrap();
        let id = NonoSessionId::try_from(raw_id).unwrap();
        cleanup.push(
            json!({"supervisor_pid":record["supervisor_pid"],"child_pid":record["child_pid"]}),
        );
        fs::write(
            root.join("cleanup.json"),
            serde_json::to_vec(&cleanup).unwrap(),
        )
        .unwrap();
        let inspected: Value =
            serde_json::from_str(&invoke(&binary, NonoArgs::inspect(&id)).await).unwrap();
        assert_eq!(inspected["session_id"], raw_id);
        invoke(&binary, NonoArgs::stop(&id, mode).unwrap()).await;
        // #47/#52: force can kill the supervisor before audit metadata is flushed.
        if matches!(mode, NonoStopMode::Graceful { .. }) {
            let audit: Value =
                serde_json::from_str(&invoke(&binary, NonoArgs::audit_show(&id)).await).unwrap();
            assert_eq!(audit["session_id"], raw_id);
        }
    }
    let records: Vec<Value> =
        serde_json::from_str(&invoke(&binary, NonoArgs::sessions()).await).unwrap();
    assert!(records.iter().all(|r| r["status"] == "exited"));
    // ps status is not proof of process cleanup: the harness checks cleanup.json.
}
