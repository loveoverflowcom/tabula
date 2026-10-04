# ADR-0032: Compose Multiplatform mobile host with an embedded Rust/WASM game

- **Status:** accepted for the foundation scope below; the first-party embedding is delivered by [ADR-0033](0033-webview-gamehost-first-party-embedding.md) (its Android/iOS WebView execution NOT_RUN); voice and native services remain unimplemented and gated
- **Date:** 2026-10-04
- **Supersedes:** ADR-019 in part — only its mobile-gameplay prohibition of a WebView. The rest of ADR-019 stands: Tauri stays optional and is never required for gameplay.
- **Amends:** ADR-001 (what the mobile host languages may own), the Mobile row of doc 01 §1, doc 01 §7, the native-shell part of doc 04 §3.3, and the Phase 6 deliverables in doc 07
- **Invariants touched:** none relaxed. I-1, I-5/I-6, I-9, I-10, I-13 and I-15 are preserved; ADR-010, ADR-011, ADR-0027, ADR-0029, ADR-0030 and ADR-0031 stay in force

## Context

The repository documented one mobile direction: Macroquad gameplay compiled to a
`cdylib`/`staticlib`, wrapped by a "glue only" Kotlin/Swift host, with the shell
screens drawn on the game canvas. ADR-019 gave the reason: *gameplay must not depend
on a WebView*, because WebView input latency and rendering would become the
gameplay ceiling on the platform where most players are. `mobile/android` and
`mobile/ios` held only READMEs for that plan, and `apps/game-client` still builds
`rlib` only.

The repository owner has chosen a different split for mobile:

- **Compose Multiplatform (CMP)** owns the Android and iOS application UI and
  navigation.
- **The existing Rust/WASM game** runs in a **WebView** embedded in the game screen.
- **Voice, device permissions and native services** belong to the mobile host.
- Rust rules, presentation and renderer are unchanged. Nothing moves to Kotlin.

