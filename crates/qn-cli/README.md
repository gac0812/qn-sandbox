# qn-cli

`qn` is the command-line surface for qn-sandbox. This crate currently covers
the first executable and configuration-diagnostics slice; sandbox execution is
not implemented here yet.

## Commands

```text
qn --version
qn --help
qn doctor
qn doctor --json
qn doctor --config-only
qn doctor --config-only --json
qn profile list [--json]
qn profile show <id> [--json]
qn profile install <id>
qn profile rollback <id>
```

`qn doctor` resolves `QN_HOME` (falling back to the platform default), loads
`config.toml` when present, applies `QN_*` environment overrides, and reports
the effective command timeout.

Environment overrides fail closed when present but not valid UTF-8. Paths in
both normal output and error output escape terminal control characters and
bidi controls before display. Invalid argument and subcommand diagnostics use
that same escaping before clap formats the error; help, usage layout, output
streams, and exit codes retain clap's normal behavior.

The current `doctor` output explicitly marks the sandbox engine as **not
checked**. It is a configuration diagnostic only and must not be treated as
evidence that nono or a restricted Agent session is ready.

`qn doctor --config-only` explicitly requests configuration-only diagnostics.
It currently has the same output as `qn doctor`, because engine probing is not
wired yet. The flag is reserved for the engine-aware doctor flow and keeps this
configuration contract stable when that flow is added.

## Structured configuration diagnostics

`qn doctor --json` emits one JSON object on stdout and exits 0 when configuration
is valid. It uses the same file loading and environment precedence as text mode.

| Field | Version 1 value / meaning |
| --- | --- |
| `schema_version` | Numeric `1`; incompatible format changes require a new version |
| `home`, `config_file` | Terminal-safe display paths, strings |
| `config_source` | `defaults` for a missing file, otherwise `file` |
| `command_timeout_ms` | Effective timeout as an unsigned number of milliseconds |
| `sandbox_engine` | `not_checked`; no nono, isolation or credential checks performed |
| `configuration` | `ok`; configuration validity only |

Display paths escape controls and bidi characters **before** JSON encoding.
Decoding JSON therefore still gives safe display text, such as a literal `\n`
for a newline in a path. These fields are not filesystem path round-trip values.
Normal Unicode and path separators are retained.

Configuration errors exit 1 with the existing redacted text diagnostic on stderr
and no JSON on stdout. Argument errors use stderr and exit 2. Consumers must
check the exit code before parsing stdout; errors do not use a JSON schema.
Output write failures can leave an incomplete stream and must also be treated as
failure. No credential values or raw parser text are added to the report.

## Development

```bash
cargo run -p qn-cli -- --version
cargo run -p qn-cli -- doctor
cargo test -p qn-cli
```

The CLI does not call nono directly. Concrete sandbox adapters are assembled
through `qn-bootstrap` once the core ports are frozen.

## Audit presentation

The CLI crate also exposes pure renderers for a validated `qn-core::audit::AuditTimeline`:
`render_audit_text` for terminal output and `render_audit_json` for a versioned JSON
object. They do not read files, query adapters, or invent audit evidence. The actual
reader and `qn audit` command are separate integration work; a rendered timeline keeps
`integrity` and incomplete journal states visible.
## Official profile catalog (#75 / #59)

    qn profile list --json
    qn profile show qn/claude-code --json
    qn profile install qn/claude-code
    qn profile rollback qn/claude-code

`list` includes compiled and installed versions with an explicit `builtin`/`installed`
source. `show` renders the resolved nono asset, its isolated app state directory and its
proxy allowlist. `install` copies the compiled version into `QN_HOME/profiles` and selects
it; `rollback` selects the previous verified version. JSON uses schema_version=1.
Installed assets are checked against their SHA-256 on every resolve. These commands do not
start an engine, resolve a credential or prove sandbox/production readiness.
