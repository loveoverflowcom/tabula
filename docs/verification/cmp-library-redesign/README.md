# CMP Library redesign — 2026-10-08

Owner-supplied `tabula-cmp-mobile-design` handoff, authored against develop
`6d31cef51f9f186e6bd43213c6ecb0d0d7a47162`. The work started from that clean
remote ref on `codex/cmp-library-redesign`. The primary checkout's unrelated
uncommitted changes were excluded. Reviewed staged patch SHA-256, excluding
this ledger: `df08cd24dadd259cad7378535ef2bf44467600af93cef7ff73054d193c410c8a`. The PR head supplies the immutable final source.

## Behavior and ownership

`TabulaApp` mounts shared `GamesScreen` and keeps its caller mounted under
`DetailScreen`. `DiscoveryLibrary.kt` owns list/grid presentation and modal
filter draft. Route query remains app-owned; layout choice and scroll remain
at their existing saveable route lifetime. Whole cards open public metadata,
never a GameHost. Filter dismissal cancels; Apply commits the AND-combined axes;
draft Reset preserves search. At 390dp grid has two columns; 320dp and 200% text
release a full reading column. Square icons are 72dp, detail icons 88dp.

Game-owned PNG masters/companions retain the supplied bytes and provenance.
`cargo xtask gen-mobile-catalog` checks/copies only lightweight 256/512 companions;
`DiscoveryGameIcon` selects by physical size, fences rebinding and retains a
neutral fallback. Generated adapters remain generator-owned.

The registry's separate planned mobile inventory supplies Werewolf facts and
translated copy. Its descriptor has no setup/runtime vtable, no modes/fields,
and never enters linked discovery or runtime inventories. Home excludes planned
entries. Planned detail and restored setup expose information only. Existing
rollout-disabled/staff manifest and empty production native inventory remain.
ADR-0043/0045/0046, I-5/I-6, I-9/I-10 and protocol versioning are unchanged.

## Executed checks

Host: macOS arm64, JDK 17.0.19, Kotlin 2.4.20, Compose 1.12.0, Material3 1.9.0,
AGP 9.3.1, installed Android platform 37.0 revision 2, Xcode 26.6/17F113.
Hot Reload 1.2.0 automatically provisioned JBR 21.0.10/b1163.110. No runtime override.

| Claim | Exact check | Status / scope |
|---|---|---|
| Portable core contracts, generated data/art freshness and policy | `cargo xtask check` | PASS; `just` is unavailable, this is its authoritative equivalent; workspace test summaries contain 1357 passing tests and 18 pre-existing ignored tests |
| Registry planned separation / generator facts and inert PNGs | `cargo test -p tabula-registry`; `cargo test -p tabula-registry --no-default-features`; `cargo test -p xtask mobile_catalog_cmd`; scoped clippy and catalog freshness | PASS; 43 registry, 2 no-default, 7 generator cases |
| Shared public state and ports | `ANDROID_HOME=<installed-sdk> ./gradlew --console=plain :shared:testAndroidHostTest :previewApp:test :android:assembleDebug :shared:compileKotlinIosArm64 :shared:compileKotlinIosSimulatorArm64` from `apps/mobile` | PASS; 163 host tests, 66 preview tests,0 failed / 0 skipped; Android packaged; both iOS targets compiled |
| Native host linking | `xcodebuild -project ios/TabulaApp.xcodeproj -scheme Tabula -configuration Debug -sdk iphonesimulator CODE_SIGNING_ALLOWED=NO build` from `apps/mobile` | PASS; simulator app linked; device execution not implied |
| Current navigation, typing, scroll and edit/reload/restore | `python3 tools/mobile-agent-smoke.py --evidence-dir apps/mobile/previewApp/build/reports/cmp-agentic-loop/redesign-live-01` | PASS; actual official MCP tools/list, live trees, ten stage assertions, two successful reloads, original source bytes restored |
| New list/grid/filter/detail flow | Existing smoke client's `McpClient`/`Preview` against official stdio server,390×844/vi/reduced-motion; transcript in `redesign-design-390-04` | PASS; grid selected, cancel retained 3entries, Apply seats 8 AND maximum 45min left one planned entry, no setup action, Back retained grid/filters, `ma soi` matched the supplied Vietnamese name |
| Layout, loaded PNGs and control semantics | Fresh `DiscoveryUiTest`, `AgenticSemanticSmokeTest`, `ShellNavigationTest` and full preview suite | PASS; focused 15 methods; actual decoded PNG readiness, square geometry, responsive grid, scroll/query retention, all four axis+value accessible names, planned no-launch, restoration |
| Formatting and patch hygiene | `git diff --cached --check` | PASS |
| Android/iOS device gestures, IME, TalkBack/VoiceOver, performance and native gameplay | Actual device acceptance | NOT_RUN; shared desktop pixels/semantics and native compilation establish none of these |

