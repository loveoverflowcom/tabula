// Tool-only lifecycle shell. Authority is the Rust example behind /authority.
import { mount as mountPixi } from './adapter.mjs';
import { createBridge } from './bridge.mjs';
import { exact, requireThat, validateViewport, validatePreferences, validateInput } from './boundary.mjs';

const harnessStarted = performance.now();
const session = crypto.randomUUID();
let generation = 0;
let current = null;
let modalFocus = null;
let authorityTail = Promise.resolve();
const history = [];
const dataset = Promise.all(['fixture.json', 'render-contract.json', 'runtime-assets.json'].map(async path => {
  const response = await fetch(path, { credentials: 'omit' });
  if (!response.ok) throw new Error(`${path}: HTTP ${response.status}`);
  return response.json();
})).then(([fixture, contract, runtime]) => ({ fixture, contract, runtime }));

function container() {
  const value = document.querySelector('#renderer-container');
  if (!value) throw new Error('renderer container has not mounted');
  return value;
}
function status(text) {
  const output = document.querySelector('#spike-status');
  if (output) output.textContent = text;
}
function error(state, value) {
  state.errors.push({ code: value.code ?? 'host_error', message: String(value.message ?? value).slice(0, 1024) });
  if (state.controller?.snapshot().status === 'failed' || ['context_lost', 'context_restored_remount_required', 'baseline_failure', 'ready_timeout'].includes(value.code)) state.status = 'failed';
  if (current === state) status(`${state.kind}: ${state.errors.at(-1).code}`);
}
function enqueueAuthority(task) {
  const promise = authorityTail.then(task);
  authorityTail = promise.catch(() => {});
  return promise;
}
async function requestAuthority(value) {
  const response = await fetch('/authority', { method: 'POST', credentials: 'omit', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(value) });
  if (!response.ok) throw new Error(`fixture authority: HTTP ${response.status}`);
  const result = await response.json();
  if (!result.ok) throw new Error(result.error ?? 'fixture authority rejected request');
  return result;
}
function identityFields(state) { return { session_id: state.identity.session_id, generation: state.identity.generation }; }
function authorityOperation(state, fields, inputReceipt = {}, automaticTime = false) {
  return enqueueAuthority(async () => {
    if (!state.active || current !== state) return { cancelled: true };
    // A callback belongs to the permitted frame visible when it was admitted.
    // Never reinterpret queued raw input after an earlier Rust command changes
    // its checkpoint. Same-checkpoint focus/pointer callbacks keep FIFO order.
    if (fields.op === 'input' && Object.hasOwn(inputReceipt, 'observed_checkpoint') && inputReceipt.observed_checkpoint !== state.latestFrame?.checkpoint) {
      state.dropped_callback_inputs++;
      const drop = { op: 'input', outcome: 'dropped_stale_checkpoint', elapsed_ms: 0,
        request_revision: state.authorityRevision, response_revision: state.authorityRevision,
        checkpoint: state.latestFrame?.checkpoint ?? null, ...inputReceipt };
      state.authority.push(drop);
      if (state.authority.length > 10000) state.authority.shift();
      return { cancelled: true, dropped: true, reason: 'stale_checkpoint', revision: state.authorityRevision };
    }
    const begun = performance.now();
    const request = { ...fields, ...identityFields(state), revision: state.authorityRevision };
    if (automaticTime) request.now_ms = presentationTime(state);
    const result = await requestAuthority(request);
    // A late accepted response still advances Rust's session revision. Teardown
    // follows this operation in the queue and must dispose that exact revision.
    state.authorityRevision = result.envelope.revision;
    if (!state.active || current !== state) return { cancelled: true };
    state.controller.set_view(result.envelope);
    state.authority.push({ op: fields.op, elapsed_ms: performance.now() - begun, request_revision: result.envelope.revision - 1, response_revision: result.envelope.revision, outcome: result.outcome, checkpoint: result.envelope.frame.checkpoint, ...inputReceipt });
    if (state.authority.length > 10000) state.authority.shift();
    state.latestFrame = result.envelope.frame;
    return result;
  });
}
function presentationTime(state, requested) {
  if (requested !== undefined) return Math.max(0, Math.round(requested));
  return Math.max(state.latestFrame?.at_ms ?? 0, 3000 + Math.round(performance.now() - state.started_ms));
}
function frameFields(state, now_ms) {
  return { op: 'frame', now_ms: presentationTime(state, now_ms), viewport: [state.viewport.width, state.viewport.height], dpi: state.viewport.dpi, theme: state.theme, reduced_motion: state.preferences.reduced_motion };
}
function pumpMotion(state) {
  if (!state.active || state.scenario !== 'interactive') return;
  state.pump_until = performance.now() + 500;
  if (state.motionTimer !== null) return;
  const tick = async () => {
    state.motionTimer = null;
    if (!state.active || current !== state) return;
    try { await authorityOperation(state, frameFields(state), {}, true); }
    catch (cause) { error(state, cause); return; }
    if (state.active && current === state && performance.now() < state.pump_until) state.motionTimer = setTimeout(tick, 50);
  };
  state.motionTimer = setTimeout(tick, 50);
}
function observedViewport() {
  const rect = container().getBoundingClientRect();
  return { width: rect.width - 2, height: rect.height - 2, dpi: devicePixelRatio };
}
function snapshot(state = current) {
  if (!state) return { status: 'unmounted', generation, shell: document.body.dataset.shell, iframes: container().querySelectorAll('iframe').length, canvases: container().querySelectorAll('canvas').length, host_listeners: 0, host_observers: 0, host_motion_timers: 0, host_ready_timers: 0, harness_document_listeners: 2, match_sockets: 0 };
  return structuredClone({
    kind: state.kind, status: state.status, identity: state.identity, shell: document.body.dataset.shell,
    boot_ms: state.boot_ms, viewport: state.viewport, preferences: state.preferences, scenario: state.scenario, theme: state.theme,
    ready: state.readyReceipt, metrics: state.metrics, frame_samples: state.frames, authority: state.authority,
    errors: state.errors, checkpoint: state.latestFrame?.checkpoint ?? state.readyReceipt?.checkpoint ?? null,
    dropped_callback_inputs: state.dropped_callback_inputs,
    controller: state.controller?.snapshot() ?? null, bridge: state.bridge?.snapshot() ?? null,
    host_listeners: state.removers.length, host_observers: state.observer ? 1 : 0,
    host_motion_timers: state.motionTimer === null ? 0 : 1, harness_document_listeners: 2,
    host_ready_timers: state.readyTimer === null ? 0 : 1,
    iframes: container().querySelectorAll('iframe').length, canvases: container().querySelectorAll('canvas').length,
    pending_roundtrips: state.pings.size, match_sockets: 0, audio_fixture: false,
  });
}
function listen(state, target, name, handler) {
  target.addEventListener(name, handler);
  state.removers.push(() => target.removeEventListener(name, handler));
}
function remove(state) {
  if (!state?.active) return;
  state.active = false;
  const before = snapshot(state);
  clearTimeout(state.readyTimer);
  state.readyTimer = null;
  clearTimeout(state.motionTimer); state.motionTimer = null;
  state.focusInside ||= document.activeElement === state.frame || document.activeElement === state.controller?.canvas;
  state.cancelReady(new Error('mount cancelled by unmount'));
  for (const ping of state.pings.values()) { clearTimeout(ping.timer); ping.reject(new Error('unmounted')); }
  state.pings.clear();
  state.observer?.disconnect(); state.observer = null;
  while (state.removers.length) state.removers.pop()();
  if (state.kind === 'iframe') {
    try { state.bridge?.send('dispose'); } catch (cause) { error(state, cause); }
    state.bridge?.dispose();
    // The Macroquad loader has no owned disposal API. Destroy its browsing context.
    state.frame?.remove();
  } else {
    state.controller?.dispose();
    if (state.authorityInitialized) {
      enqueueAuthority(() => requestAuthority({ op: 'dispose', ...identityFields(state), revision: state.authorityRevision })).catch(cause => error(state, cause));
    }
  }
  state.status = 'disposed';
  if (current === state) current = null;
  const focused = document.activeElement;
  if ((focused === document.body || state.focusInside) && state.focusBefore?.isConnected) state.focusBefore.focus({ preventScroll: true });
  history.push({ before, after: snapshot(state) });
  if (history.length > 100) history.shift();
}
function dispose() {
  remove(current);
  status('Unmounted; graphics surface and per-mount observers removed.');
  return snapshot();
}
async function mount(kind, options = {}) {
  if (!['iframe', 'pixi'].includes(kind)) throw new Error('kind must be iframe or pixi');
  remove(current);
  const root = container();
  const begun = performance.now();
  const state = {
    kind, active: true, status: 'initializing', identity: { session_id: session, generation: ++generation },
    scenario: options.scenario ?? 'static', theme: options.theme ?? 'light',
    viewport: validateViewport(options.viewport ?? observedViewport()),
    preferences: validatePreferences({ reduced_motion: options.reduced_motion ?? false, audio_enabled: false, volume: 0, modal: false }),
    focusBefore: document.activeElement, focusInside: false, started_ms: begun, removers: [], observer: null, pings: new Map(), motionTimer: null, readyTimer: null, pump_until: 0,
    errors: [], metrics: [], frames: [], authority: [], authorityRevision: 0, authorityInitialized: false, dropped_callback_inputs: 0, input_sequence: 0,
    controller: null, bridge: null, readyReceipt: null, boot_ms: null, latestFrame: null,
  };
  current = state;
  status(`${kind}: initializing generation ${state.identity.generation}…`);
  let resolveReady;
  let rejectReady;
  const ready = new Promise((resolve, reject) => { resolveReady = resolve; rejectReady = reject; });
  // Consumers may intentionally dispose while initialization is pending.
  ready.catch(() => {});
  state.cancelReady = rejectReady;
  const timeout = setTimeout(() => rejectReady(new Error('renderer ready timeout after 30 seconds')), 30000);
  state.readyTimer = timeout;
  listen(state, window, 'pagehide', () => remove(state));
  function complete(value) {
    if (!state.active || current !== state) return;
    state.readyReceipt = value;
    state.boot_ms = performance.now() - begun;
    state.status = 'ready';
    clearTimeout(timeout);
    state.readyTimer = null;
    status(`${kind}: ready in ${state.boot_ms.toFixed(1)} ms; generation ${state.identity.generation}`);
    resolveReady(value);
  }
  try {
    if (kind === 'iframe') {
      const frame = document.createElement('iframe');
      state.frame = frame;
      frame.title = 'Isolated Macroquad game document';
      frame.src = 'macroquad.html';
      // Same-origin trusted code; allow-scripts+allow-same-origin is not a sandbox
      // against that child. No credential, match context or token goes in its URL.
      root.appendChild(frame);
      state.bridge = createBridge({ targetWindow: frame.contentWindow, origin: location.origin, identity: state.identity,
        onError: cause => error(state, cause),
        onMessage: message => {
          if (!state.active || current !== state) return;
          if (message.kind === 'ready') complete(message.payload);
          else if (message.kind === 'metrics') { state.metrics.push(message.payload); if (state.metrics.length > 200) state.metrics.shift(); }
          else if (message.kind === 'error') { const wasInitializing = state.status === 'initializing'; error(state, message.payload); if (wasInitializing) rejectReady(new Error(message.payload.message)); }
          else if (message.kind === 'focus_exit') {
            const targets = [...document.querySelectorAll('button,a[href],input,select,textarea,iframe,[tabindex="0"]')].filter(element => !element.disabled && element.getClientRects().length && !element.closest('[hidden],[inert]'));
            const position = targets.indexOf(frame);
            const next = targets[(position + (message.payload.backward ? -1 : 1) + targets.length) % targets.length];
            next?.focus({ preventScroll: true });
          }
          else if (message.kind === 'pong') {
            const ping = state.pings.get(message.payload.nonce);
            if (ping && ping.sent_ms === message.payload.sent_ms) { clearTimeout(ping.timer); state.pings.delete(message.payload.nonce); ping.resolve(performance.now() - ping.sent_ms); }
          }
        },
      });
      listen(state, frame, 'load', () => {
        if (!state.active || current !== state) return;
        state.bridge.send('init', { viewport: state.viewport, preferences: state.preferences });
      });
    } else {
      const { fixture, contract, runtime } = await dataset;
      if (!state.active || current !== state) throw new Error('mount cancelled');
      const assets = structuredClone(fixture.assets);
      for (const file of assets.files) {
        const staged = runtime.assets.find(asset => asset.name === file.name);
        if (!staged || staged.bytes !== file.bytes || staged.blake3 !== file.hash) throw new Error(`staged fixture mismatch: ${file.name}`);
        file.sha256 = staged.sha256;
        file.width = staged.width; file.height = staged.height;
      }
      state.controller = mountPixi({ container: root, identity: state.identity, contract, assets, themes: fixture.themes, viewport: state.viewport, preferences: state.preferences, theme: state.theme, font: runtime.font,
        on_error: cause => error(state, cause),
        on_frame: value => { state.frames.push(value); if (state.frames.length > 10000) state.frames.shift(); },
        on_input: value => {
          if (!state.active || current !== state) { state.dropped_callback_inputs++; return; }
          try {
            exact(value, ['session_id', 'generation', 'revision', 'input'], 'callback');
            requireThat(value.session_id === state.identity.session_id && value.generation === state.identity.generation, 'stale_identity', 'callback');
            requireThat(value.revision === state.controller.snapshot().revision, 'stale_revision', 'callback');
            validateInput(value.input);
            // Admission binds revision and checkpoint to this permitted frame;
            // execution rechecks the checkpoint before choosing a Rust revision.
            const receipt = { observed_revision: value.revision, observed_checkpoint: state.latestFrame.checkpoint, input_sequence: ++state.input_sequence };
            receipt.input_kind = value.input.kind;
            if (value.input.phase !== undefined) receipt.phase = value.input.phase;
            if (value.input.pressed !== undefined) receipt.pressed = value.input.pressed;
            authorityOperation(state, { op: 'input', now_ms: presentationTime(state), input: value.input }, receipt, true).then(result => { if (!result.cancelled) pumpMotion(state); }).catch(cause => error(state, cause));
          } catch (cause) { state.dropped_callback_inputs++; error(state, cause); }
        },
      });
      await state.controller.ready;
      if (!state.active || current !== state) throw new Error('mount cancelled');
      if (state.controller.snapshot().status !== 'ready') throw new Error(state.errors.at(-1)?.message ?? 'Pixi failed to initialize');
      const result = await enqueueAuthority(() => requestAuthority({ op: 'init', ...identityFields(state), revision: 0, viewport: [state.viewport.width, state.viewport.height], dpi: state.viewport.dpi, theme: state.theme, reduced_motion: state.preferences.reduced_motion, scenario: state.scenario }));
      state.authorityInitialized = true;
      if (!state.active || current !== state) {
        enqueueAuthority(() => requestAuthority({ op: 'dispose', ...identityFields(state), revision: result.envelope.revision })).catch(cause => error(state, cause));
        throw new Error('mount cancelled');
      }
      state.latestFrame = result.envelope.frame;
      state.authorityRevision = result.envelope.revision;
      state.controller.set_view(result.envelope);
      // The static corpus and #59 warm steady capture begin after initial motion.
      const settled = await authorityOperation(state, frameFields(state, 3000));
      if (!state.active || current !== state) throw new Error('mount cancelled');
      complete({ renderer: 'pixi', checkpoint: settled.envelope.frame.checkpoint });
    }
    listen(state, window, 'message', event => {
      if (state.frame && event.source === state.frame.contentWindow && event.origin === location.origin) state.focusInside = document.activeElement === state.frame;
    });
    state.observer = new ResizeObserver(() => {
      if (state.active && state.status === 'ready') {
        const actual = observedViewport();
        if (actual.width !== state.viewport.width || actual.height !== state.viewport.height || actual.dpi !== state.viewport.dpi) resize(actual.width, actual.height, actual.dpi);
      }
    });
    state.observer.observe(root);
    await ready;
    return snapshot(state);
  } catch (cause) {
    clearTimeout(timeout);
    error(state, cause);
    remove(state);
    throw cause;
  }
}
function resize(width, height, dpi = devicePixelRatio) {
  const viewport = validateViewport({ width, height, dpi });
  const root = container();
  root.style.width = `${width}px`; root.style.height = `${height}px`;
  if (current?.active) {
    current.viewport = viewport;
    if (current.kind === 'iframe') current.bridge.send('resize', viewport);
    else { const state = current; state.controller.resize(viewport); advance().catch(cause => error(state, cause)); }
  }
  return snapshot();
}
function preferences(value) {
  if (!current?.active) throw new Error('renderer is unmounted');
  const previous = current.preferences;
  current.preferences = validatePreferences(value);
  modalDom(value.modal);
  if (current.kind === 'iframe') {
    if (value.modal) current.bridge.send('suspend'); else current.bridge.send('resume');
    if (value.modal) document.querySelector('#spike-modal:not([hidden]) button,main button')?.focus({ preventScroll: true });
    // Compile-time motion remains explicit instead of a false runtime preference ack.
    if (value.reduced_motion !== previous.reduced_motion || value.audio_enabled !== previous.audio_enabled || value.volume !== previous.volume) current.bridge.send('preferences', value);
  } else { const state = current; state.controller.set_preferences(value); advance().catch(cause => error(state, cause)); }
  return snapshot();
}
function advance(now_ms) {
  const state = current;
  if (!state?.active || state.kind !== 'pixi') return Promise.reject(new Error('Rust view advance requires the Pixi prototype'));
  return authorityOperation(state, frameFields(state, now_ms), {}, now_ms === undefined);
}
function input(value, now_ms) {
  const state = current;
  validateInput(value);
  if (!state?.active || state.kind !== 'pixi') return Promise.reject(new Error('raw input requires the Pixi prototype'));
  return authorityOperation(state, { op: 'input', now_ms: presentationTime(state, now_ms), input: value }, { input_kind: value.kind, ...(value.phase === undefined ? {} : { phase: value.phase }), ...(value.pressed === undefined ? {} : { pressed: value.pressed }) }, now_ms === undefined).then(result => { if (!result.cancelled) pumpMotion(state); return result; });
}
function set_view(frame, revision) {
  if (current?.kind !== 'pixi') throw new Error('set_view requires Pixi');
  current.controller.set_view({ schema_version: 1, ...identityFields(current), revision, frame });
  current.latestFrame = frame;
  return snapshot();
}
async function cycles(kind, count = 50) {
  if (!Number.isInteger(count) || count < 1 || count > 100) throw new Error('cycle count must be 1..100');
  const result = [];
  for (let i = 0; i < count; i++) {
    const mounted = await mount(kind);
    const state = current;
    const deadline = performance.now() + 2000;
    let observedFrames = 0;
    while (performance.now() < deadline && state.active) {
      if (kind === 'pixi') observedFrames = state.frames.filter(value => value.rendered_revision === state.authorityRevision).length;
      else observedFrames = state.metrics.find(value => value.name === 'rendered')?.value.minimum_completed_frames ?? 0;
      if (observedFrames > 0) break;
      await new Promise(resolve => requestAnimationFrame(resolve));
    }
    if (observedFrames < 1) { dispose(); throw new Error('cycle renderer produced no observed frame within two seconds'); }
    dispose();
    const last = history.at(-1);
    result.push({ cycle: i + 1, generation: mounted.identity.generation, boot_ms: mounted.boot_ms, ready: mounted.ready, frames_before_dispose: observedFrames, frame_count_kind: kind === 'pixi' ? 'exact_settled_revision_adapter_callbacks' : 'child_raf_receipt_lower_bound', controller_after: last.after.controller, bridge_after: last.after.bridge, host_listeners_after: last.after.host_listeners, host_observers_after: last.after.host_observers, host_motion_timers_after: last.after.host_motion_timers, host_ready_timers_after: last.after.host_ready_timers, canvases_after: last.after.canvases, iframes_after: last.after.iframes });
  }
  await authorityTail;
  return { kind, requested: count, completed: result.length, cycles: result, final: snapshot(), resources_claim: 'Owned listeners/RAF/context DOM only; no process/GPU reclamation claim' };
}
async function roundtrips(count = 100) {
  const state = current;
  if (!state?.active || state.kind !== 'iframe' || state.status !== 'ready') throw new Error('ready iframe required');
  if (!Number.isInteger(count) || count < 1 || count > 1000) throw new Error('roundtrip count must be 1..1000');
  const samples = [];
  for (let nonce = 0; nonce < count; nonce++) {
    samples.push(await new Promise((resolve, reject) => {
      const sent_ms = performance.now();
      const timer = setTimeout(() => { state.pings.delete(nonce); reject(new Error('bridge ping timeout')); }, 5000);
      state.pings.set(nonce, { sent_ms, timer, resolve, reject });
      state.bridge.send('ping', { nonce, sent_ms });
    }));
  }
  return { kind: 'postmessage_roundtrip', unit: 'ms', samples, includes: 'Host→child→host task dispatch; no Rust input or paint' };
}
function modal() {
  const open = document.querySelector('#spike-modal').hidden;
  if (current?.active) preferences({ ...current.preferences, modal: open });
  else modalDom(open);
}
function modalDom(open) {
  const dialog = document.querySelector('#spike-modal');
  const changed = dialog.hidden === open;
  if (open && changed) modalFocus = document.activeElement;
  dialog.hidden = !open;
  document.querySelector('main').inert = open;
  if (open) dialog.querySelector('button').focus({ preventScroll: true });
  else if (changed && modalFocus?.isConnected) modalFocus.focus({ preventScroll: true });
}
function action(value) {
  if (value.startsWith('mount:')) mount(value.slice(6)).catch(cause => status(cause.message));
  else if (value === 'dispose') dispose();
  else if (value === 'focus') { if (current?.kind === 'iframe') current.bridge.send('focus_request'); else current?.controller && container().querySelector('canvas')?.focus(); }
  else if (value === 'modal') modal();
  else if (value === 'motion' && current?.active) preferences({ ...current.preferences, reduced_motion: !current.preferences.reduced_motion });
}
window.tabulaSpikeAction = action;
window.spikeHarness = window.tabulaSpike = {
  mount, dispose, report: snapshot, cycles, roundtrips, resize, preferences, advance, input, set_view,
  theme(value) { if (!current?.active || current.kind !== 'pixi') throw new Error('live theme requires Pixi'); current.theme = value; current.controller.set_theme(value); return advance(); },
  suspend() { if (current?.kind === 'iframe') current.bridge.send('suspend'); else current?.controller?.suspend(); },
  resume() { if (current?.kind === 'iframe') current.bridge.send('resume'); else current?.controller?.resume(); },
  history: () => structuredClone(history),
  settled: async () => { await authorityTail; return snapshot(); },
  flush: async () => { await authorityTail; return snapshot(); },
  environment: () => ({ user_agent: navigator.userAgent, device_pixel_ratio: devicePixelRatio, js_heap_bytes: performance.memory?.usedJSHeapSize ?? null, shell: document.body.dataset.shell, elapsed_ms: performance.now() - harnessStarted }),
};
document.addEventListener('click', event => { const button = event.target.closest?.('[data-action]'); if (button) action(button.dataset.action); });
document.addEventListener('keydown', event => {
  const dialog = document.querySelector('#spike-modal');
  if (dialog && !dialog.hidden && event.key === 'Tab') {
    event.preventDefault();
    dialog.querySelector('button').focus({ preventScroll: true });
  }
});
dataset.then(() => status('Experiment ready. Mount a renderer to begin.')).catch(cause => status(cause.message));
