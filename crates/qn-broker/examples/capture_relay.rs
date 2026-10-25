//! Host-side relay for pinned nono capture. Binding must be private to the host.
use qn_broker::capture::{CaptureBinding, CaptureBytes, CaptureError};
use std::{
    io::{Read, Write},
    time::Duration,
};
use zeroize::Zeroizing;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if relay().await.is_err() {
        eprintln!("host capture refused; verify the private binding and live session grant");
        std::process::exit(1);
    }
}
async fn relay() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or(CaptureError::Rejected)?;
    let mut config = Zeroizing::new(Vec::new());
    std::fs::File::open(path)?
        .take(4097)
        .read_to_end(&mut config)?;
    if config.len() > 4096 {
        return Err(CaptureError::Rejected.into());
    }
    let binding: CaptureBinding = serde_json::from_slice(&config)?;
    let mut request = Zeroizing::new(Vec::new());
    std::io::stdin().take(16_385).read_to_end(&mut request)?;
    let response = binding
        .exchange(
            CaptureBytes::new(std::mem::take(&mut request))?,
            Duration::from_secs(5),
        )
        .await?;
    // stdout belongs exclusively to the host nono capture pipe, never a logger.
    std::io::stdout().write_all(response.as_bytes())?;
    Ok(())
}
