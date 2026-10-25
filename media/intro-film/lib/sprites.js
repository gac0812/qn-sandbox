// Original pixel characters for the other agents. Grids are built procedurally so the
// shapes stay exact; each cell is a palette key or null.
import { CLAWD, CLAY, CLAY_DARK, CLAY_LIGHT } from './lib.js';

function grid(w, h) { return Array.from({ length: h }, () => Array(w).fill(null)); }
function fill(g, x0, y0, x1, y1, k) { for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) if (g[y] && x >= 0 && x < g[0].length) g[y][x] = k; }

// Codex: a little slate monitor-bot with a mint prompt face. 16×15.
export const CODEX_PAL = { s: '#64788C', S: '#4E6072', d: '#1F2A33', m: '#5FE3B5', b: '#53667A', h: '#8DA2B6' };
export const CODEX = (() => {
  const g = grid(16, 15);
  fill(g, 7, 0, 8, 0, 'm'); fill(g, 7, 1, 8, 1, 's');
  fill(g, 1, 2, 14, 10, 's'); g[2][1] = g[2][14] = g[10][1] = g[10][14] = null;
  fill(g, 2, 2, 13, 2, 'h');
  fill(g, 3, 4, 12, 8, 'd');
  g[5][5] = g[6][6] = g[7][5] = 'm';               // '>' eye
  fill(g, 8, 7, 10, 7, 'm');                        // '_' mouth
  fill(g, 4, 11, 11, 12, 'b');
  fill(g, 2, 11, 3, 11, 'b'); fill(g, 12, 11, 13, 11, 'b');
  fill(g, 4, 13, 6, 14, 'S'); fill(g, 9, 13, 11, 14, 'S');
  return g;
})();

// dsh: a round blue pixel whale with a spout (DeepSeek concept, original drawing). 22×13.
export const DSH_PAL = { w: '#3C7BE8', W: '#2D63C4', l: '#D6E7FF', e: '#14284F', c: '#F7A6B8', p: '#9FE0FF' };
export const DSH = (() => {
  const g = grid(22, 13);
  for (let y = 3; y < 13; y++) for (let x = 3; x < 22; x++) {
    const dx = (x + 0.5 - 12.5) / 9.4, dy = (y + 0.5 - 8) / 4.6;
    if (dx * dx + dy * dy <= 1) g[y][x] = y >= 9 && dx * dx + ((y + 0.5 - 8.6) / 4) ** 2 < 0.62 && x > 6 ? 'l' : 'w';
  }
  // tail fin on the left
  fill(g, 0, 4, 1, 5, 'W'); fill(g, 1, 6, 3, 7, 'W'); fill(g, 0, 8, 1, 9, 'W');
  g[6][16] = 'e'; g[7][16] = 'e';                  // eye
  g[8][18] = 'c';                                   // cheek
  // spout
  g[0][13] = g[0][16] = 'p'; fill(g, 14, 1, 15, 1, 'p'); g[2][14] = 'p'; g[2][15] = 'p';
  return g;
})();

// Clawd as a palette grid (eyes drawn separately by drawClawd; here for voxel builds)
export const CLAWD_PAL = { '#': CLAY, e: '#1E1410', t: CLAY_LIGHT, k: CLAY_DARK };
export const CLAWD_GRID = CLAWD.map((row) => [...row].map((ch) => (ch === '.' ? null : ch)));

// Draw a palette grid anchored at its feet, with squash/stretch and flip.
export function drawSprite(ctx, g, pal, cx, footY, px, o = {}) {
  const h = g.length, w = g[0].length;
  const sq = o.squash || 1, sx = px / Math.sqrt(sq), sy = px * sq;
  ctx.save();
  ctx.translate(Math.round(cx), Math.round(footY));
  if (o.flip) ctx.scale(-1, 1);
  if (o.rot) ctx.rotate(o.rot);
  if (o.shadow !== false) {
    ctx.fillStyle = o.shadowColor || 'rgba(60,30,20,0.18)';
    ctx.beginPath(); ctx.ellipse(0, 2, w * 0.5 * sx, 1.6 * px, 0, 0, Math.PI * 2); ctx.fill();
  }
  const ox = -w / 2 * sx, oy = -h * sy;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const k = g[y][x]; if (!k) continue;
    ctx.fillStyle = pal[k]; ctx.fillRect(ox + x * sx, oy + y * sy, sx + 0.5, sy + 0.5);
  }
  ctx.restore();
}
