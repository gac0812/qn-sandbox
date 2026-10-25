#!/usr/bin/env python3
"""Retained native network evidence through the actual qn CLI (#121)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import select
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', required=True, type=Path)
    parser.add_argument('--archive', required=True, type=Path)
    args = parser.parse_args()
    assert platform.system() == 'Darwin' and platform.machine() == 'arm64'
    assert hashlib.sha256(args.archive.read_bytes()).hexdigest() == (
        'ae824167b3d90531b830429ba92b7781379b1c2d79f9b11fdd88b0fd267e4e32'
    ), 'pinned archive required'
    base = args.workspace.resolve()
    with tempfile.TemporaryDirectory(prefix='qn-native-audit-', dir='/Users/Shared') as directory:
        root = Path(directory).resolve()
        os.chmod(root, 0o700)
        for name in ['home', 'config', 'state', 'cache', 'tmp', 'project', 'audit']:
            (root / name).mkdir()
        env = dict(os.environ, QN_HOME=str(root))
        for key in list(env):
            if key.startswith('QN_') and key != 'QN_HOME':
                del env[key]
        def cli(*arguments):
            return subprocess.run([str(base / 'target/debug/qn'), *arguments],
                                  env=env, capture_output=True, text=True, timeout=20)

        managed = root / 'cache/nono/aarch64-apple-darwin/0.80.0'
        managed.mkdir(parents=True)
        shutil.copyfile(args.archive, managed / 'nono-v0.80.0-aarch64-apple-darwin.tar.gz')
        installed = cli('install')
        assert installed.returncode == 0, installed.stderr
        binary = managed / 'nono'
        launcher = subprocess.Popen(
            [str(base / 'target/debug/examples/bound_launch_probe'), str(root), str(binary)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            assert select.select([launcher.stdout], [], [], 8)[0], 'reservation timeout'
            identity = json.loads(launcher.stdout.readline())
            session = identity['session_id']
            mapping = root / 'audit' / (session + '.native.json')
            retained = mapping.read_bytes()
            record = json.loads(retained)
            assert record == dict(identity, runtime_root=str(root)), 'publish before launch'
            policy = {
                'meta': {'name': 'qn-native-audit', 'version': '1'}, 'extends': 'default',
                'filesystem': {'allow': [str(root / 'project')]},
                'security': {'capability_elevation': False, 'signal_mode': 'isolated',
                             'process_info_mode': 'isolated'},
                'environment': {'allow_vars': ['PATH', 'HOME', 'LANG']},
                'network': {'allow_domain': ['example.invalid']},
            }
            (root / 'profile.json').write_text(json.dumps(policy))
            os.chmod(root / 'profile.json', 0o600)
            out, err = launcher.communicate(json.dumps([
                '/usr/bin/curl', '--max-time', '3', '--fail',
                'https://blocked.invalid/credential-canary']), timeout=25)
            assert launcher.returncode == 0, 'launcher failed: ' + err
            child = json.loads(out)
            assert child['exit_code'] == 22, 'native proxy must deny the request'
            assert not child['stdout_truncated'] and not child['stderr_truncated']
        finally:
            if launcher.poll() is None:
                launcher.terminate()
                launcher.communicate(timeout=8)

        assert mapping.read_bytes() == retained, 'association must survive owner exit'
        def query_ok():
            output = cli('audit', 'native', session, '--json')
            assert output.returncode == 0, output.stderr
            envelope = json.loads(output.stdout)
            assert envelope['schema_version'] == 1
            report = envelope['report']
            assert report['session_id'] == session and report['native_id'] == identity['native_id']
            assert report['source'] == 'nono_audit' and report['integrity'] == 'unverified'
            assert report['coverage'] == 'network_only' and report['exit_code'] == 22
            assert any(event['decision'] == 'denied' for event in report['events'])
            assert [event['index'] for event in report['events']] == list(range(1, len(report['events']) + 1))
            assert all(set(event) == {'index', 'wall_millis', 'decision'} for event in report['events'])
            assert report['reported_event_count'] >= len(report['events'])
            for secret in ['credential-canary', 'blocked.invalid', '/usr/bin/curl', str(root)]:
                assert secret not in output.stdout + output.stderr, 'free text leaked'
            return report

        report = query_ok()
        human = cli('audit', 'native', session)
        assert human.returncode == 0 and 'coverage: network_only' in human.stdout
        assert 'credential-canary' not in human.stdout + human.stderr
        print('PASS: exact retained identity, native denial, payload-free JSON and text', flush=True)

        def query_refused():
            output = cli('audit', 'native', session, '--json')
            assert output.returncode != 0 and not output.stdout, 'invalid evidence accepted'
            assert 'canary' not in output.stderr, 'rejected payload leaked'

        for key, value in [('session_id', 'sxn-019a1234-5678-7abc-8123-000000000001'),
                           ('native_id', format(int(identity['native_id'], 16) ^ 1, '016x'))]:
            replacement = dict(record, **{key: value})
            mapping.write_text(json.dumps(replacement))
            query_refused()
        mapping.write_bytes(retained)
        backing = mapping.with_suffix('.retained')
        mapping.rename(backing)
        mapping.symlink_to(backing)
        query_refused()
        mapping.unlink()
        backing.rename(mapping)
        alias = root / 'runtime-alias'
        alias.symlink_to(root, target_is_directory=True)
        mapping.write_text(json.dumps(dict(record, runtime_root=str(alias))))
        query_refused()
        mapping.write_bytes(retained)
        alias.unlink()
        print('PASS: foreign qn/native identity and symlink associations refused', flush=True)

        marker = binary.with_suffix('.invoked')
        binary.write_text("""#!/bin/sh
: > "$0.invoked"
echo 'nono 0.80.0'
""")
        query_refused()
        assert not marker.exists(), 'unverified engine executed'
        forged = subprocess.run([str(binary), '--version'], env={},
                                capture_output=True, text=True, timeout=5)
        assert forged.returncode == 0 and marker.exists(), 'positive control failed'
        assert forged.stdout.strip() == 'nono 0.80.0'
        marker.unlink()
        repaired = cli('install')
        assert repaired.returncode == 0, repaired.stderr
        assert not marker.exists() and query_ok() == report
        print('PASS: tampered executable refused before invocation; offline repair restores query', flush=True)


if __name__ == '__main__':
    main()
