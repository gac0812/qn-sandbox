# qn-broker

Broker-owned capture decisions use a bounded, host-only audit::BrokerJournal (#105).
The caller supplies a new private regular file, retains its path and persists the directory entry.
Only fixed events, qn SessionId, sequence and wall/elapsed timestamps are recorded; no credential,
request path, grant handle, native identity or provider payload enters the journal.

Each append awaits write, flush and data sync before acknowledgment. Cancellation, timeout,
IO error or exhausted capacity permanently disables the journal; credential delivery must stop.
An unacknowledged partial or complete record may remain and must be treated as uncertain.
This supplemental source is broker, not nono audit, and does not claim tamper evidence.
Native nono audit remains authoritative for native decisions. Query/rotation and capture-service
composition are pending; passing journal tests is not full Agent/Windows/security-path acceptance.
