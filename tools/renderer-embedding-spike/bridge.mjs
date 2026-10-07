// ADR-011 experiment: scoped same-origin messages, no match socket.
import { exact, requireThat, string, uint, finite, boolean, record, validateInput, validateViewport, validatePreferences } from './boundary.mjs';
export const CHANNEL = 'tabula-renderer-spike';
export const PROTOCOL = 1;
const EMPTY = value => exact(value, [], 'payload');
const ping = value => { exact(value, ['nonce', 'sent_ms'], 'payload'); uint(value.nonce, 'payload.nonce'); finite(value.sent_ms, 'payload.sent_ms', 0); };
export const payloadValidators = {
  hello: EMPTY, suspend: EMPTY, resume: EMPTY, dispose: EMPTY, focus_request: EMPTY,
  ping, pong: ping, resize: validateViewport, preferences: validatePreferences,
  focus_exit: value => {exact(value,['backward'],'payload');boolean(value.backward,'payload.backward');},
  init: value => { exact(value, ['viewport', 'preferences'], 'payload'); validateViewport(value.viewport); validatePreferences(value.preferences); },
  ready: value => { exact(value, ['renderer', 'checkpoint'], 'payload'); string(value.renderer, 'payload.renderer', 128); string(value.checkpoint, 'payload.checkpoint', 128); },
  input: validateInput,
  view: value => { exact(value, ['schema_version', 'session_id', 'generation', 'revision', 'frame'], 'payload'); },
  error: value => { exact(value, ['code', 'message'], 'payload'); string(value.code, 'payload.code', 128); string(value.message, 'payload.message', 1024); },
  metrics: value => {
    exact(value, ['name', 'value'], 'payload'); string(value.name, 'payload.name', 128); record(value.value, 'payload.value');
    const keys = Object.keys(value.value); requireThat(keys.length <= 64, 'metric_limit', 'payload.value');
    for (const key of keys) {
      requireThat(/^[a-z][a-z0-9_]*$/.test(key) && !['state', 'secret', 'token', 'seed', 'canonical', 'view', 'command'].includes(key), 'invalid_metric', key);
      const v = value.value[key];
      if (typeof v === 'number') finite(v, key, -1e12, 1e12);
      else if (typeof v === 'boolean') continue;
      else if (typeof v === 'string') string(v, key, 256);
      else if (Array.isArray(v)) { requireThat(v.length <= 4096, 'metric_limit', key); v.forEach(n => finite(n, key, -1e12, 1e12)); }
      else requireThat(false, 'invalid_metric', key);
    }
  },
};
export function makeEnvelope(kind, identity, revision, payload = {}) {
  const result = { channel: CHANNEL, protocol: PROTOCOL, kind, session: identity.session_id, generation: identity.generation, revision, payload };
  validateEnvelopeData(result, { identity, lastRevision: -1 }); return result;
}
function validateEnvelopeData(data, { identity, lastRevision = -1, validators = payloadValidators }) {
  exact(data, ['channel', 'protocol', 'kind', 'session', 'generation', 'revision', 'payload'], 'envelope');
  requireThat(data.channel === CHANNEL && data.protocol === PROTOCOL, 'protocol_mismatch', 'envelope');
  requireThat(data.session === identity.session_id && data.generation === identity.generation, 'stale_identity', 'envelope');
  uint(data.revision, 'envelope.revision'); requireThat(data.revision > lastRevision, 'stale_revision', 'envelope');
  requireThat(typeof data.kind === 'string' && Object.hasOwn(validators, data.kind), 'unknown_kind', 'envelope.kind');
  const check = validators[data.kind]; requireThat(typeof check === 'function', 'unknown_kind', 'envelope.kind'); check(data.payload);
  return data;
}
export function validateEnvelope(event, options) {
  requireThat(options.origin !== '*' && /^https?:\/\//.test(options.origin), 'invalid_origin', 'bridge');
  requireThat(event.origin === options.origin, 'wrong_origin', 'event');
  requireThat(event.source === options.source, 'wrong_source', 'event');
  return structuredClone(validateEnvelopeData(event.data, options));
}
export function createBridge({ window: owner = globalThis.window, targetWindow, expectedSource = targetWindow, origin, identity, onMessage, onError = () => {}, validators = payloadValidators, initialIncomingRevision = -1 }) {
  requireThat(new URL(origin).origin === origin && origin !== '*', 'invalid_origin', 'bridge');
  if (initialIncomingRevision !== -1) uint(initialIncomingRevision, 'bridge.initialIncomingRevision');
  let disposed = false; let lastIncoming = initialIncomingRevision; let lastOutgoing = -1; let accepted = 0; let rejected = 0;
  const listener = event => {
    if (disposed) return;
    try { const value = validateEnvelope(event, { origin, source: expectedSource, identity, lastRevision: lastIncoming, validators }); lastIncoming = value.revision; accepted++; onMessage(value); }
    catch (error) { rejected++; onError({ code: error.code ?? 'bridge_error', message: error.message }); }
  };
  owner.addEventListener('message', listener);
  return {
    send(kind, payload = {}, revision = lastOutgoing + 1) {
      requireThat(!disposed, 'disposed', 'bridge'); uint(revision, 'revision'); requireThat(revision > lastOutgoing, 'stale_revision', 'bridge');
      const message = { channel: CHANNEL, protocol: PROTOCOL, kind, session: identity.session_id, generation: identity.generation, revision, payload };
      validateEnvelopeData(message, { identity, lastRevision: lastOutgoing, validators }); lastOutgoing = revision; targetWindow.postMessage(message, origin); return message;
    },
    dispose() { if (!disposed) { disposed = true; owner.removeEventListener('message', listener); } },
    snapshot() { return { disposed, listeners: disposed ? 0 : 1, accepted, rejected, last_incoming: lastIncoming, last_outgoing: lastOutgoing }; },
  };
}
