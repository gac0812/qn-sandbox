//! Ordered environment edits delegate native key semantics to Tokio/std (#54).
use std::fmt;

use super::CmdSpec;
use tokio::process::Command;

#[derive(Clone)]
pub(super) enum EnvironmentChange {
    Set(String, String),
    Remove(String),
    Clear,
}

impl fmt::Debug for EnvironmentChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Set(..) => "Set(..)",
            Self::Remove(..) => "Remove(..)",
            Self::Clear => "Clear",
        })
    }
}

impl EnvironmentChange {
    pub(super) fn apply(&self, command: &mut Command) {
        match self {
            Self::Set(key, value) => {
                command.env(key, value);
            }
            Self::Remove(key) => {
                command.env_remove(key);
            }
            Self::Clear => {
                command.env_clear();
            }
        }
    }
}

impl CmdSpec {
    /// Add or replace a child environment value after preceding environment edits.
    /// Other variables are inherited unless [`Self::env_clear`] was called.
    ///
    /// ```
    /// use qn_infra::process::CmdSpec;
    /// let spec = CmdSpec::new("program").env_clear().env("LANG", "C");
    /// ```
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env
            .push(EnvironmentChange::Set(key.into(), value.into()));
        self
    }

    /// Disable inheritance and discard all explicit values added before this call.
    /// Rebuild an allowlist with subsequent [`Self::env`] calls. Prefer an absolute
    /// executable and set PATH explicitly if the child needs to locate programs.
    ///
    /// ```
    /// use qn_infra::process::CmdSpec;
    /// let spec = CmdSpec::new("/opt/tool").env("DISCARD", "value")
    ///     .env_clear().env("LANG", "C");
    /// ```
    pub fn env_clear(mut self) -> Self {
        self.env.push(EnvironmentChange::Clear);
        self
    }

    /// Remove an inherited or previously set value; a later [`Self::env`] restores it.
    /// Removal alone is not an allowlist: use [`Self::env_clear`] to exclude all
    /// unknown inherited values. Key case handling follows the host OS.
    ///
    /// ```
    /// use qn_infra::process::CmdSpec;
    /// let spec = CmdSpec::new("program").env_remove("EXAMPLE_SECRET");
    /// ```
    pub fn env_remove(mut self, key: impl Into<String>) -> Self {
        self.env.push(EnvironmentChange::Remove(key.into()));
        self
    }
}
