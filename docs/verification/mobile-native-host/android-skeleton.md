# Android native GameHost source skeleton

Date: 2026-10-08. Base: `develop@6d31cef51f9f186e6bd43213c6ecb0d0d7a47162`.
Actual code source SHA: `5110380cca146dac8b49a33f9daa222994705a23`.
The subsequent documentation commit records this code tree; the final PR head
also includes that ledger. No runtime binary is claimed for either commit.
PR #108's merged commit `a3865246ae1ee513b440541f1eb523d4258b0261` is an
ancestor of this base. Related: [#81](https://github.com/loveoverflowcom/tabula/issues/81),
[ADR-0043](../../adr/0043-native-mobile-gamehost.md),
[PR #108](https://github.com/loveoverflowcom/tabula/pull/108).

## Scope and actual behavior

The owner's latest request deliberately asks for **a draft PR containing source
skeleton**, with concrete TODOs or typed `NotImplemented`/`Unavailable` results.
It does not ask this revision to satisfy the attachment's complete playable DoD.
This contribution is compiled contract/gate groundwork, not native gameplay.

Android selects `AndroidNativeGameHost` inside the existing `TabulaApp` and
`GameScreen`. Its runtime inventory is empty and packaging preflight always
returns a typed unavailable error **before** `NativeRuntimeOwner.open`. No
worker, native context or surface is created. The normal shell therefore has no
native Play action. Directly invoking this host reports the existing shell-owned
`Failed` result. iOS keeps its unavailable default and empty runtime inventory.

No operation manufactures `ContextReady`, `ResourcesReady`, `InputReady`,
`FramePresented`, `ExitConfirmed`, `Stopped`, a successful fence or a joined
worker. Kotlin port errors map only to existing `Failed` facts; a missing fence
throws its explicit typed error into the existing fail-closed coordinator. A
failed Stop never releases worker admission. Production preflight prevents the
stub from reserving a nonexistent worker in the first place.

### Code owners and remaining mechanisms

| Owning source | Implemented skeleton | Explicit missing mechanism |
|---|---|---|
| `shared/src/androidMain/.../host/AndroidNativeRuntimePort.kt` | Exhaustive mapping of all 11 existing commands; typed errors; callback-posting seam; failure mapping | Approved JNI/ABI/panic boundary, real context/resize, input/cancel, render/foreground control, Rust Back/service flow, window release, fence and stop/join |
| `shared/src/androidMain/.../host/AndroidNativeRuntimeFactory.kt` | Empty packaged-id/capability inventory; failed preflight; retained PR #108 owner | Actual ABI-compatible library and verified pack facts plus backend/fence acceptance, before native admission |
| `shared/src/androidMain/.../host/AndroidNativeGameHost.kt` | Once-per-entry launch snapshot/latest event callback; existing runtime binding; gated AndroidView/SurfaceView consumer | Actual native admission and Android view/lifecycle/device acceptance; surface branch remains unreachable |
| Existing `AndroidNativeSurfaceBinding.kt` | Existing SurfaceHolder/touch/density and immediate fence contract retained | Real render-thread surface barrier and native-window ownership; callbacks are not GPU evidence |
| `crates/tabula-render-macroquad/src/embedded.rs` | Safe validated runtime/lease/geometry/token proposal; unavailable context/resize/fence/render/detach/async stop effects | Supported external-surface bootstrap, owner-thread context teardown and retained worker join |
| `apps/game-client/src/native_host.rs` | Safe host control proposal; opaque bounded GameId; empty inventory; explicit composition/assets/input/Back errors | Registry-erased selected first-party local runtime using existing LocalMatch/projection/presenter/resources |
| `apps/mobile/android/build.gradle.kts` | Explicit `prepareNativeGameRuntime` task that throws a named not-implemented error | Deterministic pinned Rust/NDK selected-game build, verified library/packs and generated runtime inventory |

PR #108's `NativeHostSession`, `NativeGameRuntime`, `NativeRuntimeOwner`, commands,
events, `GameBackPort` and `BindGameRuntime` remain the lifecycle authority. No
second state machine, engine, application root or frame ticker is added. The
generic shell does not name Chess. Voice, navigation, Web/Desktop gameplay,
rules and authoritative state stay at their current owners.

## Proposed native boundary and policy gate

`EmbeddedSurfaceProposalV1` is **only a proposed field contract**. It is not
`repr(C)`, exported, serialized or wired to JNI. Its version/runtime/lease/surface
token and geometry fields contain owned scalar values. A token is an integer
reference-table proposal, never a pointer or proof that a native window exists.
The positive token/revision domain fits Kotlin Long. Geometry and pointer
constructors enforce PR #108's finite bounds. Invalid proposals return typed
errors before the backend error; validation never acquires a native resource.

The existing `forbid(unsafe_code)` remains untouched in the workspace and product
crates. No new crate, lint waiver, dependency change or hidden FFI export is added.
An external-Surface JNI/native-window boundary and any exported symbol need an
explicit reviewed policy ADR before implementation. A source probe with pinned
rustc 1.96.1, edition 2021 and `forbid(unsafe_code)` rejects `no_mangle` on an
extern-C export as expected. That probe is lint evidence, not an Android build or
approval. See [rustc's unsafe-code lint](https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#unsafe-code).

Future native callbacks must retain exact runtime/surface tickets, contain all
panics/errors and post to the Android owner without synchronously waiting for it.
Only genuine backend facts may reach the existing coordinator. Kotlin/Swift must
never receive canonical state or per-frame RenderList data (I-5/I-6/I-10).

## Fresh pinned-source verification

Fresh official archives were downloaded outside the checkout and their SHA-256
matched `Cargo.lock`:

| Published package | Official bytes | SHA-256 |
|---|---|---|
| Macroquad 0.4.16 | [archive](https://static.crates.io/crates/macroquad/macroquad-0.4.16.crate) | `1cbd54d99d4d3ac59deed7ee4927bbbf4eb33a55635530af3be9395a55746090` |
| Miniquad 0.4.11 | [archive](https://static.crates.io/crates/miniquad/miniquad-0.4.11.crate) | `dcbaea978d741f3b60ced666b06a2a1e4a2bc81cb8b9dd29f4a40965e1f59e65` |

Both published manifests declare `MIT OR Apache-2.0` and include license files.
Archive VCS metadata reports Macroquad `5e9b5ca912ac65962c05c0da842a4a70eaae34b9`
and Miniquad `4f13d4a70caf6a8cbf0187a95461658e7812b566`. Miniquad's published
source differs from that VCS revision; the line references below refer to the
checksum-matching **published archive**, not interchangeable Git lines.

| Published source | SHA-256 |
|---|---|
| Macroquad `src/lib.rs` | `aa77fc92e771628c015d3d22460c900b84d10be1d2d894b662ba653d2a194dc4` |
| Miniquad `src/lib.rs` | `c5430eabc0332e6571e400094eac44c3f13a7b9fb14e402ab5c6af25b8ef8450` |
| Miniquad `src/native/android.rs` | `26f78ac3dd288e29199baacb96b7f8cb7e2a444c676a6769581c7839849c9422` |

Source-read findings, not device observations:

- Miniquad `src/lib.rs:461–514` exposes standalone `start(Conf, factory) -> ()`,
  with no existing-Surface argument or owned stop/join handle
- Android does already acquire a window from a Java Surface (`547–550`,
  `589–595`) and replace surfaces (`155–180`). The missing mechanism is a safe
  externally owned lifetime/termination contract, not the absence of any
  Surface integration code
- Android worker creation (`404`) discards its JoinHandle. Startup waits
  (`411–426`) discard other messages, including destruction/disconnection.
  SurfaceDestroyed (`599–603`) only queues a message and cannot prove the
  synchronous SurfaceHolder fence required by PR #108
- Pause (`234`) does not disable scheduling in `frame`/the main loop
  (`253–264`, `488–513`). Public quit flags from Miniquad lib (`177–204`) are
  not consumed by Android. The thread-local sender unwrap (`79–83`) has no
  generation-scoped admission
- EGL cleanup (`515–524`) is not an external join acknowledgement or final
  native-window release. The only native-window release is replacement (`157`).
  Activity acquisition (`554–561`) creates a global JNI reference without a
  matching DeleteGlobalRef in this source. JNI detach is unresolved (`295–324`)
- Macroquad `src/lib.rs:158–174` asserts its owner thread; `484–510` holds the
  global context. `Window::from_config` (`950–978`) constructs private Context/
  Stage inside Miniquad start. Completing its future resets GL (`971–975`),
  without providing a public embedded context factory/shutdown contract

Current upstream refs were separately fetched on this date:
[Miniquad 39c928f](https://github.com/not-fl3/miniquad/blob/39c928fb7e09fd44e45f37fe9a98503968bca412/src/native/android.rs)
and [Macroquad 8d602d3](https://github.com/not-fl3/macroquad/blob/8d602d3f8ebb8a3a582ee15d92983bbb81ee3ef6/src/lib.rs).
Miniquad's current [public start](https://github.com/not-fl3/miniquad/blob/39c928fb7e09fd44e45f37fe9a98503968bca412/src/lib.rs#L462-L517)
still lacks external-surface input/a returned handle; current Android startup,
sender, frame, destruction and cleanup retain the same barriers. No upstream
extension is claimed to exist and neither dependency is upgraded/patched here.

### Smallest proposed backend extension

This is a proposal for the next review, not implemented backend support:

1. Return one owned cancellable worker handle from a reviewed Android embedding
   entrypoint; retain CMP Activity/SurfaceView ownership. Deliver Stop before a
   surface/size arrives and under bounded-channel saturation
2. Construct the Macroquad future/Stage on the graphics owner after context
   initialization. Introduce the minimal reviewed Macroquad factory/shutdown
   extension as well as Miniquad's surface/worker extension; avoid SendHack
3. Apply the current commands/events and runtime/surface correlation. Suspend
   actual GPU scheduling; recreate surfaces without recreating the local match
4. Provide immediate idempotent surface access/input fencing, waiting only for
   in-flight renderer access. Never make worker callbacks wait for the UI owner
5. Cancel preparation/input, stop frames, destroy presentation/Macroquad/GPU
   resources on the correct owner, release EGL/window/JNI references and join
   the exact worker off the UI thread. Only that receipt permits Stopped/reopen

Patch source revision/checksum: **none**. Maintenance/upstreaming owner: **not yet
assigned**. Before a patch is selected, record its exact revision, archive/patch
checksums, retained license notices, review/maintenance owner and upstreaming
plan. A broad fork or graphics toolchain upgrade is not implied.

## Packaging and first-game gate

Packaged Android ABIs: **none**. Native library name/hash: **none**. Actually
launched game: **none**. Runtime generation/first usable frame receipt: **none**.
There is no cdylib/staticlib/export addition and no runtime executable download.

The normal `assembleDebug` remains a shell-only build. The separate explicit
`:android:prepareNativeGameRuntime` task throws
`native-game-runtime-not-implemented`; it is not wired into shell assembly and
produces no fake outputs. Its TODO requires the selected registry/local-game
composition, requested device ABI, pinned Rust/NDK, library/contract identity,
hashed bounded assets/notices and generated inventory before exposing Play.
Source features, public catalog entries and a standalone library are insufficient.

Chess remains the intended first-game implementation slice, using existing
ChessRules → authorized View → ChessPresentation → RenderList → MacroquadRenderer.
It is not composed or packaged by this skeleton. Existing game-owned
`games/chess/src/presentation/assets.rs` declares the `chess@0.3.0` asset-pack
identity, piece/grain gameplay resources and retained notices; existing
`LocalSpriteResources`/`host_resources` own verification/loading/fonts. Future
packaging must preserve those identities, select actual required resources and
exclude unrelated game packs. No Chess rules, goldens or generic-shell dispatch
are changed here. Back must reuse the existing Rust leave flow, not a synthetic exit.

## Checks and coverage

Host: Linux x86_64, JDK 21.0.12.1; Kotlin 2.4.20 focused compiler targets JVM17.
Pinned Rust: rustc/cargo 1.96.1. Focused Android source compilation uses the
previously checksum-verified Android37 platform jar, not a complete installed SDK.
Configured Gradle still requires JDK17/full SDK37. Raw logs stay in ignored
verification output/outside the checkout; no binary/capture is committed.

| Exact command / scope | Status and executed count | Limit |
|---|---|---|
| `bash tools/test-mobile-native-contract.sh` with official Kotlin2.4.20, JUnit4.13.2, Hamcrest1.3 | **PASS 60 tests**, 0 failed/skipped: 52 existing coordinator + 8 Android skeleton | Actual port/factory/coordinator, controlled thread/callback facts; no JNI/GPU |
| Same runner with `ANDROID_JAR` | **compiled PASS** | Existing actual SurfaceHolder/touch binding; not AndroidView/CMP app/APK/device |
| `cargo +1.96 test --locked -p tabula-render-macroquad --lib embedded::tests` | **PASS 10 tests**, 0 failed/ignored | Unavailable backend/boundary values only |
| `cargo +1.96 test --locked -p tabula-game-client --no-default-features --lib native_host::tests` | **PASS 9 tests**, 0 failed/ignored | Safe control proposal/composition errors; no runtime |
| Same native-host command with default features | **PASS 9 tests**, 0 failed/ignored | Standalone game features still advertise no native inventory |
| `cargo +1.96 clippy --locked -p tabula-render-macroquad -p tabula-game-client --no-default-features --lib --tests -- -D warnings` | **PASS** | Both changed Rust owners; no lint waivers |
| Authoritative `cargo xtask check` with pinned toolchain and two build jobs | **BLOCKED** only at missing `cargo-deny`, command exit 1 | All preceding prescribed stages passed; no full-gate pass is claimed |
| Core gate's whole-workspace fmt, strict all-target/all-feature Clippy and `cargo test --workspace` | **PASS 1,372 test executions**, 0 failed/filtered, 18 ignored | 63 nonempty suites; ignored cases are not executed proof. Existing game/renderer tests included; no native gameplay |
| Core gate's deps/game-id/manifests/generated tokens/generated catalog/raw-color checks | **PASS** | 30 crate dependency rows, 878 scanned files, 33 manifests; policy/source freshness, not device evidence |
| `node --test apps/game-client/web/tests/*.test.cjs` | **PASS 123 tests**, 0 failed/skipped | Existing Web loader/host/transport regressions, no browser pixels |
| `python3 tools/check-mobile-native-policy.py` | **PASS**, source/config; **0 real APK/app artifacts** | No native library/frame evidence |
| `python3 -m unittest discover -s tools/tests -p test_mobile_native_policy.py -v` | **PASS 5 tests** | Adversarial policy fixtures, not a shipping APK |
| `python3 tools/check-repository-hygiene.py`; skill checker; `bash -n tools/test-mobile-native-contract.sh`; `git diff --check` | **PASS** | Source/hygiene/syntax/metadata only |
| Configured `./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug` | **BLOCKED** at wrapper download | Transport reports Network is unreachable/Connection refused; no Gradle test/APK was selected. JDK17/full SDK37 also absent |
| `:android:prepareNativeGameRuntime` task execution | **BLOCKED / NOT_IMPLEMENTED** | Gradle cannot bootstrap; body intentionally fails until backend/ABI/packaging exist |
| `python3 tools/check-mobile-native-policy.py --apk apps/mobile/android/build/outputs/apk/debug/android-debug.apk` | **BLOCKED**, command exit 1: APK does not exist | Missing build prerequisite; no artifact inspected or stale artifact substituted |
| Direct compiler with official Compose plugin and exact metadata-resolved Android binaries | **compiled PASS**, 13 actual sources including `AndroidNativeGameHost`, GameHost/runtime/binding and new port/factory | Kotlin2.4.20, `-Werror -jvm-target 17`, Android37 jar, JRE21; no replacement source/stubs or version substitution. Not full Activity/Gradle/APK or interaction evidence |
| New AndroidView/CMP component interaction | **NOT_RUN** | Compile success establishes no mounted view, recomposition or release interaction |
| Actual Android APK/native gameplay, surface fence/thread termination, move/Back/re-entry, screenshots and performance | **BLOCKED / NOT_IMPLEMENTED** | No real backend/library/packs or suitable authorized Android execution environment; 0 frames and no measurements |
| iOS framework/Xcode/native gameplay | **NOT_RUN** | Linux host; iOS adapter remains unavailable and is outside this Android skeleton |

Test counts overlap across configurations and are not additive. Initial offline
Rust resolution was blocked by missing cached `compact_str`; the same locked
online commands resolved current pinned bytes and ran the counts above. No
lockfile or dependency version changed. Initial strict scoped Clippy found an
unused async body and manual no-op waker. The source now uses an explicit ready
error future and the supported `Waker::noop`; the same focused suites and strict
Clippy pass after those fixes, with no lint waiver or fabricated work.

The core command ran in its authoritative order and reached the final
`cargo deny check` stage. That executable is not installed in this environment;
the aggregate correctly exits 1 instead of silently skipping it. No policy was
weakened or dependency version changed to clear this prerequisite. Core log
SHA-256: `1e2441016186cffaff0462f63528110e757f0ededb788fbc825109b96af4e813`.
GitHub checks for the final PR head are inspected separately; local output and
workflow configuration do not establish remote success or merge enforcement.

The focused runner also supports the same actual-source consumer compile through
`ANDROID_COMPOSE_CLASSPATH`, plus `ANDROID_JAR` and the bundled Compose compiler
plugin. It downloads nothing and compiles no replacement classes. Supply the
official AndroidX `runtime`, `ui`, `ui-unit`, `ui-geometry`, `ui-graphics`, `ui-util`
and `foundation-layout` Android binaries at 1.12.0 plus lifecycle-common-jvm and
lifecycle-runtime-compose-android at 2.9.4. Those lifecycle versions are the exact
Android variants published for JetBrains lifecycle2.9.6, not a changed repo pin.
All nine dependency archive hashes matched their published metadata. Reproduce
with the same `bash tools/test-mobile-native-contract.sh` command/environment;
the optional classpath is the extracted official classes, not a previous compiled
host jar. Final host source SHA-256 is
`1b8018123322d34494b6cee4fd0f9edf88df12cd44f6eb22fc975594458cf710`;
the captured direct-compile output SHA-256 is
`4f2809365e45aa8bfc27bb9b77b14710a1823a412b10333f7d4d60802effa7ab`.

## Independent review and next acceptance

Read-only review uses `tabula-code-review`, with `tabula-engineering`,
`tabula-cmp-engineering` ownership/testing and `tabula-game-audit` Chess/presentation
criteria. Review covers the full base-relative code and affected callers, not a
full-game audit. The review confirmed no actionable final code defect in this
skeleton scope. Two observations were addressed: direct inventory APIs now
enforce the same 80-byte id bound before cloning into diagnostics (retained unit
assertions), and the view consumer snapshots launch facts once per entry rather
than restarting on changed preferences/capabilities. The initial consumer source
compilation gap is resolved by the actual-source compile above; full Gradle/
Activity/APK, component interaction and real native acceptance remain separate
assurance gaps. Final ledger/head review records the blocked final cargo-deny
prerequisite without relabelling the preceding stages as a full aggregate pass.

Smallest next implementation slices:

1. Explicit ADR decision for isolated FFI/export and panic/ownership containment
2. Narrow pinned Android Miniquad/Macroquad embedding extension with cancellation
   before surface, bounded controls, rendering suspension, synchronous fencing,
   exact worker stop/join and repeated-open/context-loss tests
3. Registry-owned first-party local Chess composition and deterministic library/
   asset/inventory packaging for an actual selected Android ABI
4. Same-app CMP → native frame → e2→e4 → actual board update → Back → CMP → reopen;
   repeated exit, late callbacks and an interrupted lifecycle on authorized Android
5. Device-specific performance budgets and iOS child-controller/ABI/packaging,
   separately from Android; preload/cache/memory-pressure follow their own slice

Keep the PR **draft**, use **Refs #81**, and leave #81/Phase6/store release open.
No automatic merge, deployment, production capability or native playable claim
follows from this skeleton.
