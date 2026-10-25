// 2D desktops and app windows (A-grade UI) for the film.
import { W, H, rr, CLAY, clamp } from '/lib/lib.js';
import { macMenuBar, macWindow, macDock, desktopItem, appIcon, shieldGlyph, trafficLights } from '/lib/macui.js';

export function wallpaper(b, x0 = 0, w = W, o = {}) {
  b.save(); b.beginPath(); b.rect(x0, 0, w, H); b.clip();
  const g = b.createLinearGradient(0, 0, 0, H);
  g.addColorStop(0, o.top || '#FFD7B8'); g.addColorStop(1, o.bottom || '#F7A98E');
  b.fillStyle = g; b.fillRect(x0, 0, w, H);
  const sx = x0 + w * (o.sunX ?? 0.79);
  b.fillStyle = o.sun || '#FFE9C9'; b.beginPath(); b.arc(sx, 300, 210 * (w / W) ** 0.5, 0, 7); b.fill();
  b.fillStyle = o.hill1 || '#F39A7E'; b.beginPath(); b.moveTo(x0, 760); b.bezierCurveTo(x0 + w * 0.26, 600, x0 + w * 0.47, 820, x0 + w, 640); b.lineTo(x0 + w, H); b.lineTo(x0, H); b.fill();
  b.fillStyle = o.hill2 || '#E98670'; b.beginPath(); b.moveTo(x0, 900); b.bezierCurveTo(x0 + w * 0.36, 760, x0 + w * 0.62, 940, x0 + w, 820); b.lineTo(x0 + w, H); b.lineTo(x0, H); b.fill();
  b.fillStyle = o.dots || 'rgba(160,60,40,0.08)';
  for (let y = 0; y < H; y += 18) for (let x = x0 + ((y / 18) % 2 ? 9 : 0); x < x0 + w; x += 18) {
    const r = 1 + 3.2 * Math.max(0, (y - 500) / 580);
    b.beginPath(); b.arc(x, y, r, 0, 7); b.fill();
  }
  b.restore();
}

export const DOCK = ['files', 'browser', 'mail', 'calendar', 'notes', 'music', '|', 'terminal', 'code', 'qn', 'settings', '|', 'downloads', 'trash'];

export function macDesktop(b, o = {}) {
  wallpaper(b);
  if (o.items !== false) {
    desktopItem(b, 'drive', '工作盘', 1820, 150);
    desktopItem(b, 'folder', 'my-app', 1820, 270, { selected: o.selected });
    desktopItem(b, 'doc', 'notes.md', 1820, 390);
  }
  if (o.menu !== false) macMenuBar(b, { qnActive: o.qnActive });
  if (o.dock !== false) return macDock(b, DOCK, { running: o.running || ['files', 'browser', 'terminal', 'qn'], size: 62, hover: o.dockHover, press: o.dockPress, pressIdx: o.dockPressIdx });
  return null;
}

// Terminal window with a typed command. typed: 0..1 of the command.
export const CMD = 'qn run --profile qn/claude-code -- claude';
export function terminal(b, x, y, w, h, o = {}) {
  const body = macWindow(b, x, y, w, h, { title: o.title || 'claude — ~/my-app', hover: o.hover });
  const n = Math.floor(CMD.length * clamp(o.typed ?? 1));
  b.font = `500 ${o.fs || 22}px "JetBrains Mono"`; b.textBaseline = 'alphabetic';
  const lh = (o.fs || 22) * 1.6, tx = body.x + 28;
  let ty = body.y + 50;
  b.fillStyle = '#9b8778'; b.fillText('~/my-app', tx, ty);
  const px = tx + b.measureText('~/my-app ').width;
  b.fillStyle = CLAY; b.fillText('❯', px, ty);
  b.fillStyle = '#2a1f1a'; b.fillText(CMD.slice(0, n), px + b.measureText('❯ ').width, ty);
  const caretX = px + b.measureText('❯ ' + CMD.slice(0, n)).width;
  if (o.done) {
    ty += lh; b.fillStyle = '#3E9B6F'; b.fillText('✓', tx, ty);
    b.fillStyle = '#6d5d52'; b.font = `400 ${(o.fs || 22) * 0.9}px "Noto Sans SC"`; b.fillText('沙箱已就绪 · 明文凭证 0 · 句柄 1', tx + 34, ty);
  } else if ((o.blink ?? 1) > 0.5) {
    b.fillStyle = CLAY; b.fillRect(caretX + 2, ty - (o.fs || 22) * 0.85, (o.fs || 22) * 0.55, (o.fs || 22) * 1.05);
  }
  return body;
}

