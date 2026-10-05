# Native mobile voice evidence ledger

Implementation base: `develop @ aee07128d111c0bd5c15eb2ccaacc40c880ba5c3`, tree
`07bf97c141f28120c6e1a2c36e66e48a101e86db` (account PRs #71/#72/#73 merged).
[ADR-0037](../../adr/0037-native-mobile-voice-client.md) defines the bounded slice.
This is one separate native client PR, not the trusted-Origin fix in #74 or a
production/backend voice service implementation.

## Implemented owners / changed claims

| Claim / invariant | Owner and failure mode | Oracle/check | Evidence kind / status |
|---|---|---|---|
| Production unavailable before media/permission | default grant source + controller; accidental fallback | common test `productionUnavailableNeverConnectsOrRequestsMicrophone`, CMP unavailable interaction | doubles; local Kotlin execution BLOCKED, exact-head CI recorded in PR #75 |
| Native room/media path outside game | Android/Swift adapters; JS/WS audio or secret exposure | pinned SDK API/source, restricted WebView capture/network, unchanged bridge grammar | source-read; native compilation exact-head CI recorded in PR #75 |
| Join starts receive-only | native SDK connect options; eager permission/capture | explicit Android audio=false/video=false, Swift enableMicrophone=false; common/CMP assertions | source-read + doubles, real capture NOT_RUN |
| Mic acknowledges real publication/failure | native SDK result/publication; optimistic label | Boolean/publication verification, permission-denial tests | source-read + doubles, real microphone NOT_RUN |
| Reload/retry preserves voice | app/session owner outside GameHost; WebView disposal owns voice | common hello-reload and `VoiceShellTest` reload/host-retry interactions | doubles; exact-head CI recorded in PR #75, native WebView NOT_RUN |
| Late grant/join/mic cannot restore state | attempt/command guards; reordered async completion | common stale/duplicate/terminal partitions, native cleanup chaining | doubles + source-read; native races NOT_RUN |
| Leave/logout/background/dispose retires resources | controller + native adapters; capture outlives authority | common terminal/idempotence tests, native room disconnect/release after cancelled work | source-read + doubles; real resources/background NOT_RUN |
| Ten-minute fixed local dev grants, no committed secrets | strict native-only source + ephemeral harness | Python fixture JWT signature/shape/private file tests; common hostile fixture partitions | Python 2 tests PASS; Kotlin tests exact-head CI recorded in PR #75; SFU NOT_RUN |
| Backend/SFU enforces permission | backend VoiceService; client mute falsely presented as rule | explicit unavailable production source and unchanged backend gates | NOT_IMPLEMENTED for production; actual SFU scope suite NOT_RUN |

## Executed locally

Cloud environment: Linux x86_64, Rust 1.96.1 from existing workspace toolchain,
JDK 21. No Android SDK, installed Gradle/Kotlin compiler, Swift/Xcode, emulator,
simulator, device, LiveKit SFU or native microphone/audio hardware was available.
All idle generated Cargo targets were cleaned before fetching latest develop;
source, designs, previous evidence and commits were preserved.

- PASS: Python ephemeral fixture tests, 2 nonempty tests
  `python3 tools/native-voice-harness/test_run.py`
- PASS: `cargo xtask check`: all authoritative portable gates; Rust workspace
  tests reported 1,068 passed, 0 failed, 21 existing ignored over 80 target/doc-test
  groups. Ignored targets are not promoted to acceptance
- PASS: workspace `--no-default-features` and `--all-features` checks; native
  `cargo build -p tabula-game-client`; game client WASM web target check
- PASS: unchanged page bridge tests, 10 nonempty tests
  `node --test apps/game-client/web/tests/host-bridge.test.cjs`
- PASS: existing skill/tree validation and 32 validator / 6 doc-contract tests
- PASS: `git diff --check` at the inspected working tree
- BLOCKED: repository Gradle wrapper setup. With writable `GRADLE_USER_HOME`,
  `./gradlew --version` fails downloading official pinned Gradle 9.7.0 with
  `java.net.SocketException: Network is unreachable`; reviewed execution has
  the same failure. No Kotlin/Android build or test pass is inferred
- BLOCKED: real SFU setup. `python3 tools/native-voice-harness/run.py` exits
  nonzero: official `livekit-server` is not installed; no fixture/server started
- NOT_RUN: Android emulator/device; iOS simulator/device; real two-client audio,
  simultaneous game audio, WebView reload, network loss, permission prompt,
  headset routing, foreground/background capture indicators and resource cleanup

The final exact-head CI receipts are recorded below once terminal. No native GUI/loopback access restriction
was bypassed to manufacture runtime evidence.

## CI scope

The existing `mobile` job compiles Kotlin/Android, runs Android-host common tests
and desktop CMP UI tests with labelled game/media doubles, then assembles the
Debug APK with the real packaged game bundle. It does not run an Android WebView
or microphone. The added `mobile-ios` macOS job compiles both Kotlin iOS targets
and builds/links the Xcode simulator app with exact LiveKit Swift SPM. It does
not launch a simulator or establish native audio. CI must pass on the exact
published head; source inspection is not a substitute for either SDK compilation.

## Next real acceptance

Use [the ephemeral harness](../../../tools/native-voice-harness/README.md) with two
unique clients in an authorized device/SFU environment. Follow
[the target task](../../work-plan/backlog/native-mobile-voice-acceptance.md) and
record actual SDK/server/OS/build/device versions, media in both directions,
permission denial, concurrent game audio, WebView reload, network-loss/reconnect,
rapid/stale actions, leave/logout hook/capture release, headset changes and true
background/foreground behavior. A client mute or an audio-only token fixture does
not prove Werewolf/SFU secret-channel enforcement, production revocation, native
secure stores, Kanidm authentication or Phase 6/8 exit.

## Independent source review

A separate read-only review checked pinned SDK APIs and ownership/callback paths.
It found and verified corrections for Android unsolicited local publication-state
drift, iOS Debug loopback ATS, Swift capture drain after cancelled/failed publication,
and expiry recheck after prior room cleanup. Latest source-read verdict found no
remaining concrete blocker. This is source evidence, not execution of the native
race, microphone, transport or provider boundaries. Swift full mic-on reconnect
is conservatively terminal; rejoin starts mic-off.

## CI iteration record

Initial head `46df768fc7a2dd387278dc7aefc4236624896e0a`,
[run 37266195682](https://github.com/loveoverflowcom/tabula/actions/runs/37266195682):
11/13 jobs PASS; Android and Swift app build jobs FAIL. The Android failure was
Gradle validation of an intentionally absent dev fixture (`@InputFile.optional`
allows an unset value, not a specified nonexistent file). The task now uses an
empty filtered `InputFiles` collection while retaining add/change/removal tracking
and stale resource cleanup. Shared Android/desktop compilation reached completion,
but no skipped/aborted tests from that failed aggregate are claimed passed.

macOS Xcode 16.4 / Swift 6.1 compiled both Kotlin iOS targets and linked the shared
simulator framework, then caught Swift nullable Kotlin-enum member inference in
the actual adapter. The explicit-type retry exposed the underlying generated-symbol mismatch: the
pinned Kotlin exporter lowercases whole underscore-delimited enum-name segments,
so PascalCase multi-word values did not export as camelCase. Voice enums now use
conventional UPPER_SNAKE constants, retaining ordinal/order and internal behavior
while producing the intended Swift names. The actual generated header is retained
by macOS CI. UI-thread confinement of the SDK Sendable delegate is stated explicitly. Full app compile/link and
controller/CMP test acceptance require the terminal **final-head** result in
[PR #75](https://github.com/loveoverflowcom/tabula/pull/75), where exact SHA/tree,
job links and test counts are recorded. Earlier partial compilation is not a
full native app pass. None of this executes a simulator or microphone.


Second head `807ed79208bd9aa8c74ad8a7804fdd3b50791f77`,
[run 37267495925](https://github.com/loveoverflowcom/tabula/actions/runs/37267495925):
12/13 jobs PASS, including actual Android compilation/Debug APK and 65 nonempty
common/CMP tests (0 failed, 0 skipped). The 16 VoiceController and 3 VoiceShell
cases are **doubles**, not native audio. Their
[JUnit/screenshot artifact](https://github.com/loveoverflowcom/tabula/actions/runs/37267495925/artifacts/11326748953)
was downloaded and inspected; the 390 px screenshot reuses semantic theme/controls
and labels the simulated game. Swift's actual app compile still failed on the
enum names; explicit type spelling alone was not an adequate fix.

The exact compiler rule is source-verified in
[v2.4.20 ObjCExportNamer](https://github.com/JetBrains/kotlin/blob/v2.4.20/native/objcexport-header-generator/impl/k1/src/org/jetbrains/kotlin/backend/konan/objcexport/ObjCExportNamer.kt#L697-L721)
and [K2 enum translation](https://github.com/JetBrains/kotlin/blob/v2.4.20/native/objcexport-header-generator/impl/analysis-api/src/org/jetbrains/kotlin/objcexport/translateEnumMembers.kt#L87-L102).
UPPER_SNAKE normalization changes no wire, serialized values, persisted preferences
or deployed API; none of those consume these new host-only enums. Final-head
native/CMP gate status, exact SHA/tree and successful app-link receipt remain in
PR #75 rather than being inferred from either failed iteration.
