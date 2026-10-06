/* Synthetic Node/vm helper contracts only, never browser/HTTPS/PostgreSQL acceptance. */
'use strict';

const assert = require('node:assert/strict');
const { webcrypto, createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const SOURCE = fs.readFileSync(path.join(__dirname, 'actual_fetch_observer.js'), 'utf8');
const ORIGIN = 'https://localhost:9443';
const URL_ONE = `${ORIGIN}/api/v1/matches`;
const BODY = '{"game_id":"chess","config":{}}';
const RESPONSE_LIMIT = 2 * 1024 * 1024;
const REQUEST_LIMIT = 128 * 1024;
const TOTAL_LIMIT = 8 * 1024 * 1024;
const RECORD_LIMIT = 64;
const DEADLINE_MS = 5000;
const EPOCH_MS = 1791255000000;
const hash = (body) => createHash('sha256').update(body).digest('hex');
const expected = (body = BODY, url = URL_ONE, method = 'POST') => ({
    method, url, body_sha256: hash(body),
});

function plain(value) {
    return JSON.parse(JSON.stringify(value));
}

function deferred() {
    let resolve;
    let reject;
    const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
    return { promise, resolve, reject };
}

async function turn() {
    await new Promise((resolve) => setImmediate(resolve));
}

async function settle() {
    // Native Response tee/read and WebCrypto settle across actual event-loop turns.
    for (let i = 0; i < 8; i += 1) await turn();
}

function clock() {
    let now = 0;
    let next = 0;
    const timers = new Map();
    return {
        now: () => now,
        setTimeout(fn, delay = 0, ...args) {
            const id = ++next;
            timers.set(id, { at: now + Number(delay), fn, args });
            return id;
        },
        clearTimeout(id) { timers.delete(id); },
        advance(ms) {
            now += ms;
            for (;;) {
                const due = [...timers].filter(([, timer]) => timer.at <= now)
                    .sort((left, right) => left[1].at - right[1].at || left[0] - right[0]);
                if (!due.length) break;
                const [id, timer] = due[0];
                timers.delete(id);
                timer.fn(...timer.args);
            }
        },
        pending: () => timers.size,
    };
}

function harness(fetchImpl, options = {}) {
    const timer = clock();
    const listeners = new Map();
    const calls = [];
    const originalFetch = function (...args) {
        calls.push({ receiver: this, args });
        return fetchImpl.apply(this, args);
    };
    const window = {
        fetch: originalFetch,
        location: { href: `${ORIGIN}/play/local/`, origin: ORIGIN },
        addEventListener(type, callback) {
            if (!listeners.has(type)) listeners.set(type, []);
            listeners.get(type).push(callback);
        },
        removeEventListener(type, callback) {
            listeners.set(type, (listeners.get(type) || []).filter((item) => item !== callback));
        },
    };
    const RealDate = Date;
    class FakeDate extends RealDate {
        constructor(...args) { super(...(args.length ? args : [timer.now()])); }
        static now() { return timer.now(); }
    }
    const globals = {
        window, location: window.location, document: { baseURI: window.location.href },
        Request, Response, Headers, ReadableStream, TextEncoder, TextDecoder, URL,
        Uint8Array, ArrayBuffer, Blob, AbortController, DOMException,
        crypto: options.crypto || webcrypto,
        setTimeout: timer.setTimeout, clearTimeout: timer.clearTimeout,
        queueMicrotask, Date: FakeDate, performance: { timeOrigin: EPOCH_MS, now: timer.now },
    };
    Object.assign(window, globals);
    const context = vm.createContext(globals);
    vm.runInContext(SOURCE, context, { filename: 'actual_fetch_observer.js' });
    return {
        window, calls, timer, context, originalFetch,
        observer: window.__tabulaActualFetchObserver,
        event(type, fields = {}) {
            for (const callback of listeners.get(type) || []) callback({ type, ...fields });
        },
    };
}

function response(body = '{"frames":[]}', options = {}) {
    const bytes = typeof body === 'string' ? Buffer.byteLength(body) : null;
    const headers = {
        'content-type': 'application/json; charset=utf-8',
        'cache-control': 'private, no-store',
        ...(bytes === null ? {} : { 'content-length': String(bytes) }),
        ...options.headers,
    };
    const result = new Response(body, { status: options.status || 200, headers });
    Object.defineProperty(result, 'url', { value: options.url || URL_ONE });
    return result;
}

function metadata(value) {
    const media = value.headers.get('content-type').split(';')[0].trim().toLowerCase();
    return {
        status: value.status,
        url: value.url,
        content_type: ['application/json', 'application/problem+json'].includes(media) ? media : 'other',
        no_store: value.headers.get('cache-control').split(',').some((part) => part.trim() === 'no-store'),
        content_length: value.headers.get('content-length'),
    };
}

function failure(value, error) {
    assert.deepEqual(plain(value), { ok: false, error });
}

async function bound(h, identity = expected(), owner = 'consumer-one') {
    const result = plain(await h.observer.bind(identity, owner));
    assert.equal(result.ok, true, JSON.stringify(result));
    assert.deepEqual(Object.keys(result).sort(), ['document', 'id', 'ok']);
    assert.ok(result.id);
    assert.ok(result.document);
    return result;
}

function read(h, binding, value, owner = 'consumer-one', changes = {}) {
    return h.observer.read({
        id: binding.id, owner, document: binding.document,
        metadata: metadata(value), request_started_at: EPOCH_MS + h.timer.now(), ...changes,
    });
}

async function observed(value = response(), body = BODY, url = URL_ONE) {
    // Bind the request event before native response arrival, as browser helpers do.
    const native = deferred();
    const h = harness(() => native.promise);
    const request = h.window.fetch(url, { method: 'POST', body });
    const binding = await bound(h, expected(body, url));
    native.resolve(value);
    assert.equal(await request, value);
    return { h, value, binding };
}

test('wrapper calls original fetch synchronously once with unchanged receiver and arguments', async () => {
    const original = response();
    const pending = deferred();
    const h = harness(() => pending.promise);
    const receiver = { caller: 'unchanged' };
    const init = { method: 'POST', body: BODY, credentials: 'same-origin' };
    const extra = { untouched: true };
    const result = h.window.fetch.call(receiver, URL_ONE, init, extra);
    assert.equal(h.calls.length, 1, 'the actual call happens before fetch returns');
    assert.equal(h.calls[0].receiver, receiver);
    assert.equal(h.calls[0].args[0], URL_ONE);
    assert.equal(h.calls[0].args[1], init);
    assert.equal(h.calls[0].args[2], extra);
    pending.resolve(original);
    assert.equal(await result, original);
    assert.equal(h.calls.length, 1);
});

test('observer hashing never delays the caller receiving the original Response', async () => {
    const digest = deferred();
    const value = response();
    const h = harness(() => Promise.resolve(value), {
        crypto: { getRandomValues: webcrypto.getRandomValues.bind(webcrypto),
            randomUUID: webcrypto.randomUUID.bind(webcrypto), subtle: { digest: () => digest.promise } },
    });
    assert.equal(await h.window.fetch(URL_ONE, { method: 'POST', body: BODY }), value);
    digest.resolve(await webcrypto.subtle.digest('SHA-256', new TextEncoder().encode(BODY)));
    const binding = await bound(h);
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
});

test('wrapper preserves a synchronous exception from original fetch', () => {
    const error = new TypeError('synthetic-secret from original fetch');
    const h = harness(() => { throw error; });
    assert.throws(() => h.window.fetch(URL_ONE, { method: 'POST', body: BODY }), (caught) => caught === error);
    assert.equal(h.calls.length, 1);
});

test('relative request URLs are normalized without changing arguments passed to fetch', async () => {
    const value = response();
    const h = harness(() => Promise.resolve(value));
    const init = { method: 'post', body: BODY };
    assert.equal(await h.window.fetch('/api/v1/matches', init), value);
    assert.equal(h.calls[0].args[0], '/api/v1/matches');
    assert.equal(h.calls[0].args[1], init);
    const binding = await bound(h);
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
});

test('create, join, attach and command paths are observed without collecting unrelated fetches', async () => {
    const h = harness((url) => Promise.resolve(response('{"frames":[]}', { url: String(url) })));
    for (const url of [URL_ONE, `${URL_ONE}/join`, `${URL_ONE}/${'a'.repeat(32)}/attach`, `${URL_ONE}/${'a'.repeat(32)}/command`]) {
        const value = await h.window.fetch(url, { method: 'POST', body: BODY });
        const binding = await bound(h, expected(BODY, url), url);
        assert.deepEqual(plain(await read(h, binding, value, url)), { ok: true, text: '{"frames":[]}' });
    }
    for (let i = 0; i < RECORD_LIMIT + 1; i += 1) {
        await h.window.fetch(`${ORIGIN}/assets/${i}.wasm`, { method: 'GET' });
    }
    for (const url of [
        `${URL_ONE}?token=synthetic-secret`, `${URL_ONE}#synthetic-secret`,
        'https://foreign.invalid/api/v1/matches', `${ORIGIN}/api/v1/auth/context`,
    ]) {
        await h.window.fetch(url, { method: 'POST', body: BODY });
        failure(await h.observer.bind(expected(BODY, url), 'other-consumer'), 'request_not_observed');
    }
    assert.equal(h.calls.length, RECORD_LIMIT + 9);
    const value = await h.window.fetch(URL_ONE, { method: 'POST', body: `${BODY} later` });
    const binding = await bound(h, expected(`${BODY} later`));
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
});

test('reading the observation leaves the original response and its body untouched', async () => {
    const original = '{"match_id":340282366920938463463374607431768211455,"text":"é"}';
    const { h, value, binding } = await observed(response(original));
    assert.equal(value.bodyUsed, false);
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: original });
    assert.equal(value.bodyUsed, false);
    assert.equal(await value.text(), original);
});

