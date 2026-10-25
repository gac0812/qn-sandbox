# qn-daemon

## ControlPlane

`ControlPlane` is the shared local composition root for CLI, UI and future MCP surfaces. It resolves one `QnPaths` value and assembles profile, engine setup, broker audit, native audit and session operations through `qn-bootstrap`. It does not expose transport or adapter types.

## Contract verification

The `control_plane` integration tests verify that relative roots are rejected
before adapter construction and that an absolute root exposes the shared profile,
engine, broker audit and native audit surfaces. The daemon still owns no transport;
UI, MCP and SDK consumers must use this composition rather than importing adapters.
