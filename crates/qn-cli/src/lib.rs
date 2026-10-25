//! Command-line interface for qn-sandbox.
//!
//! The CLI is a presentation adapter: it parses user intent and renders
//! diagnostics but does not implement sandbox behavior. Concrete engine and
//! credential adapters will be assembled through `qn-bootstrap`.

use std::io::Write;

use clap::{Parser, Subcommand};
use qn_config::Config;
use qn_config::paths::QnPaths;

mod audit;
mod profile;
mod session;
mod setup;
pub use audit::{AuditCommand, AuditFilter, AuditType};
pub use profile::ProfileCommand;

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
    /// Install or repair the pinned engine, then verify its version.
    Install {
        /// Emit a versioned machine-readable installation report.
        #[arg(long)]
        json: bool,
    },
    /// Read bounded broker audit evidence without changing authorization.
    Audit {
        /// Query operation for one canonical session.
        #[command(subcommand)]
        command: AuditCommand,
    },
    /// Check configuration and the integrity/version of the managed engine.
    Doctor {
        /// Emit versioned JSON diagnostics with safe display paths.
        #[arg(long)]
        json: bool,
        /// Check only configuration, without inspecting or executing the engine.
        #[arg(long)]
        config_only: bool,
    },
    /// Inspect and manage versioned official agent profiles.
    Profile {
        /// Profile catalog or registry operation.
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Run one official profile in the foreground.
    Run {
        /// Canonical managed profile identity, for example `qn/claude-code`.
        #[arg(long)]
        profile: String,
        /// Executable and arguments after `--`.
        #[arg(last = true, required = true)]
        command: Vec<String>,
        /// Emit a versioned machine-readable session report.
        #[arg(long)]
        json: bool,
    },
    /// Show the owner record and any live native observation for a session.
    Status {
        /// Canonical qn session identifier.
        session: String,
        /// Emit a versioned machine-readable session report.
        #[arg(long)]
        json: bool,
    },
    /// Ask the native engine to stop a recorded session.
    Stop {
        /// Canonical qn session identifier.
        session: String,
        /// Request immediate force-stop instead of graceful termination.
        #[arg(long)]
        force: bool,
        /// Grace period in whole seconds (1-300); ignored with --force.
        #[arg(long, default_value_t = 10)]
        timeout: u64,
        /// Emit a versioned machine-readable stop report.
        #[arg(long)]
        json: bool,
    },
}

