#!/usr/bin/env node
// Desktop-Chrome check of the staged mobile game bundle (ADR-0033). No npm dependencies.
//
// What this IS: the real staged document and WASM, served under the same path/extension/header
// policy and CSP as the native hosts (BundlePaths.kt), driven in a phone-sized Chrome through the
// DevTools protocol, with a small JavaScript stand-in for the native bridge port.
// What this is NOT: an Android WebView or an iOS WKWebView, the Kotlin GameSession, a device, touch
// hardware or a GPU. It cannot support any claim about those targets; see the evidence ledger.
//
//   cargo xtask stage-mobile-game && node tools/mobile-host-check/run.mjs [--out DIR]
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const bundle = path.join(root, 'target/tabula-mobile-game');
const outArg = process.argv.indexOf('--out');
const out = path.resolve(outArg > 0 ? process.argv[outArg + 1] : path.join(root, 'docs/verification/mobile-game-host/desktop'));
const chromePath = process.env.TABULA_CHROME ?? 'google-chrome';
fs.mkdirSync(path.join(out, 'screenshots'), { recursive: true });

// ---- policy shared with the native hosts, read from their source so it cannot silently drift ----
const bundlePathsSource = fs.readFileSync(path.join(root, 'mobile/shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/host/BundlePaths.kt'), 'utf8');
const cspBlock = bundlePathsSource.match(/const val DOCUMENT_CSP: String =\s*((?:"[^"\n]*"\s*\+?\s*)+)/);
if (!cspBlock) throw new Error('DOCUMENT_CSP not found in BundlePaths.kt; update this check with the host policy');
const CSP = [...cspBlock[1].matchAll(/"([^"]*)"/g)].map((m) => m[1]).join('');
const TYPES = { html: 'text/html; charset=utf-8', js: 'text/javascript; charset=utf-8', css: 'text/css; charset=utf-8', wasm: 'application/wasm', png: 'image/png', ttf: 'font/ttf' };
const SEGMENT = /^[A-Za-z0-9._@-]{1,128}$/;
const IMMUTABLE = /^play\/[a-z0-9-]+\/resources\/[0-9a-f]{64}\.[a-z0-9]{1,12}$/;
function resolve(requestPath) {
  if (!requestPath.startsWith('/play/')) return null;
  const rel = requestPath.slice(1);
  const parts = (rel.endsWith('/') ? rel + 'index.html' : rel).split('/');
  if (parts.length < 3 || parts.length > 6 || parts.some((p) => p === '.' || p === '..' || !SEGMENT.test(p))) return null;
  const file = parts.join('/');
  return TYPES[file.split('.').pop()] ? file : null;
}

const manifest = JSON.parse(fs.readFileSync(path.join(bundle, 'tabula-games.json'), 'utf8'));
const game = manifest.games[0];
const requests = [];
let fault = null; // 'missing-wasm' | 'corrupt-wasm'
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  const file = req.method === 'GET' ? resolve(url.pathname) : null;
  // Desktop Chrome asks for a favicon on its own; neither native WebView does, so it is not a bundle request.
  if (url.pathname === '/favicon.ico') { res.writeHead(404).end(); return; }
  const record = (status) => requests.push({ path: url.pathname, status });
  if (!file || !fs.existsSync(path.join(bundle, file))) { record(file ? 404 : 403); res.writeHead(file ? 404 : 403).end(); return; }
  if (fault && file.endsWith('.wasm')) {
    if (fault === 'missing-wasm') { record(404); res.writeHead(404).end(); return; }
    const bytes = Buffer.from(fs.readFileSync(path.join(bundle, file))); bytes[bytes.length >> 1] ^= 0xff;
    record(200); res.writeHead(200, { 'Content-Type': TYPES.wasm }).end(bytes); return;
  }
  const headers = { 'Content-Type': TYPES[file.split('.').pop()], 'Cache-Control': IMMUTABLE.test(file) ? 'public, max-age=31536000, immutable' : 'no-store', 'X-Content-Type-Options': 'nosniff', 'Cross-Origin-Resource-Policy': 'same-origin', 'Referrer-Policy': 'no-referrer' };
  if (file.endsWith('.html')) headers['Content-Security-Policy'] = CSP;
  record(200); res.writeHead(200, headers).end(fs.readFileSync(path.join(bundle, file)));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${server.address().port}`;
const documentUrl = `${origin}${game.entry}?${game.query}`;

// ---- Chrome over CDP ----
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'tabula-host-check-'));
const chrome = spawn(chromePath, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', 'about:blank'], { stdio: 'ignore' });
let wsUrl;
for (let i = 0; i < 100 && !wsUrl; i++) {
  await new Promise((r) => setTimeout(r, 100));
  try { const [port] = fs.readFileSync(path.join(profile, 'DevToolsActivePort'), 'utf8').split('\n'); wsUrl = (await (await fetch(`http://127.0.0.1:${port}/json/version`)).json()).webSocketDebuggerUrl; } catch {}
}
if (!wsUrl) throw new Error('Chrome did not expose a DevTools endpoint');
const ws = new WebSocket(wsUrl); await new Promise((r) => (ws.onopen = r));
let nextId = 1; const pending = new Map(); const listeners = [];
ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id) { const p = pending.get(m.id); pending.delete(m.id); m.error ? p.reject(new Error(JSON.stringify(m.error))) : p.resolve(m.result); } else listeners.forEach((l) => l(m)); };
const send = (method, params = {}, sessionId) => new Promise((resolve, reject) => { const id = nextId++; pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params, sessionId })); });
const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
const cdp = (method, params) => send(method, params, sessionId);
const evaluate = async (expression) => { const r = await cdp('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description ?? '')); return r.result.value; };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(predicate, what, ms = 60000) { const end = Date.now() + ms; for (;;) { const v = await predicate(); if (v) return v; if (Date.now() > end) throw new Error(`timed out: ${what}`); await sleep(50); } }

// ---- JavaScript stand-in for the native host: answers the handshake like GameSession would ----
const hostLog = []; let generation = 0; let hostMode = 'answer'; // 'answer' | 'silent'
const consoleProblems = [];
const deliver = (message) => evaluate(`window.__deliver(${JSON.stringify(JSON.stringify(message))})`);
listeners.push((m) => {
  if (m.sessionId !== sessionId) return;
  if (m.method === 'Runtime.bindingCalled' && m.params.name === '__hostPost') {
    const message = JSON.parse(m.params.payload); hostLog.push({ dir: 'page->host', at: Date.now(), message });
    if (message.type === 'hello' && hostMode === 'answer') { generation += 1; deliver({ v: 1, type: 'init', gen: generation, capabilities: ['keep-awake'], preferences: { theme: 'dark', motion: 'system', locale: 'en' } }); }
    if (message.type === 'service') { const reply = { v: 1, type: 'reply', gen: message.gen, id: message.id, ok: true, code: 'ok' }; hostLog.push({ dir: 'host->page', at: Date.now(), message: reply }); deliver(reply); }
  }
  if (m.method === 'Network.responseReceived' && m.params.response.status >= 400 && !m.params.response.url.endsWith('/favicon.ico')) consoleProblems.push({ kind: 'http', status: m.params.response.status, url: m.params.response.url, type: m.params.type, fromCache: m.params.response.fromDiskCache, protocol: m.params.response.protocol, headers: m.params.response.requestHeaders });
  if (m.method === 'Runtime.exceptionThrown') consoleProblems.push({ kind: 'exception', text: m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text });
  if (m.method === 'Log.entryAdded' && ['error', 'warning'].includes(m.params.entry.level) && !m.params.entry.url?.endsWith('/favicon.ico')) consoleProblems.push({ kind: m.params.entry.source, level: m.params.entry.level, text: m.params.entry.text, url: m.params.entry.url });
});
const sent = (type) => hostLog.filter((e) => e.dir === 'page->host' && e.message.type === type);
const host = async (type) => { const m = { v: 1, type, gen: generation }; hostLog.push({ dir: 'host->page', at: Date.now(), message: m }); await deliver(m); };

await cdp('Page.enable'); await cdp('Runtime.enable'); await cdp('Log.enable'); await cdp('Network.enable');
await cdp('Runtime.addBinding', { name: '__hostPost' });
await cdp('Page.addScriptToEvaluateOnNewDocument', { source: `
  (function () {
    const port = { onmessage: null, postMessage(text) { window.__hostPost(text); } };
    Object.defineProperty(window, 'TabulaHostNative', { value: port });
    window.__deliver = (text) => port.onmessage({ data: text });
    // Count frame-loop reschedules: the game loop requests exactly one animation frame per drawn frame.
    window.__frames = 0; const raf = window.requestAnimationFrame.bind(window);
    window.requestAnimationFrame = (fn) => raf((t) => { window.__frames++; fn(t); });
  })();` });
const viewport = async (width, height) => cdp('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 2, mobile: true, screenOrientation: width > height ? { type: 'landscapePrimary', angle: 90 } : { type: 'portraitPrimary', angle: 0 } });
const shot = async (name, clip) => { const { data } = await cdp('Page.captureScreenshot', { format: 'png', ...(clip ? { clip: { ...clip, scale: 1 } } : {}) }); if (!clip) fs.writeFileSync(path.join(out, 'screenshots', name), Buffer.from(data, 'base64')); return data; };
const frames = () => evaluate('window.__frames');
const receipts = { schema: 1, kind: 'desktop-chrome-simulation', not: ['android-webview', 'ios-wkwebview', 'device', 'touch-hardware', 'gpu'], chrome: null, csp: CSP, checks: [] };
const check = (name, pass, detail = {}) => { receipts.checks.push({ name, status: pass ? 'PASS' : 'FAIL', ...structuredClone(detail) }); console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}`); };
receipts.chrome = (await send('Browser.getVersion')).product;

// The loader reuses SHA-256-verified bytes from CacheStorage across documents, so a fault in the
// network copy is only observable after the cache is cleared.
const clearCache = () => evaluate(`location.origin === ${JSON.stringify(origin)} ? caches.keys().then((keys) => Promise.all(keys.map((key) => caches.delete(key)))).then(() => true) : true`);
async function open(url = documentUrl) { hostLog.length = 0; consoleProblems.length = 0; requests.length = 0; await cdp('Page.navigate', { url }); }

try {
  // 1. first open: handshake -> ready, under the host CSP, with every request served by policy
  await viewport(412, 800);
  const t0 = Date.now(); await open(); // a fresh profile: CacheStorage starts empty
  const ready = await until(() => sent('ready')[0], 'ready message');
  const openMs = Date.now() - t0;
  check('open: hello, init, ready arrive in order with generation 1', hostLog.filter((e) => e.dir === 'page->host').map((e) => e.message.type).slice(0, 2).join() === 'hello,ready' && ready.message.gen === 1, { bootMs: ready.message.bootMs, wallClockOpenMs: openMs });
  await sleep(500);
  check('open: keep-awake requested once and the host reply was delivered', sent('service').length === 1 && sent('service')[0].message.name === 'keep-awake' && hostLog.some((e) => e.dir === 'host->page' && e.message.type === 'reply' && e.message.ok === true && e.message.id === sent('service')[0].message.id));
  check('open: no CSP violation, console error or page exception', consoleProblems.length === 0, { problems: consoleProblems, requestsSeen: requests.map((r) => `${r.status} ${r.path.split('/').pop().slice(0, 20)}`) });
  check('open: every request was served by the bundle policy (no 403/404)', requests.every((r) => r.status === 200), { requests: requests.length, nonOk: requests.filter((r) => r.status !== 200) });
  check('open: loader fetched the WASM through the verified path', requests.some((r) => /\/resources\/[0-9a-f]{64}\.wasm$/.test(r.path)));
  receipts.dom = await evaluate(`({ canvas: [glcanvas.width, glcanvas.height, glcanvas.clientWidth, glcanvas.clientHeight], theme: document.documentElement.dataset.theme, lang: document.documentElement.lang, loaderHidden: document.getElementById('loader').hidden })`);
  check('open: host preferences applied (dark theme, English) and loader dismissed', receipts.dom.theme === 'dark' && receipts.dom.lang === 'en' && receipts.dom.loaderHidden);
  await shot('01-ready-portrait.png');
  receipts.performance = await evaluate(`(() => { const nav = performance.getEntriesByType('navigation')[0]; const res = performance.getEntriesByType('resource').map((r) => ({ name: r.name.split('/').pop().slice(0, 16), ms: Math.round(r.duration), bytes: r.transferSize })); return { domContentLoadedMs: Math.round(nav.domContentLoadedEventEnd), loadMs: Math.round(nav.loadEventEnd), resources: res }; })()`);

  // 2. interaction: a real legal move through pointer input; the board changes and then settles
  const rect = await evaluate(`(() => { const r = glcanvas.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; })()`);
  receipts.canvas = rect;
  const before = await shot('x', { x: rect.x, y: rect.y, width: rect.w, height: rect.h });
  const click = async (fx, fy) => { const x = rect.x + rect.w * fx, y = rect.y + rect.h * fy; for (const type of ['mousePressed', 'mouseReleased']) await cdp('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1, pointerType: 'mouse' }); await sleep(250); };
  const squares = JSON.parse(fs.readFileSync(path.join(path.dirname(fileURLToPath(import.meta.url)), 'squares.json'), 'utf8'));
  await click(...squares.e2); await click(...squares.e4); await sleep(1500);
  const after = await shot('x', { x: rect.x, y: rect.y, width: rect.w, height: rect.h });
  const settled = await shot('x', { x: rect.x, y: rect.y, width: rect.w, height: rect.h });
  check('interaction: e2 then e4 changes the rendered board', before !== after);
  check('interaction: the board settles (two later captures identical)', after === settled);
  await shot('02-after-e2-e4.png');

  // 3. suspend / resume: drawing stops and restarts; the document stays alive
  const f0 = await frames(); await sleep(1000); const running = (await frames()) - f0;
  await host('suspend'); await sleep(300); const f1 = await frames(); await sleep(600); const suspendedDelta = (await frames()) - f1;
  await host('suspend');
  await host('resume'); await host('resume'); await sleep(1000); const resumedDelta = (await frames()) - f1;
  check('suspend: frame loop stops, resume restarts it', running > 3 && suspendedDelta <= 1 && resumedDelta > 3, { running, suspendedDelta, resumedDelta });
  const f2 = await frames(); await sleep(1000); const cadence = (await frames()) - f2;
  receipts.frameCadence = { framesPerSecond: cadence, note: 'headless SwiftShader software GL; not a device frame-pacing measurement' };

  // 4. resize / rotation: the canvas follows the viewport and keeps rendering
  await viewport(860, 412); await sleep(800);
  const land = await evaluate(`({ w: glcanvas.clientWidth, h: glcanvas.clientHeight, iw: innerWidth, ih: innerHeight })`);
  check('resize: landscape canvas matches the viewport and frames continue', land.w === land.iw && land.h > 0 && (await frames()) > f2, land);
  await shot('03-landscape.png'); await viewport(412, 800); await sleep(600);

  // 5. back: routed to the page's own leave confirmation; second press dismisses it
  const dialogOpen = () => evaluate(`document.getElementById('leave-dialog').open`);
  await host('back-requested'); await sleep(200); const first = await dialogOpen();
  await shot('04-leave-dialog.png');
  await host('back-requested'); await sleep(200); const second = await dialogOpen();
  await host('back-requested'); await sleep(200); const third = await dialogOpen();
  check('back: opens the leave confirmation, dismisses it, opens it again', first === true && second === false && third === true && sent('exit').length === 0);

  // 6. exit: confirm-leave posts one exit and the document stays inert
  await evaluate(`document.getElementById('confirm-leave').click()`); await sleep(300);
  await evaluate(`document.getElementById('confirm-leave').click()`);
  check('exit: exactly one exit message, no navigation away', sent('exit').length === 1 && (await evaluate('location.href')) === documentUrl);
  const fx = await frames(); await host('resume'); await host('back-requested'); await sleep(600);
  check('late events: a retired document ignores resume/back and draws nothing', (await frames()) - fx <= 1 && sent('exit').length === 1);

  // 7. reopen: a new document, a new generation, a fresh match
  await open(); const again = await until(() => sent('ready')[0], 'ready after reopen');
  await sleep(500);
  check('reopen: new generation, fresh ready', again.message.gen === 2 && sent('hello').length === 1, { warmBootMs: again.message.bootMs, coldBootMs: ready.message.bootMs, note: 'warm = verified bytes reused from CacheStorage' });
  await shot('05-reopened-fresh.png');
  const fresh = await shot('x', { x: rect.x, y: rect.y, width: rect.w, height: rect.h });
  check('reopen: the board is a new match (differs from the moved position)', fresh !== after);

  // 8. dispose: after the host disposes the document, everything it later sends is ignored
  const sentBefore = hostLog.filter((e) => e.dir === 'page->host').length;
  await host('dispose'); await sleep(300); const fd = await frames(); await host('resume'); await host('suspend'); await host('back-requested'); await sleep(500);
  check('dispose: runtime retired, no frames, nothing more sent', (await frames()) - fd <= 1 && hostLog.filter((e) => e.dir === 'page->host').length === sentBefore);

  // 9. load failures
  fault = 'missing-wasm'; await clearCache(); await open();
  await until(() => evaluate(`!document.getElementById('runtime-error').hidden`), 'error overlay (missing wasm)');
  const missing = sent('failed')[0];
  check('failure: a missing WASM shows the recovery overlay and reports failed(runtime)', !!missing && missing.message.code === 'runtime', { detail: missing?.message.detail });
  await shot('06-failure-missing-wasm.png');
  fault = 'corrupt-wasm'; await clearCache(); await open();
  await until(() => evaluate(`!document.getElementById('runtime-error').hidden`), 'error overlay (corrupt wasm)');
  const corrupt = sent('failed')[0];
  check('failure: a WASM whose SHA-256 differs never runs and reports failed(runtime)', !!corrupt && /SHA-256/.test(corrupt.message.detail), { detail: corrupt?.message.detail });
  fault = null;
  hostMode = 'silent'; await clearCache(); await open();
  await sleep(5600);
  check('failure: a host that never answers the handshake fails closed after 5 s with no game fetch', (await evaluate(`!document.getElementById('runtime-error').hidden`)) && !requests.some((r) => r.path.endsWith('.wasm')), { wasmRequests: requests.filter((r) => r.path.endsWith('.wasm')).length });
  hostMode = 'answer';

  // 10. retry inside the document is a reload with a fresh handshake
  fault = 'missing-wasm'; await clearCache(); await open(); await until(() => evaluate(`!document.getElementById('runtime-error').hidden`), 'error overlay');
  fault = null; hostLog.length = 0; await evaluate(`document.getElementById('retry').click()`);
  const retried = await until(() => sent('ready')[0], 'ready after retry');
  check('retry: Try again reloads, re-handshakes with a new generation and starts', retried.message.gen > 2 && sent('hello').length === 1);
} catch (error) {
  check(`harness error: ${error.message}`, false);
} finally {
  fs.writeFileSync(path.join(out, 'desktop-receipt.json'), JSON.stringify(receipts, null, 1) + '\n');
  chrome.kill(); await new Promise((r) => chrome.once('exit', r)); server.close(); fs.rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  const failed = receipts.checks.filter((c) => c.status !== 'PASS');
  console.log(`\n${receipts.checks.length - failed.length}/${receipts.checks.length} checks passed on ${receipts.chrome}`);
  process.exit(failed.length ? 1 : 0);
}
