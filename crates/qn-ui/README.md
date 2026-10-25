# qn-ui

`qn-ui` owns the presentation contract for the future desktop control plane.
The current phase defines four safe view routes and a daemon-populated snapshot:
profiles, permissions, sessions, and audit.

This crate intentionally has no Tauri dependency yet. It performs no filesystem,
network, credential, adapter, or business logic. A later Tauri phase will render
these values and obtain them through the shared `qn-daemon` control plane after
that transport/auth contract is frozen under decision issue #192.

`Unavailable` and `Failed` states are explicit so the UI never renders missing
permissions or audit evidence as a successful empty state. Session cards keep
`cleanup: unconfirmed` visible until the execution-tree proof is complete.

## Preview fixtures

`PreviewPreset::{Ready, Degraded, Failed}` provides deterministic, credential-free
snapshots for UI smoke tests and desktop preview work. It never reads host state and
must not be presented as live daemon data.
