# Native audited capture acceptance (#107)

Synthetic macOS arm64 fixtures, pinned nono 0.80.0 with embedded SHA-256 verification.

    cargo build --locked -p qn-broker --example capture_relay
    cargo build --locked -p qn-bootstrap --example grant_capture_probe
    cargo build --locked -p qn-nono --example bound_launch_probe
    python3 tests/e2e/macos/capture_service/verify.py --workspace . --nono /verified/nono

The adapter reserves qn/native identities before broker creation. Real native proxy requests
consume CaptureService and AuditedGrant. Normal traffic succeeds once then revoke produces
503; provider-race, wrong identity and exhausted audit produce zero upstream requests.
Sandbox reads of synthetic provider, binding, audit and SSH files are refused; inherited
secret/identity variables and direct broker connections are denied. The exact journal owner
and event order are verified. Test-only control TCP is not a product control-plane API.
No real credentials or model calls. Windows/Intel/installed product and whole-tree cleanup
remain unverified; #93 blocks production stop-success. Passing this suite is not full E2E.
