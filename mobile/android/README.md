# Android

The thin Activity hosts the shared CMP shell. [ADR-0043](../../docs/adr/0043-native-mobile-gamehost.md)
requires native Rust/Macroquad gameplay in this same app; the former WebView host
and web-bundle packaging are retired. **Current app has no playable game** and no
web fallback. Native adapter/artifact packaging and real-device acceptance remain
blocked; see [the ledger](../../docs/verification/mobile-native-host/README.md).

Kotlin owns application UI/navigation and permitted native services, never rules,
projection, hashing or per-frame rendering. MainActivity calls installTabulaContent;
the existing native voice lifecycle stays separate from GameHost (ADR-0037).

Requires JDK 17 and Android SDK platform 37:

```bash
cd mobile
./gradlew :shared:testAndroidHostTest :previewApp:test :android:assembleDebug
```

The Android APK must pass the native-only packaging guard in CI. This does not
prove native gameplay. A next adapter must specify the JNI/surface/render-thread
owner and stop/join/reopen semantics, then exercise input cancel, rotation, DPI,
background/foreground, stale callbacks, context loss and process death. Pinned
Miniquad's standalone Android entrypoint is not CMP embedding acceptance.

Phase 6 exit, full device matches, battery/thermal, crash rate and Play Store
acceptance remain unproven. No network/auth or unsafe-policy exception is opened.
