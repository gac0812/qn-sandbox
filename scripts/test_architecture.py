"""Contracts for declared workspace dependency boundaries (Issue #132)."""

import contextlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from check_architecture import main, violations


def metadata(edge=None):
    names = ["qn-cli", "qn-core", "qn-nono", "qn-config", "qn-infra", "qn-bootstrap", "qn-daemon", "qn-gateway", "qn-ui", "qn-mcp"]
    packages = [
        {"id": name, "name": name, "manifest_path": f"/workspace/{name}/Cargo.toml",
         "dependencies": []}
        for name in names
    ]
    if edge:
        owner, dependency, extra = edge
        target = next(p for p in packages if p["name"] == owner)
        target["dependencies"] = [
            {"name": dependency, "path": f"/workspace/{dependency}",
             "kind": None, "source": None, **extra}
        ]
    return {"packages": packages, "workspace_members": names}


class ArchitectureTests(unittest.TestCase):
    def test_current_layer_directions(self):
        for source, target in [
            ("qn-cli", "qn-core"), ("qn-cli", "qn-bootstrap"),
            ("qn-daemon", "qn-mcp"),
            ("qn-bootstrap", "qn-nono"), ("qn-nono", "qn-core"),
            ("qn-core", "qn-config"), ("qn-nono", "qn-infra"),
        ]:
            with self.subTest(source=source, target=target):
                self.assertEqual(violations(metadata((source, target, {}))), [])

    def test_forbidden_edges_in_every_dependency_kind(self):
        for source, target in [
            ("qn-cli", "qn-nono"), ("qn-core", "qn-cli"),
            ("qn-core", "qn-bootstrap"), ("qn-config", "qn-core"),
            ("qn-nono", "qn-cli"), ("qn-nono", "qn-bootstrap"),
            ("qn-nono", "qn-gateway"), ("qn-ui", "qn-bootstrap"),
        ]:
            for kind in [None, "dev", "build"]:
                with self.subTest(source=source, target=target, kind=kind):
                    errors = violations(metadata((source, target, {
                        "kind": kind, "rename": "alias", "optional": True,
                        "target": 'cfg(windows)',
                    })))
                    self.assertTrue(errors)
                    self.assertIn(f"{source} -> {target}", errors[0])
                    self.assertIn(kind or "normal", errors[0])

    def test_unknown_member_requires_explicit_classification(self):
        data = metadata()
        data["packages"][0]["name"] = "qn-new-surface"
        self.assertIn("unclassified", " ".join(violations(data)))

    def test_internal_name_cannot_resolve_to_registry_or_wrong_path(self):
        for extra in [
            {"source": "registry+example", "path": None},
            {"path": "/some/other/checkout/qn-core"},
        ]:
            self.assertIn("workspace path", " ".join(
                violations(metadata(("qn-cli", "qn-core", extra)))))

    def test_incomplete_metadata_fails_closed(self):
        data = metadata()
        data["workspace_members"].append("missing-member")
        with self.assertRaises(ValueError):
            violations(data)

    def test_third_party_dependencies_are_not_internal_layers(self):
        data = metadata()
        data["packages"][0]["dependencies"] = [{"name": "serde", "source": "registry+x"}]
        self.assertEqual(violations(data), [])

    def test_cargo_failure_has_nonzero_exit_without_dumping_output(self):
        for error in [FileNotFoundError(), subprocess.TimeoutExpired("cargo", 60),
                      subprocess.CalledProcessError(1, "cargo", stderr="synthetic-private-data")]:
            with self.subTest(error=type(error)), patch(
                "check_architecture.subprocess.run", side_effect=error
            ), contextlib.redirect_stderr(io.StringIO()) as stderr:
                self.assertEqual(main(), 1)
                self.assertNotIn("synthetic-private-data", stderr.getvalue())

    def test_cargo_preserves_alias_inherited_and_target_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text(
                '[workspace]\nresolver="2"\nmembers=["qn-cli","qn-core","qn-nono"]\n'
                '[workspace.dependencies]\nengine={package="qn-nono",path="qn-nono"}\n')
            for name in ["qn-cli", "qn-core", "qn-nono"]:
                crate = root / name
                crate.mkdir()
                (crate / "lib.rs").write_text("")
                (crate / "Cargo.toml").write_text(
                    f'[package]\nname="{name}"\nversion="0.1.0"\n'
                    '[lib]\npath="lib.rs"\n')
            scripts = root / "scripts"
            scripts.mkdir()
            checker = scripts / "check_architecture.py"
            checker.write_text(Path(__file__).with_name("check_architecture.py").read_text())
            baseline = subprocess.run(
                ["python3", str(checker)], capture_output=True, text=True, timeout=30)
            self.assertEqual(baseline.returncode, 0, baseline.stderr)
            manifest = root / "qn-cli" / "Cargo.toml"
            base = manifest.read_text()
            for section, declaration in [
                ("dependencies", 'engine={workspace=true,optional=true}'),
                ("dev-dependencies", 'engine={workspace=true}'),
                ("build-dependencies", 'engine={workspace=true}'),
                ("target.'cfg(windows)'.dependencies", 'engine={workspace=true}'),
            ]:
                with self.subTest(section=section):
                    manifest.write_text(base + f"[{section}]\n{declaration}\n")
                    result = subprocess.run(
                        ["cargo", "metadata", "--format-version", "1", "--no-deps",
                         "--manifest-path", str(root / "Cargo.toml")],
                        capture_output=True, text=True, check=True, timeout=30)
                    errors = violations(json.loads(result.stdout))
                    self.assertTrue(errors)
                    self.assertIn("qn-cli -> qn-nono", errors[0])
                    rejected = subprocess.run(
                        ["python3", str(checker)], capture_output=True, text=True, timeout=30)
                    self.assertEqual(rejected.returncode, 1, rejected.stderr)
                    self.assertIn("qn-cli -> qn-nono", rejected.stderr)


if __name__ == "__main__":
    unittest.main()
