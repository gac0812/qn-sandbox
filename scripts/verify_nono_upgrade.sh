#!/usr/bin/env bash
# Pinned nono upgrade regression (#59 P1).
#
# Usage (on a branch that already bumps the pin and the pinned SHA assertion):
#   QN_TEST_NONO=/verified/nono \
#   QN_TEST_NONO_ARCHIVE=/verified/nono.tar.gz \
#   bash scripts/verify_nono_upgrade.sh
#
# Optional:
#   QN_TEST_PROFILE_PARENT=<private 0700 dir>   (default: /Users/Shared/qn-nono-upgrade-$$)
#   QN_TEST_REAL_AGENT=1 QN_TEST_CLAUDE=/opt/homebrew/bin/claude
#
# The script only exercises the candidate binary/archive. It does not change the
# pin, download anything, or use real credentials unless QN_TEST_REAL_AGENT=1.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "This native regression is macOS-only; run the WSL2 route separately." >&2
    exit 1
fi
: "${QN_TEST_NONO:?set QN_TEST_NONO to the checksum-verified candidate binary}"
: "${QN_TEST_NONO_ARCHIVE:?set QN_TEST_NONO_ARCHIVE to the matching pinned archive}"

NONO="$(cd -- "$(dirname -- "${QN_TEST_NONO}")" && pwd)/$(basename -- "${QN_TEST_NONO}")"
ARCHIVE="$(cd -- "$(dirname -- "${QN_TEST_NONO_ARCHIVE}")" && pwd)/$(basename -- "${QN_TEST_NONO_ARCHIVE}")"
[[ -x "${NONO}" ]] || { echo "candidate binary is not executable: ${NONO}" >&2; exit 1; }
[[ -f "${ARCHIVE}" ]] || { echo "candidate archive is missing: ${ARCHIVE}" >&2; exit 1; }
for tool in cargo python3 ruby shasum; do
    command -v "${tool}" >/dev/null 2>&1 || {
        echo "${tool} is required for the upgrade regression" >&2
        exit 1
    }
done

ORIGINAL_HOME="${HOME}"
CARGO_HOME="${CARGO_HOME:-${ORIGINAL_HOME}/.cargo}"
RUSTUP_HOME="${RUSTUP_HOME:-${ORIGINAL_HOME}/.rustup}"
PROFILE_PARENT="${QN_TEST_PROFILE_PARENT:-/Users/Shared/qn-nono-upgrade-$$}"
mkdir -p "${PROFILE_PARENT}"
chmod 700 "${PROFILE_PARENT}"
ROOT="$(mktemp -d "${PROFILE_PARENT}/root.XXXXXX")"
trap 'rm -rf "${ROOT}"' EXIT
for dir in home config state cache tmp; do
    mkdir -p "${ROOT}/${dir}"
done
chmod 700 "${ROOT}"

export QN_TEST_NONO="${NONO}"
export QN_TEST_NONO_ARCHIVE="${ARCHIVE}"
export QN_TEST_ROOT="${ROOT}"
export QN_TEST_PROFILE_PARENT="${PROFILE_PARENT}"
export QN_FAKE_SECRET="synthetic-upgrade-canary"

echo "== candidate =="
"${NONO}" --version
shasum -a 256 "${NONO}" "${ARCHIVE}"

run() {
    echo "== $* =="
    "$@"
}

# Default parser/fixture tests first; then the opt-in native contract suite.
run cargo test --locked -p qn-nono
run cargo test --locked -p qn-nono --test version_contract -- --ignored
run cargo test --locked -p qn-nono --lib \
    registry::tests::it_validates_embedded_assets_with_the_pinned_engine -- --ignored
run cargo test --locked -p qn-nono --test profile_canary -- --ignored
run cargo test --locked -p qn-nono --test managed_contract -- --ignored

# session_contract asserts HOME/XDG equal QN_TEST_ROOT subdirectories. Run it in
# a subshell so cargo keeps its real CARGO_HOME/RUSTUP_HOME.
(
    export HOME="${ROOT}/home"
    export XDG_CONFIG_HOME="${ROOT}/config"
    export XDG_STATE_HOME="${ROOT}/state"
    export XDG_CACHE_HOME="${ROOT}/cache"
    export TMPDIR="${ROOT}/tmp"
    export CARGO_HOME RUSTUP_HOME
    run cargo test --locked -p qn-nono --test session_contract -- --ignored
)

if [[ "$(uname -m)" == "arm64" ]]; then
    # This contract pins the 0.80.0 binary SHA. A version bump PR must update
    # that assertion together with the pin; otherwise the mismatch is expected.
    run cargo test --locked -p qn-nono --test proxy_port_contract -- --ignored
else
    echo "== proxy_port_contract skipped on $(uname -m) =="
fi

# Real CLI install + foreground run/status/stop against the cached archive.
run cargo test --locked -p qn-cli --test session -- --ignored

# Native capture service: four real child/socket scenarios and CLI audit queries.
run cargo build --locked -p qn-broker --example capture_relay
run cargo build --locked -p qn-bootstrap --example grant_capture_probe
run cargo build --locked -p qn-nono --example bound_launch_probe
run cargo build --locked -p qn-cli
run python3 tests/e2e/macos/capture_service/verify.py --workspace . --nono "${NONO}"

if [[ "${QN_TEST_REAL_AGENT:-0}" == "1" ]]; then
    : "${QN_TEST_CLAUDE:?set QN_TEST_CLAUDE for the real Agent regression}"
    run cargo test --locked -p qn-nono --test deepseek_claude_e2e -- --ignored
fi

echo "All pinned nono upgrade regressions passed."
