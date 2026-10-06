// Actual Macroquad browser evidence for issue #84's isolated local simulator.
// Operates public DOM controls and real pointer/keyboard input; never reads WASM memory,
// canonical state, a projection object, or a hidden role through instrumentation.
// Run with TABULA_PLAYWRIGHT and TABULA_PNGJS pointing to installed package directories.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.TABULA_PLAYWRIGHT ?? 'playwright');
const { PNG } = require(process.env.TABULA_PNGJS ?? 'pngjs');
const [origin = 'http://127.0.0.1:8085', output = 'docs/verification/werewolf-redesign-84'] = process.argv.slice(2);
const screenshotDir = path.resolve(output, 'screenshots');
const runDir = path.resolve(output, 'runs');
await fs.mkdir(screenshotDir, { recursive: true });
await fs.mkdir(runDir, { recursive: true });
const browser = await chromium.launch({ executablePath: process.env.TABULA_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
const browserVersion = await browser.version();
const receipts = [];
const captures = [];
const screenshotTempDir = await fs.mkdtemp(path.join(tmpdir(), 'tabula-werewolf84-captures-'));
function retain(name) {
  return process.env.TABULA_KEEP_ALL === '1' || name.endsWith('-public') || name.endsWith('-failure') || name.startsWith('theme-') || name.startsWith('phase-') || name.startsWith('motion-')
    || ['390x844-dpr1-revealed', '844x390-dpr1-revealed', '1200x880-dpr1-keyboard-submitted'].includes(name);
}
function assert(value, message) { if (!value) throw new Error(message); }
const rect = (x, y, width, height) => ({ x, y, width, height });

// Public layout coordinates solely locate input and screenshot crops. They are not an oracle
// for game layout correctness: full rendered screenshots need separate visual inspection.
function geometry(width, height) {
  const compact = width < 760 || height < 620;
  const landscape = width >= 600 && height < 620;
  const margin = Math.min(12, width * .03);
  const header = height < 500 ? 66 : 100;
  const footer = rect(margin, Math.max(height - 52, header), width - margin * 2, 44);
  let table, reveal, dock, card;
  if (compact && !landscape) {
    dock = rect(margin, footer.y - 112, width - margin * 2, 104);
    reveal = rect(margin, dock.y - 52, dock.width, 44);
    table = rect(margin, header, dock.width, Math.max(reveal.y - header - 8, 156));
    const cw = Math.min(width - 64, Math.max(height - 330, 180) / 1.5, 256);
    card = rect((width - cw) / 2, 64, cw, cw * 1.5);
  } else if (landscape) {
    const sidebar = Math.min(260, Math.max(180, width * .30));
    table = rect(margin, header, width - sidebar - margin * 3, footer.y - header - 8);
    reveal = rect(table.x + table.width + margin, header, sidebar, 44);
    dock = rect(reveal.x, header + 52, sidebar, 104);
    const cw = Math.min(240, Math.max(120, height - 100)) / 1.5;
    card = rect(24, 54, cw, cw * 1.5);
  } else {
    const sidebar = Math.min(300, Math.max(220, width * .23));
    dock = rect(margin, footer.y - 112, width - margin * 2, 104);
    table = rect(margin, header, width - sidebar - margin * 3, dock.y - header - 12);
    const cw = Math.min(sidebar, Math.max(table.height - 100, 120) / 1.5, 230);
    card = rect(table.x + table.width + margin + (sidebar - cw) / 2, header + 34, cw, cw * 1.5);
    reveal = rect(card.x, card.y + card.height + 8, cw, 44);
  }
  return { compact, landscape, table, reveal, dock, card, footer, dialog: rect(margin, Math.max(height * .16, 8), width - margin * 2, Math.max(height * .68, 244)) };
}
function pixel(png, x, y) {
  const at = (Math.floor(y) * png.width + Math.floor(x)) * 4;
  return [...png.data.subarray(at, at + 3)];
}
function differences(before, after, bounds, dpr) {
  let changed = 0, total = 0;
  for (let y = Math.ceil(bounds.y * dpr); y < Math.floor((bounds.y + bounds.height) * dpr); y++) {
    for (let x = Math.ceil(bounds.x * dpr); x < Math.floor((bounds.x + bounds.width) * dpr); x++) {
      total++;
      if (pixel(before, x, y).some((value, index) => Math.abs(value - pixel(after, x, y)[index]) > 8)) changed++;
    }
  }
  return { changed, total, fraction: changed / total };
}
async function screenshot(page, name, clip) {
  const retained = retain(name);
  const filename = path.join(retained ? screenshotDir : screenshotTempDir, `${name}.png`);
  let bytes;
  if (clip) {
    // Public compositor pixels only. Direct CDP capture avoids Playwright's stability wait
    // spanning most of a short reveal transition; it does not pause or advance the app.
    const session = await page.context().newCDPSession(page);
    try {
      const result = await session.send('Page.captureScreenshot', { format: 'png', clip: { ...clip, scale: 1 }, fromSurface: true, captureBeyondViewport: false });
      bytes = Buffer.from(result.data, 'base64');
      await fs.writeFile(filename, bytes);
    } finally { await session.detach(); }
  } else bytes = await page.screenshot({ path: filename });
  const capture = { name, filename, retained, clip, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
  captures.push(capture);
  return { ...capture, pixels: PNG.sync.read(bytes) };
}
async function key(page, value) { await page.keyboard.press(value, { delay: 70 }); await page.waitForTimeout(80); }
async function click(page, box) { await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { delay: 70 }); await page.waitForTimeout(100); }
async function openedTools(page, g) { await click(page, rect(g.footer.x, g.footer.y, Math.min(220, (g.footer.width - 8) / 2), 44)); }

try {
  for (const dpr of [1, 2]) for (const [width, height] of [[1200, 880], [1100, 850], [390, 844], [320, 640], [844, 390]]) {
    const started = Date.now();
    const name = `${width}x${height}-dpr${dpr}`;
    if (process.env.TABULA_CASES && !process.env.TABULA_CASES.split(',').includes(name)) continue;
    const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: dpr, reducedMotion: 'reduce' });
    const page = await context.newPage();
    const errors = [], consoleMessages = [], steps = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (['error', 'warning'].includes(message.type())) consoleMessages.push({ type: message.type(), message: message.text(), url: message.location().url }); });
    const receipt = { name, width, height, dpr, started_at: new Date(started).toISOString(), scope: 'Actual Chromium/Macroquad local simulator; public DOM and real input only; no native/online/a11y-mirror claim', steps };
    try {
      await page.goto(`${origin}/play.html?game=werewolf&mode=simulator&seats=12&theme=dark&motion=reduced&locale=vi`, { waitUntil: 'load' });
      await page.waitForFunction(() => document.querySelector('#loader').hidden || !document.querySelector('#runtime-error').hidden, null, { timeout: 45000 });
      assert(await page.locator('#runtime-error').isHidden(), await page.locator('#error-detail').textContent());
      await page.locator('#glcanvas').focus();
      await page.waitForFunction(() => document.querySelector('#privacy-shield').hidden);
      await page.waitForTimeout(500);
      receipt.runtime_artifact = await page.evaluate(() => window.TabulaResourceManifest.files['tabula-game-client.wasm']);
      const layout = await page.evaluate(() => {
        const c = document.querySelector('#glcanvas').getBoundingClientRect(), f = document.querySelector('.runtime-access').getBoundingClientRect();
        return { canvas: { x: c.x, y: c.y, width: c.width, height: c.height, bottom: c.bottom }, hostFooter: { top: f.top, bottom: f.bottom, width: f.width }, canvasPixels: { width: document.querySelector('#glcanvas').width, height: document.querySelector('#glcanvas').height }, theme: document.documentElement.dataset.theme };
      });
      assert(layout.canvas.bottom <= layout.hostFooter.top && layout.hostFooter.bottom <= height, 'Host controls overlap the canvas or viewport');
      assert(layout.canvasPixels.width === width * dpr && layout.canvasPixels.height === layout.canvas.height * dpr, 'Canvas backing dimensions do not match its DPR and CSS viewport');
      const g = geometry(layout.canvas.width, layout.canvas.height);
      const initial = await screenshot(page, `${name}-public`);
      const colors = new Set();
      for (let y = 20; y < layout.canvas.height - 20; y += 10) for (let x = 20; x < width - 20; x += 10) colors.add(pixel(initial.pixels, x * dpr, y * dpr).join(','));
      assert(colors.size > 30, 'Actual canvas is blank or has no rendered scene');
      steps.push({ action: 'startup and host footer layout', status: 'PASS', layout, sampled_colors: colors.size, screenshot: initial.filename });
      // A real public simulator control chooses seat0. No role assignment is inspected.
      await openedTools(page, g);
      const tools = await screenshot(page, `${name}-tools`);
      await key(page, 'ArrowRight'); // Tools opens focused on Previous; Next is its real neighbor.
      await key(page, 'Enter');
      const concealed = await screenshot(page, `${name}-seat-concealed`);
      await key(page, 'Enter'); // The perspective switch deliberately restores focus to Reveal.
      await page.waitForTimeout(650);
      const revealed = await screenshot(page, `${name}-revealed`);
      const revealDifference = differences(concealed.pixels, revealed.pixels, g.card, dpr);
      assert(revealDifference.fraction > .05, 'Keyboard Reveal produced no visible card/drawer change');
      steps.push({ action: 'open tools, ArrowRight/Enter next seat, Enter reveal', status: 'PASS', tools: tools.filename, screenshot: revealed.filename, card_difference: revealDifference });
      if (g.compact) {
        const close = g.landscape
          ? rect(g.card.x + g.card.width + 24, 116, width - g.card.x - g.card.width - 40, 44)
          : rect(g.dialog.x + 8, g.card.y + g.card.height + 64, g.dialog.width - 16, 44);
        await click(page, close);
        const closed = await screenshot(page, `${name}-drawer-closed`);
        const closeDifference = differences(revealed.pixels, closed.pixels, g.card, dpr);
        assert(closeDifference.fraction > .05, 'Explicit Close did not dismiss the private drawer');
        steps.push({ action: 'explicit drawer close returns to table', status: 'PASS', screenshot: closed.filename, drawer_difference: closeDifference });
        await key(page, 'Enter');
      }
      await key(page, 'Escape');
      const escaped = await screenshot(page, `${name}-escape-concealed`);
      const escapedDifference = differences(concealed.pixels, escaped.pixels, g.card, dpr);
      assert(escapedDifference.fraction < .02, 'Escape did not restore the concealed card/table pixels');
      steps.push({ action: 'Escape conceals role and closes drawer', status: 'PASS', screenshot: escaped.filename, concealed_difference: escapedDifference });
      await key(page, 'Enter');
      await page.waitForTimeout(400);
      await page.locator('#help').focus();
      const shield = await page.evaluate(() => {
        const s = document.querySelector('#privacy-shield'), r = s.getBoundingClientRect(), c = document.querySelector('#glcanvas').getBoundingClientRect();
        return { visible: !s.hidden, opacity: getComputedStyle(s).opacity, coversCanvas: r.left <= c.left && r.top <= c.top && r.right >= c.right && r.bottom >= c.bottom };
      });
      assert(shield.visible && shield.opacity === '1' && shield.coversCanvas, 'Focus loss did not synchronously install an opaque covering privacy surface');
      const shieldShot = await screenshot(page, `${name}-focus-concealed`);
      await page.locator('#resume-private').click();
      await page.waitForFunction(() => document.querySelector('#privacy-shield').hidden);
      await page.waitForTimeout(150);
      const resumed = await screenshot(page, `${name}-resumed-concealed`);
      const resumeDifference = differences(concealed.pixels, resumed.pixels, g.card, dpr);
      assert(resumeDifference.fraction < .02, 'Focus return restored a revealed card without a fresh reveal');
      steps.push({ action: 'focus loss shields immediately; resume stays concealed', status: 'PASS', shield, screenshot: shieldShot.filename, resume: resumed.filename, concealed_difference: resumeDifference });
      // Public controls advance Night → Dawn → Day → Vote without reading authority internals.
      for (let advance = 0; advance < 3; advance++) {
        await openedTools(page, g);
        const w = Math.max((g.dialog.width - 32) / 2, 44);
        await click(page, rect(g.dialog.x + 12 + w + 8, g.dialog.y + 70 + 52, w, 44));
      }
      await key(page, 'Tab');
      await key(page, 'Enter'); // First enabled target from the concealed Reveal focus.
      const selected = await screenshot(page, `${name}-target-selected`);
      const focusCss = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--sys-focus-ring-color').trim());
      assert(/^#[a-f\d]{6}$/i.test(focusCss), `Unsupported generated focus token: ${focusCss}`);
      const focusRgb = [1, 3, 5].map(at => Number.parseInt(focusCss.slice(at, at + 2), 16));
      const submit = rect(g.dock.x + 8, g.dock.y + 54, Math.max((g.dock.width - 24) / 2, 44), 44);
      let submitFocused = false, tabCount = 0;
      for (; tabCount < 18; tabCount++) {
        await key(page, 'Tab');
        const pixels = PNG.sync.read(await page.screenshot());
        const at = pixel(pixels, (submit.x - 3) * dpr, (submit.y + 22) * dpr);
        if (at.every((value, index) => Math.abs(value - focusRgb[index]) <= 8)) { submitFocused = true; break; }
      }
      assert(submitFocused, 'Keyboard navigation never produced the actual Submit focus ring');
      const beforeSubmit = await screenshot(page, `${name}-submit-focused`);
      await key(page, 'Enter');
      await page.waitForTimeout(650);
      const submitted = await screenshot(page, `${name}-keyboard-submitted`);
      const boardDifference = differences(beforeSubmit.pixels, submitted.pixels, g.table, dpr);
      assert(boardDifference.changed > 20 * dpr * dpr, 'Keyboard submission produced no visible public-board consequence');
      steps.push({ action: 'public phase controls, keyboard target and keyboard vote submit', status: 'PASS', selected: selected.filename, submit_focused: beforeSubmit.filename, screenshot: submitted.filename, tabs_to_submit: tabCount + 1, public_board_difference: boardDifference });
      assert(errors.length === 0, `Page errors: ${errors.join('; ')}`);
      const gameErrors = consoleMessages.filter(message => message.type === 'error' && message.url !== `${origin}/favicon.ico`);
      assert(gameErrors.length === 0, `Console errors: ${JSON.stringify(gameErrors)}`);
      receipt.document_residuals = consoleMessages.filter(message => message.url === `${origin}/favicon.ico`); // Unowned browser-default favicon request; essential resources still fail.
      receipt.status = 'PASS';
    } catch (error) {
      receipt.status = 'FAIL'; receipt.error = error.message;
      try { receipt.failure_screenshot = (await screenshot(page, `${name}-failure`)).filename; } catch { /* retain original failure */ }
    } finally {
      receipt.elapsed_ms = Date.now() - started;
      receipt.errors = errors; receipt.console_messages = consoleMessages;
      receipt.captures = captures.filter(capture => capture.name.startsWith(name));
      receipts.push(receipt);
      await fs.writeFile(path.join(runDir, `${name}.json`), `${JSON.stringify(receipt, null, 2)}\n`);
      console.log(JSON.stringify({ name, status: receipt.status, error: receipt.error, elapsed_ms: receipt.elapsed_ms }));
      await context.close();
    }
  }
  // Additional screenshots exercise only the documented local simulator controls. Their
  // phase/death/outcome meaning is deliberately left for an explicit screenshot inspection.
  if (process.env.TABULA_SKIP_EXTRAS !== '1') {
    const extraCases = process.env.TABULA_EXTRA_CASES ? new Set(process.env.TABULA_EXTRA_CASES.split(',')) : null;
    for (const theme of ['light', 'hc-light', 'hc-dark']) {
      if (extraCases && !extraCases.has(`theme-${theme}`)) continue;
      const context = await browser.newContext({ viewport: { width: 1200, height: 880 }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      try {
        await page.goto(`${origin}/play.html?game=werewolf&mode=simulator&seats=12&theme=${theme}&motion=reduced&locale=vi`);
        await page.waitForFunction(() => document.querySelector('#loader').hidden || !document.querySelector('#runtime-error').hidden, null, { timeout: 45000 });
        assert(await page.locator('#runtime-error').isHidden(), await page.locator('#error-detail').textContent());
        await page.locator('#glcanvas').focus();
        await page.waitForFunction(() => document.querySelector('#privacy-shield').hidden);
        await page.waitForTimeout(300);
        const resolved = await page.evaluate(() => document.documentElement.dataset.theme);
        assert(resolved === theme, 'Runtime theme differs from requested public preference');
        const shot = await screenshot(page, `theme-${theme}`);
        assert(errors.length === 0, `Theme runtime errors: ${errors.join('; ')}`);
        receipts.push({ name: `theme-${theme}`, status: 'PASS', evidence: 'screenshot-captured', theme: resolved, screenshot: shot.filename, runtime_artifact: await page.evaluate(() => window.TabulaResourceManifest.files['tabula-game-client.wasm']), errors });
      } catch (error) { receipts.push({ name: `theme-${theme}`, status: 'FAIL', error: error.message, errors }); }
      await context.close();
    }
    if (!extraCases || extraCases.has('public-phase-captures')) {
      const context = await browser.newContext({ viewport: { width: 1200, height: 880 }, deviceScaleFactor: 1, reducedMotion: 'reduce' });
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      try {
        await page.goto(`${origin}/play.html?game=werewolf&mode=simulator&seats=12&theme=dark&motion=reduced&locale=vi`);
        await page.waitForFunction(() => document.querySelector('#loader').hidden || !document.querySelector('#runtime-error').hidden, null, { timeout: 45000 });
        assert(await page.locator('#runtime-error').isHidden(), await page.locator('#error-detail').textContent());
        await page.locator('#glcanvas').focus();
        await page.waitForFunction(() => document.querySelector('#privacy-shield').hidden);
        await page.waitForTimeout(300);
        const bounds = await page.locator('#glcanvas').boundingBox();
        const g = geometry(bounds.width, bounds.height);
        const w = Math.max((g.dialog.width - 32) / 2, 44);
        const nextSeat = rect(g.dialog.x + 12 + w + 8, g.dialog.y + 70, w, 44);
        const publicView = rect(g.dialog.x + 12, g.dialog.y + 70 + 52, w, 44);
        const advance = rect(g.dialog.x + 12 + w + 8, g.dialog.y + 70 + 52, w, 44);
        const deadline = async () => { await openedTools(page, g); await click(page, advance); };
        await deadline();
        const dawn = await screenshot(page, 'phase-public-dawn');
        // Explicit deadlines reach Day and then Vote; no hidden state or timer manipulation.
        await deadline(); await deadline();
        const count = 12, index = 1;
        const angle = -Math.PI / 2 + index * Math.PI * 2 / count;
        const seatWidth = Math.min(90, Math.max(62, g.table.width / 7));
        const seatHeight = Math.min(100, Math.max(44, (g.table.height - 66) / 5));
        const center = { x: g.table.x + g.table.width / 2, y: g.table.y + g.table.height / 2 };
        const radius = { x: (g.table.width - seatWidth - 40) / 2, y: (g.table.height - seatHeight - 66) / 2 };
        const target = rect(center.x + radius.x * Math.cos(angle) - seatWidth / 2, center.y + radius.y * Math.sin(angle) - seatHeight / 2, seatWidth, seatHeight);
        const submit = rect(g.dock.x + 8, g.dock.y + 54, Math.max((g.dock.width - 24) / 2, 44), 44);
        const votingOperators = [];
        for (let operator = 0; operator < 8; operator++) {
          await openedTools(page, g); await click(page, nextSeat);
          if (operator === index) continue; // Never rely on a self-vote being legal.
          await click(page, target); await click(page, submit);
          votingOperators.push(`Người ${operator + 1}`);
        }
        await openedTools(page, g); await click(page, publicView);
        const tally = await screenshot(page, 'phase-public-vote-tally');
        await deadline();
        const death = await screenshot(page, 'phase-public-death');
        // Dusk is intentionally only two seconds. Advance to the long Night phase before
        // checking private pagination, so an authority deadline cannot masquerade as paging.
        await deadline();
        // The public vote consequence identifies the eliminated seat. Choose it through the
        // same real operator controls, then inspect only its authorized private drawer.
        for (let next = 0; next < 2; next++) { await openedTools(page, g); await click(page, nextSeat); }
        await page.setViewportSize({ width: 390, height: 844 });
        await page.waitForTimeout(200);
        const deadBounds = await page.locator('#glcanvas').boundingBox();
        const deadGeometry = geometry(deadBounds.width, deadBounds.height);
        const deadConcealed = await screenshot(page, 'phase-dead-concealed');
        await key(page, 'Enter');
        const deadDrawer = await screenshot(page, 'phase-dead-private-drawer');
        assert(differences(deadConcealed.pixels, deadDrawer.pixels, deadGeometry.card, 1).fraction > .05, 'Eliminated player Reveal did not visibly open the authorized drawer');
        await click(page, rect(deadGeometry.dialog.x + 8, deadBounds.height - 52, deadGeometry.dialog.width - 16, 44));
        const deadPaged = await screenshot(page, 'phase-dead-private-page2');
        const infoTop = deadGeometry.card.y + deadGeometry.card.height + 112;
        const deadPageDifference = differences(deadDrawer.pixels, deadPaged.pixels,
          rect(deadGeometry.dialog.x + 8, infoTop, deadGeometry.dialog.width - 16, Math.max(deadBounds.height - infoTop - 56, 44)), 1);
        assert(deadPageDifference.changed > 10, 'Eliminated player information pagination produced no visible text change');
        const deadPagedCardDifference = differences(deadDrawer.pixels, deadPaged.pixels, deadGeometry.card, 1);
        assert(deadPagedCardDifference.fraction < .02, 'Pagination changed/closed the private card instead of only paging its information');
        await page.locator('#help').focus();
        assert(await page.locator('#privacy-shield').isVisible(), 'Eliminated player drawer did not conceal on blur');
        await page.locator('#resume-private').click();
        await page.waitForFunction(() => document.querySelector('#privacy-shield').hidden);
        await page.waitForTimeout(100);
        const deadResumed = await screenshot(page, 'phase-dead-resumed-concealed');
        const deadResumeDifference = differences(deadConcealed.pixels, deadResumed.pixels, deadGeometry.card, 1);
        assert(deadResumeDifference.fraction < .02, 'Eliminated player focus return restored private roster without fresh reveal');
        const deadDialogWidth = Math.max((deadGeometry.dialog.width - 32) / 2, 44);
        await openedTools(page, deadGeometry);
        await click(page, rect(deadGeometry.dialog.x + 12, deadGeometry.dialog.y + 122, deadDialogWidth, 44));
        await page.setViewportSize({ width: 1200, height: 880 });
        await page.waitForTimeout(200);
        // max_rounds=10. Additional requests after terminal are ordinary disabled controls.
        // Record attempted controls, never equate them with accepted authority transitions.
        for (let attempt = 0; attempt < 50; attempt++) { await deadline(); await key(page, 'Escape'); }
        const terminal = await screenshot(page, 'phase-public-terminal');
        assert(errors.length === 0, `Phase runtime errors: ${errors.join('; ')}`);
        receipts.push({ name: 'public-phase-captures', status: 'PASS', evidence: 'screenshot-captured-and-privacy-input-assertions', semantic_verdict: 'Dawn, vote/death, authorized dead drawer and terminal labels require separate screenshot inspection; no internal state queried', attempted_vote_operators: votingOperators, public_vote_target: 'Người 2', attempted_deadline_controls: 55, eliminated_drawer: { viewport: [390, 844], scope: 'Chosen with real NextSeat controls after public elimination, then opened during long Night phase', concealed: deadConcealed.filename, drawer: deadDrawer.filename, paginated: deadPaged.filename, page_difference: deadPageDifference, paged_card_difference: deadPagedCardDifference, resumed_concealed: deadResumed.filename, resumed_difference: deadResumeDifference }, screenshots: { dawn: dawn.filename, public_tally: tally.filename, public_death: death.filename, terminal: terminal.filename }, runtime_artifact: await page.evaluate(() => window.TabulaResourceManifest.files['tabula-game-client.wasm']), errors });
      } catch (error) { receipts.push({ name: 'public-phase-captures', status: 'FAIL', error: error.message, errors }); }
      await context.close();
    }
    if (!extraCases || extraCases.has('full-motion')) {
      const motionContext = await browser.newContext({ viewport: { width: 1200, height: 880 }, deviceScaleFactor: 1, reducedMotion: 'no-preference' });
      const motionPage = await motionContext.newPage();
      const motionErrors = [];
      motionPage.on('pageerror', error => motionErrors.push(error.message));
      try {
        await motionPage.goto(`${origin}/play.html?game=werewolf&mode=simulator&seats=12&theme=dark&motion=system&locale=vi`);
        await motionPage.waitForFunction(() => document.querySelector('#loader').hidden || !document.querySelector('#runtime-error').hidden, null, { timeout: 45000 });
        assert(await motionPage.locator('#runtime-error').isHidden(), await motionPage.locator('#error-detail').textContent());
        await motionPage.locator('#glcanvas').focus();
        await motionPage.waitForFunction(() => document.querySelector('#privacy-shield').hidden);
        assert(!await motionPage.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches), 'Full-motion context unexpectedly requests reduced motion');
        await motionPage.waitForTimeout(600);
        const bounds = await motionPage.locator('#glcanvas').boundingBox(), g = geometry(bounds.width, bounds.height);
        await openedTools(motionPage, g); await key(motionPage, 'ArrowRight'); await key(motionPage, 'Enter');
        const revealStarted = Date.now();
        await motionPage.mouse.click(g.reveal.x + g.reveal.width / 2, g.reveal.y + 22, { delay: 20 });
        const early = await screenshot(motionPage, 'motion-reveal-early', g.card), earlyMs = Date.now() - revealStarted;
        await motionPage.waitForTimeout(70);
        const middle = await screenshot(motionPage, 'motion-reveal-mid', g.card), middleMs = Date.now() - revealStarted;
        await motionPage.waitForTimeout(600);
        const settled = await screenshot(motionPage, 'motion-reveal-settled', g.card), settledMs = Date.now() - revealStarted;
        const revealDifference = differences(early.pixels, settled.pixels, rect(0, 0, g.card.width, g.card.height), 1);
        assert(revealDifference.fraction > .02, 'Full-motion reveal early and settled card pixels are identical');
        await key(motionPage, 'Escape');
        await openedTools(motionPage, g);
        const w = Math.max((g.dialog.width - 32) / 2, 44);
        const advance = rect(g.dialog.x + 12 + w + 8, g.dialog.y + 122, w, 44);
        const phaseStarted = Date.now();
        await motionPage.mouse.click(advance.x + advance.width / 2, advance.y + 22, { delay: 20 });
        const phaseEarly = await screenshot(motionPage, 'motion-dawn-start'), phaseEarlyMs = Date.now() - phaseStarted;
        await motionPage.waitForTimeout(650);
        const phaseSettled = await screenshot(motionPage, 'motion-dawn-settled'), phaseSettledMs = Date.now() - phaseStarted;
        const phaseDifference = differences(phaseEarly.pixels, phaseSettled.pixels, g.table, 1);
        assert(phaseDifference.fraction > .02, 'Full-motion phase start and settled board pixels are identical');
        // Start another reveal and immediately blur through a real host control. The host
        // shield and resumed concealed pixels are the observable cancellation contract.
        await motionPage.mouse.click(g.reveal.x + g.reveal.width / 2, g.reveal.y + 22, { delay: 20 });
        await motionPage.locator('#help').focus();
        assert(await motionPage.locator('#privacy-shield').isVisible(), 'Full-motion blur did not immediately shield the private scene');
        const blur = await screenshot(motionPage, 'motion-blur-shield');
        await motionPage.locator('#resume-private').click();
        await motionPage.waitForFunction(() => document.querySelector('#privacy-shield').hidden);
        await motionPage.waitForTimeout(150);
        const resumed = await screenshot(motionPage, 'motion-resumed-concealed');
        const motionResumeDifference = differences(phaseSettled.pixels, resumed.pixels, g.card, 1);
        assert(motionResumeDifference.fraction < .02, 'Full-motion resume restored private role pixels without fresh reveal');
        assert(motionErrors.length === 0, `Full-motion runtime errors: ${motionErrors.join('; ')}`);
        receipts.push({ name: 'full-motion', status: 'PASS', scope: 'Actual browser pixel changes and blur shielding only; capture times are wall-clock samples, not a frame pacing benchmark', preference: { query: 'motion=system', browser: 'no-preference' }, reveal_samples_ms: [earlyMs, middleMs, settledMs], phase_samples_ms: [phaseEarlyMs, phaseSettledMs], reveal_difference: revealDifference, phase_difference: phaseDifference, resumed_difference: motionResumeDifference, screenshots: { early: early.filename, middle: middle.filename, settled: settled.filename, phase_start: phaseEarly.filename, phase_settled: phaseSettled.filename, blur: blur.filename, resumed: resumed.filename }, runtime_artifact: await motionPage.evaluate(() => window.TabulaResourceManifest.files['tabula-game-client.wasm']), errors: motionErrors });
      } catch (error) { receipts.push({ name: 'full-motion', status: 'FAIL', error: error.message, errors: motionErrors }); }
      await motionContext.close();
    }
  }

} finally {
  await browser.close();
  for (const receipt of receipts) {
    receipt.captures ??= captures.filter(capture => capture.name.startsWith(receipt.name === 'public-phase-captures'
      ? 'phase-' : receipt.name === 'full-motion' ? 'motion-' : receipt.name));
    await fs.writeFile(path.join(runDir, `${receipt.name}.json`), `${JSON.stringify(receipt, null, 2)}\n`);
  }
  await fs.writeFile(path.join(runDir, 'summary.json'), `${JSON.stringify({ origin, run_at: new Date().toISOString(), browser: 'Google Chrome, disposable headless profiles', browser_version: browserVersion, temporary_captures: screenshotTempDir, captures, cases: receipts, passed: receipts.filter(receipt => receipt.status === 'PASS').length, failed: receipts.filter(receipt => receipt.status === 'FAIL').length }, null, 2)}\n`);
}
if (receipts.some(receipt => receipt.status !== 'PASS')) process.exitCode = 1;