test('Request body is hashed from original cloned bytes without consuming the caller body', async () => {
    const body = '{ "large_id": 340282366920938463463374607431768211455, "text": "é" }';
    const request = new Request(URL_ONE, { method: 'POST', body });
    const value = response();
    const h = harness(() => Promise.resolve(value));
    await h.window.fetch(request);
    assert.equal(h.calls[0].args[0], request);
    const binding = await bound(h, expected(body));
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
    assert.equal(request.bodyUsed, false);
    assert.equal(await request.text(), body);
});

test('streamed Request fingerprints concatenate exact chunks in original order', async () => {
    const chunks = ['{ "id": 340282366920938463463374607431768211455,', ' "text": "', 'é" }'];
    const request = new Request(URL_ONE, {
        method: 'POST', duplex: 'half',
        body: new ReadableStream({ start(controller) {
            for (const chunk of chunks) controller.enqueue(new TextEncoder().encode(chunk));
            controller.close();
        } }),
    });
    const value = response();
    const h = harness(() => Promise.resolve(value));
    await h.window.fetch(request);
    const binding = await bound(h, expected(chunks.join('')));
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
    assert.equal(request.bodyUsed, false);
    assert.equal(await request.text(), chunks.join(''));
});

test('Request init body and method overrides identify the bytes actually sent', async () => {
    const request = new Request(URL_ONE, { method: 'PUT', body: 'replaced-body' });
    const value = response();
    const h = harness(() => Promise.resolve(value));
    const init = { method: 'POST', body: BODY };
    await h.window.fetch(request, init);
    const binding = await bound(h);
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
    assert.equal(request.bodyUsed, false);
});

