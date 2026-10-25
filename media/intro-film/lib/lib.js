// Shared drawing helpers for the qn sandbox film. Everything is a pure function of its inputs.

export const W = 1920, H = 1080;

export function mulberry(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
export const lerp = (a, b, k) => a + (b - a) * k;
export const easeOut = (k) => 1 - Math.pow(1 - clamp(k), 3);
export const easeInOut = (k) => { k = clamp(k); return k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2; };
export const backOut = (k, s = 1.7) => { k = clamp(k) - 1; return 1 + (s + 1) * k * k * k + s * k * k; };

export async function loadFonts(base = '/fonts/') {
  const faces = [
    ['Noto Sans SC', 'NotoSansSC.ttf', '100 900'],
    ['ZCOOL KuaiLe', 'ZCOOLKuaiLe.ttf', '400'],
    ['Fredoka', 'Fredoka.ttf', '300 700'],
    ['JetBrains Mono', 'JetBrainsMono.ttf', '100 800'],
  ];
  for (const [fam, file, weight] of faces) {
    const f = new FontFace(fam, `url(${base}${file})`, { weight });
    await f.load();
    document.fonts.add(f);
  }
}

export function rr(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, r);
}

// ---------- Clawd: 18×10 pixel mascot ----------
// '#' body, 'e' eye. Arms on row 3–4, four legs on rows 7–9.
export const CLAWD = [
  '..##############..',
  '..##############..',
  '..###e######e###..',
  '#####e######e#####',
  '##################',
  '..##############..',
  '..##############..',
  '...#..#....#..#...',
  '...#..#....#..#...',
  '...#..#....#..#...',
];
export const CLAY = '#D97757';
export const CLAY_DARK = '#B95D3F';
export const CLAY_LIGHT = '#EE9B7E';

// eyes: 'open' | 'happy' | 'wide' | 'shut' | 'look-l' | 'look-r' | 'look-up'
// squash: 1 = neutral; <1 squashed (wider), >1 stretched (taller). Anchored at feet.
export function drawClawd(ctx, cx, footY, px, o = {}) {
  const eyes = o.eyes || 'open';
  const sq = o.squash || 1;
  const sx = px / Math.sqrt(sq), sy = px * sq;
  const flip = o.flip ? -1 : 1;
  const legPhase = o.legPhase || 0;
  ctx.save();
  ctx.translate(Math.round(cx), Math.round(footY));
  ctx.scale(flip, 1);
  if (o.shadow !== false) {
    ctx.fillStyle = o.shadowColor || 'rgba(60,30,20,0.18)';
    ctx.beginPath();
    ctx.ellipse(0, 2, 9.5 * sx, 1.6 * px, 0, 0, Math.PI * 2);
    ctx.fill();
  }
  const ox = -9 * sx, oy = -10 * sy;
  for (let r = 0; r < 10; r++) {
    for (let c = 0; c < 18; c++) {
      const ch = CLAWD[r][c];
      if (ch === '.') continue;
      let yy = r;
      // walk cycle: alternate leg pairs lift by one pixel row
      if (r >= 7 && legPhase) {
        const pairA = c === 3 || c === 11;
        if ((legPhase > 0) === pairA && r === 9) continue;
      }
      let col = o.color || CLAY;
      if (ch === 'e') col = eyeColor(eyes, r, o);
      if (ch === '#' && o.shade !== false) {
        if (r === 0) col = o.light || CLAY_LIGHT;
        else if (r === 6 || r >= 7) col = o.dark || CLAY_DARK;
      }
      ctx.fillStyle = col;
      ctx.fillRect(ox + c * sx, oy + yy * sy, sx + 0.5, sy + 0.5);
    }
  }
  drawEyes(ctx, ox, oy, sx, sy, eyes, o);
  ctx.restore();
}

function eyeColor(eyes, r, o) {
  // Eye cells are repainted by drawEyes; base them on body colour.
  return o.color || CLAY;
}

function drawEyes(ctx, ox, oy, sx, sy, eyes, o) {
  const ink = o.ink || '#1E1410';
  ctx.fillStyle = ink;
  const cols = [5, 12];
  for (const c of cols) {
    const x = ox + c * sx, y = oy + 2 * sy;
    if (eyes === 'open') ctx.fillRect(x, y, sx, sy * 2);
    else if (eyes === 'wide') { ctx.fillRect(x - sx * 0.25, y - sy * 0.5, sx * 1.5, sy * 2.6); ctx.fillStyle = '#fff'; ctx.fillRect(x + sx * 0.5, y - sy * 0.2, sx * 0.5, sy * 0.6); ctx.fillStyle = ink; }
    else if (eyes === 'shut') ctx.fillRect(x - sx * 0.3, y + sy * 1.2, sx * 1.6, sy * 0.5);
    else if (eyes === 'happy') { ctx.fillRect(x - sx * 0.5, y + sy * 1.0, sx * 0.6, sy * 0.6); ctx.fillRect(x, y + sy * 0.4, sx, sy * 0.6); ctx.fillRect(x + sx * 0.9, y + sy * 1.0, sx * 0.6, sy * 0.6); }
    else if (eyes === 'look-l') ctx.fillRect(x - sx * 0.5, y, sx, sy * 2);
    else if (eyes === 'look-r') ctx.fillRect(x + sx * 0.5, y, sx, sy * 2);
    else if (eyes === 'look-up') ctx.fillRect(x, y - sy * 0.5, sx, sy * 1.6);
  }
}

