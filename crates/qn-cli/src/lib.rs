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

/// Renders an error for terminal presentation, escaping controls and bidi controls.
///
/// # Examples
///
/// ```
/// let error = qn_cli::Error::Output(std::io::Error::other("line\nbreak"));
/// let rendered = qn_cli::render_error(&error);
///
/// assert!(rendered.contains("line\\nbreak"));
/// ```
#[must_use]
pub fn render_error(error: &Error) -> String {
    escape_terminal_text(&error.to_string())
}

fn doctor(output: &mut dyn Write) -> Result<(), Error> {
    let paths = QnPaths::resolve()?;
    let config_file_exists = paths.config_file.exists();
    let config = Config::load(&paths)?.apply_env_overrides()?;
    let timeout_ms = u64::try_from(config.command_timeout().as_millis()).unwrap_or(u64::MAX);

    writeln!(output, "qn doctor")?;
    writeln!(
        output,
        "home: {}",
        escape_terminal_text(paths.root.as_str())
    )?;
    writeln!(
        output,
        "config: {}",
        escape_terminal_text(paths.config_file.as_str())
    )?;
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

fn escape_terminal_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if needs_terminal_escape(character) {
            escaped.extend(character.escape_debug());
        } else {
            escaped.push(character);
        }
    }
    escaped
}

fn needs_terminal_escape(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

#[cfg(test)]
mod tests {
    use super::escape_terminal_text;

    #[test]
    fn it_escapes_terminal_and_bidi_controls() {
        let cases = [
            ('\0', "\\0"),
            ('\u{85}', "\\u{85}"),
            ('\u{61c}', "\\u{61c}"),
            ('\u{200e}', "\\u{200e}"),
            ('\u{200f}', "\\u{200f}"),
            ('\u{2028}', "\\u{2028}"),
            ('\u{2029}', "\\u{2029}"),
            ('\u{202a}', "\\u{202a}"),
            ('\u{202b}', "\\u{202b}"),
            ('\u{202c}', "\\u{202c}"),
            ('\u{202d}', "\\u{202d}"),
            ('\u{202e}', "\\u{202e}"),
            ('\u{2066}', "\\u{2066}"),
            ('\u{2067}', "\\u{2067}"),
            ('\u{2068}', "\\u{2068}"),
            ('\u{2069}', "\\u{2069}"),
        ];

        for (control, expected) in cases {
            assert_eq!(
                escape_terminal_text(&control.to_string()),
                expected,
                "terminal control U+{:04X} must be rendered visibly",
                u32::from(control)
            );
        }
    }
}
