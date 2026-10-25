# Control-plane contract fixture

`control-plane-v1.json` is a checked-in, human-readable contract fixture for
future generated TypeScript/Python SDK bindings. It is not generated source,
not a transport implementation, and not an authorization policy.

The fixture deliberately includes only credential-free argument shapes and the
credential posture visible to consumers. Runtime request/result wire types stay
in `qn-mcp`; the daemon transport must become the single source before SDK code
is generated. Keep this fixture synchronized with the approved protocol issue
before adding `src/gen` or `_gen` output.
