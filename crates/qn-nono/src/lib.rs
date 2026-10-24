//! Sandbox engine adapter for the nono sandbox kernel.
//!
//! `qn-nono` is the only crate allowed to touch nono (01 §3). It drives a
//! **version-locked `nono` CLI as a subprocess** — never the `nono` /
//! `nono-proxy` libraries — and parses only `--json` / `--diagnostics-json`
//! output (spike ③, decision issue #28; spec 04 §3).
//!
//! Security constraints carried over from the spike ③ decision (issue #28):
//!
//! - only `--credential` (reverse-proxy injection) may be used to hand
//!   credentials to nono;
//! - `--env-credential` / `--env-credential-map` must never be generated:
//!   they inject plaintext credentials into the sandbox environment and
//!   violate the workspace security invariant "credentials never enter the
//!   sandbox env" (upstream `nono-cli` flags, spike ③ F8).
//!
//! # Example
//!
//! ```no_run
//! use qn_nono::{install, InstallOptions};
//!
//! let binary = install(&InstallOptions::new("/home/user/.qn/cache"))?;
//! println!("nono {} installed at {}", binary.version, binary.path.display());
//! # Ok::<(), qn_nono::InstallerError>(())
//! ```

pub mod args;
pub mod error;
pub mod installer;
pub mod manifest;
pub mod target;

pub use args::{NonoArgs, RunArgs, RunMode};
pub use error::InstallerError;
pub use installer::{InstallOptions, InstalledBinary, install};
pub use manifest::LOCKED_NONO_VERSION;
pub use target::NonoTarget;
