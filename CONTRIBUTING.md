# Contributing

## Repository Baseline

`v0` is an immutable tag on the empty baseline commit created before any team
implementation. Do not move, recreate, or force-push this tag.

`P0`, `P1`, and `P2` are delivery priorities represented by GitHub labels. They
are not version tags, branch suffixes, file names, or class names.

## Workflow

1. Create or select a focused GitHub Issue.
2. Create a short-lived branch from `main`:

   ```text
   <type>/<issue>-<slug>
   ```

   Examples:

   ```text
   feat/4-credential-broker
   test/7-attack-suite
   docs/8-architecture
   ```

3. Use an English Conventional Commit title and keep each commit buildable:

   ```text
   feat(broker): add scoped credential exchange
   ```

4. Open a pull request with an English title, link the Issue, and include:
   - the problem and intended behavior
   - the security boundary affected
   - verification evidence
   - compatibility and rollback notes
   - known limitations
5. Obtain at least one human review before merging to `main`.

GitHub Actions CI is temporarily paused for the private repository under #145 due to
runner/quota availability. Until a restoration PR re-enables it, every PR must include
fresh local fmt, clippy, test, strict rustdoc, architecture and dependency evidence;
restoring CI requires a separate reviewed PR.
6. Delete the branch after merge.

## Priority Labels

- `P0`: required for the hackathon demonstration and submission.
- `P1`: valuable after the complete P0 path works.
- `P2`: optional polish or expansion that must not block P0.

## Security Rules

- Never commit real API keys, OAuth tokens, passwords, private keys, Keychain
  exports, host agent configuration, or internal hackathon materials.
- Do not print secrets in Issues, pull requests, logs, screenshots, tests, or
  demo output.
- Run a staged-diff review before every commit:

  ```bash
  git diff --cached
  ```

- Prefer synthetic credentials in tests and revoke any credential that may
  have been exposed.
- Report security defects through a private channel rather than a public Issue.

## Definition of Done

A change is complete only when:

- its acceptance criteria are met;
- relevant tests and security checks pass;
- the PR contains reproducible verification evidence;
- documentation and run instructions are updated when behavior changes;
- a human reviewer approves the exact revision being merged.
