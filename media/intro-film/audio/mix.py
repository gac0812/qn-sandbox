"""Score + foley for the qn sandbox film, from the same timeline as the picture.

python3 audio/mix.py <events.json> <out.wav> [--from T] [--to T]

Music: VCSL (CC0) kalimba, marimba, glockenspiel, harp, vibraphone, cabasa. No drum kit, no
piano-and-strings. Foley: synthesised (numpy) and placed on the picture's event times.
"""
import json, re, sys, glob, os
import numpy as np
from scipy.io import wavfile

SR = 48000
HERE = os.path.dirname(os.path.abspath(__file__))
SAMPLES = os.path.join(HERE, '..', 'samples')
rng = np.random.default_rng(7)

# ---------------------------------------------------------------- sampler
NOTE = {'C': 0, 'C#': 1, 'D': 2, 'D#': 3, 'E': 4, 'F': 5, 'F#': 6, 'G': 7, 'G#': 8, 'A': 9, 'A#': 10, 'B': 11}

def midi_of(name):
    m = re.search(r'_([A-G]#?)(\d)(?=[_\.])', name)
    return None if not m else 12 * (int(m.group(2)) + 1) + NOTE[m.group(1)]

def load(path):
    sr, x = wavfile.read(path)
    x = x.astype(np.float32) / 32768.0
    if x.ndim == 1: x = np.stack([x, x], 1)
    # trim leading silence so the onset sits on the event time
    env = np.abs(x).max(1)
    i = int(np.argmax(env > env.max() * 0.02))
    return x[max(0, i - 24):]

INST = {}
for d in ['kalimba', 'marimba', 'glock', 'harp', 'vibraphone']:
    INST[d] = sorted([(midi_of(os.path.basename(f)), load(f)) for f in glob.glob(f'{SAMPLES}/{d}/*.wav') if midi_of(os.path.basename(f))], key=lambda p: p[0])
CABASA = load(glob.glob(f'{SAMPLES}/cabasa/*.wav')[0])
# per-instrument level trims (samples differ a lot in recorded level)
TRIM = {k: 1.0 / max(1e-4, np.mean([np.sqrt(np.mean(s ** 2)) for _, s in v])) * 0.05 for k, v in INST.items()}

def pitched(inst, midi):
    m0, s = min(INST[inst], key=lambda p: abs(p[0] - midi))
    ratio = 2 ** ((midi - m0) / 12)
    n = int(len(s) / ratio)
    idx = np.arange(n) * ratio
    i0 = idx.astype(int); fr = (idx - i0)[:, None]
    i1 = np.minimum(i0 + 1, len(s) - 1)
    return s[i0] * (1 - fr) + s[i1] * fr

class Bus:
    def __init__(self, dur):
        self.x = np.zeros((int(dur * SR) + SR * 4, 2), np.float32)
    def add(self, y, t, gain=1.0, pan=0.0):
        i = int(round(t * SR))
        if i < 0: y = y[-i:]; i = 0
        y = y[: len(self.x) - i]
        l, r = np.cos((pan + 1) * np.pi / 4), np.sin((pan + 1) * np.pi / 4)
        self.x[i:i + len(y), 0] += y[:, 0] * gain * l * 1.414
        self.x[i:i + len(y), 1] += y[:, 1] * gain * r * 1.414

def note(bus, inst, midi, t, vel=0.7, pan=0.0, dur=None):
    y = pitched(inst, midi) * TRIM[inst]
    if dur is not None:
        n = int(dur * SR); fade = int(0.08 * SR)
        y = y[: n + fade].copy()
        if len(y) > n: y[n:] *= np.linspace(1, 0, len(y) - n)[:, None]
    bus.add(y, t, vel, pan)

# ---------------------------------------------------------------- synth helpers
def env_exp(n, tau): return np.exp(-np.arange(n) / (tau * SR))
def stereo(m, width=0.0):
    return np.stack([m, m], 1).astype(np.float32)
# Zero-phase FFT filters with Butterworth magnitude responses (scipy.signal is unavailable here).
def _filt(x, gain_fn):
    x = np.asarray(x, np.float64)
    n = x.shape[-1]
    N = 1 << int(np.ceil(np.log2(n + 1024)))
    X = np.fft.rfft(x, N, axis=-1)
    f = np.fft.rfftfreq(N, 1 / SR)
    y = np.fft.irfft(X * gain_fn(np.maximum(f, 1e-3)), N, axis=-1)[..., :n]
    return y.astype(np.float32)
