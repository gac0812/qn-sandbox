# DeepSeek Claude Code end-to-end fixture

Opt-in real authenticated model task through the pinned engine. The fixture uses the
host's existing Claude Code configuration (`~/.claude/settings.json`:
`ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic`, `ANTHROPIC_AUTH_TOKEN`) but
never places the real token in the sandbox.

## How the credential moves

1. The host-side test reads `ANTHROPIC_AUTH_TOKEN` from `~/.claude/settings.json`.
2. It writes the token to a `0600` temporary file under the host-only fixture root.
3. The test-only nono profile declares a `file://` custom credential and activates it
   with `network.credentials: ["deepseek"]`. nono reads the file in its supervisor and
   injects `Authorization: Bearer …` for `api.deepseek.com`.
4. The sandboxed Claude Code process receives only a 64-character phantom in
   `ANTHROPIC_AUTH_TOKEN`; the real token never enters its filesystem, environment,
   argv or output.

## Run

```sh
QN_TEST_NONO=/verified/nono \
QN_TEST_PROFILE_PARENT=/Users/Shared/qn-profile-parent \
QN_TEST_CLAUDE=/opt/homebrew/bin/claude \
cargo test --locked -p qn-nono --test deepseek_claude_e2e -- --ignored --nocapture
```

Expected: `deepseek claude answer: qn-deepseek-ok` and `1 passed`.

## Profile and flags

- `claude-deepseek.json` is test-only. It grants the project and the isolated
  `$HOME`, allows only `api.deepseek.com`, and sets the DeepSeek base URL/model.
- `allow_parent_of_protected` is required because Claude Code reads its isolated
  `$HOME`; nono still keeps its own `$HOME/.nono` state denied.
- `CLAUDE_CODE_ALLOW_OTHER_MODELS=1` suppresses the SDK's unknown-model warning for
  DeepSeek model names; the warning is non-fatal.
- `--dangerously-skip-permissions` disables Claude Code's **inner** permission checks.
  nono remains the outer sandbox; this is not a product default.
- The keychain, setuid helper and `/private/tmp/cc-socks` socket denials in nono's
  diagnostics are expected and are not credential paths.

## Limitations

macOS arm64 only, pinned nono 0.80.0, synthetic test profile. This proves the real
authenticated model task and the credential-injection path. It does not prove #93
execution-tree cleanup, Windows/WSL, Intel Mac or product profile activation.
