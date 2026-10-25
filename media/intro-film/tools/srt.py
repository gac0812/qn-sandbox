"""events.json → .srt (Chinese line + English line) and a reading-time check."""
import json, re, sys
meta = json.load(open(sys.argv[1])); out = sys.argv[2]
def ts(t):
    ms = int(round(t * 1000)); h, ms = divmod(ms, 3600000); m, ms = divmod(ms, 60000); s, ms = divmod(ms, 1000)
    return f'{h:02d}:{m:02d}:{s:02d},{ms:03d}'
def chars(text):
    # CJK characters count one each; a Latin word counts as one
    return len(re.findall(r'[一-鿿]', text)) + len(re.findall(r'[A-Za-z0-9]+', text))
ok = True
with open(out, 'w', encoding='utf-8') as f:
    for i, s in enumerate(meta['SUBS'], 1):
        f.write(f"{i}\n{ts(s['t0'])} --> {ts(s['t1'])}\n{s['text']}\n{s['en']}\n\n")
        need = chars(s['text']) / 4.5 + 1.5; have = s['t1'] - s['t0']
        flag = 'OK ' if have >= need - 1e-6 else 'SHORT'
        ok &= flag == 'OK '
        print(f"{flag} {have:4.1f}s ≥ {need:4.1f}s  {s['text']}")
print('wrote', out); sys.exit(0 if ok else 1)
