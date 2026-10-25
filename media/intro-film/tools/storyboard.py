import json, sys
from PIL import Image, ImageDraw, ImageFont
D = sys.argv[1]; OUT = sys.argv[2]
shots = json.load(open(f'{D}/shots.json'))
def font(sz, w=400, f='fonts/NotoSansSC.ttf'):
    ft = ImageFont.truetype(f, sz)
    try: ft.set_variation_by_axes([w])
    except Exception: pass
    return ft
TW, TH, G, TXT = 600, 338, 30, 230
cols = 3; rows = (len(shots) + cols - 1) // cols
S = Image.new('RGB', (G + cols * (TW + G), 130 + rows * (TH + TXT + G)), '#F4EEE8')
d = ImageDraw.Draw(S)
dur = shots[-1]['t1']
d.text((G, 34), f'qn sandbox · 分镜（{len(shots)} 镜 · {dur:.0f} 秒 · B+A 风格 · 含旁白）', fill='#2b2420', font=font(36, 700))
d.text((G, 84), '每格都是正片绘制代码在该镜关键帧的实际渲染；【留白】= 静默处', fill='#7b6a5f', font=font(20))
def mmss(t): return f'{int(t//60)}:{t%60:04.1f}'
def wrap(text, ft, width, maxl=3):
    lines, line = [], ''
    for ch in text:
        if d.textlength(line + ch, font=ft) > width: lines.append(line); line = ''
        line += ch
    if line: lines.append(line)
    return lines[:maxl]
for i, s in enumerate(shots):
    x = G + (i % cols) * (TW + G); y = 130 + (i // cols) * (TH + TXT + G)
    im = Image.open(f"{D}/{str(s['id']).zfill(2)}.png").convert('RGB').resize((TW, TH), Image.LANCZOS)
    S.paste(im, (x, y))
    d.rounded_rectangle((x + 12, y + 12, x + 64, y + 52), 10, fill='#D97757')
    d.text((x + 38, y + 32), str(i + 1), fill='white', font=font(24, 700), anchor='mm')
    ty = y + TH + 12
    d.text((x, ty), f"{mmss(s['t0'])}–{mmss(s['t1'])}  ({s['t1']-s['t0']:.1f}s)", fill='#D97757', font=font(19, 700))
    d.text((x + 230, ty), s['framing'], fill='#2b2420', font=font(19, 700))
    ty += 34
    for l in wrap(s['action'], font(17), TW):
        d.text((x, ty), l, fill='#4a3f38', font=font(17)); ty += 26
    ty = y + TH + TXT - 66
    line = s['line'] or '—'
    for l in wrap('旁白：' + line, font(16, 600), TW, 2):
        d.text((x, ty - 14), l, fill='#7b6a5f', font=font(16, 600)); ty += 24
S.save(OUT, quality=90); print(S.size)
