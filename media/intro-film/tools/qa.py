"""Frame-level checks on a rendered frames folder: solid-colour / black frames and odd colours."""
import sys, glob
import numpy as np
from PIL import Image
d = sys.argv[1]
files = sorted(glob.glob(f'{d}/f*.jpg'))
bad = []
for i, p in enumerate(files):
    a = np.asarray(Image.open(p).reduce(8), np.float32)
    mean, std = a.mean(), a.std()
    # magenta/green "missing texture" style colours or NaN-black
    odd = ((a[..., 0] > 240) & (a[..., 1] < 20) & (a[..., 2] > 240)).mean()
    if std < 6 or mean < 12 or odd > 0.01: bad.append((i, round(float(mean), 1), round(float(std), 1), round(float(odd), 3)))
print(f'{len(files)} frames scanned, {len(bad)} flagged')
for b in bad[:30]: print('  frame', b)
sys.exit(1 if bad else 0)
