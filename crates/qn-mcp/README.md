# qn-mcp

`qn-mcp` defines the versioned, data-only MCP tool catalog consumed by future
`qn-daemon`, transport and SDK surfaces.

## Contract v1

The catalog is serialized with `schema_version: 1` and provides these stable
snake_case tools:

| Tool | Credential posture | Purpose |
| --- | --- | --- |
| `profile_list` | `none` | List official profiles |
| `profile_show` | `none` | Show one resolved profile |
| `session_run` | `host_mediated` | Start a restricted agent session |
| `session_status` | `none` | Read session state |
| `session_stop` | `none` | Request a session stop |
| `audit_query` | `none` | Query bounded audit evidence |
| `mcp_route_plan` | `host_mediated` | Plan an approved route |

`ToolCatalog::standard()` returns deterministic name ordering. `validate()`
rejects duplicate/invalid names and credential-shaped input fields. The
catalog does not contain API keys, passwords, tokens, handles, file paths or
provider payloads. `host_mediated` means a later host-side broker/gateway
performs authentication; it never means the tool receives a credential value.

## Boundary

This crate is metadata only. It does not open an MCP transport, bind a socket,
call `qn-daemon`, read files, access `qn-broker`/`qn-nono`, or authorize a
request. Later PRs will add request/result envelopes, daemon dispatch,
transport authentication and generated SDK bindings against this contract.

Consumers should validate the catalog before publishing it and treat
`schema_version` as a compatibility boundary. Unknown versions must be
rejected or handled by an explicit compatibility path.