def lp(x, fc, order=2): return _filt(x, lambda f: 1 / np.sqrt(1 + (f / fc) ** (2 * order)))
def hp(x, fc, order=2): return _filt(x, lambda f: 1 / np.sqrt(1 + (fc / f) ** (2 * order)))
def bp(x, lo, hi, order=2): return _filt(x, lambda f: 1 / np.sqrt(1 + (f / hi) ** (2 * order)) / np.sqrt(1 + (lo / f) ** (2 * order)))
def sweep(f0, f1, dur, shape='sine'):
    n = int(dur * SR); f = np.geomspace(f0, f1, n); ph = 2 * np.pi * np.cumsum(f) / SR
    return np.sin(ph) if shape == 'sine' else np.sign(np.sin(ph)) * 0.6

def sfx_step(i):
    n = int(0.05 * SR); f = [880, 740][i % 2]
    y = np.sign(np.sin(2 * np.pi * f * np.arange(n) / SR)) * env_exp(n, 0.015)
    return stereo(lp(y, 5000) * 0.5)

def sfx_hop():
    y = sweep(320, 760, 0.13, 'square') * env_exp(int(0.13 * SR), 0.06)
    return stereo(lp(y, 6000) * 0.5)

def sfx_thud():
    n = int(1.4 * SR); t = np.arange(n) / SR
    body = np.sin(2 * np.pi * np.cumsum(np.geomspace(150, 85, n)) / SR) * env_exp(n, 0.07)
    click = lp(rng.standard_normal(n), 2500) * env_exp(n, 0.008) * 0.6
    ring = sum(g * np.sin(2 * np.pi * 1150 * r * t) * env_exp(n, d) for r, g, d in [(1, 1, 0.5), (2.76, 0.5, 0.3), (5.4, 0.28, 0.18), (8.93, 0.12, 0.1)])
    return stereo(body * 1.0 + click + ring * 0.22)

def sfx_stamp(light=False):
    n = int(0.5 * SR)
    f0, f1 = (160, 90) if light else (120, 55)
    thump = np.sin(2 * np.pi * np.cumsum(np.geomspace(f0, f1, n)) / SR) * env_exp(n, 0.05)
    slap = bp(rng.standard_normal(n), 700, 3200) * env_exp(n, 0.018)
    y = thump * (0.6 if light else 1.0) + slap * (0.5 if light else 0.8)
    return stereo(y)

def sfx_notify():
    b = Bus(1.5)
    note(b, 'glock', 88, 0.0, 0.5); note(b, 'glock', 93, 0.09, 0.45)
    return b.x[: int(1.5 * SR)]

def sfx_zip():
    n = int(0.45 * SR)
    a = np.linspace(0, 1, n) ** 2
    y = bp(rng.standard_normal(n), 1500, 6000) * a * 0.5 + sweep(400, 1300, 0.45, 'square') * a * 0.15
    return stereo(y)

def sfx_whoosh(dur=0.38, lo=500, hi=4000):
    n = int(dur * SR); k = np.linspace(0, 1, n)
    a = np.sin(np.pi * k) ** 2
    y = bp(rng.standard_normal(n), lo, hi) * a
    return stereo(y * 0.7)

def sfx_paper():
    n = int(0.9 * SR); k = np.linspace(0, 1, n)
    am = 0.5 + 0.5 * np.sin(2 * np.pi * 9 * k + 3 * np.sin(2 * np.pi * 2 * k))
    y = bp(rng.standard_normal(n), 1800, 6000) * am * np.sin(np.pi * k) * 0.35
    return stereo(y)

def sfx_click():
    n = int(0.06 * SR)
    y = bp(rng.standard_normal(n), 2500, 6000) * env_exp(n, 0.004) + np.sin(2 * np.pi * 3800 * np.arange(n) / SR) * env_exp(n, 0.006) * 0.4
    return stereo(y)

def sfx_key(i):
    n = int(0.07 * SR)
    f = 1800 + (i * 337) % 900
    y = bp(rng.standard_normal(n), f, f * 2.2) * env_exp(n, 0.01) + lp(rng.standard_normal(n), 400) * env_exp(n, 0.02) * 0.5
    return stereo(y * 0.6)

def sfx_pop():
    y = sweep(300, 900, 0.06) * env_exp(int(0.06 * SR), 0.03)
    return stereo(y * 0.8)

