"""Export measured schema fixtures, never credential-capture output (#46)."""
import argparse
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('evidence', type=Path)
parser.add_argument('destination', type=Path)
args = parser.parse_args()
records = json.loads((args.evidence / 'results.json').read_text())
args.destination.mkdir(parents=True, exist_ok=True)
sources = {'profile-valid.json': ('validate', 'stdout'),
           'profile-invalid.json': ('validate_bad', 'stdout'),
           'ps-running.json': ('graceful_ps', 'stdout'),
           'inspect-running.json': ('graceful_inspect', 'stdout'),
           'audit-list.json': ('audit_list', 'stdout'),
           'diagnostics-denied.txt': ('filesystem', 'stderr'),
           'diagnostics-empty.txt': ('environment', 'stderr')}
for key, result in records.items():
    if key.startswith('audit_network_') and 'blocked.invalid' in result['stdout']:
        sources['audit-show.json'] = (key, 'stdout')
assert 'audit-show.json' in sources
for name, (key, stream) in sources.items():
    text = records[key][stream].replace(str(args.evidence), '/fixture')
    text = re.sub(r'/Users/[^/\s"\\]+', '/host/user', text)
    text = re.sub(r'"[0-9a-f]{16}"', '"0123456789abcdef"', text)
    text = re.sub(r'2026-10-24T[^"\s]+', '2026-10-24T00:00:00+00:00', text)
    if name.endswith('.json'):
        text = json.dumps(json.loads(text), separators=(',', ':'))
    (args.destination / name).write_text(text.rstrip() + '\n')
