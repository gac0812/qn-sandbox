// Mac-style interface grammar, drawn from scratch: squircle app icons, dock, menu bar status
// items, windows, notification banners. Original glyphs only: no OS logos, no real app icons.
import { W, H, rr, CLAY, drawClawd } from './lib.js';

// superellipse-ish squircle
export function squircle(ctx, x, y, s) {
  const r = s * 0.235;
  ctx.beginPath();
  ctx.roundRect(x, y, s, s, r);
}

function grad(ctx, y, s, a, b) {
  const g = ctx.createLinearGradient(0, y, 0, y + s);
  g.addColorStop(0, a); g.addColorStop(1, b);
  return g;
}

// ---- glyphs: each draws inside an icon of size s at (x, y) ----
const G = {
  files(ctx, x, y, s) {
    ctx.fillStyle = '#fff';
    rr(ctx, x + s * 0.2, y + s * 0.3, s * 0.6, s * 0.44, s * 0.06); ctx.fill();
    rr(ctx, x + s * 0.2, y + s * 0.25, s * 0.24, s * 0.1, s * 0.04); ctx.fill();
    ctx.fillStyle = 'rgba(80,140,230,0.35)'; ctx.fillRect(x + s * 0.2, y + s * 0.38, s * 0.6, s * 0.03);
  },
  browser(ctx, x, y, s) {
    const cx = x + s / 2, cy = y + s / 2, r = s * 0.31;
    ctx.fillStyle = grad(ctx, cy - r, r * 2, '#5FB4FF', '#2F6FE0');
    ctx.beginPath(); ctx.arc(cx, cy, r, 0, 7); ctx.fill();
    ctx.strokeStyle = 'rgba(255,255,255,0.9)'; ctx.lineWidth = s * 0.028;
    ctx.beginPath(); ctx.ellipse(cx, cy, r * 0.45, r, 0, 0, 7); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(cx - r, cy); ctx.lineTo(cx + r, cy); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(cx, cy - r); ctx.lineTo(cx, cy + r); ctx.stroke();
    ctx.beginPath(); ctx.ellipse(cx, cy - r * 0.5, r * 0.86, r * 0.12, 0, 0, Math.PI); ctx.stroke();
    ctx.beginPath(); ctx.ellipse(cx, cy + r * 0.5, r * 0.86, r * 0.12, 0, Math.PI, 0); ctx.stroke();
  },
  mail(ctx, x, y, s) {
    ctx.fillStyle = '#fff';
    rr(ctx, x + s * 0.18, y + s * 0.3, s * 0.64, s * 0.42, s * 0.05); ctx.fill();
    ctx.strokeStyle = '#4A90E8'; ctx.lineWidth = s * 0.035; ctx.lineJoin = 'round';
    ctx.beginPath(); ctx.moveTo(x + s * 0.2, y + s * 0.33); ctx.lineTo(x + s * 0.5, y + s * 0.53); ctx.lineTo(x + s * 0.8, y + s * 0.33); ctx.stroke();
  },
  calendar(ctx, x, y, s) {
    ctx.save(); squircle(ctx, x, y, s); ctx.clip();
    ctx.fillStyle = '#F05A4F'; ctx.fillRect(x, y, s, s * 0.28);
    ctx.restore();
    ctx.fillStyle = '#fff'; ctx.font = `600 ${s * 0.15}px "Noto Sans SC"`; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
    ctx.fillText('周六', x + s / 2, y + s * 0.145);
    ctx.fillStyle = '#2b2622'; ctx.font = `500 ${s * 0.44}px Fredoka`; ctx.fillText('25', x + s / 2, y + s * 0.63);
    ctx.textAlign = 'left'; ctx.textBaseline = 'alphabetic';
  },
  notes(ctx, x, y, s) {
    ctx.save(); squircle(ctx, x, y, s); ctx.clip();
    ctx.fillStyle = grad(ctx, y, s * 0.26, '#FFD45E', '#F5B932'); ctx.fillRect(x, y, s, s * 0.26);
    ctx.restore();
    ctx.strokeStyle = 'rgba(160,140,110,0.45)'; ctx.lineWidth = s * 0.018;
    for (let i = 0; i < 4; i++) { const yy = y + s * (0.42 + i * 0.13); ctx.beginPath(); ctx.moveTo(x + s * 0.16, yy); ctx.lineTo(x + s * 0.84, yy); ctx.stroke(); }
  },
  music(ctx, x, y, s) {
    ctx.fillStyle = '#fff';
    ctx.beginPath(); ctx.ellipse(x + s * 0.36, y + s * 0.68, s * 0.1, s * 0.08, -0.3, 0, 7); ctx.fill();
    ctx.beginPath(); ctx.ellipse(x + s * 0.64, y + s * 0.62, s * 0.1, s * 0.08, -0.3, 0, 7); ctx.fill();
    ctx.fillRect(x + s * 0.43, y + s * 0.26, s * 0.04, s * 0.42);
    ctx.fillRect(x + s * 0.71, y + s * 0.2, s * 0.04, s * 0.42);
    ctx.beginPath(); ctx.moveTo(x + s * 0.43, y + s * 0.26); ctx.lineTo(x + s * 0.75, y + s * 0.2); ctx.lineTo(x + s * 0.75, y + s * 0.3); ctx.lineTo(x + s * 0.43, y + s * 0.36); ctx.fill();
  },
  terminal(ctx, x, y, s) {
    ctx.fillStyle = '#1d1d1f'; rr(ctx, x + s * 0.12, y + s * 0.18, s * 0.76, s * 0.64, s * 0.08); ctx.fill();
    ctx.strokeStyle = '#fff'; ctx.lineWidth = s * 0.05; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.beginPath(); ctx.moveTo(x + s * 0.25, y + s * 0.36); ctx.lineTo(x + s * 0.37, y + s * 0.46); ctx.lineTo(x + s * 0.25, y + s * 0.56); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(x + s * 0.43, y + s * 0.58); ctx.lineTo(x + s * 0.6, y + s * 0.58); ctx.stroke();
  },
  code(ctx, x, y, s) {
    ctx.strokeStyle = '#fff'; ctx.lineWidth = s * 0.06; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.beginPath(); ctx.moveTo(x + s * 0.36, y + s * 0.33); ctx.lineTo(x + s * 0.22, y + s * 0.5); ctx.lineTo(x + s * 0.36, y + s * 0.67); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(x + s * 0.64, y + s * 0.33); ctx.lineTo(x + s * 0.78, y + s * 0.5); ctx.lineTo(x + s * 0.64, y + s * 0.67); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(x + s * 0.55, y + s * 0.28); ctx.lineTo(x + s * 0.45, y + s * 0.72); ctx.stroke();
  },
  settings(ctx, x, y, s) {
    const cx = x + s / 2, cy = y + s / 2;
    ctx.fillStyle = '#5b5f66';
    ctx.beginPath();
    for (let i = 0; i < 24; i++) { const a = i / 24 * Math.PI * 2; const r = i % 2 ? s * 0.27 : s * 0.33; ctx.lineTo(cx + Math.cos(a) * r, cy + Math.sin(a) * r); }
    ctx.closePath(); ctx.fill();
    ctx.fillStyle = '#d5d8dd'; ctx.beginPath(); ctx.arc(cx, cy, s * 0.17, 0, 7); ctx.fill();
    ctx.fillStyle = '#5b5f66'; ctx.beginPath(); ctx.arc(cx, cy, s * 0.07, 0, 7); ctx.fill();
  },
  qn(ctx, x, y, s) {
    // our product's own icon: a shield with Clawd's face
    const cx = x + s / 2, cy = y + s * 0.5, r = s * 0.32;
    ctx.fillStyle = CLAY;
    ctx.beginPath(); ctx.moveTo(cx, cy - r); ctx.lineTo(cx + r * 0.9, cy - r * 0.62);
    ctx.quadraticCurveTo(cx + r * 0.9, cy + r * 0.65, cx, cy + r);
    ctx.quadraticCurveTo(cx - r * 0.9, cy + r * 0.65, cx - r * 0.9, cy - r * 0.62); ctx.closePath(); ctx.fill();
    ctx.fillStyle = '#1E1410';
    ctx.fillRect(cx - r * 0.38, cy - r * 0.2, r * 0.16, r * 0.34);
    ctx.fillRect(cx + r * 0.22, cy - r * 0.2, r * 0.16, r * 0.34);
  },
  downloads(ctx, x, y, s) {
    ctx.fillStyle = '#fff';
    ctx.fillRect(x + s * 0.45, y + s * 0.24, s * 0.1, s * 0.3);
    ctx.beginPath(); ctx.moveTo(x + s * 0.3, y + s * 0.5); ctx.lineTo(x + s * 0.7, y + s * 0.5); ctx.lineTo(x + s * 0.5, y + s * 0.7); ctx.fill();
    ctx.fillRect(x + s * 0.28, y + s * 0.74, s * 0.44, s * 0.05);
  },
};

