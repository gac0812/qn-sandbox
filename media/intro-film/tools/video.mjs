// Render a time range to video: node tools/video.mjs <page> <out.mp4> [--from 0] [--to DUR] [--fps 30] [--workers 3]
// Each worker owns one browser page; frames are rendered independently (render(t) is pure) and
// written as JPEGs, then encoded with ffmpeg. Also writes events.json (DUR + EV) beside out.mp4.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
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

const args = process.argv.slice(2);
const opt = (k, d) => { const i = args.indexOf('--' + k); return i >= 0 ? args[i + 1] : d; };
const [page, out] = args;
const fps = +opt('fps', 30), workers = +opt('workers', 3);
const dir = path.join(path.dirname(out), 'frames_' + path.basename(out, '.mp4'));
fs.rmSync(dir, { recursive: true, force: true }); fs.mkdirSync(dir, { recursive: true });

const srv = await serve();
const port = srv.address().port;
const launch = () => chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const b0 = await launch();
const p0 = await openPage(b0, port, page);
const meta = await p0.evaluate(() => ({ DUR: window.DUR, EV: window.EV || [], TL: window.TL, SUBS: window.SUBS || [], VO: window.VO || [] }));
await b0.close();
fs.writeFileSync(path.join(path.dirname(out), 'events.json'), JSON.stringify(meta, null, 1));
const from = +opt('from', 0), to = +opt('to', meta.DUR);
const n = Math.round((to - from) * fps);
console.log(`rendering ${n} frames ${from}→${to}s @${fps} with ${workers} workers`);
const t0 = Date.now();
let done = 0;
await Promise.all(Array.from({ length: workers }, async (_, w) => {
  const br = await launch();
  try {
    const pg = await openPage(br, port, page);
    for (let i = w; i < n; i += workers) {
      // sample at the frame centre so motion is not biased early
      await frame(pg, from + (i + 0.5) / fps);
      await pg.screenshot({ path: path.join(dir, `f${String(i).padStart(5, '0')}.jpg`), type: 'jpeg', quality: 94 });
      if (++done % 30 === 0) console.log(`${done}/${n} (${((Date.now() - t0) / done).toFixed(0)} ms/frame)`);
    }
  } finally { await br.close(); }
}));
srv.close();
const r = spawnSync('ffmpeg', ['-loglevel', 'error', '-y', '-framerate', String(fps), '-i', path.join(dir, 'f%05d.jpg'),
  '-c:v', 'libx264', '-preset', 'medium', '-crf', '16', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', out], { stdio: 'inherit' });
if (r.status !== 0) process.exit(1);
console.log('wrote', out, `${((Date.now() - t0) / 1000).toFixed(0)}s`);
