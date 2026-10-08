# CMP discovery density correction — 2026-10-08

Base: `develop @ 119f205`, pulled before implementation. Owner requested a direct
commit/push to `develop`. Architecture doc 00, ADR-0027/0043/0045 and the
[mobile shell specification](../../ui/screens/mobile-shell.md) retain authority.
This change concerns shared shell presentation; no native gameplay or phase exit
is established.

## Changed claims and observations

| Claim | Owner/oracle | Evidence |
|---|---|---|
| Compact page/hero headings retain serif identity with a quieter hierarchy | Authored `sys.type.shell-display`; Design01 reference at `fe6a6bac` | Generated 28sp/36sp, weight 400; existing body/label/display roles unchanged |
| Home exposes a complete first card before scrolling at 390×844dp | Shared Home layout; actual Compose bounds/pixels | Existing registry UI test now checks the first card's unclipped bounds against the initial scroll viewport, before any scrolling |
| Compact density preserves readable copy and usable actions | Shared actions and large-text reflow | Existing 320/390dp, vi/en, four-scheme and 200% text tests; ≥44dp target checks remain |
| Catalog metadata does not grant native launch authority | Existing detail/setup interaction tests | Registry setup remains disabled; simulated host creation count remains zero |

Before/after screenshots use actual 390×844 pixels at 100% text. Enlarging an
export does not change this logical viewport. Inspected English/light and
Vietnamese/dark Home captures: hero height drops from approximately 455dp to
273dp; the first card occupies approximately y=511–712, entirely above the
content boundary at y=760. The 320dp English Home also shows a complete first
card. At 200% text, content grows and scrolls; complete first-viewport cards are
not required. Reviewed captures show no horizontal text clipping.

The hero uses a 96dp landscape image beside its heading and full-width body/action
below. Compact cards use an 84dp landscape thumbnail beside name/tagline, then
full metadata/detail action. Above 130% text, composition reflows vertically.
Generic wide catalog cards retain landscape artwork above their copy. No density
or font-scale transform shrinks the entire interface (I-10).

## Executed checks

Environment: macOS 26.5.2 (25F84), Rust/Cargo 1.96.1, Gradle 9.7.0,
Microsoft OpenJDK 17.0.19 project runtime, Kotlin 2.4.20, CMP 1.12.0,
Compose Hot Reload 1.2.0; Xcode 26.6 (17F113).

Commands below run from the repository root unless a working directory is named.

| Check | Result |
|---|---|
| Baseline `./gradlew --console=plain :previewApp:test --tests '*DiscoveryUiTest.registryHomeLibraryAndDetailRenderAcrossPhoneTabletThemeAndLocalePartitions'` (`apps/mobile`) | PASS: 1 test with 7 viewport/theme/locale partitions; baseline images preserved outside Git |
| `cargo xtask gen-tokens` | PASS: authored Rust/CSS/JSON/Kotlin adapters regenerated |
| `cargo test -p xtask tokens_cmd::tests` / `cargo test -p tabula-design` | PASS: 15 token tests / 12 design tests |
| `cargo check -p tabula-testkit` | PASS: exhaustive presentation style naming updated |
| `./gradlew --console=plain :previewApp:test --tests '*DiscoveryUiTest'` (`apps/mobile`) | PASS: 5 discovery tests after correcting a Kotlin receiver compilation error in the new hero branch |
| `./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug` (`apps/mobile`) | PASS: 163 shared tests + 61 Desktop tests; zero failures/errors/skips; Android APK assembled |
| `python3 tools/check-mobile-native-policy.py --apk apps/mobile/android/build/outputs/apk/debug/android-debug.apk` | PASS: source/configuration and one built artifact inspected |
| `./gradlew --console=plain :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64` (`apps/mobile`) | PASS: both Kotlin targets compiled |
| `xcodebuild -project ios/TabulaApp.xcodeproj -scheme Tabula -configuration Debug -sdk iphonesimulator -derivedDataPath /private/tmp/tabula-mobile-density-xcode CODE_SIGNING_ALLOWED=NO build` (`apps/mobile`) | PASS: shared framework and simulator host linked; no device run |
| `cargo xtask check` | PASS: authoritative ten-gate core check; 1,353 actual tests passed, zero failed, 18 ignored doc examples |
| Live official MCP smoke at 320×640dp/100% text | PASS: semantic navigation/search/scroll/Back, source edit/reload/restore; 2 successful reloads, zero failed reloads, no UI error; original source bytes restored |
| `git diff --check` | PASS |

Initial sandboxed Gradle invocations could not write the existing cache lock;
approved cache access allowed execution. Compilation/build evidence is separate
from shared UI interactions and actual Android/iOS device rendering.
`just` is unavailable (exit 127), so its exact `cargo xtask check` recipe ran
directly. The first core attempt reached cargo-deny but could not write the
advisory database lock; an approved cache-access retry passed. Initial sandboxed
Xcode dependency resolution could not reach GitHub; the approved retry linked
successfully. The existing ICU simulator deployment warning remains recorded.

Live MCP used the existing verified local JBR
`25.0.2+10-b329.72-jcef`. The first automatic launch connected before its window
appeared: `list_windows` returned `[]`, so that run failed and preserved its
diagnostics in `mobile-density-live/`. Launching the existing preview separately
and waiting for its `Tabula preview content: 320x640` log resolved that timing
issue. The successful invocation was:

```bash
bash tools/mobile-preview.sh run --no-auto -Ppreview.width=320 -Ppreview.height=640 \
  -Ppreview.language=en -Ppreview.reducedMotion=true \
  -Pcompose.reload.jbr.binary=/private/tmp/tabula-jbr25/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72/Contents/Home/bin/java
python3 tools/mobile-agent-smoke.py --connect-only \
  --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/mobile-density-live-02 \
  --gradle-arg=-Pcompose.reload.jbr.binary=/private/tmp/tabula-jbr25/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72/Contents/Home/bin/java
```

Official `tools/list` reported Compose Hot Reload 1.2.0. Selected semantic
assertions: `shell-home` → `shell-games`; `discovery-search` retained `Chess`
through detail/setup/Back; `shell-start-local` remained disabled; the temporary
`Agent loop verified` text appeared after reload and `Your play space` returned
after restoring the original bytes. Ten selected semantic trees and MCP
screenshots plus the transcript/runtime log remain in the successful run directory.

Raw captures/results remain in ignored build reports. Selected SHA-256 hashes:

| Capture | Before | After |
|---|---|---|
| `discovery-390-light-en-registry-home.png` | `294c9398a23e8461b84e272e07446f67f58589cac317f7ac1b3fd44069959ad1` | `8a3c8022df3202bc6c00d23bde9856c79d24e11fab382863a2b7e957ec7080b2` |
| `discovery-390-dark-vi-registry-home.png` | `99edcb48df2e2a3c864dcadf286c07c049c61377962b0d7f1bcf91f2f811302c` | `d4977df43f9df2a6205259c449bb8b8080f072ffcda07c072897fcef474ce6e1` |

After captures: `apps/mobile/previewApp/build/reports/shell-screenshots/`.
Core/iOS logs: `apps/mobile/previewApp/build/reports/cmp-agentic-loop/mobile-density/`.
Complete test XML/HTML and screenshots are build evidence and are not committed.

Native device visual acceptance, TalkBack/VoiceOver and real gameplay remain
outside this shared layout correction. Existing native adapter gates stand.