const ICONS = {
  files: ['#7CC0FF', '#2E7BEA'], browser: ['#FFFFFF', '#E9EDF2'], mail: ['#6BB8FF', '#2C77E6'],
  calendar: ['#FFFFFF', '#F1F1F1'], notes: ['#FFFFFF', '#F5F2EA'], music: ['#FF7A8A', '#F0435C'],
  terminal: ['#D9DCE1', '#AEB3BA'], code: ['#9C8CFF', '#6448E8'], settings: ['#E9EBEE', '#BFC3C9'],
  qn: ['#FFF6EC', '#F6E3CF'], downloads: ['#8CCBFF', '#3E8BEB'],
};

export function appIcon(ctx, name, x, y, s, o = {}) {
  ctx.save();
  ctx.shadowColor = 'rgba(30,20,10,0.28)'; ctx.shadowBlur = s * 0.14; ctx.shadowOffsetY = s * 0.05;
  const [a, b] = ICONS[name];
  ctx.fillStyle = grad(ctx, y, s, a, b); squircle(ctx, x, y, s); ctx.fill();
  ctx.restore();
  ctx.save();
  // top sheen + hairline
  ctx.save(); squircle(ctx, x, y, s); ctx.clip();
  const sh = ctx.createLinearGradient(0, y, 0, y + s * 0.5);
  sh.addColorStop(0, 'rgba(255,255,255,0.35)'); sh.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = sh; ctx.fillRect(x, y, s, s * 0.5);
  ctx.restore();
  G[name](ctx, x, y, s);
  ctx.strokeStyle = 'rgba(0,0,0,0.08)'; ctx.lineWidth = 1; squircle(ctx, x + 0.5, y + 0.5, s - 1); ctx.stroke();
  ctx.restore();
}

