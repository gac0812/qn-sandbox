//! qn-wsl：Windows 供给链路适配器（spec 01 §2；R6 允许 `#[cfg(target_os)]` 的位置）。
//!
//! 首模块 [`health`]：五项平台探测（WSL/发行版/内核/nono/ABI），产出结构化
//! `ProbeReport` 供 doctor 类呈现（#44/#66 对接位）消费。供给动作（安装/升级/修复）
//! 属后续 distro/kernel/bootstrap 模块——探测先行是它们的前置能力（#74）。

#![cfg(target_os = "windows")]

pub mod decode;
pub mod health;
pub mod kernel;

mod error;

pub use error::Error;

/// 受管发行版名。命名与生命周期归 distro 模块（后续 issue，#74 关联），
/// health 只按此名探测存在性与状态。
pub const MANAGED_DISTRO_NAME: &str = "qn-sandbox";
