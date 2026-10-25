// Re-render random frames in a fresh page and compare with the frames used in the video (PSNR).
// node tools/psnr.mjs film/index.html out/final/frames_video 30 8
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
const [page, dir, fps, count] = process.argv.slice(2);
const files = fs.readdirSync(dir).filter((f) => f.endsWith('.jpg')).sort();
let seed = 12345; const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const picks = Array.from({ length: +count }, () => Math.floor(rnd() * files.length));
const srv = await serve();
const br = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const pg = await openPage(br, srv.address().port, page);
fs.mkdirSync(dir + '_psnr', { recursive: true });
for (const i of picks) {
  await frame(pg, (i + 0.5) / +fps);
  await pg.screenshot({ path: `${dir}_psnr/f${String(i).padStart(5, '0')}.png` });
  console.log(i);
}
await br.close(); srv.close();
