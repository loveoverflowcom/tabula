# CMP Material icons and leading game artwork — 2026-10-08

Base: `develop @ b2ea5ddd57f7796144ba10a7a50adcc15113cf3e`. Owner requested
stock Material/M3 icon equivalents and corrected the image scope to the small
cover left of each game name. Direct commit/push to `develop` remains authorized.
Architecture doc 00, ADR-0043/0045/0046 and the
[mobile shell specification](../../ui/screens/mobile-shell.md) retain authority.

## Changed presentation

The shell uses seven official Material Symbols Outlined vector resources through
Material3 `Icon`: Home, `grid_view` Library, Person Account/neutral avatar,
auto-mirrored Back, Search and checked/unchecked radio. `ShellIcon` supplies
explicit generated tint/size, with decorative child semantics. Existing parent
actions retain localized names, selected state, touch/focus bounds and tags.
Managed avatar identity fencing remains at its existing owner.

Symbols come from Google's pinned commit
`737e3324305806514d7909874fa1818ae1808232`, with the original Apache-2.0 license
packaged. [Source attribution and import transformations](../../../apps/mobile/MATERIAL-SYMBOLS.md)
record the exact subset. Android theme/external color references become an
opaque white alpha mask; the displayed color comes from Tabula's semantic tint.
Material3 is pinned to stable `1.9.0`, as paired by the installed Compose 1.12
plugin. The current official [Compose icon guidance](https://developer.android.com/develop/ui/compose/graphics/images/material)
recommends Material Symbols resources; an additional legacy icons pack is not
needed. The shared [Material3 Icon API](https://kotlinlang.org/api/compose-multiplatform/material3/androidx.compose.material3/-icon.html)
supports explicit tint and a null decorative description.

Compact game images previously inherited a 16dp top-corner clip inside an
additional 8dp thumbnail clip. In an 84×37dp image this removed substantial
corner area. `DiscoveryGameCover` now accepts the caller's shape; compact cards
apply one 4dp clip. The declared SVG aspect ratio is preserved, with a 120dp-wide
thumbnail in cards ≥340dp and 84dp below that width, preserving narrow reading
space. Hero, large/detail cover composition and game-owned SVG source retain
their previous layout/content. The principal chess pieces are both visible;
the background board's authored extension beyond its SVG viewport is intentional.

## Inspected pixels and interactions

Saved before/reference captures in `/private/tmp/tabula-icon-art-before/`.
Reference is the owner-attached Design01 PNG, SHA-256
`7135ba0d15895ceb7ef96aa6b7dab232959f91225e8f363973e63875f8a491e9`.
Opened paired Home/Library at 320dp English/light and 390dp English/light and
Vietnamese/dark, detail at 390dp English/light, 390dp English/200% Home/detail,
320dp Vietnamese/200% Home and high-contrast Account at 320dp English/200%.
The 390dp leading image is approximately 120×53dp, with both chess pieces and
the Tiles building/road clearly visible. At 320dp the smaller radius restores
the full rectangular scene. Names/taglines retain their reading width; no
horizontal clipping or overlap was found. First complete Home card ends at
approximately y=720 (390dp) and y=756 (320dp), above navigation at y=771.

Raw preview captures remain in ignored
`apps/mobile/previewApp/build/reports/shell-screenshots/`. Selected SHA-256 hashes:

| Capture | Before | After |
|---|---|---|
| `discovery-390-light-en-registry-home.png` | `8363dc6ff02acf19585404d03c175be443062a30da61b5b226b9a789081a6a3e` | `5ced21b4b9b0b18b9bdf0b15248c83108c2bdb2e2067f2d6a265685f5fa9b960` |
| `discovery-390-light-en-registry-library.png` | `e9d43a2c5940f663d1c7ef95072c65851e19c8ce20ec9df9b9616f927bfacae1` | `0eabd1d9a8fc78d493f96dc286e642d4384147c0e2952bc279e85d78342fd1d9` |
| `discovery-320-light-en-registry-home.png` | `a6a25cf87488b057e94b75a4ab0585bd43e4b7b18dbf8c8356600c5ddedf34c9` | `c10b7b148817fc4ecdbe670afce73a420c61a7ab1fa5c364848a3a6a6760789e` |

Live official MCP 1.2.0 at 390×844dp, density 1 and 100% English/light reported
the Home cover at x=32/y=543/width=120/height=53; Library covers at x=32/y=467
and x=32/y=684, both 120×53. The three bottom actions retained widths 124/125/125
and height 64, with a 390×73dp bar. Home→Library selection, search for `Chess`,
scroll, detail/setup and Back passed; search state survived Back and native launch
remained disabled. Opened an actual MCP Home capture with the new glyphs and
leading cover. The temporary reload marker appeared; baseline text returned after
source restoration. Two reloads succeeded, zero failed, no UI error, original
source bytes restored.

## Executed checks

Toolchain matches the preceding [bottom navigation ledger](../mobile-bottom-navigation/README.md).
This run inspected Xcode 26.6/17F113, Swift 6.3.3 and simulator SDK 26.5.
Commands run from the repository root unless `apps/mobile` is specified.

| Check | Result |
|---|---|
| `./gradlew --console=plain :previewApp:test --tests '*DiscoveryUiTest' --tests '*ResponsiveShellTest' --tests '*AgenticSemanticSmokeTest'` (`apps/mobile`) | PASS: 9 nonempty tests; actual resource decoding/semantics/pixels across phone/rail, vi/en, four schemes and 100/200% text |
| `./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug` (`apps/mobile`) | PASS: 163 shared + 61 Desktop tests freshly executed, zero failures/errors/skips; debug APK assembled |
| `./gradlew :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64` (`apps/mobile`) | PASS: both compile tasks executed |
| `xcodebuild -project ios/TabulaApp.xcodeproj -scheme Tabula -configuration Debug -sdk iphonesimulator CODE_SIGNING_ALLOWED=NO build` (`apps/mobile`) | PASS: simulator framework and host linked |
| `python3 tools/check-mobile-native-policy.py --apk apps/mobile/android/build/outputs/apk/debug/android-debug.apk` | PASS: one fresh Android artifact inspected |
| `python3 tools/check-mobile-native-policy.py --app <Xcode Debug-iphonesimulator/Tabula.app>` | PASS: one fresh iOS artifact inspected; exact path/command in local provenance |
| `cargo xtask check-no-raw-colors` / `git diff --check` | PASS |
| Official live MCP smoke below | PASS: ten selected semantic stages, edit/reload/restore and inspected current pixels |

The first focused resource build failed because an attribution README was placed
at `composeResources/` root, which accepts only resource directories. Moving the
document to `apps/mobile/MATERIAL-SYMBOLS.md` fixed generation; its failure log was
preserved and the focused/full gates then passed. Newly generated Xcode
`Package.resolved` was saved outside Git; preexisting daemon properties remain
byte-identical. No Rust/token/catalog source changed; the raw-color guard is the
applicable Rust check for this follow-up, alongside the preceding full core gate.

```bash
bash tools/mobile-preview.sh run --no-auto -Ppreview.width=390 -Ppreview.height=844 \
  -Ppreview.language=en -Ppreview.reducedMotion=true \
  -Pcompose.reload.jbr.binary=/private/tmp/tabula-jbr25/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72/Contents/Home/bin/java
python3 tools/mobile-agent-smoke.py --connect-only \
  --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/material-icons-thumbnails-live \
  --gradle-arg=-Pcompose.reload.jbr.binary=/private/tmp/tabula-jbr25/jbrsdk_jcef-25.0.2-osx-aarch64-b329.72/Contents/Home/bin/java
```

Waited for the actual 390×844 content log before connecting. Raw logs and exact
iOS/toolchain/artifact provenance remain under
`cmp-agentic-loop/material-icons-thumbnails/`; MCP transcript/trees/pixels and
`result.json` remain under `material-icons-thumbnails-live/`. Shared host tests,
live desktop interaction, inspected pixels and native compilation/packaging are
distinct evidence. Android/iOS device visuals, safe areas, TalkBack/VoiceOver,
RTL runtime behavior and native gameplay were not executed; existing gates stand.
