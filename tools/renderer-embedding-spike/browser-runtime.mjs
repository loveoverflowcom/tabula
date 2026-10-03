// Actual headed Chromium evidence. Run with an installed Playwright driver path.
// This does not install browsers, change production routes or consume user tabs.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
const require = createRequire(import.meta.url);
const driver = process.env.TABULA_PLAYWRIGHT;
if (!driver) throw new Error('Set TABULA_PLAYWRIGHT to an installed playwright-core package.');
const { chromium } = require(driver);
const [kind = 'document', output = 'docs/verification/issue-60', origin = 'http://127.0.0.1:8060'] = process.argv.slice(2);
const executable = process.env.TABULA_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const server = await chromium.launchServer({ executablePath: executable, headless: false, args: ['--no-first-run', '--no-default-browser-check'] });
const browser = await chromium.connect(server.wsEndpoint());
const context = await browser.newContext({ viewport: kind === 'document' ? { width: 900, height: 720 } : { width: 1100, height: 1100 }, deviceScaleFactor: 1 });
await fs.mkdir(path.join(output, 'runs'), { recursive: true });
await fs.mkdir(path.join(output, 'screenshots'), { recursive: true });
const checkpoint = 'e4ed3465b826a55c12d68d8f8bcbef5422fa5d128a251da1f4f659470060d032';
function assert(condition, message) { if (!condition) throw new Error(message); }
function cpuSeconds(value) {
  const fields = value.split(':').map(Number);
  return fields.reduce((total, next) => total * 60 + next, 0);
}
function processes(pid) {
  const rows = execFileSync('ps', ['-ax', '-o', 'pid=,ppid=,time=,rss=,command='], { encoding: 'utf8' }).split('\n').map(line => {
    const m = line.match(/^\s*(\d+)\s+(\d+)\s+(\S+)\s+(\d+)\s+(.*)$/);
    return m ? { pid: +m[1], ppid: +m[2], cpu_s: cpuSeconds(m[3]), rss_kib: +m[4], role: m[5].match(/--type=(\S+)/)?.[1] ?? 'browser' } : null;
  }).filter(Boolean);
  const included = new Set([pid]);
  for (let i = 0; i < 10; i++) for (const row of rows) if (included.has(row.ppid)) included.add(row.pid);
  const members = rows.filter(row => included.has(row.pid));
  return { processes: members, cpu_s_sum: members.reduce((sum, row) => sum + row.cpu_s, 0), rss_kib_sum: members.reduce((sum, row) => sum + row.rss_kib, 0), caveat: 'RSS sum double-counts shared pages; CPU sum excludes exited children; includes browser/GPU/shell' };
}
function quantiles(values) {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!sorted.length) return null;
  const rank = p => sorted[Math.max(0, Math.ceil(sorted.length * p) - 1)];
  return { samples: sorted.length, mean: sorted.reduce((a, b) => a + b, 0) / sorted.length, p50: rank(0.5), p95: rank(0.95), max: sorted.at(-1) };
}
async function observation(page) {
  return page.evaluate(() => ({ ua: navigator.userAgent, dpr: devicePixelRatio, visibility: document.visibilityState, focused: document.hasFocus(), js_heap_bytes: performance.memory?.usedJSHeapSize ?? null,
    resources: performance.getEntriesByType('resource').map(r => ({ name: r.name.replace(location.origin, ''), duration_ms: r.duration, transfer_bytes: r.transferSize, encoded_bytes: r.encodedBodySize, decoded_bytes: r.decodedBodySize })),
    navigation: performance.getEntriesByType('navigation').map(r => ({ duration_ms: r.duration, dom_content_loaded_ms: r.domContentLoadedEventEnd, load_ms: r.loadEventEnd })) }));
}
try {
  for (let run = 1; run <= 3; run++) {
    const page = await context.newPage(); const receipts = []; const consoleErrors = []; const pageErrors = [];
    const processSamples = []; const begun = Date.now();
    const sampler = setInterval(() => processSamples.push({ elapsed_ms: Date.now() - begun, ...processes(server.process().pid) }), 1000);
    page.on('console', message => {
      const value = message.text(); const offset = value.indexOf('TABULA_BASELINE ');
      if (offset >= 0) { try { receipts.push({ observed_ms: Date.now() - begun, value: JSON.parse(value.slice(offset + 16)) }); } catch {} }
      if (['error', 'warning'].includes(message.type())) consoleErrors.push({ type: message.type(), message: value, url: message.location().url });
    });
    page.on('pageerror', error => pageErrors.push(error.message));
    let mounted = null; let final = null; let samples = []; let postMessage = null;
    try {
      await page.goto(`${origin}/${kind === 'document' ? 'macroquad.html' : 'leptos.html'}`, { waitUntil: 'load', timeout: 30000 });
      if (kind === 'document') {
        await page.waitForFunction(() => window.tabulaMacroquad?.report().receipts.some(r => r.kind === 'measurement'), { timeout: 30000 });
        final = await page.evaluate(() => window.tabulaMacroquad.report());
        await page.locator('#glcanvas').screenshot({ path: path.join(output, 'screenshots', `chromium-document-${run}.png`) });
      } else {
        await page.waitForFunction(() => window.spikeHarness && document.querySelector('#renderer-container'), { timeout: 30000 });
        mounted = await page.evaluate(type => window.spikeHarness.mount(type, { scenario: 'static', viewport: { width: 900, height: 720, dpi: 1 } }), kind);
        assert(mounted.shell === 'leptos-example', 'Expected actual isolated Leptos runtime');
        if (kind === 'iframe') {
          await page.waitForFunction(() => window.spikeHarness.report().metrics.some(m => m.name === 'baseline_measurement'), { timeout: 30000 });
          final = await page.evaluate(() => window.spikeHarness.report());
          const child = page.frames().find(frame => frame.url().endsWith('/macroquad.html'));
          assert(child, 'Missing running iframe');
          await child.locator('#glcanvas').screenshot({ path: path.join(output, 'screenshots', `chromium-iframe-${run}.png`) });
          postMessage = await page.evaluate(() => window.spikeHarness.roundtrips(100));
        } else {
          // Warm-up matches #59's settled initial workload; transport is excluded
          // from static frame drawing and measured independently below.
          await page.evaluate(() => window.spikeHarness.advance(3000));
          await page.waitForTimeout(3000);
          const start = await page.evaluate(() => window.spikeHarness.report().controller.frames);
          await page.waitForFunction(n => window.spikeHarness.report().controller.frames >= n + 300, start, { timeout: 30000 });
          final = await page.evaluate(() => window.spikeHarness.report());
          samples = final.frame_samples.slice(-300);
          await page.locator('#renderer-container canvas').screenshot({ path: path.join(output, 'screenshots', `chromium-pixi-${run}.png`) });
          assert(final.checkpoint === checkpoint, 'Pixi permitted fixture checkpoint mismatch');
          assert(final.controller.viewport.width === 900 && final.controller.viewport.height === 720 && final.controller.viewport.dpi === 1, 'Pixi viewport/DPI mismatch');
          assert(final.controller.inputs === 0 && final.errors.length === 0, 'Controlled Pixi run had input/errors');
        }
        await page.screenshot({ path: path.join(output, 'screenshots', `chromium-${kind}-shell-${run}.png`) });
      }
      const baseline = receipts.find(r => r.value.kind === 'measurement')?.value;
      const ready = receipts.find(r => r.value.kind === 'ready')?.value;
      if (kind !== 'pixi') {
        assert(baseline?.samples === 300 && baseline.final_checkpoint === checkpoint && baseline.uncontrolled_input_events === 0, 'Controlled Macroquad receipt mismatch');
        assert(ready.viewport[0] === 900 && ready.viewport[1] === 720 && ready.dpi === 1, 'Macroquad viewport/DPI mismatch');
      }
      assert(pageErrors.length === 0, `Uncaught browser errors: ${pageErrors.join('; ')}`);
      const expectedConsole = consoleErrors.filter(error => error.url.endsWith('/favicon.ico') && /404/.test(error.message));
      const unexpectedConsole = consoleErrors.filter(error => !expectedConsole.includes(error));
      assert(unexpectedConsole.length === 0, `Unexpected browser warnings/errors: ${JSON.stringify(unexpectedConsole)}`);
      const artifact = { status: 'PASS', kind, run, browser: browser.version(), executable, driver: 'existing Playwright core', cache: run === 1 ? 'fresh browser process and fresh renderer; disk/OS/driver caches not purged' : 'same browser/context, new document/renderer; HTTP cache warm unless resource timing shows otherwise',
        viewport: [900, 720], dpr: 1, scenario: 'static', warmup_ms: 3000, samples: 300, receipts, mounted, final, process_samples: processSamples,
        observation: await observation(page), console_errors: consoleErrors, expected_console: expectedConsole, page_errors: pageErrors,
        draw_method_ms: kind === 'pixi' ? quantiles(samples.map(s => s.draw_ms)) : null,
        frame_interval_ms: kind === 'pixi' ? quantiles(samples.map(s => s.frame_interval_ms)) : null,
        postmessage_roundtrip_ms: postMessage ? quantiles(postMessage.samples) : null,
        postmessage_samples: postMessage, method_comparison: 'Pixi draw includes scene rebuild + app.render; Macroquad submit/end excludes final frame flush. These are different boundaries, not engine-equivalent CPU timers.' };
      await fs.writeFile(path.join(output, 'runs', `chromium-${kind}-${run}.json`), JSON.stringify(artifact, null, 2) + '\n');
      console.log(JSON.stringify({ status: 'PASS', kind, run, checkpoint, file: `runs/chromium-${kind}-${run}.json` }));
    } catch (error) {
      await fs.writeFile(path.join(output, 'runs', `chromium-${kind}-${run}-failure.json`), JSON.stringify({ status: 'FAIL', kind, run, error: error.message, receipts, mounted, final, consoleErrors, pageErrors, observation: await observation(page).catch(() => null) }, null, 2) + '\n');
      throw error;
    } finally {
      clearInterval(sampler);
      if (kind !== 'document') await page.evaluate(async () => {
        window.spikeHarness?.dispose();
        await window.spikeHarness?.settled?.();
      }).catch(() => {});
      await page.close();
    }
  }
} finally { await context.close(); await browser.close(); await server.close(); }
