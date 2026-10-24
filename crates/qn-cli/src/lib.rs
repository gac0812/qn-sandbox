//! Command-line interface for qn-sandbox.
//!
//! The CLI is a presentation adapter: it parses user intent and renders
//! diagnostics but does not implement sandbox behavior. Concrete engine and
//! credential adapters will be assembled through `qn-bootstrap`.

use std::io::Write;

use clap::{Parser, Subcommand};
use qn_config::Config;
use qn_config::paths::QnPaths;

/// Parsed command-line invocation for the `qn` executable.
#[derive(Debug, Parser)]
#[command(name = "qn", version, about = "Secure sandbox for AI coding agents")]
pub struct Cli {
    /// Command to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// User-facing commands available in this CLI slice.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check local configuration and show unresolved prerequisites.
    Doctor,
}

/// Errors surfaced by CLI command execution.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Configuration paths or values could not be resolved or loaded.
    #[error(transparent)]
    Config(#[from] qn_config::Error),

    /// Diagnostic output could not be written.
    #[error("failed to write diagnostic output: {0}")]
    Output(#[from] std::io::Error),
}

/// Executes one parsed CLI command and writes its user-facing output.
///
/// # Errors
///
/// Returns [`Error::Config`] when qn configuration cannot be resolved or
/// loaded, and [`Error::Output`] when writing diagnostics fails.
pub fn run(cli: &Cli, output: &mut dyn Write) -> Result<(), Error> {
    match cli.command {
        Command::Doctor => doctor(output),
    }
}

fn doctor(output: &mut dyn Write) -> Result<(), Error> {
    let paths = QnPaths::resolve()?;
    let config_file_exists = paths.config_file.exists();
    let config = Config::load(&paths)?.apply_env_overrides()?;
    let timeout_ms = u64::try_from(config.command_timeout().as_millis()).unwrap_or(u64::MAX);

    writeln!(output, "qn doctor")?;
    writeln!(output, "home: {}", paths.root)?;
    writeln!(output, "config: {}", paths.config_file)?;
    if config_file_exists {
        writeln!(output, "config source: loaded")?;
    } else {
        writeln!(output, "config source: defaults (file not found)")?;
    }
    writeln!(output, "command timeout: {timeout_ms} ms")?;
    writeln!(
        output,
        "sandbox engine: not checked (qn-nono integration pending)"
    )?;
    writeln!(output, "configuration: OK")?;

    Ok(())
}
