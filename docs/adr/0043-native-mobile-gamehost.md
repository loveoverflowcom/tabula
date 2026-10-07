# ADR-0043: Native Macroquad gameplay in the CMP mobile app

- **Status:** accepted direction from [issue #81](https://github.com/loveoverflowcom/tabula/issues/81); adapter implementation and device acceptance blocked. This change retires mobile WebView selection and packaging; it does not deliver native gameplay.
- **Date:** 2026-10-06
- **Supersedes:** ADR-0032/0033 only for mobile WebView/WKWebView gameplay, the JavaScript bridge and mobile web-bundle pipeline. Their CMP shell, token authority and ownership boundaries remain. ADR-0037's native voice scope remains unchanged.
- **Invariants:** I-1, I-5/I-6, I-9, I-10, I-13 and I-15 unchanged. No exception to `#![forbid(unsafe_code)]` is granted.

## Decision and current implementation

CMP owns Android/iOS application UI, navigation and permitted device services. The
existing Rust rules, projection, local runtime, presenter, `RenderList` and
Macroquad renderer must run natively in the **same application**, through a native
`GameHost`. Kotlin/Swift neither calculate rules nor receive canonical hidden
state or per-frame drawing commands. Web retains its separate Rust/WASM Macroquad
document (ADR-011); desktop retains its native runtime. A system browser for
future native login remains subject to ADR-0031.

The repository owner's direction does not depend on proving WebView performance
inadequate. Native performance still needs measurement. There is no promise of
60/120 FPS and no alternative engine chosen by this ADR.

At `develop@c6d55a6fc3b326e14a466b6e9d988f897bb579e5`, both mobile entrypoints
selected WebView hosts and packaged an HTML/JavaScript/WASM document. This change
removes those selections and mobile packaging consumers. Until a native adapter
exists, the mobile app shows gameplay unavailable and offers no game launch. It
does not fall back to web. The shell's placeholder reports `Failed` if invoked;
it cannot report `Ready`. Historical WebView evidence stays dated and scoped to
its original implementation.

## Source spike and blockers

The lockfile pins Macroquad **0.4.16** and Miniquad **0.4.11**. The checksum-bound
published source is reviewed in the [upstream report](../verification/mobile-native-host/upstream-source-review.md).

- Android's upstream `QuadSurface`/JNI path supplies a native surface and a render
  thread, but its standalone activity/runtime ownership is not evidence of CMP
  embedding. Stop/join, repeated open/close, generation fencing, input cancel and
  surface recreation need a reviewed adapter and real execution.
- iOS `miniquad::start` enters `UIApplicationMain`. SwiftUI/CMP already owns that
  application loop; calling the standalone entrypoint does not attach a game to
  its game screen. The pinned package exposes no supported child-view/controller
  attach/detach API. A bounded upstream extraction/patch must be reviewed and
  pinned before integration. A separate native demo app does not satisfy #81.

No engine fork, ABI, native library or performance harness is represented as
implemented by this change. A future patch must name its maintenance/upstreaming
cost and preserve the current renderer. If it requires repository-owned unsafe
Rust, its ADR must explicitly change the policy and fence that ownership; this
ADR does not silently authorize it.

## Executable adapter-contract prototype

The next bounded #81 draft implements the common `NativeRuntimeOwner`,
`NativeHostSession` and `NativeGameRuntime` control seam, plus an Android
`SurfaceHolder`/touch binding. The [prototype ledger](../verification/mobile-native-host/adapter-prototype.md)
records its source, tests and backend decision gates. It is not a playable engine
adapter: no shipping port, native artifact/assets package or iOS child-controller
is delivered. Both production entrypoints remain unavailable with empty runtime
inventory. The existing unsafe-code prohibition and all unrelated gates stand.

## Contract for the native backend

The existing `GameHost` seam remains launch/preferences/capabilities in and
`Ready`/`Failed`/`Exited` out, plus `GameBackPort`. The native adapter must specify
the surface/context owner, event loop/render thread, valid callback thread and
error mapping. No panic may cross an FFI boundary. Launch facts are bounded and
validated; a caller's requested game never grants authority.

Runtime, surface and match lifetimes are separate. Recomposition, resize, a new
event lambda or a recreated surface must not create another match/render loop.
Disposal retires callbacks before releasing resources and waits for render-thread
termination before reopen; events from a retired generation have no effect.
Only the current interactive surface can issue `Ready`, after required resources
and input are usable. Failure cleanup and Back have explicit outcomes. Process
death discards this unsaved local match. Suspending rendering does not change
rules-owned clock semantics or network authority.

Safe area and DPI are applied once in logical coordinates. Touch cancellation,
orientation, background/foreground, surface/context loss and memory pressure
need real platform checks. Native voice/media stays outside the gameplay bridge;
this change adds no permission, audio-session, auth or network implementation.

Preload is a separate next slice: verified bounded pack bytes and CPU decoding
can be prepared from a future room/lobby with real progress, cancellation and
failure. GPU upload/warmup occurs only with the correct context/thread. Cache
limits, memory-pressure eviction and inactive rendering shutdown need tests;
neither an always-running hidden GPU nor fabricated preload progress is allowed.

## Evidence and release boundary

The [ledger](../verification/mobile-native-host/README.md) records actual commands,
source provenance, rendered CMP preview images and limits. Packaging/configuration
guards reject mobile web gameplay; they are not native execution evidence.
Desktop CMP tests use a labelled stand-in and cannot establish Android/iOS input,
GPU rendering, frame pacing or native lifetime safety.

Before an adapter is described as playable, Android and iOS must each execute CMP
open → native Macroquad surface → tap/drag/animation → Back → CMP, repeated
open/close and the lifecycle cases above. Shipping acceptance additionally needs
real devices: cold/warm first usable frame, frame-time p50/p95/p99, dropped frames,
touch latency, CPU/RAM and background behavior, with build/commit/device/OS/backend,
asset/cache state and a device-specific budget. Simulator/build-only results are
reported separately. Missing prerequisites remain `BLOCKED`/`NOT_RUN`.

This bounded direction/retirement PR does not close #81 or Phase 6. Native
adapters, native artifact/assets packaging, preload and device acceptance are
required follow-up slices. Production networked mobile play, native accounts,
voice authority, clocks/private effects, store release and other phase gates
remain subject to their own contracts and evidence.
