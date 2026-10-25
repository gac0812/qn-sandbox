"""Check declared workspace dependency layers from Cargo metadata (spec 01 §3)."""

import json
from pathlib import Path
import subprocess
import sys

BASE = {"qn-config", "qn-infra"}
CORE = {"qn-core"}
ADAPTERS = {"qn-nono", "qn-broker", "qn-gateway", "qn-wsl", "qn-vm"}
SURFACES = {"qn-cli", "qn-daemon", "qn-ui", "qn-mcp"}
LAYERS = BASE | CORE | ADAPTERS | SURFACES | {"qn-bootstrap"}


def allowed_dependencies(name):
    """Return allowed internal targets under the existing architecture rules."""
    if name in BASE:
        return BASE - {name}
    if name == "qn-wsl":
        # Decision #188: qn-wsl stages the pinned nono Linux artifact through
        # qn-nono's installer (documented by upstream as the qn-wsl consumer).
        # This is the single allowed adapter-to-adapter edge; all others stay
        # forbidden. qn-nono modules other than installer remain off-limits
        # via review (spec 01 section 3 note).
        return (BASE | CORE | {"qn-nono"}) - {name}
    if name == "qn-core" or name in ADAPTERS:
        return (BASE | CORE) - {name}
    if name == "qn-bootstrap":
        return BASE | CORE | ADAPTERS
    return BASE | CORE | ({"qn-bootstrap"} if name in {"qn-cli", "qn-daemon"} else set()) | ({"qn-mcp"} if name == "qn-daemon" else set())


def violations(metadata):
    """Return violations for every declared dependency, including inactive cfgs."""
    members = set(metadata["workspace_members"])
    packages = [p for p in metadata["packages"] if p["id"] in members]
    if {p["id"] for p in packages} != members:
        raise ValueError("incomplete workspace metadata")
    roots = {p["name"]: Path(p["manifest_path"]).parent.resolve() for p in packages}
    errors = []
    for package in packages:
        name = package["name"]
        if name not in LAYERS:
            errors.append(f"unclassified workspace crate: {name}; update spec 01 and layer policy")
            continue
        for dependency in package["dependencies"]:
            target = dependency["name"]  # Cargo preserves the package name separately from rename.
            if target not in roots and not target.startswith("qn-"):
                continue
            kind = dependency.get("kind") or "normal"
            edge = f"{name} -> {target} ({kind})"
            if target not in roots:
                errors.append(f"{edge}: internal crate must be a classified workspace member")
                continue
            path = dependency.get("path")
            if dependency.get("source") is not None or not path or Path(path).resolve() != roots[target]:
                errors.append(f"{edge}: internal dependency must use its workspace path")
            if target not in allowed_dependencies(name):
                errors.append(f"{edge}: forbidden by spec 01 section 3")
    return errors


def main():
    """Inspect the repository, reporting a nonzero status on any check failure."""
    root = Path(__file__).resolve().parent.parent
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps",
             "--manifest-path", str(root / "Cargo.toml")],
            capture_output=True, text=True, check=True, timeout=60)
        errors = violations(json.loads(result.stdout))
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, TypeError):
        print("architecture: unable to inspect Cargo workspace; run cargo metadata locally", file=sys.stderr)
        return 1
    for error in errors:
        print(f"architecture: {error}", file=sys.stderr)
    if errors:
        return 1
    print("architecture: all declared workspace dependencies respect spec 01")
    return 0


if __name__ == "__main__":
    sys.exit(main())
