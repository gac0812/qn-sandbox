# qn-cli

`qn` is the command-line surface for qn-sandbox. This crate currently covers
configuration diagnostics, managed engine installation/inspection and broker audit queries.
Production sandbox execution is not implemented here yet.

## Commands

```text
qn --version
qn --help
qn install
qn doctor
qn doctor --json
qn doctor --config-only
qn doctor --config-only --json
qn profile list [--json]
qn profile show <id> [--json]
qn profile install <id>
qn profile rollback <id>
qn profile validate <id> [--json]
qn profile publish --id <id> --version <version> <absolute-asset-path>
```

`qn doctor` resolves `QN_HOME` (falling back to the platform default), loads
`config.toml` when present, applies `QN_*` environment overrides, and reports
the effective command timeout.

Environment overrides fail closed when present but not valid UTF-8. Paths in
both normal output and error output escape terminal control characters and
bidi controls before display. Invalid argument and subcommand diagnostics use
that same escaping before clap formats the error; help, usage layout, output
streams, and exit codes retain clap's normal behavior.

The default doctor validates configuration, checks the installed binary against the
checksum-pinned cached archive, then executes the exact managed binary to verify its
version. It never downloads or repairs. Missing/corrupt installs fail with guidance to
run the explicit mutation command, qn install. The managed root is
QN_HOME/cache/nono/<host-target>/<version>. No executable is discovered through PATH.

qn install reuses the existing pinned downloader and repair path, then probes the
version. Both commands await their owned blocking operation. Success verifies the
binary only, not sandbox isolation, credentials, full process cleanup or WSL readiness.
Native Windows setup remains unsupported until the WSL installation route is connected.

qn doctor --config-only preserves the configuration-only text and JSON contract,
without inspecting or executing an engine.

## Structured configuration diagnostics

`qn doctor --json` emits one JSON object on stdout and exits 0 when configuration and
engine inspection succeed. With --config-only, configuration validity is sufficient. It uses the same file loading and environment precedence as text mode.

| Field | Version 1 value / meaning |
| --- | --- |
| `schema_version` | Numeric `1`; incompatible format changes require a new version |
| `home`, `config_file` | Terminal-safe display paths, strings |
| `config_source` | `defaults` for a missing file, otherwise `file` |
| `command_timeout_ms` | Effective timeout as an unsigned number of milliseconds |
| `sandbox_engine` | `not_checked`; configuration-only mode. Default doctor reports binary_verified after integrity/version checks |
| `configuration` | `ok`; configuration validity only |

Display paths escape controls and bidi characters **before** JSON encoding.
Decoding JSON therefore still gives safe display text, such as a literal `\n`
for a newline in a path. These fields are not filesystem path round-trip values.
Normal Unicode and path separators are retained.

Configuration or engine errors exit 1 with the existing redacted text diagnostic on stderr
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
through `qn-bootstrap` using the existing core ports.

## Audit presentation

The CLI crate also exposes pure renderers for a validated `qn-core::audit::AuditTimeline`:
`render_audit_text` for terminal output and `render_audit_json` for a versioned JSON
object. They do not read files, query adapters, or invent audit evidence. The actual
reader and `qn audit` command consume these renderers through core/bootstrap; a rendered timeline keeps
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

`qn profile validate <id>` verifies the managed engine's archive and binary integrity,
then runs the pinned `nono profile validate --json --strict` against the resolved asset in
a private temporary file. It never downloads or repairs. Success means the engine accepted
the profile syntax and references; it does not start a sandbox or prove runtime behavior.

`qn profile publish` validates an absolute, bounded, regular local asset the same way, then
installs it as the requested strict version and selects it. Only compiled official
identities are accepted, so the app state directory stays catalog-owned. The asset
`meta.version` must equal `--version`; conflicting bytes for an existing version are
refused. Rollback remains available to the previous verified version.

## Broker audit commands (#110 / #120)

    qn audit show sxn-<canonical-uuid-v7> --json
    qn audit list --session sxn-<canonical-uuid-v7> --type denial --since 0
    qn audit follow --session sxn-<canonical-uuid-v7> --type grant --json

Reads QN_HOME/audit/<session>.broker.jsonl through core/bootstrap. JSON retains
schema_version=1 and timeline, including source/integrity/state. Filters apply only after
full validation. Follow emits new matching records, detects observed prefix changes and
stops at closure or Ctrl-C. Missing, partial and corrupt evidence remain distinct.
No cross-session ordering, native filesystem denial inference or authenticated provenance.

Default doctor JSON adds an engine object (name, version, terminal-safe binary,
runtime_checks=not_checked). Configuration-only JSON omits this object and retains
the original schema_version=1 fields and values.

The denial filter includes denied and denied_after_revoke. The latter is emitted
only from retained exact unexpired broker evidence; old generic journals remain readable.

## Retained native audit (#121)

    qn audit native sxn-<canonical-uuid-v7> --json

Resolves the launch-owned association, verifies engine integrity and queries the exact
native record. JSON is schema_version=1 with report (source=nono_audit,
integrity=unverified, coverage=network_only). No raw target/path/command/reason text is
printed. A missing association or record fails; neither zero events nor a successful
exit proves filesystem coverage, complete cleanup or authorization.