for (const initBody of [null, undefined]) {
    test(`Request init body ${String(initBody)} retains and fingerprints the original nonempty body`, async () => {
        const request = new Request(URL_ONE, { method: 'POST', body: BODY });
        const init = { body: initBody };
        // Native Request construction is the independent Fetch body-retention oracle.
        assert.equal(await new Request(request.clone(), init).text(), BODY);
        const value = response();
        const h = harness(() => Promise.resolve(value));
        assert.equal(await h.window.fetch(request, init), value);
        assert.equal(h.calls[0].args[0], request);
        assert.equal(h.calls[0].args[1], init);
        failure(await h.observer.bind(expected(''), 'empty-body-consumer'), 'request_not_observed');
        const binding = await bound(h);
        assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
        assert.equal(request.bodyUsed, false);
        assert.equal(await request.text(), BODY);
    });
}

test('same consumer repeatedly binds and reads one record until explicit body retirement', async () => {
    const { h, value, binding } = await observed();
    for (let i = 0; i < 3; i += 1) assert.deepEqual(await bound(h), binding);
    for (let i = 0; i < 3; i += 1) {
        assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
    }
    assert.deepEqual(await bound(h), binding);
    assert.deepEqual(plain(h.observer.retireBody({
        id: binding.id, owner: 'consumer-one', document: binding.document,
    })), { ok: true });
    failure(await read(h, binding, value), 'response_body_unavailable');
    assert.equal(h.calls.length, 1);
});

test('body retirement requires the bound owner and current document identity', async () => {
    const { h, value, binding } = await observed();
    failure(h.observer.retireBody({
        id: binding.id, owner: 'consumer-two', document: binding.document,
    }), 'request_owner_mismatch');
    failure(h.observer.retireBody({
        id: binding.id, owner: 'consumer-one', document: `${binding.document}-old`,
    }), 'document_mismatch');
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
});

test('sequential identical requests can be separately bound and never confuse response bodies', async () => {
    const first = response('{"attempt":1}');
    const second = response('{"attempt":2}');
    const values = [first, second];
    const h = harness(() => Promise.resolve(values.shift()));
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const one = await bound(h, expected(), 'consumer-one');
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const two = await bound(h, expected(), 'consumer-two');
    assert.notEqual(one.id, two.id);
    assert.equal(one.document, two.document);
    assert.deepEqual(plain(await read(h, one, first)), { ok: true, text: '{"attempt":1}' });
    assert.deepEqual(plain(await read(h, two, second, 'consumer-two')), { ok: true, text: '{"attempt":2}' });
});

test('concurrent identical unbound records are ambiguous rather than first-match wins', async () => {
    const h = harness(() => Promise.resolve(response()));
    await Promise.all([h.window.fetch(URL_ONE, { method: 'POST', body: BODY }),
        h.window.fetch(URL_ONE, { method: 'POST', body: BODY })]);
    failure(await h.observer.bind(expected(), 'consumer-one'), 'ambiguous_request');
});

test('identical pending records participate in ambiguity', async () => {
    const pending = deferred();
    let first = true;
    const h = harness(() => first ? (first = false, pending.promise) : Promise.resolve(response()));
    const request = h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    failure(await h.observer.bind(expected(), 'consumer-one'), 'ambiguous_request');
    pending.resolve(response());
    await request;
});

test('failed and aborted fetch records cannot be ignored to remove ambiguity', async () => {
    for (const error of [new TypeError('synthetic-secret'), new DOMException('synthetic-secret', 'AbortError')]) {
        let first = true;
        const native = deferred();
        const h = harness(() => first ? (first = false, native.promise) : Promise.resolve(response()));
        const failureResult = assert.rejects(h.window.fetch(URL_ONE, { method: 'POST', body: BODY }), (caught) => caught === error);
        await settle();
        native.reject(error);
        await failureResult;
        await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
        failure(await h.observer.bind(expected(), 'consumer-one'), 'ambiguous_request');
    }
});

