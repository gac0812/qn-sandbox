# SDK contract boundary

The TypeScript and Python SDK directories are reserved for generated clients and
small ergonomics layers. Their protocol source is not defined yet; the daemon
transport and request/result envelope must stabilize before generation begins.

Do not hand-write `gen/` or `_gen/` types, add a second HTTP client, or put API
keys/passwords/tokens in SDK request models. The current PR only establishes the
repository boundary and documentation.
