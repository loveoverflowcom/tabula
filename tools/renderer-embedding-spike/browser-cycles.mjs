// Fifty real rendered mounts per path, with observed heap/process curves.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
const require = createRequire(import.meta.url);
if (!process.env.TABULA_PLAYWRIGHT) throw new Error('TABULA_PLAYWRIGHT driver path required');
const { chromium } = require(process.env.TABULA_PLAYWRIGHT);
const [kind = 'pixi', output = 'docs/verification/issue-60', origin = 'http://127.0.0.1:8060'] = process.argv.slice(2);
const server = await chromium.launchServer({ executablePath: process.env.TABULA_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: false });
const browser = await chromium.connect(server.wsEndpoint());
const context = await browser.newContext({ viewport: { width: 1100, height: 1100 }, deviceScaleFactor: 1 });
const page = await context.newPage(); const cycles = []; const heap = []; const errors = []; const diagnostics = [];
page.on('pageerror', error => errors.push(error.message));
page.on('console', m => { if (['error', 'warning'].includes(m.type())) diagnostics.push({ type: m.type(), message: m.text(), url: m.location().url }); });
function processRss() {
  const rows = execFileSync('ps', ['-ax', '-o', 'pid=,ppid=,rss='], { encoding: 'utf8' }).split('\n').map(s => s.trim().split(/\s+/).map(Number)).filter(r => r.length === 3 && r.every(Number.isFinite));
  const ids = new Set([server.process().pid]); for (let n = 0; n < 10; n++) for (const r of rows) if (ids.has(r[1])) ids.add(r[0]);
  const selected = rows.filter(r => ids.has(r[0])); return { rss_kib_sum: selected.reduce((n, r) => n + r[2], 0), process_count: selected.length, caveat: 'RSS sum double-counts shared pages; includes browser/GPU/shell; not GPU allocation' };
}
let failure = null;
try {
  await page.goto(`${origin}/leptos.html`); await page.waitForFunction(() => window.spikeHarness && document.querySelector('#renderer-container'));
  for (let i = 1; i <= 50; i++) {
    const result = await page.evaluate(type => window.spikeHarness.cycles(type, 1), kind);
    const c = result.cycles[0];
    if (result.completed !== 1 || c.frames_before_dispose < 1 || c.canvases_after !== 0 || c.iframes_after !== 0 || c.host_listeners_after !== 0 || c.host_observers_after !== 0 || c.host_motion_timers_after !== 0 || c.host_ready_timers_after !== 0) throw new Error(`Mount ${i} left active owned resources`);
    if (kind === 'pixi' && (c.controller_after.status !== 'disposed' || c.controller_after.listeners !== 0 || c.controller_after.pending_raf !== 0 || c.controller_after.resources.textures !== 0 || c.controller_after.resources.sources !== 0)) throw new Error(`Pixi mount ${i} leaked owned backend resources`);
    if (kind === 'iframe' && (!c.bridge_after.disposed || c.bridge_after.listeners !== 0)) throw new Error(`Iframe mount ${i} leaked bridge listener`);
    cycles.push({ ...c, cycle: i });
    if (i === 1 || i % 5 === 0) heap.push({ cycle: i, environment: await page.evaluate(() => window.spikeHarness.environment()), ...processRss() });
    if (i % 10 === 0) console.log(JSON.stringify({ kind, completed: i, owned_resources_after: 0 }));
  }
  if (errors.length) throw new Error(errors.join('; '));
} catch (error) { failure = error.message; }
finally {
  const result = { status: failure ? 'FAIL' : 'PASS', kind, requested: 50, completed: cycles.length, browser: browser.version(), shell: 'actual isolated Leptos', frame_count_kind: kind === 'pixi' ? 'Exact callback count for settled Rust revision' : 'Child RAF receipt lower bound, not exact engine frame count', cycles, memory_curve: heap, uncaught_errors: errors, console_diagnostics: diagnostics, failure, cleanup_claim: 'Owned DOM/listener/RAF/font/texture-source handles are zero. No forced GC, no exact process/GPU reclamation claim. Harness history keeps at most100 diagnostic snapshots.' };
  await fs.mkdir(path.join(output, 'runs'), { recursive: true });
  await fs.writeFile(path.join(output, 'runs', `chromium-${kind}-cycles.json`), JSON.stringify(result, null, 2) + '\n');
  await context.close(); await browser.close(); await server.close();
  if (failure) { console.error(failure); process.exitCode = 1; }
}
