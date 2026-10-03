// #60 wrapper around the pinned #59 executable, never a JS game implementation.
import { createBridge, validateEnvelope, payloadValidators } from './bridge.mjs';

const embedded = window.parent !== window;
const canvas = document.querySelector('#glcanvas');
const begun = performance.now();
const receipts = [];
const observations = [];
const errors = [];
let bridge = null;
let identity = null;
let started = false;
let disposed = false;
let suspended = false;
let ready = false;
let interval = null;
let timeout = null;
let readyFrame = null;
let pauses = 0;
let resumes = 0;
let rejected = 0;
const nativeConsole = Object.fromEntries(['log', 'info', 'warn', 'error'].map(key => [key, console[key].bind(console)]));
const gatedInputs = ['keydown', 'keyup', 'keypress', 'mousedown', 'mouseup', 'mousemove', 'wheel', 'touchstart', 'touchend', 'touchcancel', 'touchmove'];

function emit(kind, payload) {
  if (bridge && !disposed) bridge.send(kind, payload);
}
function metric(name, value) { emit('metrics', { name, value }); }
function fail(code, message) {
  const value = { code, message: String(message).slice(0, 1024) };
  errors.push(value);
  emit('error', value);
}
function flattenReceipt(value) {
  const flat = {};
  for (const [key, item] of Object.entries(value)) {
    if (typeof item === 'number' || typeof item === 'boolean' || typeof item === 'string') flat[key] = item;
    else if (Array.isArray(item) && item.every(n => typeof n === 'number')) flat[key] = item;
    else if (key === 'asset_hashes') flat.asset_hashes = item.join(',');
    else if (item && typeof item === 'object') {
      for (const [subkey, subvalue] of Object.entries(item)) {
        if (typeof subvalue === 'number') flat[`${key}_${subkey}`] = subvalue;
      }
    }
  }
  return flat;
}
for (const key of Object.keys(nativeConsole)) {
  console[key] = (...args) => {
    nativeConsole[key](...args);
    for (const argument of args) {
      if (typeof argument !== 'string') continue;
      const offset = argument.indexOf('TABULA_BASELINE ');
      if (offset < 0) continue;
      try {
        const receipt = JSON.parse(argument.slice(offset + 'TABULA_BASELINE '.length));
        receipts.push(receipt);
        metric(`baseline_${receipt.kind}`, flattenReceipt(receipt));
        if (receipt.kind === 'ready' && !ready) {
          ready = true;
          clearTimeout(timeout);
          emit('ready', { renderer: 'macroquad', checkpoint: receipt.fixture_hash });
          metric('boot', { document_ready_ms: performance.now() - begun });
          observe();
          // This callback runs after the Rust frame that emitted ready has
          // returned through the pinned loader and submitted its draw list.
          readyFrame = requestAnimationFrame(() => {
            readyFrame = null;
            if (!disposed) metric('rendered', { minimum_completed_frames: 1 });
          });
        } else if (receipt.kind === 'failure') fail('baseline_failure', receipt.error);
      } catch (error) { fail('receipt_parse', error.message); }
    }
  };
}

