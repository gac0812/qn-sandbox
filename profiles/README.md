# Official agent profiles

Versioned nono profile assets published by qn-sandbox. Each asset is plain
[nono profile JSON](https://nono.sh/docs) with `extends: "default"`, so the
required `deny_credentials`, keychain and shell-history deny groups stay active.

| qn id | asset | app state | outbound allowlist | credential service |
| --- | --- | --- | --- | --- |
| `qn/claude-code` | `claude-code.json` | `$HOME/.claude` | `api.anthropic.com`, `console.anthropic.com`, `statsig.anthropic.com` | `anthropic` |
| `qn/codex` | `codex.json` | `$HOME/.codex` | `api.openai.com`, `auth.openai.com`, `chatgpt.com`, `platform.openai.com` | `openai` |

## Invariants

- `$HOME` is the managed, isolated session home created by qn, never the host home.
- Filesystem grants are explicit; `$WORKDIR` is the selected project directory.
- Outbound traffic is proxy-filtered by `network.allow_domain`; there is no
  `block: false` bypass and no `allow_domain: ["*"]`.
- `network.credentials` names a nono reverse-proxy service. Real credential
  material stays in the host keystore and is never copied into the sandbox.
- `claude_code_macos` / `codex_macos` policy groups are deliberately **not**
  included: they grant read/write access to the login keychain.
- These assets contain no secret, token, host path or user identifier.

## Validation

Every asset must pass the pinned engine's strict validator before publication:

```sh
nono profile validate --json --strict -- profiles/claude-code.json
nono profile validate --json --strict -- profiles/codex.json
```

Run the same validation through the product with `qn profile validate <id>`; it verifies
the managed engine bytes before executing and never downloads.

`manifest.json` is the qn publication metadata. It is parsed with unknown-field
rejection by `qn-nono`; the embedded assets are the compile-time source of
truth for the built-in catalog. Installing a profile copies the asset and its
SHA-256 into the qn profile registry; rollback selects the previous verified
version.

## Ownership

Member A owns these assets. A change to a filesystem, network or credential
binding is a security-relevant change: update the version, re-run the native
validation above, and include the evidence in the PR.
