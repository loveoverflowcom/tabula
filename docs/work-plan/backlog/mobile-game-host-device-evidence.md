# Mobile `GameHost` — device evidence still owed

Status: deferred evidence work. The embedded first-party host and its desktop/Chrome evidence are
recorded in [the ledger](../../verification/mobile-game-host/README.md) and
[ADR-0033](../../adr/0033-webview-gamehost-first-party-embedding.md). ADR-0032's embedding evidence
requirement remains **unmet** for both shipping targets.

## Next small review boundaries

1. **Android WebView on an emulator or device.** Launch the debug APK built with
   `-Ptabula.requireGameBundle=true`, play a real move, and record logcat `TabulaGameHost` lines (they
   print the runtime's creation, readiness and disposal). Check: open/close/reopen leaves
   `created == disposed + 1` while a game is open; rotation keeps the match; Home/overview and return
   suspend and resume without a second runtime; the system back gesture shows the leave confirmation;
   killing the renderer shows the shell panel and Try again starts a new game; `chrome://inspect` shows
   no CSP violation. Then measure touch-to-visible latency against the web document on the same device,
   frame pacing on a mid-tier device, and WASM instantiate time and memory.
2. **iOS on macOS.** Build `TabulaApp.xcodeproj`, run in the simulator, and first confirm that the
   custom-scheme document finds `crypto.subtle` and streamed fetch bodies. If not, take the loopback
   fallback in ADR-0033 (a new decision record), never a weaker loader. Verify the "Package game bundle"
   phase, the origin check on script messages and the lifecycle mapping on a device.
3. **WebGL context loss** in each WebView: reproduce, confirm the page's recovery overlay and the shell's
   Try again, and record whether a remount is needed.
4. **Process death** on Android: confirm the app returns to Home with no stale match.
5. **Landscape layout** of the game page on phones (the page's action buttons overlap its side panel at
   860×412) and soft-keyboard behaviour: a game-page change, not a host change.
6. Replace the deprecated `BackHandler` with `NavigationEventHandler` once its common API is verified on
   both targets.

Do not infer from the desktop Chrome check, the simulated page or a successful APK build that any of the
above passes. Record `BLOCKED` or `NOT_RUN` with the reason for any target without an environment.
