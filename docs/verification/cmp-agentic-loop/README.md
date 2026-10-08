# CMP agentic coding loop — executed verification

On **2026-10-08**, the existing Desktop Preview completed the real official MCP
loop twice: inspect → semantic interaction → edit shared Kotlin source → Hot
Reload → assert changed semantics → restore → Hot Reload → assert baseline.
Both runs exited **0**. This is `interaction-tested` Desktop CMP shell evidence,
with actual screenshots; it establishes no native Android/iOS gameplay or login.

## Contract and environment

The preview remains the ADR-0033 testing host over `shared`. Its generated public
registry catalog follows ADR-0045; synthetic account defaults to unavailable.
Setup retains disabled native launch (ADR-0043). No production authority,
Rust/Macroquad behavior, phase gate or semantic ID changed (I-5/I-6, I-9/I-10).

| Item | Executed version / scope |
|---|---|
| Source base | `develop`, `cbb5f94150b30fa86d128ad55f5fd19b8dc0ac64` |
| Local host | macOS 26.5.2, Apple aarch64, graphical desktop session |
| Compose / compiler | Compose Multiplatform 1.12.0, Kotlin/Compose compiler 2.4.20 |
| Hot Reload / MCP | Official JetBrains 1.2.0; runtime `serverInfo` confirmed 1.2.0 |
| Gradle | Wrapper 9.7.0; launcher Microsoft OpenJDK 17.0.19 |
| Local daemon | Adoptium 21.0.7 via pre-existing untracked daemon criteria; excluded from this PR |
| Hot Reload runtime | Verified JBR `25.0.2+10-b329.72-jcef`, explicit binary override |
| MCP client | Python 3.9 stdlib, JSON-RPC stdio, negotiated protocol `2024-11-05` |
| Preview | Existing `DesktopPreviewKt`; 320×640 dp, en, reduced motion, registry catalog |
| Native build tools | Android SDK 37; Xcode 26.6 (17F113); Rust 1.96.1 |

