# Native mobile voice evidence ledger

Implementation base: `develop @ aee07128d111c0bd5c15eb2ccaacc40c880ba5c3`, tree
`07bf97c141f28120c6e1a2c36e66e48a101e86db` (account PRs #71/#72/#73 merged).
[ADR-0037](../../adr/0037-native-mobile-voice-client.md) defines the bounded slice.
This is one separate native client PR, not the trusted-Origin fix in #74 or a
production/backend voice service implementation.

## Implemented owners / changed claims

| Claim / invariant | Owner and failure mode | Oracle/check | Evidence kind / status |
|---|---|---|---|
| Production unavailable before media/permission | default grant source + controller; accidental fallback | common test `productionUnavailableNeverConnectsOrRequestsMicrophone`, CMP unavailable interaction | doubles; local Kotlin execution BLOCKED, CI pending |
| Native room/media path outside game | Android/Swift adapters; JS/WS audio or secret exposure | pinned SDK API/source, restricted WebView capture/network, unchanged bridge grammar | source-read; native compilation CI pending |
| Join starts receive-only | native SDK connect options; eager permission/capture | explicit Android audio=false/video=false, Swift enableMicrophone=false; common/CMP assertions | source-read + doubles, real capture NOT_RUN |
| Mic acknowledges real publication/failure | native SDK result/publication; optimistic label | Boolean/publication verification, permission-denial tests | source-read + doubles, real microphone NOT_RUN |
| Reload/retry preserves voice | app/session owner outside GameHost; WebView disposal owns voice | common hello-reload and `VoiceShellTest` reload/host-retry interactions | doubles; CI pending, native WebView NOT_RUN |
| Late grant/join/mic cannot restore state | attempt/command guards; reordered async completion | common stale/duplicate/terminal partitions, native cleanup chaining | doubles + source-read; native races NOT_RUN |
| Leave/logout/background/dispose retires resources | controller + native adapters; capture outlives authority | common terminal/idempotence tests, native room disconnect/release after cancelled work | source-read + doubles; real resources/background NOT_RUN |
| Ten-minute fixed local dev grants, no committed secrets | strict native-only source + ephemeral harness | Python fixture JWT signature/shape/private file tests; common hostile fixture partitions | Python 2 tests PASS; Kotlin tests CI pending; SFU NOT_RUN |
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
