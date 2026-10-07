# Main recovery and CMP prototype adaptation — 2026-10-07

The owner requested removal of two commits accidentally pushed to `main`, preservation as
patches, review, and delivery on `develop`. After reviewing the draft, the owner requested
actual code changes using the patch as input, rather than a mechanical cherry-pick/rebase.

## Pinned source and recovery

| Role | Commit |
|---|---|
| Main before the accidental push | `70b0fcd721b0b04cc76559cbc4fc72151bee613a` |
| First accidental commit | `55d2354e6c4817fb67d1655859e8c1fa161f4d84` |
| Second accidental commit / original source | `6170dddc2769fb1a476c37132438bce29bac6c7e` |
| Develop implementation base | `80d9fdb96f18cd59fa85c9401b533d66bf04b5d7` |

The two original commits remain on
[`codex/recover-main-20261007`](https://github.com/loveoverflowcom/tabula/tree/codex/recover-main-20261007).
An atomic push created that recovery branch and moved `main` to `70b0fcd`, guarded by the
exact original head SHA. The recovery does not add revert commits to `main` or merge the
old main ancestry into `develop`.

The [input review](REVIEW.md) records concrete defects and the duplicate-content inventory.
56 changed paths already matched `develop`; existing lockfiles, maintained skills, executable
modes, workflow updates and SQLx caches were preserved when adapting the actual code.

## Actual adaptation

- One mobile tree remains in `apps/mobile`. The prototype's separate Android project, Kotlin
  rules, local result/rating database and custom debug signing configuration are not used.
- Home packages the prototype's board-game illustration as a common Compose resource, using
  its actual aspect ratio and adaptive layout. Copy stays on a token-backed surface.
- Library exposes registry categories directly and retains AND-combined advanced filters,
  search, query restoration and catalog failure states.
- Account links to Rooms, History and Settings. Rooms/History have explicit native-adapter
  unavailable states with real Library recovery actions, without sample data or replay claims.
- Settings changes theme, language and reduced motion immediately and retains bounded public
  choices with shell saved state. Fresh GameLaunch receives the resolved presentation snapshot.

Doc 00 and ADR-0032/0043/0045/0046 retain authority. Rust owns game rules, projection, presenter
and renderer. Native adapters, account/social services, room/history/replay services and all
phase/device acceptance gates retain their existing status.

The image is byte-identical to original blob `d27fa7fb7552a5eb2be078a1e924c835ddc71c79`:
819,958 bytes; SHA256 `5af4068f59046b059fe62fcbf3a3c8bc07930b4dd5d7d32cb3413ec6936d334a`.
Original embedded Android patch SHA256 is
`d5aaa918cfcc618560ff4122d442f8b0f25caeca37103072c88c4226d7d84a3b`;
complete original net patch SHA256 is
`25b5e439abad73f64ad95e110f91d5289a1ce264739c6a7bc138307acaddd0cb`.
Both remain recoverable from the pinned Git source; binary roundtrip checks cover preservation,
not Android gameplay correctness.

## Evidence

The final implemented mobile source and test inventory contains 19 changed paths relative to
`80d9fdb`; its sorted `sha256sum`-format manifest has SHA256
`d22cc4609098133fd74a19e428daa8788eccac720e1620e667d70492da207cd6`.
The containing commit identifies the full implementation and documentation. Local logs and
patches are retained under `/private/tmp/tabula-main-recovery-20261007/`.

| Check | Final result and scope |
|---|---|
| `cargo xtask check` | PASS: portable core gate, including 1,292 passing Rust tests and 18 ignored tests; fmt, clippy, dependency/manifest/token/catalog/color checks and cargo-deny completed |
| `./gradlew :shared:testAndroidHostTest :android:assembleDebug :previewApp:test` from `apps/mobile` | PASS: 163 Android host tests and 59 desktop Compose tests; no failures, errors or skipped tests; Android debug APK built |
| `python3 tools/check-mobile-native-policy.py --apk apps/mobile/android/build/outputs/apk/debug/android-debug.apk` | PASS: source/config policy and one packaged artifact |
| APK illustration inspection | PASS: exact resource path and all 819,958 bytes match the source illustration SHA256 above |
| `python3 -m unittest discover -s tools/tests -p test_mobile_native_policy.py -v` | PASS: 5 policy tests |
| `./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64` from `apps/mobile` | PASS: common/platform Kotlin compilation for both iOS targets |
| `git diff --check` | PASS |

Toolchain: Rust/Cargo 1.96.1, JDK 17.0.19, Gradle 9.7.0, Kotlin 2.4.20, Compose 1.12.0,
AGP 9.3.1 and Android SDK 37.0. Gradle commands used the discovered JDK/SDK through
`JAVA_HOME` and `ANDROID_HOME`. Stale Rust build outputs with paths to a retired worktree
were rebuilt; no golden or Rust source was changed. Required dependency/advisory caches were
refreshed for the final successful gates.

Android KMP resource processing is explicitly enabled in `shared/build.gradle.kts`, as required
by the [Compose resources setup documentation](https://kotlinlang.org/docs/multiplatform/compose-multiplatform-resources-setup.html#resources-in-the-androidlibrary-target).
Compilation alone missed this packaging requirement; the final APK additionally contains
`assets/composeResources/com.loveoverflow.tabula.mobile.resources/drawable/tabula_discovery_hero.jpg`.

Model tests exercise route restoration, all bounded preference combinations, malformed saves,
host accessibility and account-task separation. Compose tests exercise navigation, actual local
setting changes/restoration and the preference snapshot delivered to a simulated GameHost.
Inspected screenshots in `apps/mobile/previewApp/build/reports/shell-screenshots/`:

- `discovery-768-light-vi-registry-home.png`: packaged artwork and wide Home layout.
- `tools-320-vi-font200-settings.png`: Vietnamese Settings at 320 dp and 200% font scale.
- `tools-390-dark-vi-settings-restored.png`: selected options and restored dark appearance.

Desktop Compose screenshots establish shared shell pixels and interactions only. Android APK
and iOS compilation establish the named builds. Linked Xcode application/resource execution,
native gameplay and Android/iOS device acceptance are NOT_RUN and remain outside this slice.