// MCP configuration card (inside the sandbox): only a handle, no secret field.
export function mcpCard(f, x, y, o = {}) {
  const w = 760, h = 420;
  const body = macWindow(f, x, y, w, h, { title: 'MCP 服务 · 沙箱内配置', body: '#FFFCF8' });
  const px = body.x + 36; let py = body.y + 54;
  f.fillStyle = '#7b6a5f'; f.font = '600 15px "Noto Sans SC"'; f.fillText('服务', px, py);
  f.fillStyle = '#2a1f1a'; f.font = '700 26px "Noto Sans SC"'; f.fillText('github', px, py + 38);
  py += 92;
  const row = (label, value, mono, accent, k) => {
    f.globalAlpha = clamp(k ?? 1);
    f.fillStyle = '#7b6a5f'; f.font = '600 15px "Noto Sans SC"'; f.fillText(label, px, py);
    f.fillStyle = accent ? '#FFF3CF' : '#F5EEE7'; rr(f, px, py + 12, w - 72, 52, 12); f.fill();
    if (accent) { f.strokeStyle = '#E6CF8F'; f.lineWidth = 2; rr(f, px, py + 12, w - 72, 52, 12); f.stroke(); }
    f.fillStyle = '#2a1f1a'; f.font = mono ? '600 21px "JetBrains Mono"' : '500 20px "Noto Sans SC"'; f.fillText(value, px + 18, py + 46);
    f.globalAlpha = 1;
    py += 92;
  };
  row('凭证', 'qn://handle/github', true, true, o.k1);
  row('密钥', '无需填写 · 由宿主侧网关代答', false, false, o.k2);
  // gateway badge
  f.globalAlpha = clamp(o.k3 ?? 1);
  f.fillStyle = '#E3F2E9'; rr(f, px, py - 20, 330, 40, 20); f.fill();
  shieldGlyph(f, px + 22, py, 9, '#3E9B6F');
  f.fillStyle = '#2f7a55'; f.font = '700 16px "Noto Sans SC"'; f.fillText('沙箱内零明文凭证', px + 42, py + 6);
  f.globalAlpha = 1;
  return { x, y, w, h };
}

// qn control panel: sidebar with four views (会话/配置档/权限/审计), content for the active one.
export const TABS = ['会话', '配置档', '权限', '审计'];
export function controlPanel(f, x, y, o = {}) {
  const w = 1120, h = 640;
  const body = macWindow(f, x, y, w, h, { title: 'qn sandbox', body: '#FFFCF8' });
  // sidebar
  f.fillStyle = '#F4EDE6'; f.fillRect(body.x, body.y, 230, body.h);
  appIcon(f, 'qn', body.x + 24, body.y + 22, 44);
  f.fillStyle = '#2a1f1a'; f.font = '700 18px "Noto Sans SC"'; f.fillText('qn sandbox', body.x + 80, body.y + 50);
  TABS.forEach((t, i) => {
    const ty = body.y + 100 + i * 56;
    const active = o.tab === i, hover = o.hoverTab === i;
    if (active) { f.fillStyle = CLAY; rr(f, body.x + 14, ty - 22, 202, 44, 10); f.fill(); }
    else if (hover) { f.fillStyle = 'rgba(217,119,87,0.14)'; rr(f, body.x + 14, ty - 22, 202, 44, 10); f.fill(); }
    f.fillStyle = active ? '#fff' : '#3a2e27'; f.font = `${active ? 700 : 500} 18px "Noto Sans SC"`; f.fillText(t, body.x + 44, ty + 7);
  });
  const cx = body.x + 262, cy = body.y + 40;
  const tab = o.tab ?? 0;
  f.fillStyle = '#2a1f1a'; f.font = '700 28px "Noto Sans SC"'; f.fillText(TABS[tab], cx, cy + 20);
  f.font = '500 17px "Noto Sans SC"'; f.fillStyle = '#7b6a5f';
  if (tab === 0) {
    f.fillText('正在运行的会话', cx, cy + 56);
    sessionRow(f, cx, cy + 90, 'claude', 'qn/claude-code', '运行中 · 12 分钟', o);
    sessionRow(f, cx, cy + 190, 'codex', 'qn/codex', '运行中 · 3 分钟', { ...o, revoke: null });
  } else if (tab === 1) {
    f.fillText('官方配置档', cx, cy + 56);
    ['qn/claude-code', 'qn/codex'].forEach((p, i) => { pill(f, cx, cy + 90 + i * 70, p, i === 0); });
  } else if (tab === 2) {
    f.fillText('qn/claude-code · 文件与网络默认拒绝', cx, cy + 56);
    const perms = [['~/my-app', '读写', true], ['~/.ssh', '拒绝', false], ['api.github.com', '经代理放行', true], ['其他域名', '拒绝', false]];
    perms.forEach(([p, s, ok], i) => permRow(f, cx, cy + 90 + i * 66, p, s, ok));
  } else {
    f.fillText('特权动作与拦截记录', cx, cy + 56);
    const rows = [['10:24:31', '已拦截', '读取 ~/.ssh', '#E8584A'], ['10:24:33', '已拦截', '外连 x.example', '#E8584A'], ['10:23:02', '已代用', 'github · qn://handle/github', '#3E9B6F']];
    rows.forEach(([t, s, d, c], i) => auditRow(f, cx, cy + 90 + i * 66, t, s, d, c));
  }
  return { x, y, w, h, body };
}

