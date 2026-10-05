# ADR-0033: WebView `GameHost` for the first-party packaged game

- **Status:** accepted for the first-party, packaged, local-play scope below; **Android WebView and iOS WKWebView execution is NOT_RUN** (see the evidence ledger). This is an embedded first-party game, not a dynamic plugin system.
- **Date:** 2026-10-04
- **Extends:** ADR-0032 §5 item 3 (the WebView `GameHost` that loads the ADR-0030 local document, with typed lifecycle events). Supersedes nothing.
- **Invariants touched:** none relaxed. I-1, I-5/I-6, I-9, I-10, I-13 and I-15 are preserved; ADR-010, ADR-011, ADR-0029, ADR-0030, ADR-0031 and ADR-0032 stay in force.

## Subsequent bounded native voice slice

[ADR-0037](0037-native-mobile-voice-client.md) later adds native host-only network/
microphone permission and a separate VoiceClient, while preserving this game
WebView’s network/capture denial and unchanged bridge grammar. The no-permission/
no-voice descriptions below record this ADR’s original local embedding scope;
they do not authorize voice through the page or open production/backend gates.

## Context

ADR-0032 chose Compose Multiplatform (CMP) for the app shell and a WebView for the existing Rust/WASM
game, and reserved a `GameHost` seam. It deliberately stopped there: no WebView, no bridge, no packaged
game. This change builds that embedding for the one game ADR-0030 already delivers (local two-player,
standalone authority) and nothing else.

Three facts from the code shaped the design:

- The game's loader (`resources.js`) is a deliberate security boundary: it requires a **same-origin,
  http(s)** document, a secure context (WebCrypto SHA-256), content-addressed names whose SHA-256 it
  verifies before the bytes are used, per-file and inventory limits, bounded in-flight loads and a
  bounded cache. `file://` has no usable origin and would force that boundary off.
- The game document already owns its lifecycle (generation/abort, loading/error/retry, an explicit
  Return) and its launch configuration (`/play/local/?...`, validated by the registry and by
  `launch-options.js`). Reimplementing any of it in Kotlin would duplicate authority.
- `tabula-registry` already decides which game has a local document and what its validated default
  launch is (`ErasedGame::normalize` + `resolve_with_locale`). The shell must not carry that (I-9).

## Decision

### 1. Serving path: a virtual origin over the app bundle, no file access, no network

The game document is packaged in the app and served to the WebView by **request interception**, so the
loader sees a real same-origin secure document and runs unchanged:

| Target | Document URL | Mechanism |
|---|---|---|
| Android | `https://game.tabula.invalid/play/local/?…` | `WebViewClient.shouldInterceptRequest` answers every request from `assets/tabula-game/` or refuses |
| iOS | `tabula-game://app/play/local/?…` | `WKURLSchemeHandler` answers from the app bundle's `tabula-game/` folder or refuses |

`.invalid` never resolves, so a request that somehow escaped interception could not reach a server. The
Android manifest declares **no** permissions (no `INTERNET`); the WebView sets `blockNetworkLoads`,
disables file and content access, multiple windows and geolocation, and enables remote debugging only
for a debuggable build.

`BundlePaths` (shared Kotlin, used by both targets) is the only path policy: only `/play/...`; only the
extensions the document loads (`html js css wasm png ttf`); no percent-decoding, backslash, dot or empty
segment; a bounded depth. `tabula-games.json` and everything else in the bundle is native-only.
Responses carry `X-Content-Type-Options: nosniff`, `Cross-Origin-Resource-Policy: same-origin`,
`Referrer-Policy: no-referrer`, immutable caching only for `resources/<sha256>.<ext>`, and, on the HTML
document, a CSP: `default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self';
font-src 'self'; img-src 'self' data: blob:; connect-src 'self'; base-uri 'none'; form-action 'none';
frame-ancestors 'none'`. Navigation is locked to the bundle origin and `/play/`; anything else is
cancelled, never opened in another app.

