//! Opt-in pinned-nono canary for the prepared offline profile (#75).
#![cfg(unix)]

use camino::Utf8PathBuf;
use process_wrap::tokio::{ProcessGroup, TokioCommandWrap};
use qn_core::{ProfileRef, RunSpec, SessionId};
use qn_infra::process::{ProcessPolicy, TerminationSafety, spawn_managed};
use qn_nono::profile::{PreparedProfile, RuntimePaths};
use std::{error::Error, fs};
use tokio_util::sync::CancellationToken;

struct GroupPolicy;
impl ProcessPolicy for GroupPolicy {
    fn configure(&self, command: &mut TokioCommandWrap) -> TerminationSafety {
        command.wrap(ProcessGroup::leader());
        TerminationSafety::WhileParentOwned
    }
}

#[tokio::test]
#[ignore = "requires QN_TEST_NONO, QN_TEST_PROFILE_PARENT and /usr/bin/ruby"]
async fn it_runs_the_prepared_offline_profile_canary() -> Result<(), Box<dyn Error>> {
    let binary = Utf8PathBuf::from(std::env::var("QN_TEST_NONO")?);
    if !binary.is_absolute() {
        return Err("absolute verified nono required".into());
    }
    let parent = std::env::var("QN_TEST_PROFILE_PARENT")?;
    let directory = tempfile::tempdir_in(parent)?;
    let root = Utf8PathBuf::from_path_buf(directory.path().canonicalize()?)
        .map_err(|_| "UTF-8 root required")?;
    for dir in ["home", "project", "host/.ssh", "control"] {
        fs::create_dir_all(root.join(dir))?;
    }
    fs::write(root.join("host/.ssh/canary"), "synthetic-host-secret")?;
    let script = root.join("project/canary.rb");
    fs::write(
        &script,
        r"require 'json'
checks = {}
begin
  File.write(ARGV[0], 'ok')
  checks['project_write'] = true
rescue StandardError
  checks['project_write'] = false
end
begin
  File.read(ARGV[1])
  checks['host_secret_denied'] = false
rescue Errno::EACCES, Errno::EPERM
  checks['host_secret_denied'] = true
end
checks['env_clean'] = ENV['QN_FAKE_SECRET'].nil?
puts JSON.generate(checks)
",
    )?;
    let spec = RunSpec::new(
        SessionId::new(),
        ProfileRef::try_from("qn/claude-code")?,
        vec![
            "/usr/bin/ruby".into(),
            script.to_string(),
            root.join("project/canary.txt").to_string(),
            root.join("host/.ssh/canary").to_string(),
        ],
        root.join("project"),
    )?;
    let paths = RuntimePaths {
        control_root: root.join("control"),
        host_homes: vec![root.join("host")],
        engine_binary: binary,
        tool_dirs: vec![Utf8PathBuf::from("/usr/bin"), Utf8PathBuf::from("/bin")],
    };
    let prepared = PreparedProfile::prepare(spec, &paths)?;
    let output = spawn_managed(&prepared.run_command()?, &GroupPolicy)?
        .wait(CancellationToken::new())
        .await?;
    assert!(
        output.output().success,
        "profile canary failed: {}",
        output.output().stderr
    );
    let checks: serde_json::Value = serde_json::from_str(output.output().stdout.trim())?;
    assert_eq!(checks["project_write"], true);
    assert_eq!(checks["host_secret_denied"], true);
    assert_eq!(checks["env_clean"], true);
    assert!(fs::read_to_string(root.join("project/canary.txt"))?.contains("ok"));
    Ok(())
}
