//! 打印五项平台探测结果——#74 验收证据与 doctor（#44）对接预览。

#[cfg(target_os = "windows")]
#[tokio::main]
async fn main() {
    for report in qn_wsl::health::run_all().await {
        println!("[{:?}/{:?}] {}", report.id, report.status, report.detail);
        if let Some(remediation) = report.remediation {
            println!("    fix: {remediation}");
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("qn-wsl probes are Windows-only; nothing to do on this platform.");
}