def sfx_dust():
    # pixels dissolving: a falling 8-bit arpeggio
    b = np.zeros(int(0.9 * SR))
    for k, f in enumerate([1568, 1319, 1047, 880, 659, 523]):
        n = int(0.09 * SR); s = int(k * 0.11 * SR)
        b[s:s + n] += np.sign(np.sin(2 * np.pi * f * np.arange(n) / SR)) * env_exp(n, 0.04) * 0.35
    return stereo(lp(b, 7000))

def sfx_turn():
    # key turning: two metallic ticks
    n = int(0.25 * SR); y = np.zeros(n)
    for s in [0, int(0.09 * SR)]:
        m = int(0.04 * SR)
        y[s:s + m] += bp(rng.standard_normal(m), 3000, 8000) * env_exp(m, 0.005) + np.sin(2 * np.pi * 4200 * np.arange(m) / SR) * env_exp(m, 0.01) * 0.5
    return stereo(y)

def sfx_enter():
    a, b = sfx_key(3) * 1.6, sfx_click() * 0.5
    y = np.zeros((max(len(a), len(b)), 2), np.float32); y[:len(a)] += a; y[:len(b)] += b
    return y

def sfx_land():
    n = int(0.18 * SR)
    y = np.sin(2 * np.pi * np.cumsum(np.geomspace(220, 90, n)) / SR) * env_exp(n, 0.04)
    y = y + np.sign(np.sin(2 * np.pi * 180 * np.arange(n) / SR)) * env_exp(n, 0.02) * 0.25
    return stereo(lp(y, 3000))

def sfx_ding():
    b = Bus(1.2); note(b, 'glock', 96, 0.0, 0.4); return b.x[: int(1.2 * SR)]

def sfx_tick():
    b = Bus(0.6); note(b, 'marimba', 84, 0.0, 0.35); return b.x[: int(0.6 * SR)]

SFX = {
    'enter': lambda e: (sfx_enter(), 0.45, 0.0),
    'land': lambda e: (sfx_land(), 0.4, 0.0),
    'ding': lambda e: (sfx_ding(), 0.45, 0.3),
    'tick': lambda e: (sfx_tick(), 0.4, -0.1),
    'step': lambda e: (sfx_step(e.get('i', 0)), 0.07, -0.3),
    'hop': lambda e: (sfx_hop(), 0.3, 0),
    'thud': lambda e: (sfx_thud(), 0.55, -0.35),
    'stamp': lambda e: (sfx_stamp(), 0.65, 0.1),
    'stampSmall': lambda e: (sfx_stamp(True), 0.55, 0.0),
    'notify': lambda e: (sfx_notify(), 0.7, 0.6),
    'zip': lambda e: (sfx_zip(), 0.35, -0.1),
    'whoosh': lambda e: (sfx_whoosh(), 0.35, 0.4),
    'paper': lambda e: (sfx_paper(), 0.12, 0.5),
    'click': lambda e: (sfx_click(), 0.5, 0.0),
    'key': lambda e: (sfx_key(e.get('i', 0)), 0.12, 0.0),
    'pop': lambda e: (sfx_pop(), 0.45, 0.0),
    'dust': lambda e: (sfx_dust(), 0.35, 0.0),
    'turn': lambda e: (sfx_turn(), 0.6, 0.45),
}
DUCK = {'thud': 0.35, 'stamp': 0.5, 'notify': 0.75, 'click': 0.8}

# ---------------------------------------------------------------- score
BEAT = 60 / 96
BAR = BEAT * 4
CHORDS = [  # Fmaj7 · Em7 · Dm7 · Cmaj7 (descending, warm)
    {'bass': 41, 'pad': [53, 57, 60, 64]},
    {'bass': 40, 'pad': [52, 55, 59, 62]},
    {'bass': 38, 'pad': [50, 53, 57, 60]},
    {'bass': 36, 'pad': [48, 52, 55, 59]},
]
# kalimba motif: per bar, 8 eighth-note slots (None = rest)
MOTIF = [
    [69, None, 72, None, 76, 74, 72, None],
    [71, None, 67, None, 71, 72, 74, None],
    [69, None, 65, None, 69, 72, 76, 74],
    [72, None, None, 67, 64, None, 67, None],
]

