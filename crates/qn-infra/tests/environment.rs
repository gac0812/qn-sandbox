//! Re-execute Rust test probes with synthetic env; never mutate the host env.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use qn_infra::process::{self, CmdSpec};
use std::time::Duration;

const SECRET: &str = "QN_TEST_INHERITED_SECRET";
const VALUE: &str = "synthetic-env-canary";

fn probe(name: &str) -> CmdSpec {
    let exe = std::env::current_exe()
        .unwrap()
        .into_os_string()
        .into_string()
        .unwrap();
    CmdSpec::new(exe)
        .args(["--exact", name, "--ignored"])
        .timeout(Duration::from_secs(10))
        .label("synthetic environment probe")
}

#[tokio::test]
async fn it_enforces_environment_edits_in_child_processes() {
    for case in [
        "inherit",
        "remove_inherited",
        "remove",
        "clear",
        "restore",
        "clear_twice",
    ] {
        let spec = probe("environment_parent")
            .env(SECRET, VALUE)
            .env("QN_TEST_ENV_CASE", case);
        let output = process::run(&spec.clone()).await.unwrap();
        assert!(
            output.success,
            "environment case {case} failed: {}",
            output.stdout
        );
    }
}

#[test]
fn it_redacts_environment_edits_from_debug_and_clone() {
    let spec = CmdSpec::new("program")
        .env(SECRET, VALUE)
        .env_remove(SECRET)
        .env_clear();
    let debug = format!("{spec:?} {:?}", spec.clone());
    assert!(!debug.contains(SECRET));
    assert!(!debug.contains(VALUE));
}

#[tokio::test]
#[ignore = "subprocess probe invoked by the environment integration test"]
async fn environment_parent() {
    assert_eq!(std::env::var(SECRET).unwrap(), VALUE);
    let case = std::env::var("QN_TEST_ENV_CASE").unwrap();
    let spec = probe("environment_leaf");
    let spec = match case.as_str() {
        "inherit" => spec,
        "remove_inherited" => spec.env_remove(SECRET),
        "remove" => spec.env(SECRET, "override").env_remove(SECRET),
        "clear" => spec.env("QN_TEST_DISCARD", "discard").env_clear(),
        "restore" => spec.env_remove(SECRET).env(SECRET, "restored"),
        "clear_twice" => spec.env_clear().env(SECRET, "discard").env_clear(),
        _ => panic!("unexpected synthetic case"),
    };
    let spec = spec
        .env("QN_TEST_ALLOWED", "kept")
        .env("QN_TEST_ENV_CASE", &case);
    let output = process::run(&spec).await.unwrap();
    assert!(output.success, "leaf case {case} failed: {}", output.stdout);
}

#[test]
#[ignore = "subprocess probe invoked by the environment parent"]
fn environment_leaf() {
    let case = std::env::var("QN_TEST_ENV_CASE").unwrap();
    assert_eq!(std::env::var("QN_TEST_ALLOWED").unwrap(), "kept");
    match case.as_str() {
        "inherit" => assert_eq!(std::env::var(SECRET).unwrap(), VALUE),
        "restore" => assert_eq!(std::env::var(SECRET).unwrap(), "restored"),
        _ => assert!(
            std::env::var_os(SECRET).is_none(),
            "inherited secret must be absent"
        ),
    }
    assert!(std::env::var_os("QN_TEST_DISCARD").is_none());
}
