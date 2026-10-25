# qn-daemon

## ControlPlane

`ControlPlane` is the shared local composition root for CLI, UI and future MCP surfaces. It resolves one `QnPaths` value and assembles profile, engine setup, broker audit, native audit and session operations through `qn-bootstrap`. It does not expose transport or adapter types.