function sessionRow(f, x, y, name, profile, status, o) {
  f.fillStyle = '#F6EFE8'; rr(f, x, y, 800, 84, 16); f.fill();
  appIcon(f, name === 'claude' ? 'terminal' : 'code', x + 18, y + 16, 52);
  f.fillStyle = '#2a1f1a'; f.font = '700 20px "Noto Sans SC"'; f.fillText(name, x + 88, y + 36);
  f.fillStyle = '#7b6a5f'; f.font = '500 15px "JetBrains Mono"'; f.fillText(profile + ' · ' + status, x + 88, y + 62);
  if (o.revoke !== null) revokeButton(f, x + 800 - 168, y + 20, o.revoke || {});
}

// The "revoke session" button and its interaction states.
export function revokeButton(f, x, y, s = {}) {
  const press = clamp(s.press || 0), hover = s.hover ? 1 : 0, done = s.done;
  const w = 148, h = 44;
  f.save();
  f.translate(x + w / 2, y + h / 2 + press * 2); f.scale(1 - press * 0.05, 1 - press * 0.05);
  if (!press) { f.shadowColor = 'rgba(200,60,40,0.35)'; f.shadowBlur = hover ? 18 : 8; f.shadowOffsetY = hover ? 6 : 3; }
  f.fillStyle = done ? '#9C8F86' : (hover ? '#F06352' : '#E8584A'); rr(f, -w / 2, -h / 2, w, h, 12); f.fill();
  f.shadowColor = 'transparent';
  f.fillStyle = '#fff'; f.font = '700 18px "Noto Sans SC"'; f.textAlign = 'center'; f.textBaseline = 'middle';
  f.fillText(done ? '已撤销' : '撤销会话', 0, 1);
  f.restore();
  if (s.ripple) {
    const k = s.ripple; f.strokeStyle = `rgba(232,88,74,${0.6 * (1 - k)})`; f.lineWidth = 4;
    f.beginPath(); f.roundRect(x - 30 * k, y - 30 * k, w + 60 * k, h + 60 * k, 12 + 20 * k); f.stroke();
  }
  return { x, y, w, h };
}

function pill(f, x, y, t, active) {
  f.fillStyle = active ? '#FFF1E8' : '#F6EFE8'; rr(f, x, y, 560, 54, 14); f.fill();
  if (active) { f.strokeStyle = CLAY; f.lineWidth = 2; rr(f, x, y, 560, 54, 14); f.stroke(); }
  f.fillStyle = '#2a1f1a'; f.font = '600 20px "JetBrains Mono"'; f.fillText(t, x + 20, y + 34);
}
function permRow(f, x, y, p, s, ok) {
  f.fillStyle = '#F6EFE8'; rr(f, x, y, 800, 54, 14); f.fill();
  f.fillStyle = '#2a1f1a'; f.font = '600 19px "JetBrains Mono"'; f.fillText(p, x + 20, y + 34);
  f.fillStyle = ok ? '#E3F2E9' : '#FBE3DF'; rr(f, x + 600, y + 11, 180, 32, 16); f.fill();
  f.fillStyle = ok ? '#2f7a55' : '#C2412F'; f.font = '700 15px "Noto Sans SC"'; f.textAlign = 'center'; f.fillText(s, x + 690, y + 33); f.textAlign = 'left';
}
function auditRow(f, x, y, t, s, d, c) {
  f.fillStyle = '#F6EFE8'; rr(f, x, y, 800, 54, 14); f.fill();
  f.fillStyle = '#9b8778'; f.font = '500 16px "JetBrains Mono"'; f.fillText(t, x + 20, y + 33);
  f.fillStyle = c; rr(f, x + 120, y + 11, 82, 32, 16); f.fill();
  f.fillStyle = '#fff'; f.font = '700 15px "Noto Sans SC"'; f.textAlign = 'center'; f.fillText(s, x + 161, y + 33); f.textAlign = 'left';
  f.fillStyle = '#2a1f1a'; f.font = '500 18px "JetBrains Mono"'; f.fillText(d, x + 222, y + 34);
}