export function trashIcon(ctx, x, y, s) {
  ctx.save();
  ctx.shadowColor = 'rgba(30,20,10,0.22)'; ctx.shadowBlur = s * 0.12; ctx.shadowOffsetY = s * 0.04;
  ctx.fillStyle = 'rgba(235,240,245,0.85)';
  ctx.beginPath(); ctx.moveTo(x + s * 0.2, y + s * 0.18); ctx.lineTo(x + s * 0.8, y + s * 0.18); ctx.lineTo(x + s * 0.73, y + s * 0.95); ctx.lineTo(x + s * 0.27, y + s * 0.95); ctx.closePath(); ctx.fill();
  ctx.restore();
  ctx.strokeStyle = 'rgba(120,130,140,0.6)'; ctx.lineWidth = s * 0.02;
  for (let i = 0; i < 5; i++) { const xx = x + s * (0.3 + i * 0.1); ctx.beginPath(); ctx.moveTo(xx, y + s * 0.26); ctx.lineTo(xx + (0.5 - (0.3 + i * 0.1)) * s * -0.1, y + s * 0.88); ctx.stroke(); }
  ctx.fillStyle = 'rgba(200,206,214,0.95)'; rr(ctx, x + s * 0.16, y + s * 0.12, s * 0.68, s * 0.08, s * 0.04); ctx.fill();
}

