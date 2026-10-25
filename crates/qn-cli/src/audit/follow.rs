//! Polling continuity checks are observational, not cryptographic authentication (#120).
use super::{AuditFilter, render};
use crate::Error;
use qn_core::{
    SessionId,
    audit::{AuditEvent, AuditQuery, AuditTimeline, JournalState},
};
use std::{future::Future, io::Write, time::Duration};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Default)]
struct Cursor(Option<AuditTimeline>);
impl Cursor {
    fn observe(
        &mut self,
        next: AuditTimeline,
        filter: &AuditFilter,
    ) -> Result<Option<AuditTimeline>, Error> {
        let mut previous_len = 0;
        let mut state_changed = true;
        if let Some(previous) = &self.0 {
            if previous.session_id != next.session_id
                || previous.source != next.source
                || previous.integrity != next.integrity
                || next.events.len() < previous.events.len()
                || !previous
                    .events
                    .iter()
                    .zip(&next.events)
                    .all(|(a, b)| same_event(a, b))
            {
                return Err(Error::AuditChanged);
            }
            previous_len = previous.events.len();
            state_changed = previous.state != next.state;
        }
        let batch = AuditTimeline {
            session_id: next.session_id,
            source: next.source,
            integrity: next.integrity,
            state: next.state,
            events: next.events[previous_len..]
                .iter()
                .filter(|event| filter.matches(event))
                .cloned()
                .collect(),
        };
        self.0 = Some(next);
        Ok((state_changed || !batch.events.is_empty()).then_some(batch))
    }

    fn is_closed(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|value| value.state == JournalState::ObservedClosed)
    }
}

fn same_event(a: &AuditEvent, b: &AuditEvent) -> bool {
    a.sequence == b.sequence
        && a.wall_millis == b.wall_millis
        && a.elapsed_micros == b.elapsed_micros
        && a.event == b.event
}

pub(super) async fn stream(
    query: &dyn AuditQuery,
    session: SessionId,
    filter: &AuditFilter,
    json: bool,
    output: &mut dyn Write,
    cancel: impl Future<Output = std::io::Result<()>>,
) -> Result<(), Error> {
    tokio::pin!(cancel);
    let mut cursor = Cursor::default();
    loop {
        // Poll cancellation first so the OS listener exists before the first output.
        let next = tokio::select! {
            biased;
            result = &mut cancel => return result.map_err(Error::Signal),
            result = query.read(session) => result?,
        };
        if let Some(batch) = cursor.observe(next, filter)? {
            render(&batch, json, output)?;
        }
        if cursor.is_closed() {
            return Ok(());
        }
        tokio::select! {
            biased;
            result = &mut cancel => return result.map_err(Error::Signal),
            () = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }
}

#[cfg(test)]
mod tests;