**The loader is unchanged**, and so are its origin, hash and size/count limits. Nothing is disabled to
make the demo run. The desktop-Chrome check serves the real staged document under exactly this policy
and the real game starts, loads and plays.

**Known unverified point (iOS).** The loader needs `crypto.subtle`, which WebKit exposes only in a secure
context, and whether a custom-scheme document counts as one is not established on this repository's
hardware. The loader fails closed with a visible error if it does not. The recorded fallback, if
macOS testing shows it, is a loopback `http://127.0.0.1` document served by the app (ATS permits local
networking); weakening the loader is not an option. The URL-scheme handler also relies on streamed
response bodies, which the loader requires.

### 2. What is packaged and who decides

`cargo xtask stage-mobile-game` stages the **same integrated document** as `stage-local-play` (the same
HTML with SRI-pinned, content-addressed scripts, the same manifest-listed WASM, fonts and pack) into
`target/tabula-mobile-game/play/local/`, pruned to `index.html` and the content-addressed files it and its
manifest/stylesheets reference (7.3 MB → 1.8 MB; the pruning fails if a reference is missing). It also
writes `tabula-games.json`, derived **from the registry**: games that have a local document, with the
registry's validated launch query and display names. The shell treats the query as opaque; there is no
game id in any Kotlin or Swift branch. Gradle packages the directory as Android assets
(`-Ptabula.requireGameBundle=true` makes a missing bundle a build failure) and an Xcode run-script phase
copies it into the app. Only first-party, build-time bundled games exist here.

### 3. The bridge: typed, bounded, capability-gated, three jobs

A single control channel between `GameHost` and the game document, protocol version 1, one JSON object
per message, at most 4096 bytes. The page sees it only as `window.TabulaHostNative`, an origin-restricted
port: Android `WebViewCompat.addWebMessageListener` (allowed origin = the bundle origin, main frame only,
and the host re-checks origin and frame on every message), iOS a `WKScriptMessageHandler` plus a
main-frame document-start script, with the host re-checking `securityOrigin`. A page without the port
(any ordinary browser) behaves exactly as before.

- **Lifecycle.** Page → host: `hello`, `ready{gen,bootMs}`, `failed{gen,code,detail}`, `exit{gen}`.
  Host → page: `init`, `suspend`, `resume`, `dispose`, `back-requested`.
- **Preferences.** `init` carries `{theme, motion, locale}` and the game starts only after it. They replace
  just those three launch fields; every game option still comes from the registry-validated query. The
  board reads them **at launch only**: a later OS change applies to the next game.
- **Host services.** `service{gen,id,name,enabled}` → `reply{gen,id,ok,code}`. The launch grants an
  explicit capability set (`FirstPartyCapabilities`); a request outside it is denied by the page and
  again by the host. The only service is `keep-awake` (no OS permission). Voice, microphone, push,
  files, share and credentials do not exist on the bridge.

Every document load says `hello` and gets a new **generation**; every later message carries it, so a
message from an earlier document is dropped. Service request ids only increase per document, so a
replay is dropped. Inbound page text is decoded by a strict reader that rejects duplicate keys, floats,
exponents, leading zeros, control characters, surrogate escapes, extra or missing fields and nesting
deeper than the grammar. A host never answers a silent page: no `hello` within 10 s is a host failure,
and the page itself fails closed after 5 s without `init`. The wire grammar is pinned by one vector file
(`apps/game-client/web/tests/bridge-vectors.json`) executed by both the page's `host-bridge.js` and the
Kotlin codec.

**No session credential reaches JavaScript** (ADR-0031 is untouched: this change stores, transports and
exposes none), and **no state, projection or render command crosses the bridge**: the board is drawn
by the Rust/Macroquad runtime inside the page. Per frame there is zero bridge traffic.

### 4. Lifecycle ownership

