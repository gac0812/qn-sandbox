#!/usr/bin/env python3
"""Rebuild exact pending broker revisions in a disposable worktree (#99)."""
import argparse
import pathlib
import shutil
import subprocess
import sys
import tempfile

BASE = '9fab27643401ea37b5ebb81843f366cbcc5784ef'
GRANTS = 'a483fe1fb696f28c7deee69cd3fdec1ec58adeb2'
CHANNEL = '54c7bda238df9a2bac670c8a4e06e7a5f284f421'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--nono', required=True, type=pathlib.Path)
args = parser.parse_args()
fixtures = pathlib.Path(__file__).resolve().parent
repo = pathlib.Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], cwd=fixtures, text=True).strip())
for revision in [BASE, GRANTS, CHANNEL]:
    subprocess.run(['git', 'cat-file', '-e', revision + '^{commit}'], cwd=repo, check=True)


def show(revision, path):
    return subprocess.check_output(['git', 'show', revision + ':' + path], cwd=repo)


with tempfile.TemporaryDirectory(prefix='qn-capture-replay-') as temporary:
    workspace = pathlib.Path(temporary) / 'workspace'
    subprocess.run(['git', 'worktree', 'add', '--detach', str(workspace), BASE], cwd=repo, check=True)
    try:
        broker = workspace / 'crates/qn-broker'
        (workspace / 'Cargo.lock').write_bytes(show(CHANNEL, 'Cargo.lock'))
        # These are exact library sources from the pending PRs, never a rewritten registry.
        for revision, names in [(GRANTS, ['src/grants.rs', 'src/grant_tests.rs']),
                                (CHANNEL, ['src/capture.rs', 'src/capture_tests.rs'])]:
            for name in names:
                (broker / name).write_bytes(show(revision, 'crates/qn-broker/' + name))
        manifest = show(CHANNEL, 'crates/qn-broker/Cargo.toml').decode()
        core_dependency = 'qn-core = { path = "../qn-core", version = "0.1.0" }'
        assert core_dependency in show(GRANTS, 'crates/qn-broker/Cargo.toml').decode()
        (broker / 'Cargo.toml').write_text(manifest.replace('[dependencies]', '[dependencies]' + chr(10) + core_dependency))
        (broker / 'src/lib.rs').write_bytes(show(GRANTS, 'crates/qn-broker/src/lib.rs') + b'pub mod capture;')
        (broker / 'examples').mkdir()
        for name in ['capture_relay.rs', 'grant_capture_probe.rs']:
            shutil.copyfile(fixtures / name, broker / 'examples' / name)
        subprocess.run(['cargo', 'build', '-p', 'qn-broker', '--examples'], cwd=workspace, check=True)
        subprocess.run(['cargo', 'build', '-p', 'qn-nono', '--example', 'probe'], cwd=workspace, check=True)
        subprocess.run(['cargo', 'test', '--locked', '-p', 'qn-broker'], cwd=workspace, check=True)
        subprocess.run(['cargo', 'clippy', '--locked', '-p', 'qn-broker', '--all-targets', '--', '-D', 'warnings'], cwd=workspace, check=True)
        subprocess.run([sys.executable, str(fixtures / 'verify.py'), '--workspace', str(workspace), '--nono', str(args.nono.resolve())], cwd=workspace, check=True)
    finally:
        # Only this fresh disposable worktree is removed, including its generated lock/target.
        subprocess.run(['git', 'worktree', 'remove', '--force', str(workspace)], cwd=repo, check=True)
