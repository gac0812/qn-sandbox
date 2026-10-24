//! Opt-in spike transport; launch with a clean synthetic environment (#46).
use qn_infra::process::{self, CmdSpec};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let binary = args.next().ok_or("provide an absolute nono binary path")?;
    if !camino::Utf8Path::new(&binary).is_absolute() {
        return Err("provide an absolute nono binary path".into());
    }
    let spec = CmdSpec::new(binary)
        .args(args)
        .timeout(Duration::from_secs(45))
        .max_output_bytes(1 << 20)
        .label("isolated nono contract probe");
    let out = process::run(&spec).await?;
    println!(
        "{}",
        serde_json::json!({
            "exit_code": out.exit_code, "stdout": out.stdout, "stderr": out.stderr,
            "stdout_truncated": out.stdout_truncated,
            "stderr_truncated": out.stderr_truncated,
        })
    );
    Ok(())
}