`GameSession` (shared Kotlin, pure, no I/O) owns the surface lifecycle for both targets: phases,
generations, the granted capabilities, keep-awake state, and what to send or report for every input.
Platform runtimes only execute its effects and own the WebView.

- **One runtime per composition entry.** `rememberGameRuntime` builds the runtime once when the game
  screen enters the composition, delivers events to the latest `onEvent`, and `BindGameRuntime` disposes
  it exactly once on leaving. A recomposition, resize or new lambda cannot rebuild it; only leaving and
  re-entering can. All three hosts (Android, iOS, desktop test host) use this one function.
- **Open / close / reopen.** Reopening builds a fresh runtime and a fresh match; local matches are not
  saved (ADR-0030). Disposal is idempotent, destroys the WebView, removes the bridge port and releases
  keep-awake.
- **Back.** Routed to the page only while a match is live (it shows its own leave confirmation; a second
  press dismisses it); while loading, failed or closed the shell pops at once.
- **Suspend/resume.** The host stops the page drawing and clears held input; **the game clock keeps
  wall-clock time exactly as for a hidden browser document (ADR-0030)**. Suspend is not pause.
- **Failure.** A failure the game explains with its own overlay (`shownByGame`) is only recorded. A host
  failure (no handshake, load error, renderer gone, bridge unsupported) replaces the surface with a shell
  panel; **Try again** mounts a new runtime and starts a new game.
- **Late events.** Anything from a closed session, an earlier generation or a replayed request is
  dropped, and a closed session has no authority left.

### 5. Desktop CMP target: testing only

`mobile/shared` gains a `desktop` JVM target and `mobile/previewApp` runs and tests the shared shell on a
laptop (`./gradlew :previewApp:run`, `:previewApp:test`). The game page there is a **labelled
simulation** speaking the real wire protocol through the real `GameSession`; it draws no game. It is not
a desktop product, an installer or evidence about any WebView.

## Not opened

- **Dynamic plugins, a marketplace, a third-party sandbox, remote updates.** The document is bundled at
  build time and trusted as first-party code. The same-origin WebView is **not** a sandbox for untrusted
  games, and nothing here designs one.
- **Networked or resumed play, accounts, credentials in the WebView** (ADR-0032 requires an ADR against
  ADR-0031 first), **voice, push, deep links, store billing, asset-delivery services.**
- Any further host service: each needs its own named consumer.
- Phase 3, 4, 5, 6 or 8 exit. This does not complete Phase 6.

## Evidence

See [the ledger](../verification/mobile-game-host/README.md). In short: the page-side bridge, the Kotlin
session/codec/path policy, the shell's navigation and recomposition rules, the packaging and the real
game document under the host's CSP in a phone-sized desktop Chrome are executed. **Not executed:**
the Android WebView, the iOS WKWebView and Xcode build, any device, input latency on touch, frame pacing
on a mid-tier device, WASM instantiate cost and memory on a device, WebGL context-loss recovery in a
WebView, process death, soft keyboard, orientation on hardware and the system back gesture. ADR-0032's
embedding evidence requirement is therefore **still owed** for both shipping targets.

## Consequences

Easy: one verified loader and one document for web and mobile; a small, closed bridge that cannot carry
rules or credentials; the shell and lifecycle logic is shared and testable on a laptop; adding a second
first-party game is a registry entry plus bundle content, not host code.

Hard: two WebView implementations (Android, iOS) of one contract, one of which cannot be built or run
without macOS; a document that must keep working under a strict CSP and a custom origin; keep-awake and
suspend semantics that differ per OS and are unproven on devices.

## Revisit when

ADR-0032's revisit conditions fire on real-device evidence; the iOS custom-scheme secure-context check
fails (adopt the loopback fallback above); a second host service or a networked mode is proposed (new
ADR); or a third-party game is proposed (a sandbox ADR, not an extension of this one).