async function earlyResponseFailure(mode, retryBody) {
    const failureError = new TypeError('synthetic-secret immediate response failure');
    const originalValue = mode === 'fetch rejection' ? response() : response(new ReadableStream({
        start(controller) { controller.error(failureError); },
    }));
    const retryValue = response('{"genuine_attempt":2}');
    const digests = [];
    let first = true;
    const h = harness(() => {
        if (!first) return Promise.resolve(retryValue);
        first = false;
        return mode === 'fetch rejection' ? Promise.reject(failureError) : Promise.resolve(originalValue);
    }, {
        crypto: { randomUUID: webcrypto.randomUUID.bind(webcrypto), subtle: {
            digest(algorithm, bytes) {
                const pending = deferred();
                digests.push({ ...pending, algorithm, bytes: new Uint8Array(bytes) });
                return pending.promise;
            },
        } },
    });
    const original = h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    if (mode === 'fetch rejection') await assert.rejects(original, (error) => error === failureError);
    else assert.equal(await original, originalValue);
    await settle();
    assert.equal(digests.length, 1);
    assert.ok(h.timer.pending() > 0, 'the body digest remains pending after immediate response failure');
    assert.equal(await h.window.fetch(URL_ONE, { method: 'POST', body: retryBody }), retryValue);
    assert.equal(digests.length, 2);
    for (const pending of digests) {
        pending.resolve(await webcrypto.subtle.digest(pending.algorithm, pending.bytes));
    }
    return { h, originalValue, retryValue };
}

for (const mode of ['fetch rejection', 'response reader failure']) {
    test(`immediate ${mode} before digest settlement cannot poison a different-body response`, async () => {
        const retryBody = '{"game_id":"chess","config":{"attempt":2}}';
        const { h, originalValue, retryValue } = await earlyResponseFailure(mode, retryBody);
        const retryBinding = await bound(h, expected(retryBody), 'retry-consumer');
        assert.deepEqual(plain(await read(h, retryBinding, retryValue, 'retry-consumer')), {
            ok: true, text: '{"genuine_attempt":2}',
        });
        const originalBinding = await bound(h, expected(), 'failed-consumer');
        failure(await read(h, originalBinding, originalValue, 'failed-consumer'),
            mode === 'fetch rejection' ? 'original_fetch_failed' : 'response_body_read_failed');
        await settle();
        assert.equal(h.timer.pending(), 0);
        assert.equal(h.calls.length, 2);
    });

    test(`immediate ${mode} before digest settlement keeps identical-body retry ambiguous`, async () => {
        const { h } = await earlyResponseFailure(mode, BODY);
        failure(await h.observer.bind(expected(), 'retry-consumer'), 'ambiguous_request');
        await settle();
        assert.equal(h.timer.pending(), 0);
        assert.equal(h.calls.length, 2);
    });
}

test('a duplicate arriving while binding hashes are pending still makes the bind ambiguous', async () => {
    const digests = [];
    const h = harness(() => Promise.resolve(response()), {
        crypto: { randomUUID: webcrypto.randomUUID.bind(webcrypto), subtle: {
            digest() { const result = deferred(); digests.push(result); return result.promise; },
        } },
    });
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const binding = h.observer.bind(expected(), 'consumer-one');
    await turn();
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    assert.equal(digests.length, 2);
    const digest = await webcrypto.subtle.digest('SHA-256', new TextEncoder().encode(BODY));
    for (const result of digests) result.resolve(digest);
    failure(await binding, 'ambiguous_request');
});

test('an unavailable request fingerprint cannot be excluded from matching candidates', async () => {
    let count = 0;
    const h = harness(() => Promise.resolve(response()), {
        crypto: { randomUUID: webcrypto.randomUUID.bind(webcrypto), subtle: {
            digest(...args) {
                count += 1;
                return count === 1 ? Promise.reject(new Error('synthetic-secret hashing error'))
                    : webcrypto.subtle.digest(...args);
            },
        } },
    });
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    failure(await h.observer.bind(expected(), 'consumer-one'), 'request_fingerprint_unavailable');
});

test('a rejected digest clears its timeout rather than retaining an orphaned timer', async () => {
    const h = harness(() => Promise.resolve(response()), {
        crypto: { randomUUID: webcrypto.randomUUID.bind(webcrypto), subtle: {
            digest: () => Promise.reject(new Error('synthetic-secret digest failure')),
        } },
    });
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    failure(await h.observer.bind(expected(), 'consumer-one'), 'request_fingerprint_unavailable');
    await settle();
    assert.equal(h.timer.pending(), 0);
});

test('a never-resolving digest is bounded by the fixed five-second deadline', async () => {
    const h = harness(() => Promise.resolve(response()), {
        crypto: { randomUUID: webcrypto.randomUUID.bind(webcrypto), subtle: {
            digest: () => new Promise(() => {}),
        } },
    });
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const binding = h.observer.bind(expected(), 'consumer-one');
    await settle();
    h.timer.advance(DEADLINE_MS);
    failure(await binding, 'request_fingerprint_unavailable');
    assert.equal(h.timer.pending(), 0);
});