// Dock: items = names or '|' (separator) or 'trash'. running = set of names with an indicator dot.
// hover = index magnified (Mac dock magnification), press = 0..1 squash on that icon.
export function macDock(ctx, items, o = {}) {
  const s = o.size || 66, gap = 10, pad = 12;
  const scales = items.map((_, i) => {
    if (o.hover == null) return 1;
    const d = Math.abs(i - o.hover);
    return 1 + 0.45 * Math.max(0, 1 - d / 2.2) ** 2;
  });
  const widths = items.map((it, i) => (it === '|' ? 14 : s * scales[i]));
  const w = widths.reduce((a, b) => a + b, 0) + gap * (items.length - 1) + pad * 2;
  const x0 = (W - w) / 2, base = H - 16;
  const shelfH = s + pad * 2;
  ctx.save();
  ctx.shadowColor = 'rgba(40,20,10,0.18)'; ctx.shadowBlur = 40; ctx.shadowOffsetY = 10;
  ctx.fillStyle = o.bg || 'rgba(255,250,245,0.5)'; rr(ctx, x0, base - shelfH, w, shelfH, 26); ctx.fill();
  ctx.restore();
  ctx.strokeStyle = 'rgba(255,255,255,0.7)'; ctx.lineWidth = 1.2; rr(ctx, x0 + 0.5, base - shelfH + 0.5, w - 1, shelfH - 1, 26); ctx.stroke();
  let x = x0 + pad;
  const rects = {};
  items.forEach((it, i) => {
    const sz = widths[i];
    if (it === '|') {
      ctx.fillStyle = 'rgba(80,60,50,0.25)'; ctx.fillRect(x + 6, base - shelfH + 16, 1.5, shelfH - 32);
    } else {
      let ss = sz, yy = base - pad - ss;
      if (o.press != null && o.pressIdx === i) { const k = 1 - 0.12 * o.press; ss = sz * k; yy = base - pad - ss; }
      const xx = x + (sz - ss) / 2;
      if (it === 'trash') trashIcon(ctx, xx, yy, ss); else appIcon(ctx, it, xx, yy, ss);
      if (o.running && o.running.includes(it)) { ctx.fillStyle = 'rgba(60,40,30,0.7)'; ctx.beginPath(); ctx.arc(x + sz / 2, base - 5, 2.6, 0, 7); ctx.fill(); }
      rects[it] = { x: xx, y: yy, s: ss };
    }
    x += sz + gap;
  });
  return rects;
}

// status glyphs for the menu bar
function wifi(ctx, x, y, c) {
  ctx.strokeStyle = c; ctx.lineWidth = 2.2; ctx.lineCap = 'round';
  for (const r of [12, 8]) { ctx.beginPath(); ctx.arc(x, y + 6, r, -Math.PI * 0.75, -Math.PI * 0.25); ctx.stroke(); }
  ctx.fillStyle = c; ctx.beginPath(); ctx.arc(x, y + 5, 2.3, 0, 7); ctx.fill();
}
function battery(ctx, x, y, c, level = 0.8) {
  ctx.strokeStyle = c; ctx.globalAlpha = 0.5; ctx.lineWidth = 1.4; rr(ctx, x, y - 6.5, 25, 13, 3.5); ctx.stroke(); ctx.globalAlpha = 1;
  ctx.fillStyle = c; rr(ctx, x + 2, y - 4.5, 21 * level, 9, 2); ctx.fill();
  ctx.globalAlpha = 0.5; rr(ctx, x + 26.5, y - 2.5, 2, 5, 1); ctx.fill(); ctx.globalAlpha = 1;
}
function search(ctx, x, y, c) {
  ctx.strokeStyle = c; ctx.lineWidth = 2; ctx.lineCap = 'round';
  ctx.beginPath(); ctx.arc(x - 2, y - 2, 6, 0, 7); ctx.stroke();
  ctx.beginPath(); ctx.moveTo(x + 2.5, y + 2.5); ctx.lineTo(x + 7, y + 7); ctx.stroke();
}
function toggles(ctx, x, y, c) {
  ctx.strokeStyle = c; ctx.lineWidth = 1.6;
  rr(ctx, x - 9, y - 7, 18, 6, 3); ctx.stroke(); rr(ctx, x - 9, y + 1, 18, 6, 3); ctx.stroke();
  ctx.fillStyle = c; ctx.beginPath(); ctx.arc(x - 6, y - 4, 2.6, 0, 7); ctx.fill(); ctx.beginPath(); ctx.arc(x + 6, y + 4, 2.6, 0, 7); ctx.fill();
}
export function shieldGlyph(ctx, x, y, s, col, tick = '#fff') {
  ctx.save();
  ctx.fillStyle = col;
  ctx.beginPath(); ctx.moveTo(x, y - s); ctx.lineTo(x + s * 0.85, y - s * 0.6);
  ctx.quadraticCurveTo(x + s * 0.85, y + s * 0.6, x, y + s);
  ctx.quadraticCurveTo(x - s * 0.85, y + s * 0.6, x - s * 0.85, y - s * 0.6); ctx.closePath(); ctx.fill();
  ctx.strokeStyle = tick; ctx.lineWidth = s * 0.24; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  ctx.beginPath(); ctx.moveTo(x - s * 0.35, y); ctx.lineTo(x - s * 0.05, y + s * 0.3); ctx.lineTo(x + s * 0.4, y - s * 0.3); ctx.stroke();
  ctx.restore();
}

