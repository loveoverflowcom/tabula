# Issue #103 — bounded CMP account UI draft

- Start: fresh `origin/develop @ a3865246ae1ee513b440541f1eb523d4258b0261`
  on 2026-10-07, including merged PR #108
- Scope: [ADR-0046](../../adr/0046-mobile-account-surfaces.md) and
  [screen contract](../../ui/screens/mobile-account.md)
- Outcome: account task UI/state/adapter draft; production native
  provider/enrollment/social integration remains unavailable
- Evidence rule: source presence, test implementation, executed tests, captured
  pixels and native execution are separate claims
- Final source baseline re-fetch: `origin/develop` remains the same `a386524`
  commit; checks below are local working-tree evidence, not published-tree CI

## Current capability reconciliation

[Issue #103](https://github.com/loveoverflowcom/tabula/issues/103)'s original
current-state paragraph predates the present isolated web composition.
ADR-0038 supplies Kanidm login; ADR-0044 supplies explicitly enabled
`account-social` enrollment, permitted profile read/edit and durable
friends/presence. Those source contracts and their
[web evidence](../issue-54-account-followup/README.md) do not enable a native
provider, secure store or social adapter. The native default is honest
`NativeAdapterMissing`, not a mock signed-out account or a simulated login.

The native read-only identity/profile types mirror only approved display fields.
No wire, server, provider or gameplay contract changes. Profile edits, other
lookup, friend data and presence are not synthesized. The managed avatar is
presentation-only and matched to the exact active identity instance; a previous
image cannot return after same-account refresh through ID equality alone.

## Changed claims and check status

| Claim / failure mode | Owner / suitable oracle | Status and evidence boundary |
|---|---|---|
| Explicit account states; unavailable becomes SignedOut or fake identity | `account/AccountState.kt`, production unavailable port; typed transitions and state tests | PASS: bounded JVM tests below; no native authority implied |
| Old/cancelled/duplicate response restores private data | `AccountSessionController`; held-callback doubles, operation identity and foreground/close laws | PASS: focused JVM tests, including synchronous collector reentry; actual OS lifecycle/Compose binding NOT_RUN |
| Wrong subject or invalid profile displays | Account ID/profile validators and current-subject controller check | PASS: focused JVM identity/Unicode/subject tests; no native DTO/HTTP adapter execution |
| Obsolete avatar displays after same-account refresh | Fresh controller identity instance and exact-reference avatar guard | Identity-instance controller law PASS; Compose avatar rendering NOT_RUN |
| Fixed account routes retain no authority | `navigation/BackStack.kt`; parse/save/restore/Back tests and Compose navigation | PASS: 14 actual public navigation tests plus 2 Back-dismissal port tests; Compose saver/platform/navigation interaction NOT_RUN |
| Default Login/Register/Friends cannot authenticate or fabricate social data | Native capability panels and production unavailable composition | Source-read; Compose tests NOT_RUN |
| Sign-out confirmation is single-flight and uncertainty stays masked | UI confirmation, port capability and controller; Cancel/repeated/late/unconfirmed cases | Controller laws PASS; Compose confirmation/focus/keyboard NOT_RUN; unresolved suppression is in-memory only |
| Shared token/copy/layout semantics | Generated tokens, account vi/en copy, existing chrome/components | vi/en copy unit checks PASS in the 57-test run; static token gate and actual pixels remain separate |
| Mobile tests/build select real cases | Gradle test/build tasks | BLOCKED at Gradle wrapper distribution download; no compile/test task executed |
| Phone reflow/large text/theme/locale pixels | Actual shared CMP screenshots with named dimensions/state/data source | NOT_RUN; no Compose screenshot was captured |
| Android/iOS device accessibility and lifecycle | Suitable native build/device, TalkBack/VoiceOver and interrupted-flow oracles | NOT_RUN; no device execution claimed |
| Native provider/secure-store/callback/social authority | Future reviewed platform integration with real provider/session/permission oracles | NOT_IMPLEMENTED in this UI draft; conditional future integration obligation |
| Native-only source/config policy | Existing native policy checker and its 5 helper regressions | PASS source/config only, 0 packaged artifacts inspected; no APK/app/runtime acceptance |
| Existing native contract/lifecycle regressions | Unchanged current native contract tests and Android callback source compile | PASS: 52 JUnit tests; not account/UI/device/native renderer evidence |
| Portable core / exact-tree hosted CI | Required repository gate and published-head checks | Core attempt BLOCKED: cargo absent (shell exit 127); hosted exact-tree CI NOT_RUN |

The standalone focused JVM run does not replace the pinned mobile build.
An unavailable Gradle distribution prevents Gradle compilation and test discovery;
zero selected tests is not test success. Planned cases and preview code do not
establish rendered layout, screen-reader completion or current native authority.

## Original acceptance map

| Issue #103 criterion | Implementation scope | Remaining evidence / boundary |
|---|---|---|
| Account/Login/Register/Profile/Friends destinations | Fixed public shell routes and truthful native task panels | Actual parser/public save/restore/Back tests PASS (14); Compose platform/interaction NOT_RUN |
| Current supported read-only account behavior matches web | Typed current adapter state; immutable ID and optional approved self profile | Focused state/controller tests PASS under the qualified JVM toolchain; native UI/web edit/social not claimed |
| No fake collection/local authentication | No credential form, synthetic auth persistence or production fixture fallback | Source-read; inspect actual compiled UI when build becomes available |
| Loading/signed-out/authenticated/expired/unavailable/error | Explicit sealed state values | Focused state/controller tests PASS; Compose tests NOT_RUN |
| Shared visually consistent avatar/profile | Existing neutral fallback; already-loaded managed bitmap bound to identity instance | Fresh identity-instance controller law PASS; Compose owner guard/pixels NOT_RUN; image delivery absent |
| Generated semantic tokens, no new palette/raw colors | Existing generated roles and shared contained components | Static no-raw-color gate must be recorded; compiled pixels NOT_RUN |
| Narrow platform-safe native session interface | Typed port/controller/adapter; no credential, raw URL, callback payload or persistence in UI | Focused controller tests PASS; native platform adapter NOT_IMPLEMENTED |
| Future provider plugs in without shell redesign | Explicit platform/session seam; existing provider retained | Future real OIDC, secure storage and deep-link integration tests still required |
| vi/en, large text, 320/390 dp, accessibility tested | Exhaustive copy and 19 implemented shared CMP tests/previews | Copy unit checks PASS; Compose layout/interaction/accessibility BLOCKED/NOT_RUN |
| Local play remains available where policy allows | Public Library escape; no added account prerequisite or gameplay changes | Existing production native gameplay availability is unchanged under ADR-0043/PR108 |
| Canonical `apps/mobile/` paths | Account implementation in the single existing CMP tree | Source-read; no second app tree |

Issue #103 must not be closed solely because this UI draft exists. The current
requested tests/build/visual acceptance remain unresolved until executed. Real
provider callback/deep-link/secure-store/session expiry/logout and social
integration are future obligations when those adapters are connected; this
draft does not claim they already exist or are covered by screenshots.

## Command record

### Reproducible account/copy/Back/fixture run: PASS, 57 tests

`tools/test-mobile-account-contract.sh` selects 30 account state/controller,
3 account-copy, 3 existing shell-copy, 2 local-confirmation Back-port,
14 actual BackStack/Destination and 5 synthetic-preview-fixture tests. The
explicit seven JUnit classes selected and
passed all 57 tests; no tests were ignored. Compiler and JUnit exits were 0.
The [actual combined output](account-contract-tests.log) records Kotlin 2.4.20,
JRE 21.0.12.1, `-Werror`, JVM bytecode target 17 and `OK (57 tests)`.
JUnit is 4.13.2 and Hamcrest Core is 1.3. The public BackStack/Destination
source is now independent of Compose; unchanged `BackStackSaver` behavior is
retained in its Compose adapter. The runner compiles actual source/test files,
including unchanged GameHost/bridge contract inputs, with no source extraction
or stubs. This public-route PASS does not establish Compose saved-state registry
or device restoration acceptance.

From the repository root in the recorded environment:

```bash
export KOTLIN_HOME=/workspace/scratch/429f9ca90843/native-host-validation/toolchain/kotlinc
export COROUTINES_JAR="$KOTLIN_HOME/lib/kotlinx-coroutines-core-jvm.jar"
export JUNIT_JAR=/workspace/scratch/429f9ca90843/native-host-validation/toolchain/junit-4.13.2.jar
export HAMCREST_JAR=/workspace/scratch/429f9ca90843/native-host-validation/toolchain/hamcrest-core-1.3.jar
tools/test-mobile-account-contract.sh
bash -n tools/test-mobile-account-contract.sh # PASS: script syntax only
```

The explicitly selected bundled coroutines library is **1.8.0**, not the pinned
mobile **1.11.0**. These are bounded JVM contract/copy/fixture checks, not Gradle,
Compose pixel/interaction, native HTTP/provider/secure-store, device lifecycle or
accessibility acceptance. The 19 Compose UI test methods are implemented but
NOT_RUN due to the separate Gradle bootstrap blocker. No screenshot is present.

### Focused JVM state/controller and sensitivity run: PASS

The final five account source/test files were compiled with Kotlin 2.4.20,
`-jvm-target 17 -Werror`, on JRE 21.0.12.1. JUnit 4.13.2 selected
`AccountStateTest` (6 tests) and `AccountSessionControllerTest` (24 tests).
Compiler exit was 0 with no warnings; JUnit exit was 0 with `OK (30 tests)`.
The [output](state-tests.log) and [source hashes](state-source-sha256.txt) bind
that final focused selection, including fresh identity snapshot replacement and
both synchronous collector-reentry regressions. Each presentation identity uses
reference equality: a real StateFlow collector must observe a fresh equal-value
Authenticated result even when it misses a synchronous intermediate Loading.
The source hashes were verified unchanged after execution.

For sensitivity, an external temporary source copy restored the previous
identity value equality. Its 24 controller tests intentionally failed the fresh
identity law and `sameValueSynchronousRefreshIsObservableWhenSlowCollectorMissesLoading`
(expected 2 authenticated emissions, observed 1). The
[old-behavior output](value-equality-regression.log) records those two failures;
the actual repository was not mutated by this control. This is one bounded
regression control, not general mutation coverage.

This run uses the existing compiler distribution's bundled coroutines **1.8.0**,
not the Gradle-pinned **1.11.0**. It is bounded JVM state/controller compile and
example-test evidence, not the pinned Compose/Android/iOS build, native provider,
real credential handling, OS lifecycle or secure-store acceptance.

Exact invocation from `apps/mobile/` in this execution environment:

```bash
set -o pipefail
KOTLIN=/workspace/scratch/429f9ca90843/native-host-validation/toolchain/kotlinc
TOOLS=/workspace/scratch/429f9ca90843/native-host-validation/toolchain
VALIDATION=/workspace/scratch/429f9ca90843/account-state-validation
CP="$KOTLIN/lib/kotlinx-coroutines-core-jvm.jar:$KOTLIN/lib/kotlin-test.jar:$KOTLIN/lib/kotlin-test-junit.jar:$TOOLS/junit-4.13.2.jar:$TOOLS/hamcrest-core-1.3.jar"
"$KOTLIN/bin/kotlinc" -jvm-target 17 -Werror -cp "$CP" \
  shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/account/AccountState.kt \
  shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/account/AccountSessionPort.kt \
  shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/account/AccountSessionController.kt \
  shared/src/commonTest/kotlin/com/loveoverflow/tabula/mobile/AccountStateTest.kt \
  shared/src/commonTest/kotlin/com/loveoverflow/tabula/mobile/AccountSessionControllerTest.kt \
  -d "$VALIDATION/account-tests.jar" 2>&1 | tee "$VALIDATION/compile.log"
java -cp "$VALIDATION/account-tests.jar:$CP:$KOTLIN/lib/kotlin-stdlib.jar" \
  org.junit.runner.JUnitCore \
  com.loveoverflow.tabula.mobile.AccountStateTest \
  com.loveoverflow.tabula.mobile.AccountSessionControllerTest \
  2>&1 | tee "$VALIDATION/tests.log"
```

### Pinned mobile build/Compose checks: BLOCKED

From `apps/mobile/`:

```bash
GRADLE_USER_HOME=/workspace/scratch/429f9ca90843/gradle-account-validation \
  ./gradlew :shared:testAndroidHostTest :previewApp:test :android:assembleDebug --no-daemon
```

Exit 1 occurred during wrapper bootstrap while fetching
`https://services.gradle.org/distributions/gradle-9.7.0-bin.zip`:
`java.net.SocketException: Network is unreachable`. The
[actual bootstrap log](mobile-aggregate-gate.log) precedes Gradle task discovery;
no selected shared/Compose test, Android compile/build or screenshot task ran.
No CMP screenshot was captured. The 19 implemented Account UI test methods,
including Stop waiting on a dispatched sign-out, remain NOT_RUN/BLOCKED.

The environment has the reused official Kotlin compiler, JUnit/Hamcrest,
Android 37 compile jar and JDK 21. It does not supply the configured JDK 17,
complete Android SDK, Gradle or Compose runtime/UI/test/lifecycle dependencies.
The missing distribution/dependency route is an environment blocker; it is not
an observed account/backend failure. The focused fallback tests do not turn
this into a pinned-build PASS. iOS linking/device acceptance still needs macOS
and the stated native prerequisites.

### Final source/config and existing native regression checks

From the repository root:

```bash
python3 tools/check-mobile-native-policy.py
python3 -m unittest tools/tests/test_mobile_native_policy.py -v
python3 -m unittest discover -s tools/tests -p test_mobile_native_policy.py -v
git diff --check
bash -n tools/test-mobile-account-contract.sh
cargo xtask check
```

- Native-only policy: PASS, source/config only with **0 packaged artifacts**
  inspected; [output](native-policy.log)
- Native-policy helper unit selection: PASS, 5 tests (both invocation forms
  above were executed); [output](native-policy-tests.log)
- Working-tree whitespace and focused-script syntax: PASS; neither is a
  compilation/runtime/permission test
- `cargo xtask check`: BLOCKED before the gate started, shell exit 127 because
  `cargo` is absent; [output](core-gate.log). No Rust fmt/clippy/test/dependency/
  generated-token/raw-color gate is reported passed from this attempt
- Exact published-tree hosted CI: NOT_RUN by this local evidence set

The existing, unchanged native contract suite was also executed from the root:

```bash
export KOTLIN_HOME=/workspace/scratch/429f9ca90843/native-host-validation/toolchain/kotlinc
export JUNIT_JAR=/workspace/scratch/429f9ca90843/native-host-validation/toolchain/junit-4.13.2.jar
export HAMCREST_JAR=/workspace/scratch/429f9ca90843/native-host-validation/toolchain/hamcrest-core-1.3.jar
export ANDROID_JAR=/workspace/scratch/429f9ca90843/native-host-validation/toolchain/android-37.jar
./tools/test-mobile-native-contract.sh
```

This passed 52 JUnit tests and compiled Android surface-callback source against
Android 37. The runner selects current native contracts plus
`NativeGameRuntimeTest`; it does not select the new account UI. It produced no
APK/device execution, Macroquad native renderer or image/frame acceptance.
That stdout was observed in the tool result and was not separately retained;
no additional log artifact is claimed. The following final provenance check
also exited 0, confirming those host/test sources are unchanged from the base:

```bash
git diff --exit-code a386524 -- tools/test-mobile-native-contract.sh \
  apps/mobile/shared/src/commonTest/kotlin/com/loveoverflow/tabula/mobile/NativeGameRuntimeTest.kt \
  apps/mobile/shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/host \
  apps/mobile/shared/src/androidMain/kotlin/com/loveoverflow/tabula/mobile/host
```

## Residual scope

Production native auth/provider enrollment, credential storage, verified
callback/deep links, profile mutation, other-profile lookup, friend graph and
presence stream are unavailable. The UI is read-only and uses no email/password,
provider avatar URL, statistic, achievement or local account cache. Preview and
test facts are explicitly synthetic and remain outside production entrypoints.

Desktop Compose, if later executed, establishes shared shell interaction and
pixels only. Native Android/iOS builds, gestures, IME/autofill readiness for any
future enabled form, foreground/process-death behavior, TalkBack/VoiceOver and
real provider/social acceptance remain separate. No source prototype, build,
screenshot or double establishes production startup, deployment, native gameplay,
store release or Phase 4/5/6 exit.

This scope leaves the separate native GameHost prototype and lifecycle/asset/
device evidence in [its own ledger](../mobile-native-host/adapter-prototype.md).
