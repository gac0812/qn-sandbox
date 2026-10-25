//! Opt-in real `DeepSeek` Claude Code task through the pinned engine (#122/#59).
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use camino::{Utf8Path, Utf8PathBuf};
use process_wrap::tokio::{ProcessGroup, TokioCommandWrap};
use qn_core::{ProfileRef, RunSpec, SessionId};
use qn_infra::process::{ProcessPolicy, StdioMode, TerminationSafety, spawn_managed};
use qn_nono::launch::LaunchBinding;
use qn_nono::profile::{PreparedOfficial, RuntimePaths};
use std::{error::Error, fs, io::Write as _};
use tokio_util::sync::CancellationToken;

const PROFILE: &str = include_str!("../../../tests/e2e/macos/deepseek/claude-deepseek.json");

struct GroupPolicy;
impl ProcessPolicy for GroupPolicy {
    fn configure(&self, command: &mut TokioCommandWrap) -> TerminationSafety {
        command.wrap(ProcessGroup::leader());
        TerminationSafety::WhileParentOwned
    }
}

#[tokio::test]
#[ignore = "requires QN_TEST_NONO, QN_TEST_PROFILE_PARENT, QN_TEST_CLAUDE and ~/.claude/settings.json with ANTHROPIC_AUTH_TOKEN"]
async fn it_runs_a_real_deepseek_claude_task_without_exposing_the_token()
-> Result<(), Box<dyn Error>> {
    let binary = Utf8PathBuf::from(std::env::var("QN_TEST_NONO")?);
    let parent = std::env::var("QN_TEST_PROFILE_PARENT")?;
    let claude =
        std::env::var("QN_TEST_CLAUDE").unwrap_or_else(|_| "/opt/homebrew/bin/claude".to_owned());
    let settings_path = std::env::var("HOME")? + "/.claude/settings.json";
    let settings: serde_json::Value = serde_json::from_slice(&fs::read(settings_path)?)?;
    let token = settings["env"]["ANTHROPIC_AUTH_TOKEN"]
        .as_str()
        .ok_or("missing ANTHROPIC_AUTH_TOKEN in ~/.claude/settings.json")?;
    if token.is_empty() {
        return Err("empty ANTHROPIC_AUTH_TOKEN".into());
    }

    let directory = tempfile::tempdir_in(parent)?;
    let root = Utf8PathBuf::from_path_buf(directory.path().canonicalize()?)
        .map_err(|_| "UTF-8 root required")?;
    for dir in ["project", "control", "host", "runtime", "audit"] {
        fs::create_dir_all(root.join(dir))?;
    }
    let mut token_file = tempfile::Builder::new()
        .prefix("anthropic-token-")
        .tempfile_in(root.join("host"))?;
    token_file.write_all(token.as_bytes())?;
    token_file.as_file().sync_all()?;
    let token_path = token_file.path().to_str().ok_or("non-UTF8 token path")?;
    if token_path.contains([' ', '?', '#', '%']) {
        return Err("token path must not need URI escaping; use a simple parent path".into());
    }
    let asset = PROFILE.replace("TOKEN_PATH", token_path);

    let session = SessionId::new();
    let spec = RunSpec::new(
        session,
        ProfileRef::try_from("qn/claude-code")?,
        vec![
            claude.clone(),
            "-p".into(),
            "--output-format".into(),
            "text".into(),
            "--dangerously-skip-permissions".into(),
            "--model".into(),
            "deepseek-flash[1M]".into(),
            "Reply with exactly the token qn-deepseek-ok and nothing else.".into(),
        ],
        root.join("project"),
    )?;
    let tool_dirs: Vec<Utf8PathBuf> = [
        Utf8Path::new(&claude)
            .parent()
            .ok_or("claude path")?
            .to_owned(),
        Utf8PathBuf::from("/opt/homebrew/bin"),
        Utf8PathBuf::from("/usr/bin"),
        Utf8PathBuf::from("/bin"),
    ]
    .into_iter()
    .filter(|path| path.is_dir())
    .collect();
    let paths = RuntimePaths {
        control_root: root.join("control"),
        host_homes: vec![root.join("host")],
        engine_binary: binary,
        tool_dirs,
    };
    let prepared =
        PreparedOfficial::prepare(spec, &paths, root.join("runtime"), &asset, ".claude")?;
    let binding = LaunchBinding::reserve(&root.join("control"), session)?;
    binding.publish_audit(&root.join("audit"), &root.join("runtime"))?;
    let command = prepared.bound_run_command_with_stdio(&binding, StdioMode::Captured)?;
    let output = spawn_managed(&command, &GroupPolicy)?
        .wait(CancellationToken::new())
        .await?;
    let redacted_stderr = output.output().stderr.replace(token, "<redacted>");
    assert!(output.output().success, "claude failed: {redacted_stderr}");
    let answer = output.output().stdout.trim();
    assert!(!answer.is_empty(), "model returned an empty answer");
    assert!(
        answer.to_lowercase().contains("qn-deepseek-ok"),
        "unexpected answer: {answer}"
    );
    eprintln!("deepseek claude answer: {answer}");
    drop(token_file);
    Ok(())
}