function observe() {
  if (disposed) return null;
  const rect = canvas.getBoundingClientRect();
  const backend = globalThis.gl ?? null;
  const debug = backend?.getExtension('WEBGL_debug_renderer_info');
  const resource = performance.getEntriesByType('resource').find(entry => entry.name.endsWith('.wasm'));
  const value = {
    elapsed_ms: performance.now() - begun,
    viewport: [rect.width, rect.height],
    canvas_pixels: [canvas.width, canvas.height],
    device_pixel_ratio: devicePixelRatio,
    wasm_linear_memory_bytes: globalThis.wasm_memory?.buffer.byteLength ?? 0,
    js_heap_bytes: performance.memory?.usedJSHeapSize ?? 0,
    js_heap_available: Boolean(performance.memory),
    webgl_version: backend?.getParameter(backend.VERSION) ?? 'unavailable',
    webgl_renderer: debug ? backend.getParameter(debug.UNMASKED_RENDERER_WEBGL) : 'unavailable',
    visible: document.visibilityState === 'visible',
    focused: document.hasFocus(),
    suspended,
    wasm_download_duration_ms: resource?.duration ?? 0,
    wasm_download_transfer_bytes: resource?.transferSize ?? 0,
    wasm_download_encoded_bytes: resource?.encodedBodySize ?? 0,
  };
  observations.push(value);
  if (observations.length > 120) observations.shift();
  document.querySelector('#runtime-observation').textContent = JSON.stringify(value);
  metric('runtime', value);
  return value;
}
function suspend() {
  if (disposed || suspended) return;
  suspended = true;
  pauses++;
  // Pinned loader's scheduling handle; no WASM state/rules are changed.
  globalThis.blocking_event_loop = true;
  if (globalThis.animation_frame_timeout) cancelAnimationFrame(globalThis.animation_frame_timeout);
  metric('lifecycle', { paused: true, pauses, resumes });
}
function resume() {
  if (disposed || !suspended) return;
  suspended = false;
  resumes++;
  globalThis.blocking_event_loop = false;
  if (globalThis.wasm_exports && typeof globalThis.animation === 'function') {
    globalThis.animation_frame_timeout = requestAnimationFrame(globalThis.animation);
  }
  metric('lifecycle', { paused: false, pauses, resumes });
}
function dispose() {
  if (disposed) return;
  observe();
  suspend();
  disposed = true;
  clearInterval(interval);
  clearTimeout(timeout);
  if (readyFrame !== null) cancelAnimationFrame(readyFrame);
  readyFrame = null;
  window.removeEventListener('message', initialise);
  window.removeEventListener('pagehide', pagehide);
  window.removeEventListener('keydown', tabEscape, true);
  for (const name of gatedInputs) window.removeEventListener(name, inputGate, true);
  document.removeEventListener('visibilitychange', visibility);
  bridge?.dispose();
  for (const key of Object.keys(nativeConsole)) console[key] = nativeConsole[key];
  // Engine-owned anonymous handlers survive until the host removes the iframe.
  // Context destruction, rather than an invented Macroquad dispose API, owns those.
}
function visibility() { document.visibilityState === 'hidden' ? suspend() : resume(); }
function pagehide() { dispose(); }
function inputGate(event) {
  if (!suspended && !disposed) return;
  if (event.cancelable) event.preventDefault();
  event.stopImmediatePropagation();
}
function tabEscape(event) {
  if (!embedded || disposed || !ready || event.key !== 'Tab') return;
  // The pinned loader prevents Tab by default. The shell owns cross-document
  // focus order; carry this intent through the same validated bridge.
  event.preventDefault();
  event.stopImmediatePropagation();
  emit('focus_exit', { backward: event.shiftKey });
}
function start() {
  if (started || disposed) return;
  started = true;
  timeout = setTimeout(() => fail('ready_timeout', 'Macroquad did not report ready within 30 seconds'), 30000);
  interval = setInterval(observe, 1000);
  document.addEventListener('visibilitychange', visibility);
  window.addEventListener('pagehide', pagehide);
  window.addEventListener('keydown', tabEscape, true);
  for (const name of gatedInputs) window.addEventListener(name, inputGate, { capture: true, passive: false });
  globalThis.load('tabula-game-client.wasm');
}
function handle(message) {
  switch (message.kind) {
    case 'ping': emit('pong', message.payload); break;
    case 'focus_request': if (!suspended) canvas.focus({ preventScroll: true }); break;
    case 'suspend': suspend(); break;
    case 'resume': resume(); break;
    case 'dispose': dispose(); break;
    case 'resize':
      if (globalThis.wasm_exports) globalThis.resize(canvas, globalThis.wasm_exports.resize);
      observe();
      break;
    case 'preferences':
      fail('preferences_compiled', 'Macroquad baseline theme/motion are compile-time options; remount the matching executable');
      break;
    default: fail('unsupported_host_kind', message.kind);
  }
}
function initialise(event) {
  if (started || disposed || event.origin !== location.origin || event.source !== window.parent) return;
  const value = event.data;
  if (!value || typeof value !== 'object' || value.kind !== 'init') return;
  try {
    const candidate = { session_id: value.session, generation: value.generation };
    if (typeof candidate.session_id !== 'string' || candidate.session_id.length < 1 || candidate.session_id.length > 128 || !Number.isSafeInteger(candidate.generation) || candidate.generation < 0) throw new Error('invalid identity');
    validateEnvelope(event, { origin: location.origin, source: window.parent, identity: candidate, lastRevision: -1, validators: payloadValidators });
    identity = candidate;
    window.removeEventListener('message', initialise);
    bridge = createBridge({ targetWindow: window.parent, origin: location.origin, identity, initialIncomingRevision: value.revision, onMessage: handle, onError: error => { rejected++; errors.push(error); } });
    emit('hello', {});
    start();
  } catch (error) { rejected++; errors.push({ code: error.code ?? 'init_error', message: error.message }); }
}

window.tabulaMacroquad = {
  report: () => structuredClone({ embedded, ready, started, disposed, suspended, pauses, resumes, rejected, identity, receipts, observations, errors, bridge: bridge?.snapshot() ?? null, audio_fixture: false, match_sockets: 0 }),
  suspend, resume, dispose,
};
if (embedded) window.addEventListener('message', initialise);
else { identity = { session_id: crypto.randomUUID(), generation: 0 }; start(); }
