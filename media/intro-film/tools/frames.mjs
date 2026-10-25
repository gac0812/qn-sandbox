// Render the key frame of every shot (or given times): node tools/frames.mjs film/index.html out/sb [t...]
import fs from 'node:fs';
import { serve, openPage, frame } from './still.mjs';
import { createRequire } from 'node:module';
const require_ = createRequire(import.meta.url);
// Playwright: $PLAYWRIGHT_MODULE, a local/global install, or the preinstalled copy in cloud containers
const { chromium } = (() => {
  for (const p of [process.env.PLAYWRIGHT_MODULE, 'playwright', '/opt/node-tools/node_modules/playwright/index.js'].filter(Boolean)) {
    try { return require_(p); } catch { /* try the next location */ }
  }
  throw new Error('playwright not found: run `npm i playwright && npx playwright install chromium`');
})();
const [page, dir, ...ts] = process.argv.slice(2);
fs.mkdirSync(dir, { recursive: true });
const srv = await serve();
const browser = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const pg = await openPage(browser, srv.address().port, page);
  const shots = await pg.evaluate(() => window.SHOTS);
  fs.writeFileSync(`${dir}/shots.json`, JSON.stringify(shots, null, 1));
  const list = ts.length ? ts.map((t, i) => ({ id: `t${i}`, key: +t })) : shots;
  for (const s of list) {
    await frame(pg, s.key);
    const p = `${dir}/${String(s.id).padStart(2, '0')}.png`;
    await pg.screenshot({ path: p }); console.log('wrote', p, s.key);
  }
} finally { await browser.close(); srv.close(); }
