//! 内核升级闭环演示——先探测再升级（#167 验收证据）。
//!
//! 契约演示：计划非空（含 `wsl --shutdown` 破坏性步骤）时**必须显式 opt-in**
//! （`QN_WSL_UPGRADE_CONFIRM=1`）才执行，否则只打印计划——对齐 #17
//! "shutdown 须用户确认"与模块文档的调用方契约。

#[cfg(target_os = "windows")]
#[tokio::main]
async fn main() {
    // 正确用法：先经 health 内核探测判定 needs_upgrade，再调用升级闭环。
    let needs = qn_wsl::health::run_all()
        .await
        .iter()
        .find(|r| r.id == qn_wsl::health::ProbeId::Kernel)
        .is_some_and(|r| r.status == qn_wsl::health::ProbeStatus::Fail);
    let plan = qn_wsl::upgrade::plan_upgrade(needs);
    if plan.steps.is_empty() {
        match qn_wsl::upgrade::run(&plan).await {
            Ok(outcome) => println!("{outcome:?}"),
            Err(err) => {
                println!("upgrade incomplete: {err}");
                std::process::exit(1);
            }
        }
        return;
    }
    if std::env::var("QN_WSL_UPGRADE_CONFIRM").as_deref() != Ok("1") {
        println!(
            "plan requires shutdown of ALL distros (incl. Docker); re-run with QN_WSL_UPGRADE_CONFIRM=1 to execute:"
        );
        for step in &plan.steps {
            let flag = if step.disruptive {
                "DISRUPTIVE"
            } else {
                "         "
            };
            println!("  [{flag}] {}", step.label);
        }
        return;
    }
    match qn_wsl::upgrade::run(&plan).await {
        Ok(outcome) => println!("{outcome:?}"),
        Err(err) => {
            println!("upgrade incomplete: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("qn-wsl upgrade is Windows-only.");
}
