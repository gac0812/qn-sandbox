#!/usr/bin/env python3
"""nono 0.80.0 research, not product E2E; synthetic inputs only (#46)."""
import argparse
import atexit
import tarfile
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import tempfile
import time
from capture import probe_capture


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--nono', required=True, type=Path)
    parser.add_argument('--archive', required=True, type=Path)
    parser.add_argument('--probe', required=True, type=Path)
    args = parser.parse_args()
    assert platform.system() == 'Darwin', 'macOS probe only'
    digest = hashlib.sha256(args.archive.read_bytes()).hexdigest()
    assert digest == 'ae824167b3d90531b830429ba92b7781379b1c2d79f9b11fdd88b0fd267e4e32'
    with tarfile.open(args.archive) as archive:
        member = archive.extractfile('nono')
        assert hashlib.sha256(member.read()).digest() == hashlib.sha256(args.nono.read_bytes()).digest()
    root = Path(tempfile.mkdtemp(prefix='qn-nono-probe-', dir='/Users/Shared'))
    os.chmod(root, 0o700)
    project = root / 'project'
    home = root / 'home'
    for path in (project, home / '.ssh', root / 'config', root / 'state', root / 'cache', root / 'tmp'):
        path.mkdir(parents=True, exist_ok=True)
    (home / '.ssh' / 'canary').write_text('synthetic-canary')
    env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(home),
           'XDG_CONFIG_HOME': str(root / 'config'), 'XDG_STATE_HOME': str(root / 'state'),
           'XDG_CACHE_HOME': str(root / 'cache'), 'TMPDIR': str(root / 'tmp'),
           'LANG': 'en_US.UTF-8', 'QN_FAKE_TOKEN': 'synthetic-canary'}
    results = {}

    def invoke(label, *argv):
        proc = subprocess.run([str(args.probe.resolve()), str(args.nono.resolve()), *map(str, argv)],
                              cwd=project, env=env, capture_output=True, text=True, timeout=55)
        if proc.returncode:
            raise RuntimeError(label + ': transport failed: ' + proc.stderr[-500:])
        result = json.loads(proc.stdout)
        assert not result['stdout_truncated'] and not result['stderr_truncated']
        results[label] = result
        (root / 'results.json').write_text(json.dumps(results, indent=2))
        print(label, 'exit=', result['exit_code'], flush=True)
        return result

    def profile(name, content):
        path = root / (name + '.json')
        path.write_text(json.dumps(content))
        os.chmod(path, 0o600)
        return path

    def cleanup():
        remaining = json.loads(invoke('cleanup_ps', 'ps', '--json')['stdout'])
        for session in remaining:
            invoke('cleanup_stop', 'stop', session['session_id'], '--force')
    atexit.register(cleanup)

    base = {'meta': {'name': 'qn-probe', 'version': '0.1.0'}, 'extends': 'default',
            'workdir': {'access': 'readwrite'}, 'filesystem': {'allow': [str(project)]},
            'network': {'block': True},
            'environment': {'allow_vars': ['PATH', 'HOME', 'LANG'],
                            'deny_vars': ['*TOKEN*', '*KEY*', '*SECRET*'],
                            'case_insensitive_vars': True}}
    safe = profile('safe', base)
    invoke('version', '--version')
    invoke('validate', 'profile', 'validate', '--json', '--strict', '--', safe)
    bad = profile('bad', {'unknown_probe_field': True})
    invoke('validate_bad', 'profile', 'validate', '--json', '--strict', '--', bad)
    def run(label, policy, *cmd, extra=()):
        return invoke(label, 'run', '--profile', policy, '--workdir', project,
                      '--diagnostics-json', *extra, '--', *cmd)
    run('filesystem', safe, '/bin/cat', home / '.ssh' / 'canary')
    run('environment', safe, '/usr/bin/printenv', 'QN_FAKE_TOKEN')
    run('network', safe, '/usr/bin/curl', '--max-time', '2', '--noproxy', '*', 'http://127.0.0.1:9/')
    invoke('audit_list', 'audit', 'list', '--json')
    audits = json.loads(results['audit_list']['stdout'])
    if audits:
        invoke('audit_show', 'audit', 'show', audits[0]['session_id'], '--json')
    for mode in ('graceful', 'force'):
        name = 'sxn-019a1234-5678-7abc-8123-' + ('000000000001' if mode == 'graceful' else '000000000002')
        run(mode + '_start', safe, '/bin/sleep', '90', extra=('--name', name, '--detached'))
        observed = json.loads(invoke(mode + '_ps', 'ps', '--json')['stdout'])
        found = next((item for item in observed if item.get('name') == name), None)
        assert found is not None, 'detached name must resolve'
        session = found['session_id']
        invoke(mode + '_inspect', 'inspect', session, '--json')
        invoke(mode + '_stop', 'stop', session, *(['--force'] if mode == 'force' else ['--timeout', '3']))
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            observed = json.loads(invoke(mode + '_after', 'ps', '--json')['stdout'])
            if not any(item.get('session_id') == session for item in observed):
                break
            time.sleep(0.1)
        assert not observed, 'stop must remove live session'
        for pid in [found['supervisor_pid'], found['child_pid']]:
            deadline = time.monotonic() + 8
            while True:
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    break
                if time.monotonic() >= deadline:
                    state = subprocess.run(['ps', '-p', str(pid), '-o', 'pid=,ppid=,stat=,command='],
                                           capture_output=True, text=True, timeout=3).stdout
                    (root / (mode + '-stop-residue.txt')).write_text(state)
                    print('FAIL: stop residue', mode, state.strip(), flush=True)
                    if '/bin/sleep 90' in state:
                        os.kill(pid, signal.SIGKILL)
                    break
                time.sleep(0.05)
        invoke(mode + '_repeat_stop', 'stop', session)
    shadow = root / 'config' / 'nono' / 'profiles'
    shadow.mkdir(parents=True, exist_ok=True)
    (shadow / 'default.json').write_text(json.dumps({'meta': {'name': 'default'},
        'environment': {'set_vars': {'QN_SHADOW_MARKER': 'synthetic-shadow'}}}))
    run('shadow', safe, '/usr/bin/printenv', 'QN_SHADOW_MARKER')
    (shadow / 'default.json').unlink()
    proxy_policy = json.loads(json.dumps(base))
    proxy_policy['network'] = {'allow_domain': ['example.invalid']}
    run('proxy_denial', profile('proxy', proxy_policy), '/usr/bin/curl', '--max-time', '3',
        '--fail', 'https://blocked.invalid/')
    probe_capture(root, project, base, profile, invoke, run)
    invoke('audit_final', 'audit', 'list', '--json')
    for item in json.loads(results['audit_final']['stdout']):
        if item.get('network_event_count', 0):
            invoke('audit_network_' + item['session_id'], 'audit', 'show', item['session_id'], '--json')
    invoke('pull_help', 'pull', '--help')
    invoke('pack', 'pull', 'nolabs-ai/claude@0.1.4')
    invoke('final_ps', 'ps', '--json')
    assert json.loads(results['final_ps']['stdout']) == []
    assert results['filesystem']['exit_code'] == 1
    assert results['environment']['exit_code'] == 1 and not results['environment']['stdout']
    assert results['network']['exit_code'] == 7
    assert results['proxy_denial']['exit_code'] == 22
    assert results['shadow']['stdout'].strip() == 'synthetic-shadow'
    assert all(results[m + '_repeat_stop']['exit_code'] == 1 for m in ('graceful', 'force'))
    diagnostic_text = results['filesystem']['stderr']
    report = json.JSONDecoder().raw_decode(diagnostic_text[diagnostic_text.index('{\n  "session"'):])[0]
    assert any(d['path'].endswith('/.ssh/canary') for d in report['session']['denials'])
    assert any('host_denied' in v['stdout'] for k, v in results.items() if k.startswith('audit_network_'))
    atexit.unregister(cleanup)
    print('Evidence:', root)
    if list(root.glob('*-stop-residue.txt')):
        raise SystemExit('FAIL: stop left a process; see preserved residue evidence')


if __name__ == '__main__':
    main()
