// Real Chromium lifecycle, input, negative and presentation checks.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
if (!process.env.TABULA_PLAYWRIGHT) throw new Error('TABULA_PLAYWRIGHT driver path required');
const { chromium } = require(process.env.TABULA_PLAYWRIGHT);
const [output = 'verification/issue-60', origin = 'http://127.0.0.1:8060'] = process.argv.slice(2);
const browser = await chromium.launch({ executablePath: process.env.TABULA_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: false, ignoreDefaultArgs: ['--disable-backgrounding-occluded-windows'] });
const context = await browser.newContext({ viewport: { width: 1100, height: 1100 }, deviceScaleFactor: 1 });
const page = await context.newPage();
const checks = []; const uncaught = []; const consoleMessages = [];
page.on('pageerror', e => uncaught.push(e.message));
page.on('console', m => { if (['error', 'warning'].includes(m.type())) consoleMessages.push({ type: m.type(), message: m.text(), url: m.location().url }); });
await fs.mkdir(path.join(output, 'screenshots'), { recursive: true });
await fs.mkdir(path.join(output, 'runs'), { recursive: true });
function assert(value, message) { if (!value) throw new Error(message); }
async function test(name, action) {
  const started = Date.now();
  try { const evidence = await action(); checks.push({ name, status: evidence?.blocked ? 'BLOCKED' : 'PASS', elapsed_ms: Date.now() - started, evidence }); }
  catch (error) { checks.push({ name, status: 'FAIL', elapsed_ms: Date.now() - started, error: error.message }); }
  console.log(JSON.stringify(checks.at(-1)));
}
async function load() {
  await page.goto(`${origin}/leptos.html`, { waitUntil: 'load' });
  await page.waitForFunction(() => window.spikeHarness && document.querySelector('#renderer-container'));
}
async function dispose() {
  return page.evaluate(async () => { window.spikeHarness.dispose(); await window.spikeHarness.settled(); return window.spikeHarness.report(); });
}
async function mount(kind = 'pixi', options = {}) {
  await dispose();
  const receipt = await page.evaluate(async ({ kind, options }) => window.spikeHarness.mount(kind, options), { kind, options });
  if (kind === 'pixi') await page.waitForFunction(() => window.spikeHarness.report().controller.frames > 0);
  return receipt;
}
try {
  await load();
  const atlasUrl = await page.evaluate(async () => { const runtime = await (await fetch('runtime-assets.json')).json(); return new URL(runtime.assets[0].path, location.href).href; });
  await test('four themes and reduced motion use exact Rust scripted checkpoint with classified zero host input', async () => {
    const receipts = [];
    await page.mouse.move(1, 1);
    for (const [theme, reduced_motion] of [['light', false], ['dark', false], ['hc-light', false], ['hc-dark', false], ['light', true]]) {
      await mount('pixi', { scenario: 'scripted', theme, reduced_motion });
      let last;
      await page.evaluate(() => window.spikeHarness.advance(3000));
      for (let step = 1; step <= 20; step++) {
        for (const at of [3000 + step * 750 - 300, 3000 + step * 750, 3000 + step * 750 + 350]) {
          last = await page.evaluate(now => window.spikeHarness.advance(now), at);
          await page.waitForFunction(revision => window.spikeHarness.report().frame_samples.some(frame => frame.rendered_revision === revision), last.envelope.revision);
        }
      }
      const result = await page.evaluate(() => window.spikeHarness.report());
      assert(last.envelope.frame.input_count === 44 && last.envelope.frame.script_steps === 20, 'Script did not complete actual Rust commands');
      assert(last.envelope.frame.checkpoint === 'b7e81e41d5bd48f076a736858b6855b663591034127628be52b887ae1ce19a73', 'Script checkpoint mismatch');
      assert(result.controller.inputs === 0 && result.errors.length === 0, 'Unplanned host input/errors invalidate script comparison');
      await page.locator('#renderer-container canvas').screenshot({ path: path.join(output, 'screenshots', `pixi-script-${theme}${reduced_motion ? '-reduced' : ''}.png`) });
      receipts.push({ theme, reduced_motion, checkpoint: last.envelope.frame.checkpoint, input_count: last.envelope.frame.input_count, script_steps: last.envelope.frame.script_steps, host_input_events: result.controller.inputs, frame_updates: result.authority.length, final_a11y: last.envelope.frame.a11y, frames_drawn: result.controller.frames });
    }
    return { receipts, scope: 'Actual Pixi GPU/runtime drawing of Rust-scripted accepted workload; presentation clock is explicitly stepped, not realtime timing benchmark' };
  });
  await test('actual keyboard pointer drag focus and resize preserve fixture authority', async () => {
    const initial = await mount('pixi', { scenario: 'interactive' });
    const canvas = page.locator('#renderer-container canvas');
    await canvas.focus(); await page.keyboard.press('ArrowRight'); await page.keyboard.press('Space');
    const box = await canvas.boundingBox();
    await page.mouse.move(box.x + 450, box.y + 300); await page.mouse.down();
    await page.mouse.move(box.x + 530, box.y + 350, { steps: 3 }); await page.mouse.up();
    await page.waitForTimeout(700);
    const beforeResize = await page.evaluate(() => window.spikeHarness.report());
    assert(beforeResize.controller.inputs > 5 && beforeResize.checkpoint === initial.checkpoint, 'Local interaction altered canonical fixture');
    await page.evaluate(() => window.spikeHarness.resize(640, 480, 1));
    await page.waitForTimeout(200);
    const after = await page.evaluate(() => window.spikeHarness.report());
    assert(after.controller.viewport.width === 640 && after.controller.resources.canvas_width === 640 && after.checkpoint === initial.checkpoint, 'Resize changed authority or wrong pixels');
    assert(after.errors.length === 0, 'Interaction or resize reported errors');
    await page.locator('#renderer-container canvas').screenshot({ path: path.join(output, 'screenshots', 'pixi-interactive-resize.png') });
    return { initial_checkpoint: initial.checkpoint, final_checkpoint: after.checkpoint, controller: after.controller, inputs: after.authority.filter(r => r.op === 'input'), a11y: await canvas.getAttribute('aria-label') };
  });
  await test('Rust presenter accepted input is visible at its specific returned revision', async () => {
    await mount('pixi', { scenario: 'interactive' });
    const events = [{ kind: 'key', key: 'Tab', pressed: true }, { kind: 'key', key: 'Enter', pressed: true }];
    const samples = [];
    for (const input of events) {
      const sample = await page.evaluate(async input => {
        const start = performance.now();
        const result = await window.spikeHarness.input(input);
        const revision = result.envelope.revision;
        const deadline = performance.now() + 5000;
        while (!window.spikeHarness.report().frame_samples.some(f => f.rendered_revision === revision)) { if (performance.now() > deadline) throw new Error('Returned revision was not drawn in 5 seconds'); await new Promise(resolve => requestAnimationFrame(resolve)); }
        return { input_kind: input.kind, key: input.key, roundtrip_to_draw_ms: performance.now() - start, outcome: result.outcome, revision, checkpoint: result.envelope.frame.checkpoint, input_count: result.envelope.frame.input_count };
      }, input);
      samples.push(sample);
    }
    assert(samples.at(-1).input_count === 25 && samples.at(-1).outcome === 'accepted', 'Presenter did not pass actual accepted command through Rust');
    await page.locator('#renderer-container canvas').screenshot({ path: path.join(output, 'screenshots', 'pixi-accepted-placement.png') });
    return { samples, scope: 'Programmatic typed input→native HTTP/Rust presenter/rules→returned revision draw; includes serialization, scheduling and local HTTP, not physical hardware input latency' };
  });
  await test('modal and natural Tab focus keep suspended surfaces from consuming input', async () => {
    const results = [];
    for (const kind of ['pixi', 'iframe']) {
      await mount(kind, { scenario: 'interactive' });
      await page.evaluate(() => window.tabulaSpikeAction('focus'));
      await page.evaluate(() => window.tabulaSpikeAction('modal'));
      for (let i = 0; i < 3; i++) { await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab'); }
      const inside = await page.evaluate(() => ({ inert: document.querySelector('main').inert, focused: document.activeElement.closest('#spike-modal') !== null, report: window.spikeHarness.report() }));
      assert(inside.inert && inside.focused, 'Modal allowed focus to leave');
      await page.evaluate(() => window.tabulaSpikeAction('modal'));
      await page.waitForTimeout(100);
      if (kind === 'iframe') {
        const child = page.frames().find(f => f.url().endsWith('/macroquad.html'));
        await child.locator('#glcanvas').focus(); await page.keyboard.press('Tab');
        await page.waitForTimeout(100);
        assert(await page.evaluate(() => document.activeElement.tagName !== 'IFRAME'), 'Iframe trapped Tab');
        await child.locator('#glcanvas').focus(); await page.keyboard.press('Shift+Tab');
        await page.waitForTimeout(100);
        assert(await page.evaluate(() => document.activeElement.tagName !== 'IFRAME'), 'Iframe trapped Shift-Tab');
      }
      results.push({ kind, modal_focus_contained: inside.focused, main_inert: inside.inert, checkpoint: inside.report.checkpoint, final: await page.evaluate(() => window.spikeHarness.report()) });
    }
    return results;
  });
  await test('same-frame activation burst cannot change command meaning after Rust checkpoint advances', async () => {
    await mount('pixi', { scenario: 'interactive' });
    await page.evaluate(() => window.spikeHarness.input({ kind: 'key', key: 'Tab', pressed: true }));
    await page.evaluate(() => {
      const canvas = document.querySelector('#renderer-container canvas');
      for (let i = 0; i < 2; i++) canvas.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', bubbles: true }));
    });
    await page.evaluate(() => window.spikeHarness.settled());
    const result = await page.evaluate(() => window.spikeHarness.report());
    const inputs = result.authority.filter(value => value.op === 'input');
    assert(inputs.filter(value => value.outcome === 'accepted').length === 1 && inputs.filter(value => value.outcome === 'dropped_stale_checkpoint').length === 1, 'Queued activation reinterpreted against a changed checkpoint');
    assert(result.dropped_callback_inputs === 1 && result.errors.length === 0, 'Stale burst guard failed');
    return { inputs, dropped_callback_inputs: result.dropped_callback_inputs, checkpoint: result.checkpoint, scope: 'Real DOM event callbacks; both observe one permitted view. Guard uses Rust checkpoint identity, no JS key/command semantics.' };
  });
  await test('background visibility suspends real RAF and resumes without stopping authority', async () => {
    await mount('pixi');
    const cdp = await context.newCDPSession(page);
    const { windowId } = await cdp.send('Browser.getWindowForTarget');
    let hidden, visible, visibility;
    try {
      await cdp.send('Browser.setWindowBounds', { windowId, bounds: { windowState: 'minimized' } });
      try { await page.waitForFunction(() => document.visibilityState === 'hidden', null, { timeout: 5000 }); }
      catch { return { blocked: true, reason: 'Headed Chrome remained visible after OS-window minimize; tab-switch and occlusion-flag retry also did not produce actual hidden state. No synthetic visibility event substituted.', window_bounds: await cdp.send('Browser.getWindowBounds', { windowId }), observed_document_visibility: await page.evaluate(() => document.visibilityState), controller: (await page.evaluate(() => window.spikeHarness.report())).controller }; }
      await page.waitForTimeout(100);
      visibility = await page.evaluate(() => document.visibilityState);
      hidden = await page.evaluate(() => window.spikeHarness.report());
    } finally {
      await cdp.send('Browser.setWindowBounds', { windowId, bounds: { windowState: 'normal' } });
      await page.bringToFront();
    }
    await page.waitForFunction(() => document.visibilityState === 'visible', null, { timeout: 5000 });
    await page.waitForTimeout(200);
    visible = await page.evaluate(() => window.spikeHarness.report());
    assert(hidden.controller.visibility_suspended && hidden.controller.pending_raf === 0, 'Hidden canvas retained RAF');
    assert(!visible.controller.visibility_suspended && visible.controller.frames > hidden.controller.frames, 'Visible renderer failed to resume');
    return { method: 'CDP minimizes/restores the real headed Chrome OS window; document visibility state observed, no synthetic visibility event', observed_hidden: visibility, hidden: hidden.controller, resumed: visible.controller, checkpoint: visible.checkpoint, authority_timers: 'Fixture has no turn deadline/network timers; online timing not exercised' };
  });
  await test('explicit suspension cancels actual RAF and resumes a drawn frame', async () => {
    await mount('pixi');
    await page.evaluate(() => window.spikeHarness.suspend());
    await page.waitForTimeout(100);
    const paused = await page.evaluate(() => window.spikeHarness.report());
    assert(paused.controller.manual_suspended && paused.controller.pending_raf === 0, 'Explicit suspension retained RAF');
    await page.evaluate(() => window.spikeHarness.resume());
    await page.waitForFunction(n => window.spikeHarness.report().controller.frames > n, paused.controller.frames);
    const resumed = await page.evaluate(() => window.spikeHarness.report());
    return { paused: paused.controller, resumed: resumed.controller, checkpoint: resumed.checkpoint, scope: 'Actual manual lifecycle API; automatic document-hidden transition remains separately blocked.' };
  });
  await test('late native response after dispose cannot revive old generation and remount succeeds', async () => {
    await mount('pixi');
    let intercepted; const reached = new Promise(resolve => { intercepted = resolve; });
    await page.route('**/authority', async route => {
      const value = route.request().postDataJSON();
      const response = await route.fetch();
      if (value.op === 'frame') { intercepted(); await new Promise(resolve => setTimeout(resolve, 250)); }
      await route.fulfill({ response });
    });
    await page.evaluate(() => { window.lateAdvance = window.spikeHarness.advance().catch(error => ({ error: error.message })); });
    await reached; const before = await page.evaluate(() => window.spikeHarness.report());
    await dispose();
    await page.evaluate(() => window.lateAdvance);
    await page.unroute('**/authority');
    const next = await mount('pixi');
    assert(next.identity.generation > before.identity.generation && next.status === 'ready' && next.errors.length === 0, 'Late result broke remount');
    return { old_identity: before.identity, next_identity: next.identity, old_cleanup: await page.evaluate(() => window.spikeHarness.history().at(-1)), result: 'Native accepted revision disposed exactly; old renderer never reattached' };
  });
  await test('dispose during actual delayed atlas initialization leaves no canvas or callbacks', async () => {
    await dispose(); let intercepted; const reached = new Promise(resolve => { intercepted = resolve; });
    await page.route(atlasUrl, async route => {
      const response = await route.fetch(); intercepted(); await new Promise(resolve => setTimeout(resolve, 250));
      await route.fulfill({ response }).catch(() => {});
    });
    await page.evaluate(() => { window.pendingMount = window.spikeHarness.mount('pixi').then(() => ({ resolved: true }), error => ({ cancelled: error.message })); });
    await reached; await dispose();
    const result = await page.evaluate(() => window.pendingMount);
    await page.waitForTimeout(300); await page.unroute(atlasUrl);
    const after = await page.evaluate(() => window.spikeHarness.report());
    assert(result.cancelled && after.status === 'unmounted' && after.canvases === 0 && after.host_listeners === 0 && after.host_observers === 0, 'Late asset completion revived a mount');
    return { result, after };
  });
  await test('same-size corrupt atlas fails before decode and remount recovers', async () => {
    await dispose();
    await page.route(atlasUrl, async route => {
      const response = await route.fetch(); const bytes = await response.body(); bytes[100] ^= 1;
      await route.fulfill({ response, body: bytes });
    });
    const failure = await page.evaluate(() => window.spikeHarness.mount('pixi').then(() => ({ ready: true }), error => ({ error: error.message })));
    await page.unroute(atlasUrl);
    assert(failure.error?.includes('asset_integrity'), 'Corrupt atlas did not fail integrity');
    const history = await page.evaluate(() => window.spikeHarness.history().at(-1));
    const recovered = await mount('pixi');
    assert(recovered.status === 'ready' && recovered.errors.length === 0, 'Asset failure prevented recovery');
    return { failure, cleanup: history.after, recovered: recovered.identity };
  });
  await test('real WebGL loss stops drawing with structured remount requirement', async () => {
    await mount('pixi');
    const available = await page.evaluate(() => {
      const canvas = document.querySelector('#renderer-container canvas');
      const gl = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
      const extension = gl.getExtension('WEBGL_lose_context');
      if (!extension) return false; extension.loseContext(); return true;
    });
    assert(available, 'WEBGL_lose_context unavailable');
    await page.waitForTimeout(200);
    const failed = await page.evaluate(() => window.spikeHarness.report());
    assert(failed.status === 'failed' && failed.errors.some(error => error.code === 'context_lost'), 'Context loss reported false ready');
    const recovered = await mount('pixi');
    assert(recovered.status === 'ready' && recovered.errors.length === 0, 'Context loss remount failed');
    return { failed: { status: failed.status, errors: failed.errors, controller: failed.controller }, recovered: recovered.identity };
  });
  await dispose();
  await test('Chromium DPR2 framebuffer and 2x atlas control execute separately', async () => {
    const c2 = await browser.newContext({ viewport: { width: 1100, height: 1100 }, deviceScaleFactor: 2 });
    const p2 = await c2.newPage();
    try {
      await p2.goto(`${origin}/leptos.html`); await p2.waitForFunction(() => window.spikeHarness && document.querySelector('#renderer-container'));
      await p2.evaluate(() => window.spikeHarness.mount('pixi'));
      await p2.waitForFunction(() => window.spikeHarness.report().controller.frames > 0);
      const result = await p2.evaluate(() => window.spikeHarness.report());
      assert(result.controller.viewport.dpi === 2 && result.controller.resources.canvas_width === 1800 && result.controller.resources.canvas_height === 1440, 'DPR2 framebuffer mismatch');
      await p2.locator('#renderer-container canvas').screenshot({ path: path.join(output, 'screenshots', 'pixi-dpr2.png') });
      await p2.evaluate(async () => { window.spikeHarness.dispose(); await window.spikeHarness.settled(); });
      return { controller: result.controller, checkpoint: result.checkpoint, scope: 'Chromium emulated DPR2; physical high-DPI hardware display not measured' };
    } finally { await c2.close(); }
  });
} finally {
  await dispose().catch(() => {});
  const result = { status: checks.some(check => check.status === 'FAIL') || uncaught.length ? 'FAIL' : checks.some(check => check.status === 'BLOCKED') ? 'PARTIAL' : 'PASS', browser: browser.version(), target: 'actual headed Chromium, isolated Leptos example', checks, uncaught_errors: uncaught, console_messages: consoleMessages, expected_diagnostics: 'Corrupt atlas and context loss intentionally create structured errors; each has asserted cleanup and recovery.' };
  await fs.writeFile(path.join(output, 'runs', 'chromium-interactions.json'), JSON.stringify(result, null, 2) + '\n');
  await context.close(); await browser.close();
  if (result.status === 'FAIL') process.exitCode = 1;
}
