//! Internal 0.80.0 wire contract, measured on macOS in #46; decisions in #47.
//! No conversion to core states or inference of denials happens here.
use crate::{NonoError, error::ProtocolJsonError};
use qn_infra::process::CmdOutcome;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

// All payload-bearing Debug implementations intentionally hide fields.
macro_rules! record {
    ($(#[$meta:meta])* $visibility:vis $name:ident { $($(#[$attr:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Deserialize, serde::Serialize)]
        $visibility struct $name { $($(#[$attr])* pub $field: $ty),* }
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }
    };
}
// deserialize_with requires the key even when its value is nullable.
fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
record!(pub(crate) ProfileValidation { file: String, valid: bool, errors: Vec<String>, warnings: Vec<String>, deprecated_keys: u64 });
#[derive(Debug, Deserialize, serde::Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Status {
    Running,
    Paused,
    Exited,
}
record!(pub(crate) SessionRecord {
    session_id: String,
    #[serde(deserialize_with = "nullable")] name: Option<String>,
    status: Status,
    #[serde(deserialize_with = "nullable")] exit_code: Option<i32>,
});
record!(
    /// Native audit-list metadata, not a core session state or completeness proof.
    pub AuditSummary {
    /// Full native record identifier.
    session_id: String,
    /// Native start wall time when emitted; no synthetic timestamp is supplied.
    started: Option<String>,
    /// Native end wall time when emitted.
    ended: Option<String>,
    /// Native liveness observation.
    is_alive: bool,
    /// Native stale-record observation.
    is_stale: bool,
    /// Number of network events reported by nono.
    network_event_count: u64,
    /// Observed exit code, explicitly nullable.
    #[serde(deserialize_with = "nullable")] exit_code: Option<i32>,
});
record!(
    /// Native network event; fields are observations, not inferred denials.
    pub NetworkEvent {
    /// Proxy mode reported by nono.
    mode: String,
    /// Native event wall time in milliseconds, when emitted.
    timestamp_unix_ms: Option<u64>,
    /// Native allow/deny decision text.
    decision: String,
    /// Native denial category for presentation, when emitted.
    denial_category: Option<String>,
    /// Reported network target; explicit access is for audit presentation.
    target: String,
    /// Native destination port when emitted.
    port: Option<u16>,
});
record!(
    /// Native audit-show result; it does not assert complete filesystem coverage.
    pub AuditRecord {
    /// Full native record identifier.
    session_id: String,
    /// Native start wall time when emitted.
    started: Option<String>,
    /// Native end wall time when emitted.
    ended: Option<String>,
    /// Network events actually present in the native record.
    network_events: Vec<NetworkEvent>,
    /// Number of native audit events reported by nono.
    audit_event_count: u64,
    /// Observed exit code, explicitly nullable.
    #[serde(deserialize_with = "nullable")] exit_code: Option<i32>,
});
record!(pub(crate) Denial {
    path: String,
    access: String,
    reason: String
});
record!(pub(crate) Violation {
    operation: String,
    target: String
});
record!(pub(crate) Diagnostic {
    code: String,
    severity: String,
    message: String
});
record!(pub(crate) SessionDiagnostics { exit_code: i32, denials: Vec<Denial>, violations: Vec<Violation>, diagnostics: Vec<Diagnostic> });
record!(pub(crate) Diagnostics {
    session: SessionDiagnostics
});

fn rejected(operation: &'static str, reason: &'static str) -> NonoError {
    NonoError::Protocol { operation, reason }
}
fn json_error(operation: &'static str, source: serde_json::Error) -> NonoError {
    NonoError::ProtocolJson {
        operation,
        source: ProtocolJsonError(source),
    }
}
fn complete(out: &CmdOutcome, op: &'static str) -> Result<(), NonoError> {
    if out.stdout_truncated
        || out.stderr_truncated
        || out.stdout.len() > (1 << 20)
        || out.stderr.len() > (1 << 20)
    {
        return Err(rejected(op, "incomplete or oversized output"));
    }
    Ok(())
}
fn decode<T: DeserializeOwned>(out: &CmdOutcome, op: &'static str) -> Result<T, NonoError> {
    complete(out, op)?;
    serde_json::from_str(&out.stdout).map_err(|e| json_error(op, e))
}
pub(crate) fn profile(out: &CmdOutcome) -> Result<ProfileValidation, NonoError> {
    decode(out, "profile validate")
}
pub(crate) fn sessions(out: &CmdOutcome) -> Result<Vec<SessionRecord>, NonoError> {
    decode(out, "ps")
}
pub(crate) fn inspect(out: &CmdOutcome) -> Result<SessionRecord, NonoError> {
    decode(out, "inspect")
}
pub(crate) fn audit_list(out: &CmdOutcome) -> Result<Vec<AuditSummary>, NonoError> {
    decode(out, "audit list")
}
pub(crate) fn audit(out: &CmdOutcome) -> Result<AuditRecord, NonoError> {
    decode(out, "audit show")
}

/// Decode a single top-level diagnostic envelope among nono's text messages.
/// This validates shape, not provenance: stderr includes untrusted child output.
pub(crate) fn diagnostics(out: &CmdOutcome) -> Result<Diagnostics, NonoError> {
    const OP: &str = "diagnostics";
    complete(out, OP)?;
    let mut rest = out.stderr.as_str();
    let mut found = None;
    let mut candidates = 0;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.starts_with(['{', '[']) {
            candidates += 1;
            if candidates > 64 {
                return Err(rejected(OP, "too many JSON candidates"));
            }
            let mut stream = serde_json::Deserializer::from_str(rest).into_iter::<Value>();
            match stream.next() {
                Some(Ok(value)) => {
                    let end = stream.byte_offset();
                    if value.get("session").is_some() {
                        if found.is_some() {
                            return Err(rejected(OP, "ambiguous diagnostic envelopes"));
                        }
                        found = Some(
                            serde_json::from_str(&rest[..end]).map_err(|e| json_error(OP, e))?,
                        );
                    }
                    rest = &rest[end..];
                    continue;
                }
                Some(Err(e)) => {
                    // Only a closed, single-line text label can be skipped;
                    // never recover a nested report from a broken JSON container.
                    let close = if rest.starts_with('{') { '}' } else { ']' };
                    let is_text = rest[1..].split_once(close).is_some_and(|(label, _)| {
                        !label.is_empty() && label.chars().all(|c| c.is_alphabetic() || c == ' ')
                    });
                    if !is_text {
                        return Err(json_error(OP, e));
                    }
                }
                _ => (),
            }
        }
        rest = rest.split_once('\n').map_or("", |(_, tail)| tail);
    }
    found.ok_or_else(|| rejected(OP, "diagnostic envelope missing"))
}
#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