export function macMenuBar(ctx, o = {}) {
  const h = 34, ink = o.ink || '#231c18';
  ctx.save();
  ctx.fillStyle = o.bg || 'rgba(255,248,242,0.62)'; ctx.fillRect(0, 0, W, h);
  ctx.fillStyle = 'rgba(0,0,0,0.06)'; ctx.fillRect(0, h - 1, W, 1);
  ctx.fillStyle = ink; ctx.textBaseline = 'middle';
  ctx.font = '700 15.5px "Noto Sans SC", Inter';
  ctx.fillText(o.app || 'qn sandbox', 22, h / 2 + 1);
  ctx.font = '400 15.5px "Noto Sans SC", Inter';
  let x = 22 + ctx.measureText(o.app || 'qn sandbox').width + 32;
  ctx.font = '400 15.5px "Noto Sans SC", Inter';
  for (const s of o.items || ['文件', '编辑', '显示', '会话', '窗口', '帮助']) { ctx.fillText(s, x, h / 2 + 1); x += ctx.measureText(s).width + 24; }
  // right side, right-to-left
  let rx = W - 20;
  ctx.textAlign = 'right'; ctx.fillText(o.clock || '10月25日 周六 10:24', rx, h / 2 + 1); rx -= ctx.measureText(o.clock || '10月25日 周六 10:24').width + 26;
  ctx.textAlign = 'left';
  toggles(ctx, rx, h / 2, ink); rx -= 34;
  search(ctx, rx, h / 2, ink); rx -= 36;
  wifi(ctx, rx, h / 2 - 2, ink); rx -= 50;
  battery(ctx, rx, h / 2, ink); rx -= 30;
  // qn status item (our own)
  if (o.qnActive) { ctx.fillStyle = 'rgba(62,155,111,0.16)'; rr(ctx, rx - 16, 5, 32, h - 10, 6); ctx.fill(); }
  shieldGlyph(ctx, rx, h / 2, 9, o.qnActive ? '#2f8a5f' : ink);
  ctx.restore();
}

export function trafficLights(ctx, x, y, r = 7, o = {}) {
  const cols = o.inactive ? ['#d6d0cb', '#d6d0cb', '#d6d0cb'] : ['#FF5F57', '#FEBC2E', '#28C840'];
  const edges = ['#E0443E', '#DEA123', '#1AAB29'];
  cols.forEach((c, i) => {
    const cx = x + i * (r * 2 + 8);
    ctx.fillStyle = c; ctx.beginPath(); ctx.arc(cx, y, r, 0, 7); ctx.fill();
    if (!o.inactive) { ctx.strokeStyle = edges[i]; ctx.lineWidth = 0.8; ctx.beginPath(); ctx.arc(cx, y, r - 0.4, 0, 7); ctx.stroke(); }
    if (o.hover) {
      ctx.strokeStyle = 'rgba(60,20,10,0.6)'; ctx.lineWidth = 1.4; ctx.lineCap = 'round';
      ctx.beginPath();
      if (i === 0) { ctx.moveTo(cx - 3, y - 3); ctx.lineTo(cx + 3, y + 3); ctx.moveTo(cx + 3, y - 3); ctx.lineTo(cx - 3, y + 3); }
      if (i === 1) { ctx.moveTo(cx - 3.5, y); ctx.lineTo(cx + 3.5, y); }
      if (i === 2) { ctx.moveTo(cx - 3, y + 3); ctx.lineTo(cx + 3, y - 3); }
      ctx.stroke();
    }
  });
}

