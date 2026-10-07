# Native GameHost adapter contract prototype

Date: 2026-10-07 UTC. Fresh base: `develop@f9e31cfad11f77f9a017c67726bc8f9fcf74c7c9`.
Refs [#81](https://github.com/loveoverflowcom/tabula/issues/81). This draft delivers an
executable control/lifecycle seam and Android surface callback source. **It does not
deliver playable Macroquad embedding, a native artifact or iOS platform parity.**

## Implemented boundary

- `NativeRuntimeOwner` reserves one worker before calling the injected port. The owner
  survives screen entries; a rejected/unpackaged launch makes zero backend calls
- `NativeGameRequest` snapshots capabilities, requires an explicitly packaged opaque id
  and explicit theme, and translates the shell's resolved reduced-motion choice to a
  boolean. Public discovery and historical `BundledGame.entry/query` grant nothing
- `NativeHostSession` is a pure coordinator separate from the historical document
  `GameSession`. `NativeGameRuntime` runs its typed effects on the checked owner thread
  and fences queued commands, Ready and service enables when reentrant callbacks retire
  them. Throwing host callbacks are counted and cannot strand native cleanup
- Runtime identity and surface revision are separate. Resize/DPI, loss/recreation and
  foreground boundaries retire resource/input/frame tickets without starting a match
- Context, verified required resources, usable input and a **subsequent flushed gameplay
  frame** on the current foreground lease are required for Ready. A queued draw, loading
  frame, early/replayed frame or stale callback is insufficient. Boot time is host
  monotonic time, not a backend-supplied benchmark
- Input forwards bounded local physical-pixel facts. Invalid packets, surface loss,
  suspension, failure and disposal cancel pointers. Back consumes only an interactive
  current surface; Rust retains its leave confirmation and all game meaning
- Keep-awake requests are launch-capability-gated and monotonic within the runtime.
  Reconfiguration/background/failure/disposal release the setting. No new service,
  credentials, voice/audio route, projection, canonical state or RenderList is added
- Disposal retires normal callbacks immediately and requests asynchronous Stop. Only the
  exact retiring worker's **joined/resource-released Stopped** acknowledgement releases
  admission. A failed/missing acknowledgement leaves reopen unavailable; no timeout
  fabricates termination. Failed is shell-owned recovery (`shownByGame=false`)

`AndroidNativeSurfaceBinding` implements actual `SurfaceHolder.Callback` and multi-touch
callbacks over a dedicated `SurfaceView`. It owns neither the holder's Surface nor the
render thread. The native port must acquire/release its own window references.
`fenceSurface` is a separate immediate/idempotent operation, including reentrant view
destruction during a host callback; it precedes queued Detach cleanup. It must fence
surface access before `surfaceDestroyed` returns. This follows the
[Android callback contract](https://developer.android.com/reference/android/view/SurfaceHolder.Callback).
The binder applies local view dimensions/DPI once; outer CMP already consumes safe insets.
It has no frame ticker, synthetic Ready, JNI implementation or production selector.
A throwing/unproven fence cannot be admitted as a production Android adapter; worker
callbacks cannot wait synchronously for UI completion while the UI fences that worker.

The existing CMP binding now retires a changed Back port independently of runtime
disposal and immediately forwards the current lifecycle state. Its desktop regression
uses the historical simulated host; it is separate from the new native-port tests.

## Backend decisions still required

The renderer stays Rust + Macroquad. No engine migration, vendored backend, unsafe
exception, dependency update or hidden web fallback is included. Both Android/iOS
entrypoints retain the unavailable default and empty packaged runtime inventory.

The published pins remain Macroquad **0.4.16**, Miniquad **0.4.11**. The earlier
[checksum-bound report](upstream-source-review.md) is historical; this review used
published source pages, not a fresh local archive checksum assertion:

- [Macroquad 0.4.16 entry/context](https://docs.rs/crate/macroquad/0.4.16/source/src/lib.rs)
  constructs its private Stage/global Context inside Miniquad start and asserts one
  owning thread. It exposes no supported foreign-loop Stage factory
- [Miniquad 0.4.11 start](https://docs.rs/crate/miniquad/0.4.11/source/src/lib.rs)
  accepts neither an existing surface/controller nor a stoppable runtime handle
- [Pinned Android](https://docs.rs/crate/miniquad/0.4.11/source/src/native/android.rs)
  discards its worker JoinHandle, waits for initial surface/size without preserving
  destroy, and uses unfenced thread-local JNI senders. Surface replacement alone is
  not stop/join. Public quit flags are not consumed by this backend
- [Pinned iOS](https://docs.rs/crate/miniquad/0.4.11/source/src/native/ios.rs)
  enters UIApplicationMain and creates its own window. Existing view getters cannot
  bootstrap a child game controller in the CMP app. `view_ctrl` also appears to remain
  null because bootstrap initializes only `view`; this is a source inference, not
  platform execution
- Current upstream Miniquad
  [`39c928fb7e09fd44e45f37fe9a98503968bca412`](https://github.com/not-fl3/miniquad/tree/39c928fb7e09fd44e45f37fe9a98503968bca412)
  retains the Android ownership/termination and iOS application-loop barriers. Its iOS
  main-thread MTKView/CADisplayLink, paused drawing and view-bounds sizing improvements
  must not be mistaken for the pinned implementation or a supported attach API

Review before adding an actual backend:

1. Pin a bounded supported upstream embedding extension: Android cancellable worker,
   thread-scoped callbacks, synchronous surface fence and asynchronous stop/join;
   iOS child-view/controller initialization without UIApplicationMain
2. Define Macroquad context destruction on its owner thread before GPU/surface release,
   panic containment and ABI ownership. Rust's
   [unsafe-code lint](https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#unsafe-code)
   also covers exported symbol attributes. A safe-looking exported JNI body does not
   bypass the workspace prohibition; any policy change requires an explicit ADR
3. Decide native-screen/controller versus child-view integration, overlay/navigation
   trade-offs, patch maintenance/upstreaming owner and per-platform packaging
4. Resolve the first-party local Chess runtime through registry/presentation and the
   existing verified asset pipeline. No Kotlin rules, fake backend, online/mobile auth
   or fresh game authority is authorized by a surface adapter
5. Keep preload separate: real bounded/cancellable CPU preparation, valid-thread GPU
   upload, memory-pressure/cache policy and no hidden always-running GPU in the lobby

## Checks and limits

Focused reproduction with the **official Kotlin 2.4.20 compiler**, JUnit 4.13.2 and
Hamcrest Core 1.3 (all kept outside the checkout):

```bash
KOTLIN_HOME=/path/to/kotlinc JUNIT_JAR=/path/to/junit-4.13.2.jar \
HAMCREST_JAR=/path/to/hamcrest-core-1.3.jar ANDROID_JAR=/path/to/android-37/android.jar \
  bash tools/test-mobile-native-contract.sh
```

The runner compiles the actual pure shared source and commonTest class with
`-Werror -jvm-target 17` and executes JUnit; there are no replacement coordinator or
Android/Compose stubs. Its optional second compilation checks the actual Android
callback source against the supplied SDK jar. This is narrower than the configured
Gradle Android/Compose gate and never an APK, GPU/context, JNI or device run.

The Android platform archive was obtained from Google's official repository metadata:
`platforms;android-37.0`, `platform-37.0_r02.zip`, metadata SHA-1
`ed8ebf7f8822a4de5686d427f237d2fa30ff7410` matched before extracting `android.jar`.
No installed system SDK, build-tool configuration or production artifact is inferred.

### Executed local checks

| Command / selected scope | Result | Evidence limit |
|---|---|---|
| `bash tools/test-mobile-native-contract.sh` with Kotlin 2.4.20, JUnit 4.13.2, Hamcrest 1.3 | **PASS 47 tests**, 0 skipped/failed; `-Werror -jvm-target 17` | Actual coordinator with controlled native port; no GPU/JNI/device |
| Same runner with checksum-verified Android 37 SDK jar | **compiled PASS** | Actual SurfaceHolder/touch binding source, not APK/interaction execution |
| `python3 tools/check-mobile-native-policy.py` | **PASS** | Source/config, **0 real APK/app artifacts** inspected |
| `python3 -m unittest discover -s tools/tests -p test_mobile_native_policy.py -v` | **PASS 5 tests** | Adversarial policy fixtures, not native frames |
| `node --test apps/game-client/web/tests/*.test.cjs` | **PASS 122 tests**, 0 skipped/failed | Existing web host/loader/bridge tests; no new WASM compilation or browser pixels |
| `python3 .agents/skills/tabula-engineering/scripts/check_skills.py` | **PASS** | Structural skill-map checks |
| `bash -n tools/test-mobile-native-contract.sh`; `git diff --check` | **PASS** | Shell syntax and whitespace |
| `cargo xtask check` | **BLOCKED**, exit 127: `cargo: command not found` | Authoritative portable core gate not executed; unchanged Rust is not a substitute for a pass |
| `./gradlew --console=plain --offline :shared:testAndroidHostTest :previewApp:test :android:assembleDebug` from `apps/mobile` | **BLOCKED** at wrapper download, `Network is unreachable` | No selected Gradle tests or APK; JDK17/configured complete SDK are also absent |
| iOS framework/Xcode and real Android/iOS interaction/performance | **NOT_RUN / NOT_IMPLEMENTED** | Linux host, no iOS adapter, native backend/artifacts or real devices |

Host: Linux x86_64, OpenJDK 21.0.12.1. The focused compiler targets JVM17 but
does not replace the configured JDK17 Gradle build. The added changed-Back-port
desktop Compose regression exists but is **NOT_RUN** because Gradle bootstrap is
blocked. Nothing from the earlier macOS ledger is relabelled as this tree's evidence.

Independent read-only source review and executable probes checked exception-safe
cleanup, reentrant surface fencing, stale/queued notifications, foreground tickets,
and truthful stop-ack admission. Confirmed defects were corrected and retained as
regressions. This is scoped review, not native device/phase acceptance.

Missing prerequisites remain BLOCKED/NOT_RUN. Both real native gameplay paths, ABI/library/asset
packaging, preload execution, device interaction/performance and iOS callback source
remain NOT_IMPLEMENTED/NOT_RUN. Source/config policy checks inspect **0 real APK/app
artifacts**. No GitHub CI checking/waiting is part of this owner-requested local review.

This draft does not close #81, native device acceptance, store release or Phase 6.
