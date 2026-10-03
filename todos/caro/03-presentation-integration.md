# Goal

Make Caro playable locally through the existing generic LocalMatch/GamePresentation contracts and
record exactly what the SDK addition costs. Caro presentation is Phase 3; networking is Phase 4.

## Current state

`apps/game-client/src/lib.rs::LocalMatch<R,P>` already owns local input, timer/effect interpretation
and replay capture. `main.rs::run_local` is generic and takes a projected-turn function plus bot
and bot-seat values. Existing leaf wiring names Chess/Tiles using documented game-id scan markers.
No Caro presenter exists. Current startup parses native `std::env::args`; web/index.html only loads
one WASM binary and does not provide a game picker. A CLI arm alone is not proof of browser play.

The pack builder reads **assets/packs/<slug>/pack.source.toml**, not games/<slug>/assets. Backend
asset delivery/decoding is incomplete; stageable bytes do not imply live textures/audio load.

## Decisions

| Area | Recommendation | Reason / scope |
|---|---|---|
| Board geometry | Fixed board scaled/letterboxed to viewport; no canonical camera | BoardSize is bounded; visual geometry may use floats only in presentation |
| Interaction | Pointer hover/press/release target confirmation; keyboard arrows/Tab/Enter/Escape using existing FocusGraph/FocusState | Test target mapping, outside-board input and cancellation; no new input event type |
| Local state | Viewport, hover/focus/selection, transient placement/win animation | All in P::Local; no gameplay timer/state mutation from animation |
| Display | Coordinates, distinct stone shapes/labels, active seat, last move, actual WinLine, win/resign/timeout/draw banner, deadline if enabled | Read View; do not re-run win detection in presenter |
| Preview | client_preview=false; accepted ViewEvents drive animation | Do not confuse local hover feedback with authoritative prediction |
| Snapshots | Opening 15 light, opening 9 dark, focus/hover, line-win, terminal draw; include 19-cell compact layout and reduced-motion case | Stable RenderList snapshots pin layout, not rules truth |
| Assets | Minimal versioned pack in assets/packs/caro; semantic resource names, density/priority entries, catalog assets and optional sound | Exercise pack hashing independently; use existing primitive/placeholder fallback for playable local rendering |

## Proposed architecture

`View + P::Local + FrameCtx → RenderList`; `InputEvent → Option<Intent<Command>>`; semantic
ViewEvent → bounded animation/audio cues. Use existing builder, theme and motion primitives; no
new RenderCmd or renderer-specific API. Caro's layout may need a narrow float-arithmetic lint
allowance like existing presenters; canonical rules retain strict bans.

Drive the **same semantic placements** through real pointer/keyboard conversion at different
viewport sizes, focus/hover states and animation times, then compare accepted commands and final
canonical bytes. Merely applying identical Commands while changing unused Local fields proves
little. Include a control showing RenderLists differ while the resulting game state agrees.
Changing focus and pressing Enter is allowed to choose another cell; the law does not deny that.

## Scope and SDK accounting

C5 adds game-local presenter plus mechanical leaf wiring: dependency with rules/presentation/bots,
SelectedGame variant/dispatch/parser, run_caro wrapper, SetViewport implementation, app integration
test and replay capture assertion. The generic LocalMatch/run_local loop should need no change.
Use native `--game caro --solo` and hot-seat; all game semantics stay inside games/caro.

Record changed files/lines outside the game against the actual starting develop commit, not an
assumed main branch. Allowed additions include xtask dependency/dispatch, app leaf registration,
root replay fixtures, docs, CI rows and assets/packs/caro. Count these honestly as integration cost;
“four edits” is not an acceptance invariant. No platform crate/service game-id branch is permitted.

### Existing browser-selection gap (B1)

Before calling browser integration complete, inspect how the staged WASM app actually selects
Caro. Current index/argument parser provides no user-facing path. If still missing at C5:

1. Existing contract is sufficient for rendering/playing; the missing capability is a **generic
   app startup selector**, not a game rules or renderer API.