## Inspected visual evidence

Opened the supplied `png/01-mobile-list.png`, `02-mobile-grid.png`,
`05-filter-sheet.png`, `06-werewolf-detail.png` and `07-chess-detail.png`, plus
fresh CMP before/after images below. References are browser DPR2; CMP live
captures are 390×844 pixels at density1/font scale1. Large text capture is320×844
at density1/font scale2 with a named synthetic catalog. Production live scenarios
use only generated public metadata and unavailable native hosts.

| Opened scenario | Path relative to ignored reports | SHA-256 |
|---|---|---|
| Before Library,390/en/light | `cmp-agentic-loop/redesign-before/library.png` | `0eabd1d9a8fc78d493f96dc286e642d4384147c0e2952bc279e85d78342fd1d9` |
| Before detail,390/en/light | `cmp-agentic-loop/redesign-before/detail.png` | `1bad5ff8c63ae4070c9d7587ed7467d6f814d02a70cdd390280846d3e60eb6e9` |
| After list,390/vi/light | `cmp-agentic-loop/redesign-design-390-04/01-library-list.png` | `74015b64ccfdfd4bd00920cfd68345840f3945dbd4cbdd05e1d3ac8c169cbe63` |
| After grid,390/vi/light | `cmp-agentic-loop/redesign-design-390-04/02-library-grid.png` | `f208c713e637f414684f7a72002ca8fb67e0afd791306ad57815c82261c65123` |
| After filter,390/vi/light | `cmp-agentic-loop/redesign-design-390-04/03-filter-sheet.png` | `9d7f89121cc57644838dee366cfc3f51a2868822f9a11fbfdd569e9990b7a8a0` |
| After planned detail,390/vi/light | `cmp-agentic-loop/redesign-design-390-04/05-werewolf-detail.png` | `e89603079a69bcc3caed7dfea73fac75804c488eb7cf426e0dba927065d952a8` |
| Filter,320/vi/dark/200% | `shell-screenshots/redesign-filter-sheet-320-vi-font200.png` | `ead1c5b0191cb8b610ee21fb69691c2c0ebf700df609fd13076e857ab7d6ad55` |

Findings: list now shows all three real square logos without the legacy banner;
normal 390dp grid shows two reading columns and compact facts. Cards/facts/names
wrap without horizontal clipping. Filter action values use body text and the
axis+value accessible name. The initial footer was visually too narrow; final
recheck equalizes two actions at normal text and stacks them at large text.
At 320dp/200%, modal body scrolls while close and footer remain reachable.
Planned badge and native explanation remain legible; no setup/start is exposed.
These are screenshot-inspected and interaction-tested desktop claims.

## Failures retained and recovery

Raw reports retain the initial missing SDK configuration, repaired stale test
assumptions (linked-only metadata and test saver enum support), and a concurrent
Gradle classpath-snapshot failure. Final successful mobile build/test and Xcode
runs were sequential. The first exploratory MCP client requested windows before
a window existed; the next waited. A later assertion expected decorative loaded
image tags in the official merged tree, which intentionally omits them; decoded
artwork is instead asserted by unmerged production-component tests and inspected
pixels. None of these exploratory failures is counted as a pass.

Raw JSON/screenshots/logs stay in ignored build reports; no binary verification
files are committed. They are also exported outside the managed worktree before
its owner-requested removal. Source art lives under games, generated companions
under Compose resources; build caches/APKs/frameworks/JBR are excluded from the PR.
CI execution and branch enforcement are independent of these local receipts.
