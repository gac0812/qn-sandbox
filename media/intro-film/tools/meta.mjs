// Print the laid-out timeline: node tools/meta.mjs film/index.html
import { serve, openPage } from './still.mjs';
import { createRequire } from 'node:module';
const require_ = createRequire(import.meta.url);
// Playwright: $PLAYWRIGHT_MODULE, a local/global install, or the preinstalled copy in cloud containers
const { chromium } = (() => {
  for (const p of [process.env.PLAYWRIGHT_MODULE, 'playwright', '/opt/node-tools/node_modules/playwright/index.js'].filter(Boolean)) {
    try { return require_(p); } catch { /* try the next location */ }
  }
  throw new Error('playwright not found: run `npm i playwright && npx playwright install chromium`');
})();
const srv = await serve();
const br = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const pg = await openPage(br, srv.address().port, process.argv[2]);
  const m = await pg.evaluate(() => ({ DUR: window.DUR, SHOTS: window.SHOTS, TL: window.TL, nEV: window.EV.length }));
  console.log('DUR', m.DUR, 'events', m.nEV);
  for (const s of m.SHOTS) console.log(String(s.id).padEnd(3), s.t0.toFixed(2).padStart(7), s.t1.toFixed(2).padStart(7), (s.t1 - s.t0).toFixed(2).padStart(6), s.framing);
  console.log(JSON.stringify(m.TL));
} finally { await br.close(); srv.close(); }
