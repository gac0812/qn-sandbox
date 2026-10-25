# qn-sandbox Python SDK boundary

This directory reserves the generated Python SDK boundary described by
`docs/spec/03-coding-standard-sdk.md`. Generated files belong in `src/qn_sandbox/_gen/`
and must come from the single daemon protocol contract. The current repository
does not yet publish an HTTP/ConnectRPC protocol or generator, so no hand-written
client or parallel domain types are added here.

The future sync and async clients must share one channel layer, preserve error
causes, and keep credentials host-mediated. Platform details and nono paths stay
inside the control plane.