test('lifecycle disposal unblocks binding while the digest is still pending', { timeout: 1000 }, async () => {
    const h = harness(() => Promise.resolve(response()), {
        crypto: { randomUUID: webcrypto.randomUUID.bind(webcrypto), subtle: {
            digest: () => new Promise(() => {}),
        } },
    });
    await h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const binding = h.observer.bind(expected(), 'consumer-one');
    await settle();
    h.event('pagehide', { persisted: false });
    failure(await binding, 'document_disposed');
    assert.equal(h.timer.pending(), 0);
});

test('an original fetch failure is readable only as a fixed failure class', async () => {
    const error = new TypeError('synthetic-secret original transport error');
    const native = deferred();
    const h = harness(() => native.promise);
    const failureResult = assert.rejects(h.window.fetch(URL_ONE, { method: 'POST', body: BODY }), (caught) => caught === error);
    const binding = await bound(h);
    native.reject(error);
    await failureResult;
    failure(await read(h, binding, response()), 'original_fetch_failed');
});

test('method, URL and original body digest are all required to bind', async () => {
    const { h } = await observed();
    const variations = [expected(BODY, URL_ONE, 'PUT'), expected(BODY, `${URL_ONE}/join`), expected(`${BODY} `)];
    for (const identity of variations) failure(await h.observer.bind(identity, 'other-consumer'), 'request_not_observed');
});

test('read checks exact record, owner and document identity', async () => {
    const { h, value, binding } = await observed();
    failure(await read(h, binding, value, 'consumer-two'), 'request_owner_mismatch');
    failure(await read(h, binding, value, 'consumer-one', { document: `${binding.document}-other` }), 'document_mismatch');
    failure(await read(h, binding, value, 'consumer-one', { id: `${binding.id}-other` }), 'request_owner_mismatch');
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
});

test('an old native request start cannot bind to identical bytes in a newer document', async () => {
    const native = deferred();
    const h = harness(() => native.promise);
    h.timer.advance(100);
    const value = response();
    const request = h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const binding = await bound(h);
    native.resolve(value);
    await request;
    failure(await read(h, binding, value, 'consumer-one', { request_started_at: EPOCH_MS + 99 }), 'request_document_mismatch');
    for (const invalid of [null, undefined, NaN, Infinity, 'synthetic-secret']) {
        failure(await read(h, binding, value, 'consumer-one', { request_started_at: invalid }), 'request_document_mismatch');
    }
    assert.deepEqual(plain(await read(h, binding, value, 'consumer-one', {
        request_started_at: EPOCH_MS + 100,
    })), { ok: true, text: '{"frames":[]}' });
});

test('every response metadata field is compared exactly before returning body text', async () => {
    const { h, value, binding } = await observed();
    for (const change of [
        { status: 201 }, { url: `${URL_ONE}/join` }, { content_type: 'text/plain' },
        { no_store: false }, { content_length: null }, { content_length: `0${metadata(value).content_length}` },
    ]) {
        failure(await read(h, binding, value, 'consumer-one', {
            metadata: { ...metadata(value), ...change },
        }), 'response_metadata_mismatch');
    }
    const extra = { ...metadata(value), private_header: 'synthetic-secret' };
    const missing = metadata(value);
    delete missing.status;
    for (const supplied of [extra, missing]) {
        failure(await read(h, binding, value, 'consumer-one', { metadata: supplied }), 'response_metadata_mismatch');
    }
});

test('responses with changed final URL or a redirect flag never return observed body text', async () => {
    const foreign = response('{"frames":[]}', { url: `${URL_ONE}/join` });
    const redirect = response();
    Object.defineProperty(redirect, 'redirected', { value: true });
    for (const value of [foreign, redirect]) {
        const { h, binding } = await observed(value);
        failure(await read(h, binding, value), 'response_redirected');
    }
});

test('only JSON media names are retained and other Content-Type values become a fixed class', async () => {
    for (const media of ['application/problem+json; charset=utf-8', 'private/synthetic-secret; token=synthetic-secret']) {
        const { h, value, binding } = await observed(response('{"frames":[]}', { headers: { 'content-type': media } }));
        const facts = metadata(value);
        assert.ok(['application/problem+json', 'other'].includes(facts.content_type));
        assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
    }
});

test('raw invalid JSON is returned unchanged for the Python admission validator to reject', async () => {
    const invalid = '{"frames": broken';
    const { h, value, binding } = await observed(response(invalid));
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: invalid });
});

test('response body over the 2 MiB limit fails closed without truncating into success', async () => {
    const { h, value, binding } = await observed(response('x'.repeat(RESPONSE_LIMIT + 1), {
        headers: { 'content-length': String(RESPONSE_LIMIT + 1) },
    }));
    failure(await read(h, binding, value), 'response_body_limit');
    assert.equal(value.bodyUsed, false);
});

test('a response with no announced length is bounded by bytes actually read', async () => {
    const value = response('x'.repeat(RESPONSE_LIMIT + 1));
    value.headers.delete('content-length');
    const { h, binding } = await observed(value);
    failure(await read(h, binding, value), 'response_body_limit');
});

