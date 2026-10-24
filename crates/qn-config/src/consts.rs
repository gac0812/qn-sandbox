//! 平台常量（单点存在，全仓 grep 应只在此出现定义）。
//!
//! 数值来源：spike① 报告（#16 / docs/dev-notes/spike1-wsl2-landlock.md）。

/// nono 安全内核的锁定版本（spec 04 §6：锁版本跟版；升级走独立 PR）。
///
/// 升级流程：改此常量 + 安装清单 → 独立 `deps/` PR 附 release notes 与回归证据。
pub const NONO_PINNED_VERSION: &str = "0.80.0";

/// WSL2 内核下限（Landlock 网络限制需要 ≥6.7，见 #16 报告 H2/H3）。
///
/// 供给链路（qn-wsl）据此探测；不达标必须显式告知降级，禁止静默（08 §2）。
/// 语义化版本三段式，便于与 `uname -r` 解析结果比较（比较逻辑属 qn-wsl）。
pub const WSL_MIN_KERNEL_SEMVER: &str = "6.7.0";
