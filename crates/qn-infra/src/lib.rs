//! qn-infra：被各层复用的通用机制（不含领域概念，spec 01 §2）。
//!
//! 当前提供外部命令统一执行器 [`process`]。所有对外部二进制
//! （nono、`wsl.exe` 等）的调用必须经此模块（AGENTS R1、spec 08 §5）。

pub mod process;

mod error;

pub use error::Error;