// A window: frosted title/toolbar, body colour, rounded, soft shadow. Returns body rect.
export function macWindow(ctx, x, y, w, h, o = {}) {
  ctx.save();
  ctx.shadowColor = 'rgba(50,25,15,0.28)'; ctx.shadowBlur = 60; ctx.shadowOffsetY = 24;
  ctx.fillStyle = o.body || '#FFFCF8'; rr(ctx, x, y, w, h, 14); ctx.fill();
  ctx.restore();
  ctx.save(); rr(ctx, x, y, w, h, 14); ctx.clip();
  const th = o.toolbar ? 52 : 38;
  ctx.fillStyle = o.titleBg || '#F3EDE7'; ctx.fillRect(x, y, w, th);
  ctx.fillStyle = 'rgba(0,0,0,0.07)'; ctx.fillRect(x, y + th - 1, w, 1);
  trafficLights(ctx, x + 20, y + th / 2, 6.5, o);
  ctx.fillStyle = '#4a3f38'; ctx.font = '600 14.5px "Noto Sans SC", Inter'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  if (o.title) ctx.fillText(o.title, x + w / 2, y + th / 2 + 1);
  ctx.textAlign = 'left';
  ctx.restore();
  ctx.strokeStyle = 'rgba(0,0,0,0.12)'; ctx.lineWidth = 1; rr(ctx, x + 0.5, y + 0.5, w - 1, h - 1, 14); ctx.stroke();
  return { x, y: y + th, w, h: h - th };
}

// Notification banner, top right, Mac grammar: app icon, title, body, "现在".
export function macNotification(ctx, o) {
  const w = 380, h = 78, x = o.x ?? W - w - 14, y = o.y ?? 46;
  ctx.save();
  ctx.globalAlpha = o.alpha ?? 1;
  ctx.shadowColor = 'rgba(40,20,10,0.2)'; ctx.shadowBlur = 30; ctx.shadowOffsetY = 10;
  ctx.fillStyle = 'rgba(250,246,242,0.9)'; rr(ctx, x, y, w, h, 18); ctx.fill();
  ctx.shadowColor = 'transparent';
  ctx.strokeStyle = 'rgba(255,255,255,0.8)'; ctx.lineWidth = 1; rr(ctx, x + 0.5, y + 0.5, w - 1, h - 1, 18); ctx.stroke();
  appIcon(ctx, o.icon || 'qn', x + 14, y + 15, 48);
  ctx.fillStyle = '#231c18'; ctx.font = '700 15px "Noto Sans SC"'; ctx.textBaseline = 'alphabetic';
  ctx.fillText(o.title, x + 74, y + 33);
  ctx.fillStyle = '#5d524b'; ctx.font = '400 14px "Noto Sans SC"'; ctx.fillText(o.body, x + 74, y + 56);
  ctx.fillStyle = '#9a8e86'; ctx.font = '400 12.5px "Noto Sans SC"'; ctx.textAlign = 'right'; ctx.fillText(o.time || '现在', x + w - 16, y + 31); ctx.textAlign = 'left';
  ctx.restore();
}