test('response size bounds count UTF-8 bytes rather than JavaScript string length', async () => {
    const value = response('é'.repeat(RESPONSE_LIMIT / 2 + 1));
    value.headers.delete('content-length');
    const { h, binding } = await observed(value);
    failure(await read(h, binding, value), 'response_body_limit');
});

test('the exact 2 MiB response boundary remains readable', async () => {
    const body = 'x'.repeat(RESPONSE_LIMIT);
    const { h, value, binding } = await observed(response(body));
    const result = plain(await read(h, binding, value));
    assert.equal(result.ok, true);
    assert.deepEqual(Object.keys(result).sort(), ['ok', 'text']);
    assert.equal(Buffer.byteLength(result.text), RESPONSE_LIMIT);
    assert.equal(hash(result.text), hash(body));
});

test('truncated response transport errors reject partial observed bytes', async () => {
    let controller;
    const stream = new ReadableStream({ start(value) { controller = value; } });
    const value = response(stream, { headers: { 'content-length': '100' } });
    const { h, binding } = await observed(value);
    controller.enqueue(new TextEncoder().encode('{"frames":'));
    await settle();
    controller.error(new TypeError('synthetic-private partial transport failure'));
    failure(await read(h, binding, value), 'response_body_read_failed');
});

test('content-length mismatch cannot turn a normally closed short stream into success', async () => {
    const { h, value, binding } = await observed(response('{"frames":[]}', { headers: { 'content-length': '100' } }));
    failure(await read(h, binding, value), 'response_body_truncated');
});

test('invalid or unbounded announced content lengths are closed failures', async () => {
    for (const length of ['synthetic-secret', '-1', '1.5', '10000000000', String(RESPONSE_LIMIT + 1)]) {
        const { h, value, binding } = await observed(response('{"frames":[]}', {
            headers: { 'content-length': length },
        }));
        failure(await read(h, binding, value), 'response_body_limit');
    }
});

test('a valid original zero-padded content length is compared without rewriting it', async () => {
    const body = '{"frames":[]}';
    const length = `0${Buffer.byteLength(body)}`;
    const { h, value, binding } = await observed(response(body, { headers: { 'content-length': length } }));
    failure(await read(h, binding, value, 'consumer-one', {
        metadata: { ...metadata(value), content_length: String(Buffer.byteLength(body)) },
    }), 'response_metadata_mismatch');
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: body });
});

test('a stalled response hits the fixed five-second deadline', async () => {
    const value = response(new ReadableStream({ start() {} }));
    const { h, binding } = await observed(value);
    const result = read(h, binding, value);
    await settle();
    h.timer.advance(DEADLINE_MS - 1);
    await settle();
    let done = false;
    result.then(() => { done = true; });
    await turn();
    assert.equal(done, false);
    h.timer.advance(1);
    failure(await result, 'body_read_deadline');
});

test('a read begun before native response headers waits until they arrive', async () => {
    const native = deferred();
    const value = response();
    const h = harness(() => native.promise);
    const request = h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const binding = await bound(h);
    const result = read(h, binding, value);
    await settle();
    native.resolve(value);
    await request;
    assert.deepEqual(plain(await result), { ok: true, text: '{"frames":[]}' });
    assert.equal(h.timer.pending(), 0);
});

test('missing native response headers hit the fixed observation deadline', async () => {
    const native = deferred();
    const value = response();
    const h = harness(() => native.promise);
    const request = h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const binding = await bound(h);
    const result = read(h, binding, value);
    await settle();
    h.timer.advance(DEADLINE_MS);
    failure(await result, 'response_observation_deadline');
    assert.equal(h.timer.pending(), 0);
    native.resolve(value);
    assert.equal(await request, value);
});

test('disposal unblocks a read that is waiting for native response headers', async () => {
    const native = deferred();
    const value = response();
    const h = harness(() => native.promise);
    const request = h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const binding = await bound(h);
    const result = read(h, binding, value);
    await settle();
    h.event('pagehide', { persisted: false });
    failure(await result, 'document_disposed');
    assert.equal(h.timer.pending(), 0);
    native.resolve(value);
    assert.equal(await request, value);
});

test('aborted response stream fails with a fixed class and never leaks its error text', async () => {
    let controller;
    const value = response(new ReadableStream({ start(item) { controller = item; } }));
    const { h, binding } = await observed(value);
    controller.error(new DOMException('synthetic-secret abort reason', 'AbortError'));
    failure(await read(h, binding, value), 'response_body_read_failed');
});

test('invalid UTF-8 is rejected without lossy decoding of authentic response bytes', async () => {
    const bytes = new Uint8Array([0x7b, 0xc3, 0x28, 0x7d]);
    const { h, value, binding } = await observed(response(bytes));
    failure(await read(h, binding, value), 'response_body_read_failed');
    assert.deepEqual(new Uint8Array(await value.arrayBuffer()), bytes);
});

test('the request body limit permits exactly 128 KiB and rejects the next byte', async () => {
    const accepted = 'x'.repeat(REQUEST_LIMIT);
    const { h, value, binding } = await observed(response(), accepted);
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
    const excess = 'x'.repeat(REQUEST_LIMIT + 1);
    const rejected = harness(() => Promise.resolve(response()));
    await rejected.window.fetch(URL_ONE, { method: 'POST', body: excess });
    failure(await rejected.observer.bind(expected(excess), 'consumer-one'), 'request_fingerprint_unavailable');
});

