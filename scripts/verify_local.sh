#!/usr/bin/env bash
# Run the repository's local quality gates while GitHub Actions is paused (#145).
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

printf '%s\n' '== architecture tests =='
python3 -m unittest discover -s scripts -p 'test_*.py'
printf '%s\n' '== architecture check =='
python3 scripts/check_architecture.py
printf '%s\n' '== format =='
cargo fmt --all -- --check
printf '%s\n' '== clippy =='
cargo clippy --locked --workspace --all-targets -- -D warnings
printf '%s\n' '== tests =='
cargo test --locked --workspace
printf '%s\n' '== rustdoc =='
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
printf '%s\n' '== dependency audit =='
if ! command -v cargo-deny >/dev/null 2>&1; then
    echo "cargo-deny is required; install it or run 'cargo deny check licenses bans sources advisories' manually." >&2
    exit 1
fi
cargo deny check licenses bans sources advisories
printf '%s\n' 'All local quality gates passed.'
