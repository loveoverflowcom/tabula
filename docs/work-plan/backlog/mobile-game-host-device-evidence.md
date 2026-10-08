# Mobile native GameHost — implementation and device evidence owed

[Issue #81](https://github.com/loveoverflowcom/tabula/issues/81) and
[ADR-0043](../../adr/0043-native-mobile-gamehost.md) supersede the previous WebView
queue. The [ADR-0033 ledger](../../verification/mobile-game-host/README.md) remains
historical. Current mobile builds expose gameplay unavailable; native adapters
and native artifact/assets packaging are not implemented. See
[the current source spike/evidence](../../verification/mobile-native-host/README.md).

The [adapter-contract prototype](../../verification/mobile-native-host/adapter-prototype.md)
now supplies common admission/lifecycle decisions and an Android surface callback
binding. Its controlled port tests do not establish a native context or thread.
Keep the next engine/ABI decision distinct from this reviewable groundwork.

The [Android source skeleton](../../verification/mobile-native-host/android-skeleton.md)
now names the owning Kotlin port/factory/view and Rust backend/composition seams.
Every unavailable operation is explicit; production inventory is still empty.
The packaging task intentionally fails and no playable/device acceptance is closed.
Implement the slices below rather than treating TODOs or compilation as native readiness.

## Next small review boundaries

1. Review/pin a bounded Miniquad embedding API/patch: iOS attach to the existing CMP
   application rather than calling UIApplicationMain; Android surface/render-thread
   ownership with stop/join/generation-safe reopen. Keep the existing renderer.
   A repository-owned unsafe Rust need requires its own policy ADR first.
2. Implement one real local Chess native host per platform in the same CMP app,
   with typed launch/lifecycle/Back events and explicit surface/context/thread
   lifetime. Bundle native libraries and verified bounded game assets. No web
   fallback, alternate engine, per-frame Kotlin/Swift drawing or canonical-state bridge.
3. Run CMP open → native surface → tap/drag/animation → Back → CMP on Android and
   iOS; repeat open/close, recomposition, surface recreation, resize/DPI/safe area,
   input cancel, stale callbacks, context loss, suspend/resume and process death.
4. Add bounded CPU preload with real progress/cancel/error from the future room
   owner; upload GPU only on its valid thread/context. Exercise memory pressure,
   cache bounds and inactive rendering teardown.
5. On real devices, measure cold/warm first usable frame, frame-time p50/p95/p99,
   dropped frames, touch latency, CPU/RAM and background behavior. Record source/
   build/device/OS/backend/assets/cache provenance and agree device-specific budgets.
6. Replace deprecated common BackHandler only after checking the available native
   API on both targets. Preserve native voice/audio ownership and its separate checks.

Successful compilation, simulator operation or desktop shell screenshots do not
close device acceptance, #81, Phase 6 or store release. Report missing prerequisites
as BLOCKED/NOT_RUN and native implementation as NOT_IMPLEMENTED until it exists.
