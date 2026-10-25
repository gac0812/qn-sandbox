// Render stills: node tools/still.mjs <page.html> <out.png> [t] [more t...]
// Serves the project folder, waits for window.READY, calls render(t), waits two rAFs, screenshots.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
const require_ = createRequire(import.meta.url);
// Playwright: $PLAYWRIGHT_MODULE, a local/global install, or the preinstalled copy in cloud containers
const { chromium } = (() => {
  for (const p of [process.env.PLAYWRIGHT_MODULE, 'playwright', '/opt/node-tools/node_modules/playwright/index.js'].filter(Boolean)) {
    try { return require_(p); } catch { /* try the next location */ }
  }
  throw new Error('playwright not found: run `npm i playwright && npx playwright install chromium`');
})();

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json', '.ttf': 'font/ttf', '.png': 'image/png' };

export function serve() {
  return new Promise((resolve) => {
    const srv = http.createServer((req, res) => {
      const u = new URL(req.url, 'http://x');
      const p = path.join(ROOT, decodeURIComponent(u.pathname));
      if (!p.startsWith(ROOT) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) { res.writeHead(404); return res.end(); }
      res.writeHead(200, { 'Content-Type': MIME[path.extname(p)] || 'application/octet-stream' });
      fs.createReadStream(p).pipe(res);
    });
    srv.listen(0, '127.0.0.1', () => resolve(srv));
  });
}

export async function openPage(browser, port, page, w = +(process.env.VW || 1920), h = +(process.env.VH || 1080)) {
  const pg = await browser.newPage({ viewport: { width: w, height: h } });
  const errors = [];
  pg.on('pageerror', (e) => errors.push(String(e)));
  pg.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await pg.goto(`http://127.0.0.1:${port}/${page}`);
  await pg.waitForFunction(() => window.READY === true || window.FAIL, null, { timeout: 120000 });
  const fail = await pg.evaluate(() => window.FAIL);
  if (fail || errors.length) throw new Error(`page error: ${fail || ''} ${errors.join('\n')}`);
  return pg;
}

export async function frame(pg, t) {
  await pg.evaluate(async (t) => {
    await window.render(t);
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  }, t);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [page, out, ...ts] = process.argv.slice(2);
  const srv = await serve();
  const browser = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  try {
    const pg = await openPage(browser, srv.address().port, page);
    const times = ts.length ? ts.map(Number) : [0];
    for (const t of times) {
      await frame(pg, t);
      const o = times.length > 1 ? out.replace(/\.png$/, `_${t.toFixed(2)}.png`) : out;
      await pg.screenshot({ path: o });
      console.log('wrote', o);
    }
  } finally {
    await browser.close();
    srv.close();
  }
}