test('the request byte limit also applies to cloned Request streams', async () => {
    const body = 'x'.repeat(REQUEST_LIMIT + 1);
    const request = new Request(URL_ONE, { method: 'POST', body });
    const h = harness(() => Promise.resolve(response()));
    await h.window.fetch(request);
    failure(await h.observer.bind(expected(body), 'consumer-one'), 'request_fingerprint_unavailable');
    assert.equal(request.bodyUsed, false);
});

test('unsupported init body types fail closed while preserving the caller fetch', async () => {
    const body = new Uint8Array([1, 2, 3]);
    const value = response();
    const h = harness(() => Promise.resolve(value));
    const init = { method: 'POST', body };
    assert.equal(await h.window.fetch(URL_ONE, init), value);
    assert.equal(h.calls[0].args[1], init);
    failure(await h.observer.bind(expected(Buffer.from(body)), 'consumer-one'), 'request_fingerprint_unavailable');
});

test('a stalled cloned Request fingerprint stops at the fixed five-second deadline', async () => {
    const request = new Request(URL_ONE, {
        method: 'POST', duplex: 'half', body: new ReadableStream({ start() {} }),
    });
    const h = harness(() => Promise.resolve(response()));
    await h.window.fetch(request);
    const binding = h.observer.bind(expected(), 'consumer-one');
    await settle();
    h.timer.advance(DEADLINE_MS);
    failure(await binding, 'request_fingerprint_unavailable');
    assert.equal(request.bodyUsed, false);
    assert.equal(h.timer.pending(), 0);
});

test('64 observations fit but a 65th protected record fails closed', async () => {
    const h = harness(() => Promise.resolve(response()));
    const values = [];
    const bindings = [];
    for (let i = 0; i < RECORD_LIMIT; i += 1) {
        const body = `${BODY} ${i}`;
        values.push(await h.window.fetch(URL_ONE, { method: 'POST', body }));
        bindings.push(await bound(h, expected(body), `consumer-${i}`));
    }
    assert.deepEqual(plain(await read(h, bindings[0], values[0], 'consumer-0')), { ok: true, text: '{"frames":[]}' });
    const body = `${BODY} ${RECORD_LIMIT}`;
    await h.window.fetch(URL_ONE, { method: 'POST', body });
    failure(await h.observer.bind(expected(body), 'overflow-consumer'), 'record_count_limit');
});

test('failed fetch records also consume the document record count budget', async () => {
    const error = new DOMException('synthetic-secret', 'AbortError');
    const h = harness(() => Promise.reject(error));
    for (let i = 0; i <= RECORD_LIMIT; i += 1) {
        await assert.rejects(h.window.fetch(URL_ONE, { method: 'POST', body: `${BODY} ${i}` }), (caught) => caught === error);
    }
    failure(await h.observer.bind(expected(`${BODY} ${RECORD_LIMIT}`), 'overflow-consumer'), 'record_count_limit');
});

test('8 MiB total unread body budget accepts four 2 MiB records then fails closed', async () => {
    assert.equal(TOTAL_LIMIT / RESPONSE_LIMIT, 4);
    const h = harness(() => Promise.resolve(response('x'.repeat(RESPONSE_LIMIT))));
    const bindings = [];
    const values = [];
    for (let i = 0; i < TOTAL_LIMIT / RESPONSE_LIMIT; i += 1) {
        const body = `${BODY} ${i}`;
        values.push(await h.window.fetch(URL_ONE, { method: 'POST', body }));
        bindings.push(await bound(h, expected(body), `consumer-${i}`));
        await settle();
    }
    const body = `${BODY} overflow`;
    await h.window.fetch(URL_ONE, { method: 'POST', body });
    await settle();
    failure(await h.observer.bind(expected(body), 'overflow-consumer'), 'total_body_limit');
    failure(await read(h, bindings[0], values[0], 'consumer-0'), 'total_body_limit');
});

test('transferred response bytes remain charged to the combined document budget', async () => {
    const h = harness(() => Promise.resolve(response('x'.repeat(RESPONSE_LIMIT))));
    for (let i = 0; i < TOTAL_LIMIT / RESPONSE_LIMIT; i += 1) {
        const body = `${BODY} ${i}`;
        const value = await h.window.fetch(URL_ONE, { method: 'POST', body });
        const binding = await bound(h, expected(body), `consumer-${i}`);
        const result = plain(await read(h, binding, value, `consumer-${i}`));
        assert.equal(result.ok, true);
        assert.equal(Buffer.byteLength(result.text), RESPONSE_LIMIT);
        assert.deepEqual(plain(h.observer.retireBody({
            id: binding.id, owner: `consumer-${i}`, document: binding.document,
        })), { ok: true });
    }
    const body = `${BODY} overflow`;
    await h.window.fetch(URL_ONE, { method: 'POST', body });
    await settle();
    failure(await h.observer.bind(expected(body), 'overflow-consumer'), 'total_body_limit');
});

