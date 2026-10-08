# CMP testing and evidence

Use the [engineering evidence vocabulary](../../tabula-engineering/SKILL.md).
Select checks by the changed claim and inspect actual task selection/assertions;
do not infer execution from a manifest, CI YAML, a cached PNG or successful build.

| Claim | Existing evidence owner | Remaining boundary |
|---|---|---|
| Public routes, copy, catalog/query, account transition/fencing, voice policy or native lifecycle model | `shared/src/commonTest` via Android host tests; e.g. BackStack, DiscoveryCatalog, AccountSessionController, NativeGameRuntime tests | Pure/model/controlled-port evidence; no live provider/device/backend implied |
| Mounted shared semantics, navigation, actions and responsive layout | `previewApp/src/test`; e.g. AgenticSemanticSmokeTest, DiscoveryUiTest, AccountUiTest, ResponsiveShellTest | Actual shared component on Desktop Compose; no OS accessibility/device behavior implied |
| Real semantic interaction and changed UI after Kotlin edit | Official Hot Reload MCP and existing smoke client | Requires a graphical desktop and live tree assertions; distinct from deterministic tests |
| Pixel geometry, hierarchy, wrapping and state feedback | Existing preview test PNGs or MCP capture, opened and reviewed | Screenshot capture and inspection are separate evidence kinds |
| Android artifact / iOS Kotlin compilation and host linking | Existing Gradle/Xcode targets and native-policy guard | Compilation/packaging does not prove native gameplay or real-device acceptance |
| Native input, Back, surface/thread lifetime, IME, accessibility or performance | Relevant Android/iOS host and selected actual device scenario | Report missing adapter/device/harness explicitly; never substitute desktop doubles |

## Existing commands

For changes under `apps/mobile`, run its required gate from that directory:

```bash
./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug
```

For a focused agent-semantics regression:

```bash
./gradlew --console=plain :previewApp:test --tests '*AgenticSemanticSmokeTest'
```

A focused filter is not the full shared/controller or preview gate. Inspect the
fresh XML for nonempty required cases, execution/skips and failures. Gradle
up-to-date results can establish unchanged cached work only with its provenance;
do not report a new execution count. Native acceptance follows the
[mobile README](../../../../apps/mobile/README.md), including available iOS checks:

```bash
./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64
xcodebuild -project ios/TabulaApp.xcodeproj -scheme Tabula -configuration Debug -sdk iphonesimulator CODE_SIGNING_ALLOWED=NO build
```

At the Rust root, run the applicable existing generator/freshness and consumer
checks for shared tokens/catalog. `cargo xtask gen-tokens`,
`cargo xtask gen-mobile-catalog`, `cargo xtask check-mobile-catalog` and
`cargo xtask check-no-raw-colors` have real dispatch. Before a PR,
`just check` (`cargo xtask check`) is the authoritative portable core gate with
its own order; add mobile checks for any change under `apps/mobile`. Read the
current implementation before invoking a future-phase command. Do not weaken
policy or regenerate unrelated files/goldens to clear a check.

Skill-only guidance changes outside `apps/mobile` need metadata/link validation
and existing checker fixtures, not a mobile build or live source mutation.
Mobile documentation/routing changes still follow the required mobile gate.
If evaluating the new workflow, report that separately from UI acceptance; historical preview
runs do not become new evidence. Keep read-only review within its agreed checks.

## Completion receipt

Scale the report to the changed claim, retaining enough provenance to reproduce
any visual/native judgment:

```text
Behavior/invariant and production entrypoint/callers:
Source SHA + working patch / build identity:
Scenario, fixture and fake/live dependency boundary:
Target/renderer/source sets, host/device/OS, toolchain:
Viewport dp + captured px, density, font scale, theme/locale/fonts:
Semantic tree/window/roots and asserted interaction/reload result:
Exact checks, executed cases/skips, exit result and PASS/FAIL/BLOCKED/NOT_RUN:
Images opened, paths/hashes, concrete findings and same-scenario recheck:
Native/provider/performance scopes executed or remaining:
Artifact paths and local retention limits:
```

Use the existing ignored `apps/mobile/previewApp/build/reports/` locations for
raw output; live MCP receipts use its `cmp-agentic-loop/` subtree. Commit only a
bounded reviewed Markdown ledger with selected excerpts and capture hashes;
repository hygiene excludes non-Markdown files in `docs/verification`. Keep
private facts/credentials out of evidence and do not commit PNGs, full semantic
snapshots, caches, runtimes or APKs. A CI job config is distinct from a successful
run and from known branch enforcement. Performance claims need comparable
target measurements; missing measurements remain explicit.
