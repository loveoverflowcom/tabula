# CMP bottom navigation balance — 2026-10-08

Base: `develop @ 04ad5f437eac51f1cea35bda06c09fbfd5c53af0`, following the
[discovery density correction](../mobile-discovery-density/README.md). The owner
requested closer Design01 alignment and explicitly retained Home / Library /
Account. Direct commit/push to `develop` remains authorized. Architecture doc 00,
ADR-0043/0045/0046 and the [mobile shell specification](../../ui/screens/mobile-shell.md)
retain authority; this is shared shell presentation evidence.

## Observable changes

The previous phone bar assigned slot widths from localized word measurements and
filled the entire selected icon/label surface. The new bar has three equal slots,
20dp icons, a 48×32dp selected indicator around the icon only, a 4dp icon/label
gap, regular generated 12sp labels and a subtle 1dp top divider. Each full slot
remains one selected/clickable/focusable action, at least 64dp high. High-contrast
schemes add a selected indicator boundary and retain a clear top divider.

At large text, complete labels wrap between words. The existing generated 11sp
label fallback is used when the longest word cannot fit. Phone targets reserve
the focus ring without an additional interior gap, and above 130% text the bar
releases its outer horizontal inset. These changes retain whole English
`Account` at 320dp/200% while preserving equal slots. Rail sizing, typography,
icon size and selection treatment retain their existing behavior.

The Home catalog heading also uses the existing regular serif `shell-display`
role, matching the page/hero hierarchy. No token source, navigation route,
catalog authority or native host boundary changed.

## Reference and inspected renders

Reference: owner-attached Design01 PNG, 390×1631 pixels, SHA-256
`7135ba0d15895ceb7ef96aa6b7dab232959f91225e8f363973e63875f8a491e9`.
The reference has four tabs; its icon-only selection and spacing inform this
three-route composition. Saved before/reference pixels remain outside Git in
`/private/tmp/tabula-bottom-nav-before/`.

Opened five before/after Home scenarios: 390dp English/light and Vietnamese/dark,
320dp English/light, 390dp English/light/200% and 320dp Vietnamese/dark/200%.
Opened fresh 320dp Account English/200% light and high-contrast-light captures.
At normal text the phone bar is 73dp high; 390dp icon centres are approximately
70/195/320, and 320dp centres approximately 59/160/261. The first complete game
card still fits above y=771: approximately y=518–720 at 390dp and y=554–756 at
320dp. Reviewed large-text labels retain whole words; no horizontal clipping,
overlap or unreachable action was found. Focus still outlines the complete slot,
distinct from the selected icon indicator.

Raw captures are ignored build evidence in
`apps/mobile/previewApp/build/reports/shell-screenshots/`. Selected SHA-256 hashes:

| Capture | Before | After |
|---|---|---|
| `discovery-390-light-en-registry-home.png` | `8a3c8022df3202bc6c00d23bde9856c79d24e11fab382863a2b7e957ec7080b2` | `8363dc6ff02acf19585404d03c175be443062a30da61b5b226b9a789081a6a3e` |
| `discovery-390-dark-vi-registry-home.png` | `d4977df43f9df2a6205259c449bb8b8080f072ffcda07c072897fcef474ce6e1` | `a86c446310d0f63778994424923a107b1d09c8ea4b8677f7a6470132a8af3533` |
| `discovery-320-dark-vi-font200-home.png` | `6643b641428860c8f7dd92c8acd069c81d305100d9862f8d95f623fb845d23a2` | `98771bfbb1fa85f98f8e6a71940682cf0ef2d4a91d1cb190485829480733993c` |

## Executed checks

Environment matches the preceding density ledger; Xcode 26.6/17F113,
Swift 6.3.3 and simulator SDK 26.5 were inspected for this run.

| Check | Result |
|---|---|
| Focused `:previewApp:test --tests '*DiscoveryUiTest' --tests '*ResponsiveShellTest'` | PASS: 7 tests; responsive Account/Profile covers 320/390 phone and 844dp rail, vi/en, four schemes, 100/200% text |
| `./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug` (`apps/mobile`) | PASS: 163 shared + 61 Desktop tests freshly executed, zero failures/errors/skips; debug APK assembled |
| `python3 tools/check-mobile-native-policy.py --apk apps/mobile/android/build/outputs/apk/debug/android-debug.apk` | PASS: source/config and one packaged artifact inspected |
| `./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64` (`apps/mobile`) | PASS: both compile tasks executed |
| `xcodebuild -project ios/TabulaApp.xcodeproj -scheme Tabula -configuration Debug -sdk iphonesimulator CODE_SIGNING_ALLOWED=NO build` (`apps/mobile`) | PASS: shared simulator framework and Xcode host linked |
| `cargo xtask check-no-raw-colors` / `git diff --check` | PASS |
| Live official MCP at 390×844dp/100% English/light | PASS: semantic navigation/search/scroll/Back, source edit/reload/restore; 2 successful reloads, zero failed reloads; original source bytes restored |

New geometry assertions check equal slot widths (rounding tolerance ≤2dp), full
minimum touch targets, indicator containment and ≤80dp normal-text bar height.
Existing assertions verify whole words, horizontal text fit, content space and
reachable account/profile actions. Initial checks caught a merged-tree selector
mistake and genuine 320dp/200% English word wrapping. Reading the decorative
indicator from the unmerged tree and releasing unused phone padding resolved
them; the same focused suite and full mobile gate then passed.

The first iOS command hit a sandboxed Gradle cache lock; approved cache access
allowed execution. Newly generated Xcode `Package.resolved` was preserved in
`/private/tmp/tabula-bottom-navigation-ios-generated/`, outside the commit. The
preexisting untracked daemon properties file was retained byte-for-byte.

Live preview used the existing verified JBR `25.0.2+10-b329.72-jcef`:

```bash
bash tools/mobile-preview.sh run --no-auto -Ppreview.width=390 -Ppreview.height=844 \
  -Ppreview.language=en -Ppreview.reducedMotion=true \
  -Pcompose.reload.jbr.binary=/private/tmp/tabula-jbr25/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72/Contents/Home/bin/java
python3 tools/mobile-agent-smoke.py --connect-only \
  --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/bottom-navigation-live \
  --gradle-arg=-Pcompose.reload.jbr.binary=/private/tmp/tabula-jbr25/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72/Contents/Home/bin/java
```

Waited for the actual `Tabula preview content: 390x844` log before connecting.
Official MCP 1.2.0 reported three action slots at x=8/132/257, widths=124/125/125,
height=64, and bar bounds x=0/y=771/width=390/height=73. Opened its actual Home
capture (`screenshot-8.png`). Library selection, search for `Chess`, scrolling,
detail/setup and Back retained their semantics and search state. Native launch
remained disabled. The temporary reload marker appeared and the baseline text
returned after restoration. Transcript, ten selected trees, screenshots and
result JSON remain under `bottom-navigation-live/`; build/static/iOS logs remain
under `cmp-agentic-loop/bottom-navigation/`.

No Rust/token source changed, so this follow-up used the raw-color guard alongside
the preceding commit's full core gate. Shared host interactions, inspected
Desktop pixels and platform compilation/linking are separate evidence. Actual
Android/iOS device visuals, safe areas, TalkBack/VoiceOver and native gameplay
were not executed by this change; existing adapter and phase gates stand.
