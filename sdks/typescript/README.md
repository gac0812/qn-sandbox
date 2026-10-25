# qn-sandbox TypeScript SDK boundary

This directory reserves the generated TypeScript SDK boundary described by
`docs/spec/03-coding-standard-sdk.md`. Generated files belong in `src/gen/` and
must come from the single daemon protocol contract. The current repository does
not yet publish an HTTP/ConnectRPC protocol or generator, so no hand-written
client or parallel domain types are added here.

The future SDK must keep credentials host-mediated, expose no WSL/nono paths,
and map daemon errors to typed `QnError` subclasses. Restore generation before
adding runtime client code.