// Desktop item (icon + label with selection pill).
export function desktopItem(ctx, kind, label, x, y, o = {}) {
  const s = 74;
  if (kind === 'folder') {
    ctx.save(); ctx.shadowColor = 'rgba(30,20,10,0.2)'; ctx.shadowBlur = 8; ctx.shadowOffsetY = 3;
    ctx.fillStyle = '#5FA2EE'; rr(ctx, x + 4, y + 12, s - 8, s * 0.7, 7); ctx.fill();
    rr(ctx, x + 4, y + 6, s * 0.36, 14, 5); ctx.fill();
    ctx.restore();
    ctx.fillStyle = grad(ctx, y + 22, s * 0.6, '#9BCBFF', '#6FB0F7'); rr(ctx, x + 4, y + 20, s - 8, s * 0.62, 7); ctx.fill();
  } else if (kind === 'drive') {
    ctx.save(); ctx.shadowColor = 'rgba(30,20,10,0.2)'; ctx.shadowBlur = 8; ctx.shadowOffsetY = 3;
    ctx.fillStyle = grad(ctx, y + 16, 44, '#F2F2F4', '#C9CCD2'); rr(ctx, x + 4, y + 18, s - 8, 42, 8); ctx.fill();
    ctx.restore();
    ctx.fillStyle = '#9aa0a8'; ctx.fillRect(x + 12, y + 48, s - 24, 3);
    ctx.fillStyle = '#5BD07A'; ctx.beginPath(); ctx.arc(x + s - 18, y + 30, 3, 0, 7); ctx.fill();
  } else if (kind === 'doc') {
    ctx.save(); ctx.shadowColor = 'rgba(30,20,10,0.2)'; ctx.shadowBlur = 8; ctx.shadowOffsetY = 3;
    ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.moveTo(x + 16, y + 4); ctx.lineTo(x + 46, y + 4); ctx.lineTo(x + 60, y + 18); ctx.lineTo(x + 60, y + 72); ctx.lineTo(x + 16, y + 72); ctx.closePath(); ctx.fill();
    ctx.restore();
    ctx.fillStyle = '#e1dcd6'; ctx.beginPath(); ctx.moveTo(x + 46, y + 4); ctx.lineTo(x + 60, y + 18); ctx.lineTo(x + 46, y + 18); ctx.fill();
    ctx.fillStyle = '#c8bfb6'; for (let i = 0; i < 5; i++) ctx.fillRect(x + 22, y + 28 + i * 8, i === 4 ? 18 : 30, 3);
  }
  ctx.font = '500 13.5px "Noto Sans SC", Inter'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  const tw = ctx.measureText(label).width;
  if (o.selected) { ctx.fillStyle = '#2E7BEA'; rr(ctx, x + s / 2 - tw / 2 - 6, y + s + 4, tw + 12, 20, 5); ctx.fill(); }
  ctx.fillStyle = o.selected ? '#fff' : '#231c18';
  ctx.shadowColor = o.selected ? 'transparent' : 'rgba(255,255,255,0.6)'; ctx.shadowBlur = 4;
  ctx.fillText(label, x + s / 2, y + s + 14);
  ctx.shadowColor = 'transparent'; ctx.textAlign = 'left'; ctx.textBaseline = 'alphabetic';
}

// Mac-style pointer arrow with click ripple.
export function cursor(ctx, x, y, o = {}) {
  if (o.ripple) {
    const k = o.ripple;
    ctx.strokeStyle = `rgba(46,123,234,${0.5 * (1 - k)})`; ctx.lineWidth = 3;
    ctx.beginPath(); ctx.arc(x, y, 8 + 28 * k, 0, 7); ctx.stroke();
  }
  const s = (o.scale || 1) * (o.press ? 0.9 : 1);
  ctx.save(); ctx.translate(x, y); ctx.scale(s, s);
  ctx.shadowColor = 'rgba(0,0,0,0.3)'; ctx.shadowBlur = 4; ctx.shadowOffsetY = 2;
  ctx.beginPath(); ctx.moveTo(0, 0); ctx.lineTo(0, 24); ctx.lineTo(6, 18.5); ctx.lineTo(10, 27); ctx.lineTo(14, 25.2); ctx.lineTo(10, 17); ctx.lineTo(17.5, 17); ctx.closePath();
  ctx.fillStyle = '#fff'; ctx.fill();
  ctx.shadowColor = 'transparent';
  ctx.beginPath(); ctx.moveTo(2, 4.5); ctx.lineTo(2, 19.5); ctx.lineTo(6.5, 15.5); ctx.lineTo(10.5, 24); ctx.lineTo(12, 23.3); ctx.lineTo(8, 15); ctx.lineTo(13.5, 15); ctx.closePath();
  ctx.fillStyle = '#111'; ctx.fill();
  ctx.restore();
}
