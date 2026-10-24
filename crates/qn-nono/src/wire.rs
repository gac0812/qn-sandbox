//! Internal 0.80.0 wire contract, measured on macOS in #46; decisions in #47.
//! No conversion to core states or inference of denials happens here.
use crate::{NonoError, error::ProtocolJsonError};
use qn_infra::process::CmdOutcome;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

// All payload-bearing Debug implementations intentionally hide fields.
macro_rules! record {
    ($name:ident { $($(#[$attr:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Deserialize)]
        pub(crate) struct $name { $($(#[$attr])* pub $field: $ty),* }
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
record!(ProfileValidation { file: String, valid: bool, errors: Vec<String>, warnings: Vec<String>, deprecated_keys: u64 });
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Status {
    Running,
    Paused,
    Exited,
}
record!(SessionRecord {
    session_id: String,
    #[serde(deserialize_with = "nullable")] name: Option<String>,
    status: Status,
    #[serde(deserialize_with = "nullable")] exit_code: Option<i32>,
});
record!(AuditSummary {
    session_id: String, is_alive: bool, is_stale: bool, network_event_count: u64,
    #[serde(deserialize_with = "nullable")] exit_code: Option<i32>,
});
record!(NetworkEvent {
    mode: String,
    decision: String,
    target: String
});
record!(AuditRecord {
    session_id: String, network_events: Vec<NetworkEvent>, audit_event_count: u64,
    #[serde(deserialize_with = "nullable")] exit_code: Option<i32>,
});
record!(Denial {
    path: String,
    access: String,
    reason: String
});
record!(Violation {
    operation: String,
    target: String
});
record!(Diagnostic {
    code: String,
    severity: String,
    message: String
});
record!(SessionDiagnostics { exit_code: i32, denials: Vec<Denial>, violations: Vec<Violation>, diagnostics: Vec<Diagnostic> });
record!(Diagnostics {
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