def groove(bus, t0, t1, energy=1.0, glock=False, shaker=True, harp=False, melody=True):
    """4-bar loop anchored at t0, cut at t1."""
    bar = 0
    while t0 + bar * BAR < t1 - 0.05:
        tb = t0 + bar * BAR
        ch = CHORDS[bar % 4]
        # marimba bass on beats 1 and 3 (with the fifth on 3), soft
        for k, m in [(0, ch['bass'] + 12), (2, ch['bass'] + 19)]:
            if tb + k * BEAT < t1: note(bus, 'marimba', m, tb + k * BEAT, 0.42 * energy, -0.15, dur=BEAT * 1.6)
        # vibraphone chord swell at the bar line
        if energy > 0.6:
            for i, m in enumerate(ch['pad'][1:]):
                if tb < t1: note(bus, 'vibraphone', m + 12, tb + i * 0.012, 0.12 * energy, 0.2 - i * 0.2, dur=min(BAR * 0.95, t1 - tb))
        if melody:
            for s, m in enumerate(MOTIF[bar % 4]):
                ts = tb + s * BEAT / 2
                if m and ts < t1: note(bus, 'kalimba', m, ts, 0.5 * energy * (1.0 if s % 2 == 0 else 0.8), 0.15)
        if glock and bar % 2 == 1 and tb + BEAT * 3.5 < t1:
            note(bus, 'glock', MOTIF[bar % 4][0] + 12 if MOTIF[bar % 4][0] else 84, tb + BEAT * 3.5, 0.18, 0.4)
        if harp and bar % 4 == 3:
            for i, m in enumerate([60, 64, 67, 71, 72, 76]):
                if tb + BEAT * 2 + i * 0.07 < t1: note(bus, 'harp', m, tb + BEAT * 2 + i * 0.07, 0.25, -0.3 + i * 0.12)
        if shaker:
            for s in range(8):
                ts = tb + s * BEAT / 2 + 0.008 * np.sin(s * 1.7)
                if ts < t1: bus.add(CABASA, ts, 0.10 * energy * (1.0 if s % 2 else 0.6), 0.35)
        bar += 1

def hook(bus, tl):
    # problem statement: sparse A-minor kalimba, then a soft low marimba pulse under "keys could walk away",
    # turning to a bright glock figure when qn appears
    t0, t1 = tl['sections']['H']
    l2, qn = tl['hookL2'], tl['qnStart']
    am = [57, 60, 64, 67, 64, 60]
    t = t0 + 0.2; i = 0
    while t < l2 - 0.2:
        note(bus, 'kalimba', am[i % len(am)] + (12 if i % 4 == 2 else 0), t, 0.35, 0.1 - 0.05 * (i % 3)); t += BEAT; i += 1
    t = l2
    while t < qn - 0.1:
        note(bus, 'marimba', 45 if int((t - l2) / BEAT) % 2 == 0 else 52, t, 0.32, -0.1, dur=BEAT * 0.9)
        if int((t - l2) / BEAT) % 4 == 0: note(bus, 'vibraphone', 64 + 12, t, 0.08, 0.2, dur=BEAT * 3.6)
        t += BEAT
    for j, m in enumerate([72, 76, 79, 84]): note(bus, 'glock', m, qn + 0.15 + j * 0.09, 0.2, -0.2 + j * 0.13)
    t = qn + 1.2
    while t < t1 - BEAT * 2:
        note(bus, 'kalimba', [72, 67, 76, 67][int((t - qn) / BEAT) % 4], t, 0.3, 0.15); t += BEAT

def score(bus, timeline):
    S = timeline['sections']
    hook(bus, timeline)
    for i, m in enumerate([60, 64, 67, 71]):
        note(bus, 'harp', m, S['A'][0] - BEAT * 2 + i * BEAT / 2, 0.3, -0.2 + i * 0.12)
    groove(bus, S['A'][0], S['A'][1], 0.85, glock=True, shaker=True)
    groove(bus, S['B'][0], S['B'][1], 0.9, glock=False, shaker=True)
    groove(bus, S['C'][0], S['C'][1], 1.0, glock=True, shaker=True, harp=True)
    # outro: half-time vibraphone + sparse kalimba, final glock chord under the title
    t0, t1 = S['outro']
    for i, ch in enumerate([CHORDS[0], CHORDS[3]]):
        tb = t0 + i * BAR * 1.5
        for j, m in enumerate(ch['pad'][1:]): note(bus, 'vibraphone', m + 12, tb + j * 0.02, 0.16, -0.2 + j * 0.2, dur=BAR * 1.5)
        note(bus, 'marimba', ch['bass'] + 12, tb, 0.35, -0.1, dur=BAR)
    for k, m in enumerate([76, 74, 72, 67, 72]):
        note(bus, 'kalimba', m, t0 + 0.6 + k * BEAT * 1.5, 0.4, 0.15)
    for j, m in enumerate([72, 76, 79, 84]): note(bus, 'glock', m, timeline['title'] + j * 0.03, 0.2, -0.3 + j * 0.2)
    for j, m in enumerate([48, 55, 64, 71]): note(bus, 'vibraphone', m + 12, timeline['title'], 0.14, 0, dur=4.0)