The [pinned JetBrains documentation](https://github.com/JetBrains/compose-hot-reload/blob/v1.2.0/README.md)
and published plugin sources were inspected. `previewApp` is Kotlin/JVM, so its
tasks are `hotRun` and `hotMcpServer`, without a `Jvm` suffix. The existing
`jvmToolchain(17)` normally requests JBR 21 (the plugin minimum); this execution
used the supported explicit JBR 25 override. JVM bytecode targeting remains 17.

## Reproduction and commands actually executed

The [developer guide](../../../apps/mobile/AGENTIC-CODING.md) and
[mobile agent instructions](../../../apps/mobile/AGENTS.md) describe agent
registration and the same tools. From the repository root, with a compatible JBR
available through automatic provisioning:

```bash
python3 tools/mobile-agent-smoke.py \
  --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/run-01
```

This machine's Java trust store rejected automatic JBR download with
`SSLHandshakeException: PKIX path building failed`. That launch is **BLOCKED**,
preserved in `checks/tabula-cmp-hotrun.log`; no TLS check was disabled. An
official HTTPS archive and its published SHA512 were downloaded, verified and
extracted temporarily. Source: [official JBR archive](https://cache-redirector.jetbrains.com/intellij-jbr/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72.tar.gz)
and [published checksum](https://cache-redirector.jetbrains.com/intellij-jbr/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72.tar.gz.checksum).
The verified SHA512 is:

```text
1041c4c43d418069e1c6d7cde79275623484a81c4185fc2559eb3660cf26e9070463df8aaa173a25413ce1b83bb04f963901c70287a08e8eedaf112a08515c07
```

The actual successful invocations used this local-only runtime path; no machine
path is authored in application/MCP configuration:

```bash
TABULA_JBR_JAVA=/tmp/tabula-jbr25/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72/Contents/Home/bin/java
python3 tools/mobile-agent-smoke.py \
  --evidence-dir verification/cmp-agentic-loop/run-02 \
  --gradle-arg="-Pcompose.reload.jbr.binary=$TABULA_JBR_JAVA"
# The receipt was moved intact into previewApp/build/reports/cmp-agentic-loop/run-02.
PYTHONOPTIMIZE=1 python3 tools/mobile-agent-smoke.py \
  --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/run-03 \
  --gradle-arg="-Pcompose.reload.jbr.binary=$TABULA_JBR_JAVA"
```

The client launches these existing Gradle tasks from `apps/mobile`, discovers
`tools/list`, and records both commands in `result.json`:

```bash
./gradlew --console=plain :previewApp:hotRun --no-auto \
  -Ppreview.width=320 -Ppreview.height=640 -Ppreview.language=en \
  -Ppreview.reducedMotion=true -Pcompose.reload.jbr.binary="$TABULA_JBR_JAVA"
./gradlew --no-daemon --quiet --console=plain :previewApp:hotMcpServer \
  -Pcompose.reload.jbr.binary="$TABULA_JBR_JAVA"
```

## Observed loop and semantic excerpts

Run 02 recorded one window titled
`Tabula shell — synthetic account: unavailable — simulated game — 320×640 dp`.
Window ID `68a6186f-5cd4-49b1-bf60-c22448a3c481` was supplied on semantic calls.
Node IDs below are historical handles; the client reacquired them after every
transition/reload. Its selectors are existing `testTag` values.

| Step | Observed assertion / tool receipt |
|---|---|
| Inspect Home | `shell-home`, heading `Your play space`, selected `shell-nav-home` |
| Navigate Library | Click `shell-nav-games` id 34; `shell-games` appeared |
| Scroll | `scroll` id 106, `deltaY=360`; the first card gained nonzero visible bounds |
| Type | `type_text` id 110, `text=Chess`; `editableText=Chess`, exactly one detail action |
| Open detail | Click discovered `shell-details-com.tabula.chess`; `shell-detail` appeared |
| Enter setup | Click `shell-setup-action`; `shell-setup`, `shell-start-local.enabled=false` |
| Navigate Back | Setup → detail → Library; search remained `Chess`; return Home |
| Edit / reload | Unique English `ShellCopy.HomeHeading` changed; MCP `reload` returned `success=true,reloaded=true` |
| Assert edited UI | `Agent loop verified` present, `Your play space` absent in live semantic tree |
| Restore / reload | Original source bytes restored; second reload returned `success=true,reloaded=true` |
| Assert baseline | `Your play space` present, marker absent; every `get_ui_error` returned `hasError=false` |

Selected exact node fields from captured trees (unrelated children/bounds
omitted to keep this record bounded):

```json
{"id":110,"editableText":"Chess","testTag":"discovery-search"}
{"id":215,"testTag":"shell-setup"}
{"id":233,"text":"Start local game","testTag":"shell-start-local","enabled":false,"actions":["onClick"]}
{"id":293,"editableText":"Chess","testTag":"discovery-search"}
{"id":342,"text":"Your play space"}
{"id":437,"text":"Agent loop verified"}
{"id":532,"text":"Your play space"}
```

Both successful runs ended with this MCP status and `source_restored=true`:

```json
{"connected":true,"buildContinuous":false,"reloadState":"ok","lastError":null,"successfulReloads":2,"failedReloads":0}
```

The restored source SHA256 is
`a813f0c115b8225775d79ed7b9124bd2e0e5fec5dc690432338f82ec614c4454`;
`git diff -- ShellStrings.kt` was empty. Run 03 repeats the complete loop with
Python optimization enabled; checks use explicit failure branches, so that mode
cannot remove acceptance assertions.

## Checks, failures and remaining scope

| Executed check | Actual result |
|---|---|
| `./gradlew --version`, `:previewApp:tasks --all` | **PASS**, Gradle 9.7.0 and official tasks found; configuration evidence only |
| `help --task :previewApp:hotRun`, `help --task :previewApp:hotMcpServer` | **PASS**, exact CI task wiring probes |
| `:shared:testAndroidHostTest :previewApp:test :android:assembleDebug` | **PASS**, 163 shared tests + 61 Desktop tests, zero failures/errors/skips; APK assembled |
| Agent semantic CI XML guard | **PASS**, 2 English/Vietnamese `AgenticSemanticSmokeTest` cases selected, unskipped |
| `:shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64` | **PASS**, both Kotlin targets compiled |
| `xcodebuild -project ios/TabulaApp.xcodeproj -scheme Tabula -configuration Debug -sdk iphonesimulator -derivedDataPath /tmp/tabula-cmp-xcode CODE_SIGNING_ALLOWED=NO build` | **PASS**, existing host/framework linked; no device run claimed |
| `python3 tools/check-mobile-native-policy.py --apk apps/mobile/android/build/outputs/apk/debug/android-debug.apk` | **PASS**, built Android artifact inspected |
| Same policy with `--app /tmp/tabula-cmp-xcode/Build/Products/Debug-iphonesimulator/Tabula.app` | **PASS**, built iOS artifact inspected |
| `cargo xtask check` | **PASS** on rerun, authoritative portable gate completed in order; existing ignored doctests remain ignored |
| Shell/Python syntax, CI YAML parse, `git diff --check` | **PASS** |
| Live MCP run 02, repeated run 03 | **PASS**, all interaction/edit/reload/restore assertions executed |
| Remote CI execution / required branch statuses | **NOT_RUN / unknown** at local verification time; configured CI is distinct from a successful remote run |
| Native gameplay, production login, device acceptance | **NOT_APPLICABLE** to this desktop tooling change; existing unavailable boundaries retained |

The initial live run 01 correctly failed its scroll oracle: the last card
remained clipped to zero bounds after a 360 dp scroll. The corrected client
asserts that the first card moves into view. No temporary edit occurred in run
01; its failure receipt is retained. The first core gate stopped at
`check-no-game-ids` because ignored raw JSON in `verification/` is still scanned.
Receipts were moved into the existing excluded `previewApp/build` report tree;
the source scanner was not weakened. The final default/example output uses
that directory. The rerun completed every core gate.

## Retained bounded evidence

Raw local receipts live at
`apps/mobile/previewApp/build/reports/cmp-agentic-loop/{run-01,run-02,run-03,checks}`.
Run 02 contains 26 files / 1,171,603 bytes: JSON-RPC transcript/tool schemas,
ten selected semantic trees, ten MCP screenshots, runtime/server/application
logs and the result receipt. Runs 01 and 03 retain failure/repeat provenance.
Check stdout/stderr is preserved under `checks`; test XML/HTML and existing
regression captures remain in the normal preview/shared build reports.

Captured screenshots `run-02/screenshot-47.png` (setup), `screenshot-80.png`
(edited Home) and `screenshot-90.png` (restored Home) were visually inspected:
setup visibly retains unavailable native launch, and the heading changes/restores.
Selected SHA256s bind the raw receipts:

| Raw file in run 02 | SHA256 |
|---|---|
| `08-edited-semantics.json` | `5dfc85b4f223089737e69b5656d50a39e502555192a26742cf08da3f622016cf` |
| `09-restored-semantics.json` | `910fc9cb9655d5377c807342a041864e53cb975e4c3c7747f1281173d304c84f` |
| `screenshot-47.png` | `a6eaa06dcd4638bee76ab30bc58012a2405cdfa40d4d7893ee704891158091ec` |
| `screenshot-80.png` | `91ee7d14fd7a6076fcdf3e29bb0b79ac67c774e0f4b01a4e103fe14b0ce1f2b6` |
| `screenshot-90.png` | `bc665d3a17897eac190f13c56fb7e1c95162484633a2cb73e1490dadd087fd71` |

Repository hygiene permits Markdown evidence records and keeps raw screenshots,
trees, downloads and builds out of Git. CI uploads deterministic semantic
snapshots/screenshots and test/task reports through the existing mobile artifact;
it does not substitute those for this separately executed interactive run.