// Windows-style desktop (interface grammar only, no logo): taskbar with centred icons,
// window controls on the right. Drawn into [x0, x0 + w].
export function winDesktop(b, x0, w) {
  wallpaper(b, x0, w, { top: '#CFE3F7', bottom: '#9CC2EA', sun: '#EAF3FF', hill1: '#86AEE0', hill2: '#7199D2', dots: 'rgba(40,70,140,0.07)', sunX: 0.75 });
  b.save(); b.beginPath(); b.rect(x0, 0, w, H); b.clip();
  // taskbar
  b.fillStyle = 'rgba(240,245,252,0.82)'; b.fillRect(x0, H - 58, w, 58);
  b.fillStyle = 'rgba(0,0,0,0.06)'; b.fillRect(x0, H - 58, w, 1);
  const icons = ['files', 'browser', 'terminal', 'qn', 'settings'];
  const iw = 40, gap = 14, total = icons.length * iw + (icons.length - 1) * gap + 150;
  let ix = x0 + (w - total) / 2;
  // search pill
  b.fillStyle = 'rgba(255,255,255,0.9)'; rr(b, ix, H - 47, 136, 36, 18); b.fill();
  b.strokeStyle = '#5b6470'; b.lineWidth = 2; b.beginPath(); b.arc(ix + 22, H - 30, 6, 0, 7); b.stroke();
  b.beginPath(); b.moveTo(ix + 26.5, H - 25.5); b.lineTo(ix + 31, H - 21); b.stroke();
  b.fillStyle = '#5b6470'; b.font = '400 15px "Noto Sans SC"'; b.fillText('搜索', ix + 42, H - 24);
  ix += 150;
  icons.forEach((n) => { appIcon(b, n, ix, H - 49, iw); if (n === 'terminal' || n === 'qn') { b.fillStyle = '#3E7BD8'; rr(b, ix + 12, H - 6, 16, 3, 1.5); b.fill(); } ix += iw + gap; });
  b.fillStyle = '#2a3340'; b.font = '400 14px "Noto Sans SC"'; b.textAlign = 'right';
  b.fillText('10:24', x0 + w - 18, H - 34); b.fillText('2026/10/25', x0 + w - 18, H - 14); b.textAlign = 'left';
  b.restore();
}

// Windows-style window frame: controls on the right (— ▢ ✕).
export function winWindow(b, x, y, w, h, title) {
  b.save();
  b.shadowColor = 'rgba(20,40,80,0.28)'; b.shadowBlur = 50; b.shadowOffsetY = 18;
  b.fillStyle = '#FBFCFE'; rr(b, x, y, w, h, 8); b.fill();
  b.restore();
  b.fillStyle = '#EEF2F8'; rr(b, x, y, w, 40, 8); b.fill(); b.fillRect(x, y + 20, w, 20);
  b.fillStyle = '#1f2a36'; b.font = '500 14px "Noto Sans SC"'; b.textBaseline = 'middle'; b.fillText(title, x + 16, y + 21);
  b.strokeStyle = '#2a3340'; b.lineWidth = 1.4;
  const cx = x + w - 140;
  b.beginPath(); b.moveTo(cx + 18, y + 21); b.lineTo(cx + 30, y + 21); b.stroke();
  b.strokeRect(cx + 64, y + 15, 11, 11);
  b.beginPath(); b.moveTo(cx + 108, y + 15); b.lineTo(cx + 119, y + 26); b.moveTo(cx + 119, y + 15); b.lineTo(cx + 108, y + 26); b.stroke();
  b.textBaseline = 'alphabetic';
  b.strokeStyle = 'rgba(0,0,0,0.1)'; b.lineWidth = 1; rr(b, x + 0.5, y + 0.5, w - 1, h - 1, 8); b.stroke();
}
