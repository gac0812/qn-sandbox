//! qn-wsl：Windows 供给链路适配器（spec 01 §2；R6 允许 `#[cfg(target_os)]` 的位置）。
//!
//! Windows 模块：`health` 五项平台探测（#74/#76）；`distro` 受管发行版幂等供给（#85）；
//! `bootstrap` 发行版内 nono 引导（#126）。kernel 升级等 #17 裁决。

#![cfg(target_os = "windows")]

pub mod bootstrap;
pub mod decode;
pub mod distro;
pub mod health;
pub mod kernel;

/// 受管发行版名。命名与生命周期归 distro 模块（#85），
/// health 只按此名探测存在性。
pub const MANAGED_DISTRO_NAME: &str = "qn-sandbox";
