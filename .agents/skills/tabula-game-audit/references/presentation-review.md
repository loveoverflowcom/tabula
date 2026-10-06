# Presentation boundary and behavior layer

Read doc 00 I-10/I-12,
[doc 04](../../../../docs/architecture/04-frontend-and-design-system.md), the
actual feature-gated presenter, and
[presentation API](../../../../crates/tabula-presentation/src/lib.rs).

| Claim | Evidence to inspect |
|---|---|
| Canonical state stays independent | Presenter takes projections/events; camera, hover, focus, selection, drag and animation live in `GamePresentation::Local`; no mutation enters `State` |
| Intent is input, not authority | Pointer/keyboard/touch command construction, invalid/cancelled drag, promotion/rotation/claim selection, pending/rejected command behavior, phase/actor gating |
| Renderer stays replaceable | `RenderList`, semantic tokens and typed asset declarations; no direct renderer calls in game rules/presentation |
| Time changes presentation only | Event-driven animation, sparse/dense sampling convergence, usable input during motion, no duplicate pieces |
| Camera changes pixels only | Screen/world round trips, zoom clamps, stable HUD, same intended inputs across cameras yield identical canonical state |
| UI is usable | Responsive bounds, theme/contrast, keyboard/focus, semantic accessible descriptions and affordances; screenshots/manual scenarios for actual visible quality |

Render-list snapshots test command lists, not the renderer, font metrics,
browser event integration or visible quality. Review diffs before updating
expectations. A WASM build/staged host is build evidence; record whether the
app was actually launched and exercised in a browser.

Implemented presenters and tests:

- [Chess](../../../../games/chess/src/presentation/mod.rs): projection →
  presentation → intent → `apply`, promotion, tap/keyboard/drag equivalence,
  event-driven motion, local focus/drag, terminal UI, responsive/theme snapshots.
- [Tiles](../../../../games/tiles/src/presentation.rs): placement/claim,
  pan/zoom, keyboard navigation, HUD transform, camera noninterference,
  event cues, accessible cursor description and snapshots.
- [Werewolf](../../../../games/werewolf/src/presentation/mod.rs): ADR-0035's
  opt-in isolated-seat local simulator; authorized reveal/conceal, target then
  explicit command, projection privacy, responsive roster/drawer, local motion,
  and semantic accessibility. Use the [Werewolf rubric](werewolf.md) and
  [#84 ledger](../../../../docs/verification/werewolf-redesign-84/README.md).

Enable the feature and confirm nonzero selected tests:

```bash
cargo nextest run -p tabula-game-chess --features presentation --lib -E 'test(presentation::tests)'
cargo nextest run -p tabula-game-tiles --features presentation --lib -E 'test(presentation::tests)'
cargo nextest run -p tabula-game-werewolf --features presentation --lib -E 'test(presentation::tests)'
```

Tiles has no camera rotation, wheel/pinch events or full Board Reader. Caro
has no presenter. Werewolf's bounded local presentation exception does not open
Phase-7 online/social UX or Phase-8 voice.
Do not add fake presentation checks or launch unsupported games to fill a row.
