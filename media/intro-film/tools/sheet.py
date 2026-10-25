import sys
from PIL import Image, ImageDraw, ImageFont
F = 'fonts/NotoSansSC.ttf'
def font(sz, w=700):
    f = ImageFont.truetype(F, sz)
    try: f.set_variation_by_axes([w])
    except Exception: pass
    return f
tiles = [
 ('out/a.png', 'A  高清界面 × 像素 Clawd', '真实感最强：像一流产品的录屏发布片，界面交互细节能做到最精致。适合把命令、通知、审计、控制面板这些“真东西”讲清楚。'),
 ('out/b.png', 'B  卡通 3D × 2D 漫画特效', '最有记忆点：描边 3D 玻璃屋 + 体素 Clawd，速度线 / 爆炸框 / 贴纸字叠在 3D 上。适合“撞墙被拦截、钥匙在外面”这类有戏剧性的镜头。'),
 ('out/c.png', 'C  纸艺立体', '最温暖、最“舒服”：剪纸角色带白边和纸纹，描图纸当玻璃。适合讲“待着很舒服”的情绪，节奏可以更慢、更松弛。'),
 ('out/d.png', 'D  等距玻璃微缩', '最高级、最安静：极光渐变 + 磨砂玻璃卡片，桌面变成一块微缩台面。适合讲“结构与边界”：哪里是沙箱、哪里是宿主侧。'),
]
TW, TH, PAD, LAB = 940, 529, 26, 150
S = Image.new('RGB', (PAD + 2 * (TW + PAD), 120 + 2 * (TH + LAB + PAD)), '#F4EEE8')
d = ImageDraw.Draw(S)
d.text((PAD, 34), 'qn sandbox · 风格探索（同一主画面：桌面 / 沙箱 / Clawd / 窗外的钥匙）', fill='#2b2420', font=font(34))
for i, (p, title, desc) in enumerate(tiles):
    x = PAD + (i % 2) * (TW + PAD); y = 120 + (i // 2) * (TH + LAB + PAD)
    im = Image.open(p).convert('RGB').resize((TW, TH), Image.LANCZOS)
    S.paste(im, (x, y))
    d.text((x, y + TH + 16), title, fill='#2b2420', font=font(30))
    # wrap description by width
    f = font(21, 400); line = ''; yy = y + TH + 62
    for ch in desc:
        if d.textlength(line + ch, font=f) > TW:
            d.text((x, yy), line, fill='#5a4d45', font=f); yy += 32; line = ''
        line += ch
    d.text((x, yy), line, fill='#5a4d45', font=f)
S.save(sys.argv[1], quality=92)
print('ok', S.size)
