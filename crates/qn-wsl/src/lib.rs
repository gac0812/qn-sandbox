//! qn-wsl：Windows 供给链路适配器（spec 01 §2；R6 允许 `#[cfg(target_os)]` 的位置）。
//!
//! 首模块 [`health`]：五项平台探测（WSL/发行版/内核/nono/ABI），产出结构化
//! `ProbeReport` 供 doctor 类呈现（#44/#66 对接位）消费。distro 供给已落地（#85）；bootstrap（发行版内 nono 引导）与 kernel 升级
//! 属后续模块（kernel 等 #17 裁决）。

#![cfg(target_os = "windows")]

pub mod decode;
pub mod distro;
pub mod health;
pub mod kernel;

/// 受管发行版名。命名与生命周期归 distro 模块（后续 issue，#74 关联），
/// health 只按此名探测存在性。
pub const MANAGED_DISTRO_NAME: &str = "qn-sandbox";
