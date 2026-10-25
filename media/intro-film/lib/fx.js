// Hand-drawn 2D effects layered over the 3D: bursts, stars, speed lines, stamps, sticky
// notes, subtitle pill. Deterministic (seeded).
import { W, H, rr, mulberry, clamp } from './lib.js';

export const INK = '#2a1a14';

export function burst(f, cx, cy, r, n, col, seed, text, o = {}) {
  const R = mulberry(seed);
  f.save(); f.translate(cx, cy); f.rotate(o.rot || 0); f.scale(o.scale || 1, o.scale || 1);
  f.beginPath();
  for (let i = 0; i < n * 2; i++) {
    const a = i / (n * 2) * Math.PI * 2;
    const rad = i % 2 ? r * (0.62 + R() * 0.1) : r * (0.95 + R() * 0.15);
    f.lineTo(Math.cos(a) * rad * 1.25, Math.sin(a) * rad);
  }
  f.closePath(); f.fillStyle = col; f.fill(); f.lineWidth = 6; f.strokeStyle = INK; f.stroke();
  if (text) {
    f.font = `400 ${o.size || 34}px "ZCOOL KuaiLe"`; f.textAlign = 'center'; f.textBaseline = 'middle';
    f.fillStyle = o.ink || INK; f.fillText(text, 0, 2);
  }
  f.restore();
}

export function star(f, x, y, s, col) {
  f.save(); f.translate(x, y);
  f.beginPath();
  for (let i = 0; i < 8; i++) { const a = i * Math.PI / 4; const rad = i % 2 ? s * 0.28 : s; f.lineTo(Math.cos(a) * rad, Math.sin(a) * rad); }
  f.closePath(); f.fillStyle = col; f.fill(); f.lineWidth = 3.5; f.strokeStyle = INK; f.stroke();
  f.restore();
}

export function speedLines(f, cx, cy, r0, n, seed, o = {}) {
  const R = mulberry(seed);
  f.save(); f.strokeStyle = o.col || INK; f.lineCap = 'round';
  for (let i = 0; i < n; i++) {
    const a = (o.a0 ?? 0) + R() * (o.span ?? Math.PI * 2), a0 = r0 + R() * 25, a1 = a0 + 30 + R() * 50;
    f.lineWidth = 2 + R() * 3;
    f.beginPath(); f.moveTo(cx + Math.cos(a) * a0, cy + Math.sin(a) * a0 * (o.sy || 1.3)); f.lineTo(cx + Math.cos(a) * a1, cy + Math.sin(a) * a1 * (o.sy || 1.3)); f.stroke();
  }
  f.restore();
}

// Rubber stamp sticker ("已拦截"), with print misregistration.
export function stamp(f, text, x, y, o = {}) {
  const col = o.col || '#E8584A';
  f.save(); f.translate(x, y); f.rotate(o.rot ?? -0.12); const s = o.scale || 1; f.scale(s, s);
  f.font = `400 ${o.size || 46}px "ZCOOL KuaiLe"`;
  const tw = f.measureText(text).width, w = tw + 56, h = (o.size || 46) + 34;
  f.fillStyle = INK; rr(f, -w / 2 + 6, -h / 2 + 8, w, h, 16); f.fill();
  f.fillStyle = col; rr(f, -w / 2, -h / 2, w, h, 16); f.fill();
  f.lineWidth = 5; f.strokeStyle = INK; rr(f, -w / 2, -h / 2, w, h, 16); f.stroke();
  f.strokeStyle = 'rgba(255,255,255,0.8)'; f.lineWidth = 2.5; rr(f, -w / 2 + 8, -h / 2 + 8, w - 16, h - 16, 10); f.stroke();
  f.textAlign = 'center'; f.textBaseline = 'middle';
  f.fillStyle = 'rgba(0,170,220,0.5)'; f.fillText(text, -2.5, 1);
  f.fillStyle = '#fff'; f.fillText(text, 0, 3);
  f.restore();
}

// Sticky note that floats in (the prompt-injection lure).
export function stickyNote(f, lines, x, y, rot, o = {}) {
  f.save(); f.translate(x, y); f.rotate(rot);
  const w = o.w || 330, h = o.h || 150;
  f.shadowColor = 'rgba(40,20,10,0.3)'; f.shadowBlur = 24; f.shadowOffsetY = 12;
  f.fillStyle = '#FFE680'; f.fillRect(-w / 2, -h / 2, w, h);
  f.shadowColor = 'transparent';
  f.fillStyle = 'rgba(255,255,255,0.35)'; f.fillRect(-w / 2, -h / 2, w, 22);
  f.lineWidth = 4; f.strokeStyle = INK; f.strokeRect(-w / 2, -h / 2, w, h);
  f.fillStyle = INK; f.font = '400 30px "ZCOOL KuaiLe"'; f.textBaseline = 'middle';
  lines.forEach((l, i) => f.fillText(l, -w / 2 + 22, -h / 2 + 52 + i * 42));
  f.restore();
}

// Subtitle pill (A-style), bottom centre unless y is given.
export function caption(f, text, en, o = {}) {
  if (!text) return;
  const a = clamp(o.alpha ?? 1);
  f.save(); f.globalAlpha = a;
  f.font = '600 30px "Noto Sans SC"';
  const tw = f.measureText(text).width;
  const w = tw + 80, h = en ? 78 : 62, x = (W - w) / 2, y = o.y ?? 880;
  f.fillStyle = 'rgba(40,28,22,0.76)'; rr(f, x, y, w, h, h / 2); f.fill();
  f.fillStyle = '#fff'; f.textAlign = 'center'; f.textBaseline = 'alphabetic'; f.fillText(text, W / 2, y + (en ? 40 : 41));
  if (en) { f.font = '500 14px Fredoka'; f.fillStyle = 'rgba(255,255,255,0.65)'; f.fillText(en, W / 2, y + 63); }
  f.restore();
}

export function vignette(f, k, col = '20,10,6') {
  const g = f.createRadialGradient(W / 2, H / 2, H * 0.3, W / 2, H / 2, H * 0.95);
  g.addColorStop(0, `rgba(${col},0)`); g.addColorStop(1, `rgba(${col},${0.6 * k})`);
  f.fillStyle = g; f.fillRect(0, 0, W, H);
}

// Keystroke HUD (screencast grammar)
export function keyHud(f, keys, x, y) {
  f.save(); f.font = '600 24px "JetBrains Mono"';
  let cx = x;
  for (const k of keys) {
    const w = Math.max(54, f.measureText(k).width + 30);
    f.fillStyle = 'rgba(40,28,22,0.78)'; rr(f, cx, y, w, 54, 12); f.fill();
    f.fillStyle = '#fff'; f.textAlign = 'center'; f.textBaseline = 'middle'; f.fillText(k, cx + w / 2, y + 28);
    cx += w + 10;
  }
  f.restore();
}