// ---------- desktop chrome (interface grammar only: no OS logos, no real app icons) ----------
export function menuBar(ctx, o = {}) {
  const h = o.h || 38;
  ctx.save();
  ctx.fillStyle = o.bg || 'rgba(255,255,255,0.55)';
  ctx.fillRect(0, 0, W, h);
  ctx.fillStyle = o.ink || '#2b2622';
  // generic app glyph: a small rounded square with a dot (our own mark, not an OS logo)
  rr(ctx, 26, h / 2 - 9, 18, 18, 5); ctx.fill();
  ctx.fillStyle = o.accent || CLAY; ctx.beginPath(); ctx.arc(35, h / 2, 4, 0, 7); ctx.fill();
  ctx.fillStyle = o.ink || '#2b2622';
  ctx.font = '600 17px "Noto Sans SC", Inter';
  ctx.textBaseline = 'middle';
  ctx.fillText('qn sandbox', 60, h / 2 + 1);
  ctx.font = '400 16px "Noto Sans SC", Inter';
  let x = 175;
  for (const s of o.items || ['会话', '配置档', '权限', '审计']) { ctx.fillText(s, x, h / 2 + 1); x += ctx.measureText(s).width + 26; }
  ctx.textAlign = 'right';
  ctx.fillText(o.clock || '周六 10:24', W - 28, h / 2 + 1);
  // shield status glyph
  const sx = W - 150;
  shield(ctx, sx, h / 2, 9, o.accent || '#3E9B6F');
  ctx.restore();
}

export function shield(ctx, x, y, s, col) {
  ctx.save();
  ctx.fillStyle = col;
  ctx.beginPath();
  ctx.moveTo(x, y - s);
  ctx.lineTo(x + s * 0.85, y - s * 0.6);
  ctx.quadraticCurveTo(x + s * 0.85, y + s * 0.6, x, y + s);
  ctx.quadraticCurveTo(x - s * 0.85, y + s * 0.6, x - s * 0.85, y - s * 0.6);
  ctx.closePath();
  ctx.fill();
  ctx.strokeStyle = '#fff'; ctx.lineWidth = s * 0.22; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  ctx.beginPath(); ctx.moveTo(x - s * 0.35, y); ctx.lineTo(x - s * 0.05, y + s * 0.3); ctx.lineTo(x + s * 0.4, y - s * 0.3); ctx.stroke();
  ctx.restore();
}

export function dock(ctx, o = {}) {
  const n = o.colors.length, s = o.size || 62, gap = 14;
  const w = n * s + (n - 1) * gap + 28;
  const x0 = (W - w) / 2, y0 = H - s - 34;
  ctx.save();
  ctx.fillStyle = o.bg || 'rgba(255,255,255,0.45)';
  ctx.shadowColor = 'rgba(40,20,10,0.15)'; ctx.shadowBlur = 30; ctx.shadowOffsetY = 8;
  rr(ctx, x0, y0 - 12, w, s + 24, 24); ctx.fill();
  ctx.shadowColor = 'transparent';
  o.colors.forEach((c, i) => {
    const x = x0 + 14 + i * (s + gap);
    const g = ctx.createLinearGradient(0, y0, 0, y0 + s);
    g.addColorStop(0, c[0]); g.addColorStop(1, c[1]);
    ctx.fillStyle = g; rr(ctx, x, y0, s, s, 15); ctx.fill();
    if (c[2]) c[2](ctx, x + s / 2, y0 + s / 2, s);
  });
  ctx.restore();
  return { x0, y0, w, s };
}

export function trafficLights(ctx, x, y, r = 7) {
  ['#FF5F57', '#FEBC2E', '#28C840'].forEach((c, i) => {
    ctx.fillStyle = c; ctx.beginPath(); ctx.arc(x + i * (r * 2 + 8), y, r, 0, 7); ctx.fill();
  });
}

// A simple vector key (host-side credential). Drawn, not an icon from any system.
export function key(ctx, x, y, s, o = {}) {
  ctx.save();
  ctx.translate(x, y); ctx.rotate(o.rot || 0);
  const gold = o.color || '#F2B84B', dark = o.dark || '#B9822A', ink = o.ink;
  ctx.lineJoin = 'round';
  const path = () => {
    ctx.beginPath();
    ctx.arc(0, 0, s, 0, Math.PI * 2);
    ctx.moveTo(s * 0.92, -s * 0.22);
    ctx.lineTo(s * 3.1, -s * 0.22); ctx.lineTo(s * 3.1, s * 0.22);
    ctx.lineTo(s * 2.85, s * 0.22); ctx.lineTo(s * 2.85, s * 0.7); ctx.lineTo(s * 2.5, s * 0.7); ctx.lineTo(s * 2.5, s * 0.22);
    ctx.lineTo(s * 2.15, s * 0.22); ctx.lineTo(s * 2.15, s * 0.55); ctx.lineTo(s * 1.85, s * 0.55); ctx.lineTo(s * 1.85, s * 0.22);
    ctx.lineTo(s * 0.92, s * 0.22);
  };
  if (ink) { ctx.strokeStyle = ink; ctx.lineWidth = o.inkW || s * 0.32; path(); ctx.stroke(); }
  ctx.fillStyle = gold; path(); ctx.fill();
  ctx.fillStyle = dark; ctx.beginPath(); ctx.arc(0, 0, s * 0.42, 0, 7); ctx.fill();
  ctx.fillStyle = 'rgba(255,255,255,0.55)'; ctx.beginPath(); ctx.arc(-s * 0.35, -s * 0.45, s * 0.22, 0, 7); ctx.fill();
  ctx.restore();
}

export function setupCanvas(id = 'c') {
  const c = document.getElementById(id);
  c.width = W; c.height = H;
  return c.getContext('2d');
}
