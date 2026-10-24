# Secure Agent Sandbox

Desktop-grade security sandbox for mainstream AI coding agents.

The product is designed around one primary promise: users can run agents such
as Claude Code, Codex, and dsh without exposing API keys, passwords, OAuth
tokens, or MCP credentials inside the sandbox. Security comes first; setup must
remain approachable for users who do not want to write technical policy files.

## Status

This repository is under active development for the Qiniu 1024 Hackathon.

The repository starts from an empty, immutable `v0` baseline. The first
deliverable, tracked as `P0`, is a working security path rather than a broad
feature surface:

1. Sandbox adapter
2. Host-side credential broker
3. Host-side MCP gateway
4. Claude Code and Codex profiles
5. Minimal desktop control plane
6. Audit and attack-demonstration flow

## Security Invariants

Every implementation decision must preserve these invariants:

- Real credentials never enter the sandbox filesystem, environment, process
  arguments, or agent context.
- Agents receive only short-lived, scoped placeholders or broker handles.
- MCP secrets are injected on the host side and are never sent to the agent.
- Filesystem and network permissions default to deny and are granted per
  profile.
- Privileged actions produce an auditable event that is visible to the user.
- Revoking a session invalidates its placeholders and broker access.

## Target Architecture

The planned product layer is intentionally thin rather than a broad fork:

- **Desktop control plane:** Tauri-based UX for profiles, permissions, status,
  and audit review.
- **Sandbox engine:** `nono` as the security kernel and enforcement boundary.
- **Credential broker:** host-side exchange of scoped placeholders for real
  credentials, with explicit upstream allowlists.
- **MCP gateway:** host-side MCP mediation so agent configuration contains
  handles, not secrets.
- **Agent profiles:** tested, least-privilege defaults for supported agents.
- **Audit and approvals:** human-readable records of sandbox and credential
  activity.

## Repository Scope

The internal hackathon materials and local credentials are not part of this
repository. Proof-of-concept code is kept separate from product implementation
and is reviewed through its own pull request.

See [CONTRIBUTING.md](CONTRIBUTING.md) for branch, review, and security rules.