2. It is reusable for Chess, Tiles and Caro. Implement as a separate app prerequisite using existing
   presentation/input contracts, with game labels/factories at the current leaf wiring boundary.
3. No ADR is needed unless it changes a locked contract or phase; do not implement the Phase-4
   registry early. Record B1 separately from mechanically required Caro registration so SDK-friction
   reporting does not hide pre-existing app work.

Phase-3 Caro browser demo depends on B1 or a subsequently implemented supported startup path.
WASM compilation alone cannot close it. Responsive usability also needs actual inspection: a
15×15 board on a small phone does not automatically satisfy doc 04's 44dp interaction target intent.
Use target confirmation/keyboard fallback within existing contracts and document remaining device limits.

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| Layout remains reviewable | Clipping/bad contrast/no win marker | Named RenderList snapshots at fixed geometry/theme/time | example-tested (planned) | Every PR | Current headless rasterizer cannot rasterize all text/path/sprite output |
| Input means the intended placement | DPI/grid rounding, outside click, wrong key mapping | Pointer boundary table + keyboard sequence + local-state metamorphic property | example-tested + property-tested (planned) | Every PR | Physical touch precision needs manual check |
| I-10 holds | Animation delays authority or changes command | Same semantic interaction through different viewport/animation states | property-tested (planned) | Every PR | Do not compare deliberately different user targets |
| Local game is playable | Bot never moves, end/replay wrong | LocalMatch integration to terminal, View-driven bot and recorded replay verification | example-tested (planned) | Every PR | Macroquad loop inspected manually |
| Pack builds and verifies | Wrong path or resource/hash declaration | cargo xtask pack-assets caro; inspect staged pack manifest/resources | statically checked (planned) | Every PR | Delivery/decoder not proven by packaging |
| Browser can launch Caro | Only default Chess launches | Staged WASM manual play using supported generic selector | example-tested (manual, planned) | Phase exit | B1 prerequisite explicit |
| SDK boundaries preserved | Hidden generic/platform edits | check-deps/check-no-game-ids + scoped diff review | statically checked (planned) | Phase exit | Approved leaf markers are narrow, not blanket exceptions |

## Expected file changes

`games/caro/src/{presentation.rs,lib.rs,snapshots/*.snap}`, Cargo.toml;
`assets/packs/caro/pack.source.toml` and source assets; app Cargo.toml, main.rs, tests/local_match.rs;
`docs/games/caro.md`. B1, if needed, is a separate app PR with its own exact files and validation.
Use semantic tokens and existing localization conventions; missing generic i18n infrastructure is
recorded, not invented as part of a Caro presenter.

## Implementation steps

1. Present opening board; add pointer/focus mapping with tests.
2. Render accepted placement and each terminal reason; add reduced-motion/interruption cases.
3. Stage/verify minimal pack, retain usable fallback through current backend.
4. Add leaf client wiring, bot selection, LocalMatch replay/terminal integration case.
5. Demonstrate native and browser play, resolving B1 separately if needed; record actual SDK cost.

## Acceptance criteria

- [ ] Hot-seat and solo reach terminal state; keyboard-only placement/gameplay works.
- [ ] Reviewed snapshots use real supported primitives/tokens, no claimed full raster coverage.
- [ ] Asset source path matches actual builder; pack and fallback are demonstrable.
- [ ] Browser can select Caro; build-only evidence does not substitute for demo.
- [ ] No generic LocalMatch refactor or platform change hidden in registration accounting.
- [ ] Core gate and applicable feature/WASM checks green.

## Residual risks

Backend asset loading, small touch targets and browser selection may expose app-level gaps.
Keep them separately owned and do not weaken rules/presentation boundaries to claim completion.
Screen-reader Board Reader depth is Phase 9; Phase 3 still owns useful describe text and keyboard input.

## Next dependency

[C5/C6](04-pr-sequence.md), with explicit B1 dependency if browser selection remains absent.