# ---------------------------------------------------------------- mix
def main():
    ev_path, out = sys.argv[1], sys.argv[2]
    a = sys.argv[3:]
    meta = json.load(open(ev_path))
    dur = meta['DUR']
    t_from = float(a[a.index('--from') + 1]) if '--from' in a else 0.0
    t_to = float(a[a.index('--to') + 1]) if '--to' in a else dur
    tl = meta['TL']
    music, fx = Bus(dur), Bus(dur)
    score(music, tl)
    # hard silences: music stops (with a short release) for the two held breaths
    m = music.x
    for s0, s1 in tl['silences']:
        i0, i1 = int(s0 * SR), int(s1 * SR); rel = int(0.12 * SR)
        m[i0:i0 + rel] *= np.linspace(1, 0, rel)[:, None]
        m[i0 + rel:i1] = 0
    duck = np.ones(len(m), np.float32)
    for e in meta['EV']:
        if e['type'] not in SFX: continue
        y, g, pan = SFX[e['type']](e)
        fx.add(y, e['t'], g, pan)
        if e['type'] in DUCK:
            i = int(e['t'] * SR); n = int(0.6 * SR); att = int(0.01 * SR)
            shape = np.concatenate([np.linspace(1, DUCK[e['type']], att), np.linspace(DUCK[e['type']], 1, n)])
            seg_ = duck[i:i + len(shape)]; duck[i:i + len(shape)] = np.minimum(seg_, shape[: len(seg_)])
    m *= duck[:, None]
    # narration: each line levelled to the same RMS, music ducked ~7 dB while the voice speaks
    vo = Bus(dur); venv = np.zeros(len(m), np.float32)
    for v in meta.get('VO', []):
        sr_, y = wavfile.read(os.path.join(HERE, '..', 'film', 'vo', f"{v['id']}.wav"))
        y = y.astype(np.float32) / 32768.0
        # voice EQ: high-pass the rumble, a little presence (2.5–6 kHz) for clarity
        y = hp(y, 90) ; y = y + 0.45 * bp(y, 2500, 6000)
        y = y / max(1e-6, np.sqrt(np.mean(y ** 2))) * 0.1
        vo.add(np.stack([y, y], 1), v['t'], 1.0, 0.0)
        i = int(v['t'] * SR); venv[i:i + len(y)] = 1.0
    # smooth the voice-activity envelope (attack ~60 ms, release ~350 ms)
    k = int(0.35 * SR); ker = np.ones(k, np.float32) / k
    venv = np.convolve(venv, ker, mode='same')
    m *= (1 - 0.55 * np.clip(venv, 0, 1))[:, None]
    # gentle low shelf cut on the music so the synthetic score is not bass-heavy
    m = m - 0.35 * np.stack([lp(m[:, 0], 120), lp(m[:, 1], 120)], 1)
    # room tone: very quiet pink-ish noise so silences are "near silence", not digital zero
    room = lp(rng.standard_normal(len(m)), 900, 1) * 0.0018
    mix = m * 0.9 + fx.x * 0.85 + vo.x + np.stack([room, room], 1)
    mix = hp(mix.T, 30).T
    i0, i1 = int(t_from * SR), int(t_to * SR)
    y = mix[i0:i1]
    # soft clip, leave headroom; loudness is set by ffmpeg loudnorm afterwards
    y = np.tanh(y * 1.2) / 1.2
    y = y / max(1e-6, np.abs(y).max()) * 0.8
    wavfile.write(out, SR, (y * 32767).astype(np.int16))
    print('wrote', out, f'{(i1 - i0) / SR:.2f}s')

if __name__ == '__main__':
    main()
