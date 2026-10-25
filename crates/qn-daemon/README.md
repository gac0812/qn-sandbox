# qn-daemon

## ControlPlane

`ControlPlane` is the shared local composition root for CLI, UI and future MCP surfaces. It resolves one `QnPaths` value and assembles profile, engine setup, broker audit, native audit and session operations through `qn-bootstrap`. It does not expose transport or adapter types.

## Contract verification

The `control_plane` integration tests verify that relative roots are rejected
before adapter construction and that an absolute root exposes the shared profile,
engine, broker audit and native audit surfaces. The daemon still owns no transport;
UI, MCP and SDK consumers must use this composition rather than importing adapters.


## C1 delivery boundary

The C1 control-plane slice is complete at the local composition boundary. `ControlPlane` provides these shared capabilities:

| Capability | Surface method | Source |
| --- | --- | --- |
| Profile catalog and resolution | `list_profiles`, `resolve_profile`, `install_profile`, `rollback_profile`, `validate_profile`, `publish_profile` | `qn-bootstrap` profile registry and validators |
| Managed engine | `inspect_engine`, `install_engine` | `qn-core::setup::EngineSetup` |
| Broker audit | `read_audit` | `qn-core::audit::AuditQuery` |
| Native audit | `read_native_audit` | `qn-core::native_audit::NativeAuditQuery` |
| Foreground sessions | `run_session`, `status_session`, `stop_session` | `qn-core::runtime` through `qn-bootstrap` |

The CLI consumes this composition for engine, audit, native-audit, session and
profile operations. A surface receives typed core results and maps errors at its
own presentation boundary; it never opens adapter files or calls `nono` directly.

C1 deliberately stops before transport. The daemon has no HTTP/RPC listener,
local authentication protocol, UI state protocol or MCP tool transport. Those
contracts belong to the next control-plane/API work and require coordination with
the UI and MCP/SDK owners. The composition root is the stable handoff point for
those consumers.

Every session result preserves `cleanup: unconfirmed` until the execution-tree
proof is available. Audit results preserve source, integrity and incomplete-state
markers; the daemon does not upgrade observed records into authenticated claims.
The composition root performs no credential resolution and does not widen grants.
