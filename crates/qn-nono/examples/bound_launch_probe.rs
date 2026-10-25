//! Synthetic host coordinator for the pinned launch hook; never a product runner (#108).
use camino::Utf8PathBuf;
use qn_core::SessionId;
use qn_infra::process::{self, CmdSpec};
use qn_nono::{NonoArgs, RunArgs, RunMode, launch::LaunchBinding, verify_version};
use std::{
    io::{Read, Write},
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let root: Utf8PathBuf = args.next().ok_or("private fixture root required")?.into();
    let binary: Utf8PathBuf = args.next().ok_or("verified native binary required")?.into();
    let binding = LaunchBinding::reserve(&root, SessionId::new())?;
    println!(
        "{}",
        serde_json::json!({"session_id":binding.session().to_string(),
        "native_id":binding.native_id().as_str()})
    );
    std::io::stdout().flush()?;
    // The parent verifier owns this fixture process and a bounded handshake timeout.
    // No grant/listener/native process exists until it acknowledges the prepared route.
    let mut input = String::new();
    std::io::stdin()
        .take(16 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 16 * 1024 {
        return Err("fixture command too large".into());
    }
    let command: Vec<String> = serde_json::from_str(&input)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        verify_version(&binary).await?;
        let profile = root.join("profile.json");
        let validation = NonoArgs::profile_validate(&profile)?;
        let checked = process::run(&environment(
            CmdSpec::new(binary.as_str())
                .args(validation.as_slice().iter().cloned())
                .env_clear(),
            &root,
        ))
        .await?;
        if !checked.success || checked.stdout_truncated || checked.stderr_truncated {
            return Err("native profile validation failed".into());
        }
        let command = binding.command(
            &binary,
            RunArgs {
                profile,
                name: binding.session(),
                workdir: root.join("project"),
                mode: RunMode::Foreground,
                command,
            },
        )?;
        let output = process::run(&environment(command, &root)).await?;
        println!(
            "{}",
            serde_json::json!({"exit_code":output.exit_code,"stdout":output.stdout,
            "stderr":output.stderr,"stdout_truncated":output.stdout_truncated,
            "stderr_truncated":output.stderr_truncated})
        );
        Ok(())
    })
}

fn environment(mut command: CmdSpec, root: &camino::Utf8Path) -> CmdSpec {
    for (key, dir) in [
        ("HOME", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
        ("TMPDIR", "tmp"),
    ] {
        command = command.env(key, root.join(dir).as_str());
    }
    command
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C")
        .cwd(root.join("project"))
        .timeout(Duration::from_secs(20))
        .max_output_bytes(1 << 20)
        .label("synthetic bound native launch")
}
