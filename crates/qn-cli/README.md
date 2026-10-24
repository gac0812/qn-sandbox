# qn-cli

`qn` is the command-line surface for qn-sandbox. This crate currently covers
the first executable and configuration-diagnostics slice; sandbox execution is
not implemented here yet.

## Commands

```text
qn --version
qn --help
qn doctor
```

`qn doctor` resolves `QN_HOME` (falling back to the platform default), loads
`config.toml` when present, applies `QN_*` environment overrides, and reports
the effective command timeout.

The current `doctor` output explicitly marks the sandbox engine as **not
checked**. It is a configuration diagnostic only and must not be treated as
evidence that nono or a restricted Agent session is ready.

## Development

```bash
cargo run -p qn-cli -- --version
cargo run -p qn-cli -- doctor
cargo test -p qn-cli
```

The CLI does not call nono directly. Concrete sandbox adapters are assembled
through `qn-bootstrap` once the core ports are frozen.