test('pagehide disposes completed observations and blocks subsequent captures and reads', async () => {
    const { h, value, binding } = await observed();
    h.event('pagehide', { persisted: false });
    failure(await read(h, binding, value), 'document_disposed');
    failure(await h.observer.bind(expected(), 'consumer-one'), 'document_disposed');
    assert.equal(await h.window.fetch(URL_ONE, { method: 'POST', body: BODY }), value);
    assert.equal(h.calls.length, 2, 'disposal still allows the actual fetch');
    assert.equal(h.timer.pending(), 0);
});

test('persisted pageshow creates a fresh observation epoch and rejects every old binding', async () => {
    const { h, value, binding } = await observed();
    h.event('pageshow', { persisted: false });
    assert.equal((await read(h, binding, value)).ok, true);
    h.event('pagehide', { persisted: true });
    failure(await read(h, binding, value), 'document_disposed');
    assert.equal(h.timer.pending(), 0);
    h.event('pageshow', { persisted: true });
    assert.notEqual(h.observer.identity().document, binding.document);
    assert.equal(h.observer.identity().disposed, false);
    failure(await read(h, binding, value), 'document_mismatch');
    failure(h.observer.retireBody({ id: binding.id, owner: 'consumer-one', document: binding.document }), 'document_mismatch');
    failure(await h.observer.bind(expected(), 'consumer-one'), 'request_not_observed');
    // The exact same bytes and owner belong to a new request after restoration.
    assert.equal(await h.window.fetch(URL_ONE, { method: 'POST', body: BODY }), value);
    const fresh = await bound(h);
    assert.notEqual(fresh.document, binding.document);
    assert.deepEqual(plain(await read(h, fresh, value)), { ok: true, text: '{"frames":[]}' });
    assert.equal(h.calls.length, 2, 'restoration keeps the original fetch invocation intact');
    assert.equal(h.timer.pending(), 0);
});

test('late response and pending read from a retired epoch cannot poison restored observations', async () => {
    const native = deferred();
    const freshValue = response('{"fresh":true}');
    let first = true;
    const h = harness(() => first ? (first = false, native.promise) : Promise.resolve(freshValue));
    const original = h.window.fetch(URL_ONE, { method: 'POST', body: BODY });
    const old = await bound(h);
    const pending = read(h, old, response());
    await settle();
    assert.ok(h.timer.pending() > 0);
    h.event('pagehide', { persisted: true });
    h.event('pageshow', { persisted: true });
    failure(await pending, 'document_disposed');
    assert.equal(h.timer.pending(), 0);
    assert.equal(await h.window.fetch(URL_ONE, { method: 'POST', body: BODY }), freshValue);
    const fresh = await bound(h);
    // This would fail the old body budget if observed; its original caller still
    // receives the same Response while the new epoch stays usable.
    const late = response('x'.repeat(RESPONSE_LIMIT + 1));
    native.resolve(late);
    assert.equal(await original, late);
    await settle();
    assert.deepEqual(plain(await read(h, fresh, freshValue)), { ok: true, text: '{"fresh":true}' });
    failure(await read(h, old, late), 'document_mismatch');
    assert.equal(h.observer.identity().fatal, null);
    assert.equal(h.timer.pending(), 0);
});

test('lifecycle disposal unblocks a pending read and clears its deadline', async () => {
    const value = response(new ReadableStream({ start() {} }));
    const { h, binding } = await observed(value);
    const result = read(h, binding, value);
    await settle();
    assert.ok(h.timer.pending() > 0);
    h.event('pagehide', { persisted: true });
    failure(await result, 'document_disposed');
    assert.equal(h.timer.pending(), 0);
});

test('lifecycle disposal also unblocks a pending request-stream bind', async () => {
    const request = new Request(URL_ONE, {
        method: 'POST', duplex: 'half', body: new ReadableStream({ start() {} }),
    });
    const h = harness(() => Promise.resolve(response(new ReadableStream({ start() {} }))));
    await h.window.fetch(request);
    const binding = h.observer.bind(expected(), 'consumer-one');
    await settle();
    assert.ok(h.timer.pending() >= 2);
    h.event('pagehide', { persisted: true });
    failure(await binding, 'document_disposed');
    assert.equal(h.timer.pending(), 0);
});

test('a binding from another document cannot authorize this document record', async () => {
    const first = await observed();
    const second = await observed();
    assert.notEqual(first.binding.document, second.binding.document);
    failure(await read(second.h, second.binding, second.value, 'consumer-one', {
        document: first.binding.document,
    }), 'document_mismatch');
});

test('response headers and private failure text are excluded from observer return values', async () => {
    const value = response('{"frames":[]}', { headers: {
        'set-cookie': 'synthetic-cookie', 'x-csrf': 'synthetic-csrf', 'x-private': 'synthetic-private',
    } });
    const { h, binding } = await observed(value);
    assert.deepEqual(plain(await read(h, binding, value)), { ok: true, text: '{"frames":[]}' });
    const error = plain(await read(h, binding, value, 'synthetic-secret'));
    assert.deepEqual(error, { ok: false, error: 'request_owner_mismatch' });
    assert.equal(JSON.stringify({ binding, error }).includes('synthetic-'), false);
    assert.equal(h.observer.version, 1);
});
