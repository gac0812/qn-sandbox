"""Narration via MiniMax TTS (t2a_v2). Reads MINIMAX_KEY from the environment; never writes it.

python3 tools/tts.py film/vo/lines.json film/vo [--voice ID] [--only L1,L2]
Writes <id>.wav (48 kHz mono, silence trimmed) and durations.json. Lines are cached by a hash of
(text, emotion, voice, speed) so only changed lines are re-synthesised.
"""
import hashlib, json, os, subprocess, sys, time, urllib.request

lines_path, out = sys.argv[1], sys.argv[2]
args = sys.argv[3:]
VOICE = args[args.index('--voice') + 1] if '--voice' in args else 'Chinese (Mandarin)_Warm_Bestie'
ONLY = set(args[args.index('--only') + 1].split(',')) if '--only' in args else None
SPEED = float(args[args.index('--speed') + 1]) if '--speed' in args else 1.0
KEY = os.environ.get('MINIMAX_KEY')
if not KEY: sys.exit('MINIMAX_KEY is not set')
os.makedirs(out, exist_ok=True)
lines = json.load(open(lines_path))
cache_path = os.path.join(out, 'cache.json')
cache = json.load(open(cache_path)) if os.path.exists(cache_path) else {}

def synth(text, emotion, tries=6):
    for k in range(tries):
        try:
            return _synth(text, emotion)
        except RuntimeError as e:
            if '1002' not in str(e) or k == tries - 1: raise
            time.sleep(10 * (k + 1))  # rate limit (RPM): back off and retry

def _synth(text, emotion):
    body = {
        'model': 'speech-02-hd', 'text': text, 'stream': False,
        'voice_setting': {'voice_id': VOICE, 'speed': SPEED, 'vol': 1, 'pitch': 0, 'emotion': emotion},
        'audio_setting': {'sample_rate': 32000, 'bitrate': 128000, 'format': 'mp3', 'channel': 1},
    }
    req = urllib.request.Request('https://api.minimaxi.com/v1/t2a_v2', data=json.dumps(body).encode(),
                                 headers={'Authorization': f'Bearer {KEY}', 'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=120) as r:
        d = json.load(r)
    if d.get('base_resp', {}).get('status_code') != 0:
        raise RuntimeError(f"tts failed: {d.get('base_resp')}")
    return bytes.fromhex(d['data']['audio'])

def duration(path):
    r = subprocess.run(['ffprobe', '-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', path], capture_output=True, text=True)
    return float(r.stdout.strip())

durs = {}
for ln in lines:
    lid = ln['id']
    h = hashlib.sha1(json.dumps([ln['say'], ln['emotion'], VOICE, SPEED]).encode()).hexdigest()[:12]
    wav = os.path.join(out, f'{lid}.wav')
    if lid not in cache and os.path.exists(wav): cache[lid] = h  # synthesised before a crash
    if (ONLY is None or lid in ONLY) and (cache.get(lid) != h or not os.path.exists(wav)):
        mp3 = os.path.join(out, f'{lid}.mp3')
        open(mp3, 'wb').write(synth(ln['say'], ln['emotion']))
        # trim leading/trailing silence, 48 kHz mono
        af = 'silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.02,areverse,silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.05,areverse'
        subprocess.run(['ffmpeg', '-loglevel', 'error', '-y', '-i', mp3, '-af', af, '-ar', '48000', '-ac', '1', wav], check=True)
        os.remove(mp3)
        cache[lid] = h
        json.dump(cache, open(cache_path, 'w'), indent=1)
        print('synth', lid, ln['emotion'])
    if os.path.exists(wav): durs[lid] = round(duration(wav), 3)
json.dump(cache, open(cache_path, 'w'), indent=1)
json.dump(durs, open(os.path.join(out, 'durations.json'), 'w'), indent=1)
print('total speech', round(sum(durs.values()), 1), 's over', len(durs), 'lines')