This is a decision, not a measurement. The ADR-019 risk is **not retired**. The
[#60 ledger](../verification/issue-60/README.md) records Mobile/WebView execution
as NOT_RUN, and ADR-0029 names mobile/WebView as outside its demonstrated scope.
Accepting the direction without that evidence is a recorded trade, so this ADR also
records what evidence the embedding change must produce.

Reading this ADR next to the others:

- ADR-011 already requires gameplay to be a **separate document** with its own
  `.wasm`. A WebView that loads that document is consistent with it; the game is not
  moved into the shell's runtime.
- ADR-0030 gives the one delivered local gameplay path (the standalone local
  authority). It is the only game the embedding change may initially load.
- ADR-0031 chose native OS-secure-store bearer credentials and forbids reusing a
  browser cookie as a native bearer or the reverse. A WebView-hosted game document
  that needs network authority sits between those two channels. The ADR does not
  resolve that; see "Not opened".

## Decision

### 1. Layers and ownership

| Layer | Owns | Does not own |
|---|---|---|
| **CMP shell** (`mobile/shared`, Kotlin) | App screens, navigation, theming from generated tokens, composition of the game screen | Rules, legality, projection, canonical state, replay, wire decisions |
| **GameHost** (platform WebView, later change) | Presenting the Rust/WASM game document inside the game screen; its lifecycle | Any decision about the match |
| **Mobile host services** (Kotlin/Swift) | Voice capture and session audio, device permissions, secure credential store, push, deep links, OS lifecycle | Game meaning |
| **Rust** (unchanged) | `apply`/`project`/`view_event`, presentation, `RenderList`, Macroquad renderer, WASM game build | App chrome on mobile |

The shell and the game communicate through the `GameHost` interface only. Data that
crosses it is a launch request in and typed lifecycle events out. Presentation or
canonical state never crosses it (I-5/I-6/I-10).

### 2. Language policy (amends ADR-001)

ADR-001 said JS/Kotlin/Swift are "only platform glue". On mobile that limit is
replaced by an explicit ownership rule: Kotlin and Swift may own the **mobile
application UI, navigation, WebView hosting and device services**. They must not
own, duplicate or re-derive **game rules, legality, turn order, hidden-information
projection, state hashing, replay, or protocol decisions**. If Kotlin needs to show
whose turn it is, it displays what Rust projected; it does not compute it. The core
language decision of ADR-001 for rules, protocol, server and clients is unchanged.

### 3. One mobile tree

`mobile/` is the single mobile root: a Gradle build with `mobile/shared` (the CMP
library), `mobile/android` (the Android application) and `mobile/ios` (the Xcode
host of the same library). No second mobile application is created. The earlier
`cargo-apk`/`cargo-ndk` Macroquad-native Phase 6 path is not built and is
superseded for mobile by this ADR. Desktop and web are unchanged: desktop remains a
native Macroquad binary and the web shell and gameplay remain ADR-011's two
documents. `apps/game-client`'s crate types are not changed by this ADR.

### 4. One design authority (extends ADR-0027)

`tokens.toml` stays the one authored contract. `cargo xtask gen-tokens` also emits a
Kotlin adapter, `mobile/shared/.../design/TabulaTokens.kt`, which `check` verifies is
current like the Rust, CSS and JSON adapters. The CMP theme only maps those
generated values onto Compose; it introduces no colour, size or role.
`cargo xtask check-no-raw-colors` now also scans `mobile/` Kotlin and rejects
`Color(...)` constructors, palette colours and hex literals outside the generated
file. The shell follows `docs/ui/screens/foundation.md` (compact layout, 44 dp
targets, safe areas). The generated adapter carries colours, space, shape, type,
state layers, density and focus; motion and game-art tokens are added when a named
consumer needs them.

### 5. Scope opened for this chain of changes

Opened:

1. The CMP project for Android and iOS with a minimal shell and navigation, and a
   reserved game slot behind the `GameHost` interface (this change).
2. The Kotlin token adapter and its freshness/raw-colour enforcement (this change).
3. Later, one change at a time and each reviewed on its own evidence: a WebView
   `GameHost` that loads the **existing local standalone game document** of ADR-0030
   with typed lifecycle events; and the smallest host interfaces a further change
   needs (for example lifecycle). Each adds only the interface it uses.

Not opened by this ADR:

- **Networked play in the WebView.** Online/resume attachment, match grants and any
  credential reaching the embedded document need their own ADR against ADR-0031
  (native bearer in the secure store; no browser cookie as native authority; no
  credential in a URL; grants memory-only). Until then the embedded game is local.
- **Voice.** Phase 8 and ADR-016 are unchanged. `tabula-voice` and its provider stay
  gated; the host owning microphone permission is a direction, not an interface that
  exists. No voice or permission interface is created before its change.
- A native catalog, accounts, push, deep links, store billing, asset-delivery
  services, and Phase 3, 4, 5, 6 or 8 exit. No phase gate is removed or declared
  complete, and no installed app or store acceptance is claimed.

### 6. Session policy

ADR-0031 is unchanged. This ADR adds no credential storage, transport, cookie or
bearer handling. The Android manifest requests no permissions and the iOS host
declares none.

## Evidence the embedding change must produce

ADR-019's risk applies to the first change that actually embeds the game. That
change is not accepted on direction alone. It must provide, for the shipping
Android WebView and iOS WKWebView, executed evidence (not compilation) for: input
latency for touch and pointer events compared with the existing web document;
frame pacing on a mid-tier device; WASM load/instantiate cost and memory; WebGL
availability and context-loss recovery; suspend/resume and process death with the
`GameHost` lifecycle; safe areas, orientation and the soft keyboard; and the
back gesture. Missing targets are reported as BLOCKED or NOT_RUN, never inferred
from another platform. ADR-0029's lifecycle and trust contract (generation-scoped
mount/dispose, bridge schema and origin validation) is the starting point for the
bridge.

## Consequences

Easy: platform-native navigation, accessibility, text input and system integration
for the app chrome; Android and iOS share one UI codebase; the Rust game artifact is
the same one the web uses, so there is no mobile-only renderer to maintain; the
shell can ship screens before any game is embedded.

Hard: gameplay latency on mobile now depends on the WebView, which no measurement yet
supports; there are two UI implementations of the screen specifications (Leptos and
Compose) in place of Leptos and the canvas; a Kotlin/Swift host must be built, tested
and signed for two stores; and Compose cannot be compiled to a framework or run on an
iOS device without macOS tooling.

Enforcement that changes: `gen-tokens` and `check-no-raw-colors` cover Kotlin. CI
builds and tests the Android shell. CI does not yet build the iOS framework or the
Xcode project; that needs a macOS job and is a named gap, not a passing check.

## Revisit when

Revisit if embedded-WebView evidence for either shipping platform shows touch-to-
visible latency materially worse than the web document on the same device, sustained
frames below the game's authored motion budget on the mid-tier device, or a WebView
lifecycle failure that cannot be recovered by remount. In that case this ADR is
superseded and ADR-019's native-renderer path is reconsidered with the measurements
recorded. Also revisit when networked mobile play is proposed (an ADR against
ADR-0031 is required first).