/// Errors surfaced by CLI command execution.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Retained native evidence could not be queried safely.
    #[error(transparent)]
    NativeAudit(#[from] qn_core::native_audit::NativeAuditError),
    /// Engine installation or read-only integrity/version inspection failed.
    #[error(transparent)]
    Setup(#[from] qn_core::setup::SetupError),
    /// Invalid typed identifier; source never echoes user input.
    #[error(transparent)]
    Domain(#[from] qn_core::Error),
    /// Profile catalog or host registry refused the operation.
    #[error(transparent)]
    Profile(#[from] qn_core::profile::ProfileError),
    /// Owned foreground session operation failed.
    #[error(transparent)]
    Session(#[from] qn_core::runtime::SessionError),
    /// Follow detected an observed prefix rewrite or truncation.
    #[error("audit source changed; retain evidence and inspect its writer")]
    AuditChanged,
    /// Interrupt handler setup or delivery failed.
    #[error("audit interrupt handler failed: {0}")]
    Signal(#[source] std::io::Error),
    /// Configuration paths or values could not be resolved or loaded.
    #[error(transparent)]
    Config(#[from] qn_config::Error),

    /// The requested session is not a canonical or matching identity.
    #[error("invalid audit session; use the canonical sxn-<uuid-v7> session ID")]
    InvalidSession,

    /// A validated audit query failed without exposing source payloads.
    #[error(transparent)]
    Audit(#[from] qn_core::audit::AuditQueryError),

    /// Structured diagnostics could not be encoded.
    #[error("failed to encode diagnostics; retry without --json")]
    Json(#[from] serde_json::Error),

    /// Diagnostic output could not be written.
    #[error("failed to write diagnostic output: {0}")]
    Output(#[from] std::io::Error),

    /// The local runtime required by an owned operation could not start.
    #[error("failed to start the local operation runtime: {0}")]
    Runtime(#[source] std::io::Error),

    /// The current directory is unavailable or not UTF-8.
    #[error(
        "current directory is unavailable or not UTF-8; run from a dedicated project directory"
    )]
    Workdir,

    /// A graceful stop timeout is outside the supported whole-second range.
    #[error("invalid stop timeout; use a whole number of seconds from 1 to 300")]
    InvalidStopTimeout,
}

/// Executes one parsed CLI command and writes its user-facing output.
///
/// # Errors
///
/// Returns [`Error::Config`] when qn configuration cannot be resolved or
/// loaded, [`Error::Json`] when encoding fails, and [`Error::Output`] when
/// writing diagnostics fails.
pub fn run(cli: &Cli, output: &mut dyn Write) -> Result<(), Error> {
    match &cli.command {
        Command::Install { json } => {
            let paths = QnPaths::resolve()?;
            let installation = setup::inspect(qn_core::setup::SetupOperation::Install, &paths)?;
            if *json {
                setup::render_json(&installation, output)
            } else {
                setup::render(&installation, output)
            }
        }
        Command::Audit { command } => audit::run(command, output),
        Command::Doctor { json, config_only } => doctor(output, *json, *config_only),
        Command::Profile { command } => profile::run(command, output),
        Command::Run {
            profile,
            command,
            json,
        } => session::run(profile, command.clone(), *json, output),
        Command::Status { session: id, json } => session::status(id, *json, output),
        Command::Stop {
            session: id,
            force,
            timeout,
            json,
        } => session::stop(id, *force, *timeout, *json, output),
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

/// Renders one bounded audit timeline for human terminal output.
///
/// This function only formats the already validated core DTO. It does not read
/// files, query adapters, or claim that the evidence is authenticated.
///
/// # Errors
///
/// Returns [`Error::Output`] when writing the rendered timeline fails.
pub fn render_audit_text(
    timeline: &qn_core::audit::AuditTimeline,
    output: &mut dyn Write,
) -> Result<(), Error> {
    writeln!(output, "audit session: {}", timeline.session_id)?;
    writeln!(output, "source: {}", audit_source_label(timeline.source))?;
    writeln!(
        output,
        "integrity: {}",
        audit_integrity_label(timeline.integrity)
    )?;
    writeln!(output, "state: {}", audit_state_label(timeline.state))?;
    for event in &timeline.events {
        writeln!(
            output,
            "event {}: {} wall_ms={} elapsed_us={}",
            event.sequence,
            audit_event_label(event.event),
            event.wall_millis,
            event.elapsed_micros
        )?;
    }
    Ok(())
}

/// Serializes one bounded audit timeline as schema-versioned JSON.
///
/// The input is a core DTO with closed event enums; no source text or secret
/// value is accepted by this presentation boundary.
///
/// # Errors
///
/// Returns [`Error::Json`] when serialization fails or [`Error::Output`] when
/// writing the serialized document fails.
pub fn render_audit_json(
    timeline: &qn_core::audit::AuditTimeline,
    output: &mut dyn Write,
) -> Result<(), Error> {
    #[derive(serde::Serialize)]
    struct Report<'a> {
        schema_version: u8,
        timeline: &'a qn_core::audit::AuditTimeline,
    }
    let encoded = serde_json::to_string(&Report {
        schema_version: 1,
        timeline,
    })?;
    writeln!(output, "{encoded}")?;
    Ok(())
}

fn audit_source_label(source: qn_core::audit::AuditSource) -> &'static str {
    match source {
        qn_core::audit::AuditSource::Broker => "broker",
    }
}

fn audit_integrity_label(integrity: qn_core::audit::AuditIntegrity) -> &'static str {
    match integrity {
        qn_core::audit::AuditIntegrity::Unverified => "unverified",
    }
}

fn audit_state_label(state: qn_core::audit::JournalState) -> &'static str {
    match state {
        qn_core::audit::JournalState::Empty => "empty",
        qn_core::audit::JournalState::Open => "open",
        qn_core::audit::JournalState::Partial => "partial",
        qn_core::audit::JournalState::ObservedClosed => "observed_closed",
    }
}

fn audit_event_label(event: qn_core::audit::AuditEventKind) -> &'static str {
    match event {
        qn_core::audit::AuditEventKind::GrantIssued => "grant_issued",
        qn_core::audit::AuditEventKind::GrantRevoked => "grant_revoked",
        qn_core::audit::AuditEventKind::ProviderRequested => "provider_requested",
        qn_core::audit::AuditEventKind::DeliveryPrepared => "delivery_prepared",
        qn_core::audit::AuditEventKind::ReplyWritten => "reply_written",
        qn_core::audit::AuditEventKind::Denied => "denied",
        qn_core::audit::AuditEventKind::DeniedAfterRevoke => "denied_after_revoke",
        qn_core::audit::AuditEventKind::Failed => "failed",
        qn_core::audit::AuditEventKind::ServiceClosed => "service_closed",
    }
}

/// Selects the serialization format for an audit timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditOutputFormat {
    /// Human-readable terminal lines.
    Text,
    /// Versioned JSON for scripts and UI consumers.
    Json,
}

/// Selects an audit presentation after the source has validated its timeline.
///
/// This consumer boundary performs no source I/O. A later bootstrap composition
/// can pass an `AuditTimeline` here without making the CLI aware of its adapter.
///
/// # Errors
///
/// Returns [`Error::Config`] when `session` is not a canonical session ID, or
/// [`Error::Json`] / [`Error::Output`] from the selected renderer.
pub fn render_audit_for_session(
    session: &str,
    timeline: &qn_core::audit::AuditTimeline,
    format: AuditOutputFormat,
    output: &mut dyn Write,
) -> Result<(), Error> {
    let expected = qn_core::SessionId::try_from(session).map_err(|_| Error::InvalidSession)?;
    if expected != timeline.session_id {
        return Err(Error::InvalidSession);
    }
    match format {
        AuditOutputFormat::Text => render_audit_text(timeline, output),
        AuditOutputFormat::Json => render_audit_json(timeline, output),
    }
}

/// Escapes untrusted parser context before clap formats and prints diagnostics.
///
/// Static usage and help retain their original layout.
///
/// # Examples
///
/// ```
/// use clap::Parser;
/// let mut error = qn_cli::Cli::try_parse_from(["qn", "bad\ncommand"])
///     .expect_err("unknown subcommand");
/// qn_cli::sanitize_parse_error(&mut error);
/// assert!(!error.to_string().contains("bad\ncommand"));
/// ```
pub fn sanitize_parse_error(error: &mut clap::Error) {
    use clap::error::ContextValue;

    // In clap 4.6.7, this CLI's rejected arguments are String context values;
    // styled usage/help is static (the command declares no positional values).
    let replacements: Vec<_> = error
        .context()
        .filter_map(|(kind, value)| {
            let escaped = match value {
                ContextValue::String(text) => ContextValue::String(escape_terminal_text(text)),
                ContextValue::Strings(texts) => ContextValue::Strings(
                    texts
                        .iter()
                        .map(|text| escape_terminal_text(text))
                        .collect(),
                ),
                _ => return None,
            };
            Some((kind, escaped))
        })
        .collect();
    for (kind, value) in replacements {
        error.insert(kind, value);
    }
}

#[derive(serde::Serialize)]
struct DoctorReport {
    schema_version: u8,
    home: String,
    config_file: String,
    config_source: &'static str,
    command_timeout_ms: u64,
    sandbox_engine: &'static str,
    configuration: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<setup::EngineReport>,
}

fn doctor(output: &mut dyn Write, json: bool, config_only: bool) -> Result<(), Error> {
    let paths = QnPaths::resolve()?;
    let config_file_exists = paths.config_file.exists();
    let config = Config::load(&paths)?.apply_env_overrides()?;
    let timeout_ms = u64::try_from(config.command_timeout().as_millis()).unwrap_or(u64::MAX);
    let installation = if config_only {
        None
    } else {
        Some(setup::inspect(
            qn_core::setup::SetupOperation::Inspect,
            &paths,
        )?)
    };

    if json {
        let report = DoctorReport {
            schema_version: 1,
            home: escape_terminal_text(paths.root.as_str()),
            config_file: escape_terminal_text(paths.config_file.as_str()),
            config_source: if config_file_exists {
                "file"
            } else {
                "defaults"
            },
            command_timeout_ms: timeout_ms,
            sandbox_engine: if installation.is_some() {
                "binary_verified"
            } else {
                "not_checked"
            },
            configuration: "ok",
            engine: installation.as_ref().map(setup::EngineReport::from),
        };
        // Encode first so configuration/serialization failures cannot publish a partial report.
        let encoded = serde_json::to_string(&report)?;
        writeln!(output, "{encoded}")?;
        return Ok(());
    }

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
    if let Some(installation) = installation {
        setup::render(&installation, output)?;
    } else {
        writeln!(
            output,
            "sandbox engine: not checked (configuration-only mode)"
        )?;
    }
    writeln!(output, "configuration: OK")?;

    Ok(())
}

pub(crate) fn escape_terminal_text(text: &str) -> String {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod audit_render_tests {
    use super::{AuditOutputFormat, render_audit_json, render_audit_text};
    use qn_core::{
        SessionId,
        audit::{
            AuditEvent, AuditEventKind, AuditIntegrity, AuditSource, AuditTimeline, JournalState,
        },
    };
    use serde_json::Value;

    fn timeline(state: JournalState) -> AuditTimeline {
        AuditTimeline {
            session_id: SessionId::new(),
            source: AuditSource::Broker,
            integrity: AuditIntegrity::Unverified,
            state,
            events: vec![
                AuditEvent {
                    sequence: 1,
                    wall_millis: 10,
                    elapsed_micros: 20,
                    event: AuditEventKind::ProviderRequested,
                },
                AuditEvent {
                    sequence: 2,
                    wall_millis: 11,
                    elapsed_micros: 21,
                    event: AuditEventKind::ServiceClosed,
                },
            ],
        }
    }

    #[test]
    fn text_preserves_state_and_event_order() {
        for state in [
            JournalState::Empty,
            JournalState::Open,
            JournalState::Partial,
            JournalState::ObservedClosed,
        ] {
            let mut output = Vec::new();
            render_audit_text(&timeline(state), &mut output).unwrap();
            let text = String::from_utf8(output).unwrap();
            let expected_state = match state {
                JournalState::Empty => "empty",
                JournalState::Open => "open",
                JournalState::Partial => "partial",
                JournalState::ObservedClosed => "observed_closed",
            };
            assert!(text.contains(&format!("state: {expected_state}")));
            assert!(text.find("event 1").unwrap() < text.find("event 2").unwrap());
            assert!(text.contains("integrity: unverified"));
        }
    }

    #[test]
    fn consumer_rejects_invalid_or_mismatched_session_without_rendering() {
        let timeline = timeline(JournalState::Open);
        let mut output = Vec::new();
        assert!(matches!(
            super::render_audit_for_session("bad", &timeline, AuditOutputFormat::Text, &mut output),
            Err(super::Error::InvalidSession)
        ));
        assert!(output.is_empty());
        let other = qn_core::SessionId::new().to_string();
        assert!(matches!(
            super::render_audit_for_session(
                &other,
                &timeline,
                AuditOutputFormat::Text,
                &mut output
            ),
            Err(super::Error::InvalidSession)
        ));
        assert!(output.is_empty());
    }

    #[test]
    fn consumer_selects_text_or_json_for_a_matching_session() {
        let timeline = timeline(JournalState::Partial);
        let session = timeline.session_id.to_string();
        let mut text = Vec::new();
        super::render_audit_for_session(&session, &timeline, AuditOutputFormat::Text, &mut text)
            .unwrap();
        assert!(String::from_utf8(text).unwrap().contains("state: partial"));
        let mut json = Vec::new();
        super::render_audit_for_session(&session, &timeline, AuditOutputFormat::Json, &mut json)
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&json).unwrap()["schema_version"],
            1
        );
    }

    #[test]
    fn json_is_versioned_and_contains_only_core_timeline_fields() {
        let mut output = Vec::new();
        render_audit_json(&timeline(JournalState::Partial), &mut output).unwrap();
        let value: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["timeline"]["state"], "partial");
        assert_eq!(value["timeline"]["source"], "broker");
        assert_eq!(
            value["timeline"]["events"][0]["event"],
            "provider_requested"
        );
        assert_eq!(value["timeline"]["events"][1]["event"], "service_closed");
    }
}

#[cfg(test)]
mod audit_error_tests {
    use super::{Error, render_error};
    use qn_core::audit::AuditQueryError;

    #[test]
    fn audit_query_errors_preserve_safe_categories() {
        let cases = [
            (AuditQueryError::Missing, "not found"),
            (AuditQueryError::Configuration, "invalid audit source"),
            (AuditQueryError::Timeout, "timed out"),
            (AuditQueryError::Limit, "exceeds query limits"),
            (
                AuditQueryError::Invalid {
                    reason: "sequence gap",
                },
                "sequence gap",
            ),
        ];
        for (error, expected) in cases {
            let rendered = render_error(&Error::Audit(error));
            assert!(rendered.contains(expected), "{rendered}");
            assert!(!rendered.contains("secret"));
            assert!(!rendered.contains("/private/"));
        }
    }

    #[test]
    fn audit_decode_error_keeps_the_source_chain_without_raw_payload_contract() {
        let source = std::io::Error::other("synthetic audit source");
        let rendered = render_error(&Error::Audit(AuditQueryError::Decode(Box::new(source))));
        assert!(rendered.contains("audit record malformed"));
        assert!(rendered.contains("synthetic audit source"));
    }
}
