# Compact Material 3 Expressive foundation

Stage A of [issue #49](https://github.com/loveoverflowcom/tabula/issues/49).
Implementation base: `develop @ f5d73cb9355b3e057bd56ae99b34f43961270142`.
Design reference: `030da25d0098e240ab2cf36dacf9892e8b320a89`, pack `01-foundation`,
including `shared/material-component-map.md`. Architecture doc 00, doc 04 §7–§10,
ADR-011, and ADR-027 govern this specification.

## Ownership and phase

`tokens.toml` owns theme meaning; `xtask gen-tokens` owns the Rust/CSS/JSON adapters.
`tabula-presentation` owns renderer-neutral mechanics, including hit testing and focus;
game presenters own the meaning of their local interactions. Leptos components own DOM
semantics when Phase 5 opens. Preference persistence belongs to the client adapter, never
`GameRules::State` (I-10). The client consumes only projections (I-5/I-6).

At the pinned base, Chess and Tiles have presentation consumers. `apps/web/src/main.rs`
still has its PHASE 5 banner; protocol, match, storage, and net-client retain Phase-4
skeletons. No Phase-4 exit evidence establishes authorization for the shell. Stage A supplies
specifications and a static review preview. Stage B must select one current gameplay consumer
and migrate a small component slice in a separate change. Stage C requires the Phase-4 exit
from doc 07 and the opened Phase-5 gate. No phase completion is inferred from these designs.

Screen 13 owns user preferences at `/settings`. Screen 22 is developer/design documentation
and has no product route. In-match settings is a small canvas overlay sharing preferences,
not a second full settings controller. Issues #50–#55 consume this foundation and own their
screen-specific behavior; no catalog, auth, lobby, replay, or game rules are specified here.

## Visual hierarchy

Use color, shape, size, motion, and containment to identify actions and group related work.
This adapts the [Google Design research](https://design.google/library/expressive-material-design-google-research)
and the pinned pack's component mapping; no Tabula usability improvement has been measured.

- Place the task title and useful content immediately below a compact toolbar. Desktop titles
  use `HeadlineLg`; compact titles use `HeadlineSm`. UI hierarchy is sans; reserve display
  typography for game titles/results, and `Mono*` with tabular figures for clocks/numbers.
- Use `container` for related sections, `container-high` for contained rows/fields, and
  `surface` for the page. A filled primary action uses `primary`/`on-primary` and a larger
  target; routine related actions use tonal or text treatment. Limit the strongest emphasis
  to the principal action in each task region. Always retain action labels.
- Remove ornamental outlines/shadows. Keep outlines when they identify focus, selection,
  validation, an off-control affordance, or a meaningful board grid. High-contrast schemes
  deliberately flatten some surfaces; use headings, spacing, and functional control outlines
  rather than relying on tonal distinction alone.
- Keep game palettes dominant on the board. A platform action color does not redefine team,
  turn, legal-target, threat, or last-action meaning. Pair status colors with text/glyphs.

## Token inventory and mapping

This stage requires no new roles or values. The preview is a concrete consumer of existing
roles; future runtime components use the same mapping. `[comp]` remains empty. No mock palette,
native Material library, generator/schema change, or second theme source is needed.

Authored paths below are relative to `sys` except scheme colors. CSS prefixes them with
`--sys-`; Rust consumes the corresponding `Theme` field. This table distinguishes actual
runtime names from doc 04's larger, abridged future inventory.

| Authored role | Rust / CSS mapping | Consumer and purpose |
|---|---|---|
| `schemes.*.surface` | `color.surface` / `--sys-color-surface` | Page and modal surround |
| `schemes.*.container` | `color.surface_container` / `--sys-color-container` | Tonal section, sheet, quiet toolbar |
| `schemes.*.container-high` | `color.surface_container_high` / `--sys-color-container-high` | Tonal button, field, contained row |
| `on-surface`, `on-surface-variant` | `color.on_surface`, `color.on_surface_variant` | Main text; supporting text on all three surfaces |
| `primary`, `on-primary` | `color.primary`, `color.on_primary` | Principal action; selected connected option and its label |
| `outline` | `color.outline` | Off-control boundary and structural control affordance; never small body text |
| `danger`, `on-danger` | `color.danger`, `color.on_danger` | Inline error text/stroke; filled destructive confirmation |
| `success`, `on-success` | `color.success`, `color.on_success` | Confirmed success text/filled status; never speculative success |
| `turn-active`, `turn-waiting`, `legal-target`, `illegal-target`, `selected`, `last-action`, `threat`, `hidden` | Same snake-case `color` fields | Existing board meanings; status/glyph accompaniment required |
| `team`, `seat-marker` | `color.team`, `color.seat_marker` | Identity plus label/glyph/position; no color-only identity |
| `space.xxs/sm/md/lg/xxl/xxxl` | `space` / `--sys-space-*` | 2 dp group gap/focus offset; 8/12/16/24/32 dp spacing |
| `shape.full`, `shape.button`, `shape.sm`, `shape.md` | `shape` / `--sys-shape-*` | Round button; square/pressed button; connected inner corners; middle list corners |
| `shape.card/board/sheet/chip` | `shape` / `--sys-shape-*` | 16/12/24/8 dp semantic containers; do not enlarge every shape |
| `type.headline/title/body/label/mono` | `Theme::text_style(TextStyleToken::*)` | Sans task hierarchy, readable content, labeled actions, stable counters |
| `state.hover/focus/press/drag` | `state` / `--sys-state-*` | On-color overlays at 8/12/12/16%; focus has a separate ring |
| `state.disabled-content/disabled-container` | `state` / `--sys-state-disabled-*` | Disabled visuals at 38/12%; reason remains readable at full contrast |
| `focus.ring-width/ring-color` | `focus` / `--sys-focus-ring-*` | 3 dp primary ring with 2 dp surface gap |
| `density.min-target/scale` | `density` / `--sys-density-*` | Minimum 44 × 44 logical dp; scale is accessibility scaling, not compact mode |
| `motion.*`, springs, profiles, reduced policy | `motion` / generated motion variables | Existing durations/springs; immediate reduced-motion feedback |
| `elevation.low/medium/high` | `elevation` / generated elevation variables | Available abstract levels; no decorative shadow needed by this slice |

The three surfaces support `on-surface`, `on-surface-variant`, `primary`, `danger`, and
`success` text at ≥4.5:1, and `outline`/focus at ≥3:1 in all four schemes. Filled actions
retain their matching on-color. State-layer compositing must preserve text contrast at the
authored hover/focus/press alpha, including selected options. Disabled visuals are exempt from
the active-control threshold, but explanatory text is not. Do not place `on-primary` directly
on a tonal surface. A focus ring on a primary-filled control needs the surface gap; do not
paint it directly over primary. See the design-crate contrast checks below.

## Component anatomy and states

Round actions use `shape.full`; square actions use `shape.button`. Pressed shape may use
`shape.button` while the hit region stays fixed. Primary targets are 56–64 dp high when space
permits; routine actions are 48 dp, with a 44 dp lower bound at every density.

Connected selection uses labeled options, a 2 dp gap, `shape.full` on outer group corners and
`shape.sm` (8 dp) on inner corners. Selected fill is `primary`/`on-primary`; unselected fill
is `container-high`/`on-surface`. In a vertical compact stack, first/last outer corners follow
the stack axis. Shape never substitutes for checked/selected semantics or alters hit geometry.

Contained lists use `shape.card` on exposed first/last corners, `shape.md` on inner corners,
and `shape.card` on all corners for a single row. Supporting text and trailing controls stay
aligned. A toggle is a ≥44 dp control wrapper around a smaller visual mark. Avoid nested
row-button/control activation. Floating toolbars contain only related contextual actions;
rotate-left/right, deselect, and exit have distinct names. Split buttons require a main action
with a related menu; use a segmented selector for mutually exclusive modes.

The table is a shared contract, not a claim that every widget already exists.

| Component / consumer | Default, hover, pressed | Disabled, focus, busy/error | Keyboard / AT and ownership |
|---|---|---|---|
| Button / current game actions | Filled primary or tonal secondary; on-color state layer; fixed hit rect | No activation when disabled/busy; readable reason; 3 dp focus ring; busy label retained; failure shown nearby | DOM `button`; Enter/Space once per action; canvas `InputEvent` → local interaction → `Intent`; no direct state mutation |
| Icon button / contextual toolbar | 48 dp tonal/quiet wrapper, named glyph | Same state layers/target/ring; tooltip supplements name | Accessible name required; each action a focus stop; canvas mirror remains platform-gated |
| Connected selector / appearance or mode | Tonal unselected; filled checked option with textual label | Selected and focus independent; disabled option explained; selection preserved on failure | Mutually exclusive DOM radio group: one Tab stop, arrows change choice, Space selects; canvas focus graph + selected local value |
| Tabs / related panels | Text labels, selected marker and shape | Focus distinct from selection; inactive panels hidden from AT; loading panel identifies itself | DOM tablist/tab/tabpanel; arrows traverse, Home/End supported, Enter/Space activates for manual selection; canvas focus graph |
| Compact list / settings rows | Contained first/middle/last rows; secondary text | Selected label/check when relevant; control reason outside faded content | DOM list and labeled controls; static rows are not focus stops; canvas only for justified native consumer |
| Field / Phase-5 forms | Label persists; `container-high`; off-control boundary visible | Focus ring; validating status; danger stroke plus error text; preserve entry on failure | DOM native input/IME; `aria-describedby`, `aria-invalid`; validate on appropriate commit; no canvas text editor |
| Badge / capability or status | Text + optional glyph with semantic meaning | Distinguish current/planned/unavailable/stale in words; not an action | Passive text, no Tab stop; module availability never implies a working route/backend |
| Dialog/sheet / bounded confirmation | Tonal surface and explicit title/action; no ornamental shadow | Busy prevents duplicate submit; error keeps dialog/data; focus restore on close | Store invoker, move focus inside, trap Tab, Escape cancels if allowed; restore to invoker or logical successor |
| Banner/toast / feedback | Labeled inline/persistent status; contextual retry | Error has text + danger; offline/stale persists; toast never sole error evidence | Polite status for routine work; urgent failure alert once; do not steal focus unless correction requires it |
| Progress / loading | Real fraction for measurable work; short indeterminate indicator otherwise | Busy text remains visible; cancellable long task; reduced motion keeps text/fraction | `progress` or named progressbar; throttle announcements; never fabricate percent or completion |
| Empty/error / no data or failed fetch | Plain explanation and one next action | Preserve failed input/context; retry only with real adapter | Semantic heading/status; actionable retry receives focus only through normal navigation |

Pointer cancellation, leaving a target before release, repeated keydown, and lost focus must
not dispatch unintended/duplicate actions. Reconcile focus when controls disappear or become
disabled. Canvas hit bounds and focus bounds share logical geometry; a visual overlay is never
evidence of functioning keyboard behavior. Modal pointer capture must prevent board activation
through the overlay; closing it must not submit a game command.

## Responsive layout and density

| Width (logical dp) | Layout and hierarchy |
|---|---|
| Compact, <600 (verify 320 and 390) | Single task column; 58 dp top toolbar; grouped selectors wrap/stack with labels; ≥56 dp bottom navigation in the future shell; full-width sheet; 16 dp page gutter |
| Medium, 600–904 | Navigation rail; one settings column; preview follows controls; 64 dp toolbar; 24 dp gutter |
| Expanded, 905–1439 | Rail; settings and live preview in two columns when contents fit; 64 dp toolbar; 24 dp gutter |
| Large, ≥1440 | Centered content capped at 1200 dp beside a 176 dp rail; no editorial hero; control/preview hierarchy retained |

Normal density: 24 dp section padding, 16 dp internal gap, ≥64 dp rows where supporting text
fits. Compact density: 16 dp section padding, 8 dp internal gap, ≥56 dp rows; rows grow for
wrapped content. Both retain the same type sizes, labels, ≥44 dp targets, and focus spacing.
Compact mode never lowers `Density.scale` or `min_target`. Text/accessibility scaling to 200%
causes reflow, not clipped labels or a transform shrinking the whole page. A breakpoint follows
available logical width; it is not chosen from device identity. Safe-area padding is additional
to the gutter. The preview illustrates content reflow; shell navigation is deferred.

## Motion, theme, and accessibility

Resolve exactly `light`, `dark`, `hc-light`, or `hc-dark` before presenting a frame. Theme
changes replace the resolved theme atomically and preserve focus, selection, and pending data.
The scheme resolver and preference model are specified in [screen 13](13-settings.md).

Use existing snappy/standard springs for state/overlay transitions. Reduced motion combines
OS request and the user's reduction preference; remove ambient/shape movement and retain
immediate state, focus, labels, last-action information, and informative status. A busy state
may use stationary text. It must not wait for an animation to enable the next action.

Use semantic DOM buttons/radios/lists/inputs, visible labels, a sensible heading outline, and
localized en/vi message keys at runtime (doc 07 Phase 5). Keep 200% zoom enabled. Canvas uses
the existing focus/input contracts and the supported accessibility mirror; do not claim a
screen-reader implementation from this static design. Keyboard, switch access, colorblind,
no-audio, high-contrast, and touch paths require platform verification when implemented.

## Evidence ledger and next slices

| Claim / owner | Oracle and scope | Check / evidence |
|---|---|---|
| One token authority / ADR-027 | Current TOML and generated Rust/CSS/JSON; preview imports generated CSS | `cargo xtask gen-tokens`; generated-file diff; no schema/value change |
| Tonal text/control contrast / design | WCAG sRGB contrast equation; default and state-layer pairs in all four schemes | `cargo test -p tabula-design`; foundation contrast tests |
| Spec follows ownership/gates / I-5, I-10, ADR-011 | Architecture, current banners, pinned component map | Source-read; runtime shell and widgets deferred to B/C |
| Responsive/state source / design | Editable preview using the same tokens; widths, themes, density, reduced motion | Preview review evidence recorded in the change report; static data only |
| DOM/canvas input and accessibility parity | Actual widget input/focus tests and real platform interaction | NOT_IMPLEMENTED by Stage A; Stage B tests a selected game slice, Stage C tests the shell |

Stage B must test hit area, disabled/focus/activation, cancellation, reduced motion, four
schemes, and the selected presenter's regression/conformance checks, then `just check`.
Review any changed render snapshot deliberately; do not auto-regenerate goldens. Stage C
must prove the gate, wire real preferences/persistence, and exercise 320–1440 dp, 200% zoom,
deep links/back, drawer/bottom navigation, focus restoration, and theme parity. Static preview
interaction does not establish these runtime claims.

## Implemented Stage B slice

The first runtime consumer is Chess's promotion chooser. Shared
[`ActionButton` and `ButtonInteraction`](../../../crates/tabula-presentation/src/button.rs)
own semantic filled/tonal appearance, standalone/connected shape, fixed ≥44 dp targets,
exterior keyboard focus, and pointer/key cancellation. Chess owns the four upgrade intents,
projected legality, modal capture, labels, and focus restoration. Cancel remains available
when the projection disables every upgrade. Interaction stays in `ChessLocal` (I-10);
the helper neither knows a game id nor reads canonical state. Existing render commands and
opacity scopes suffice; no renderer, token-schema, or dependency extension is required.

| Claim / owner | Oracle and scope | Executed evidence |
|---|---|---|
| Stable targets / shared widget and Chess layout | 44 dp floor, unchanged hit bounds during press; portrait 320×640/390×844 and short 640×320/320×300/640×280 viewports | Widget target/connected-corner assertions and `promotion_controls_retain_44_dp_targets_on_compact_viewports` |
| Safe activation / shared local input | Same-target primary down/up, leaving/cancel, missing/disabled controls, focus loss, once per Enter/Space press, disabled-node traversal | Widget interaction tests; three pre-change Chess regressions reproduced the undersized-target, release-only, and focus-loss defects |
| Modal intent and cancellation / Chess | Projected legal upgrades only; no board input through modal; Cancel/Escape restore source focus without a command; opening-key repeat suppressed | Chess promotion presenter tests, including unavailable choices and pointer/keyboard Cancel |
| Theme and motion / I-10 | Semantic fills/labels in all four schemes; immediate feedback under reduced motion; canonical bytes unaffected | Widget/theme and Chess presenter assertions; headless command evidence, not rendered pixels |
| Deliberate render change / presenter | Promotion snapshot commands 0–67 (board, pieces, HUD) remain identical; only modal geometry, labels, state/focus scopes, and Cancel change | Reviewed one promotion snapshot; other six presenter goldens retained |

Local checks passed: `just check`, `just features`,
`cargo test -p tabula-game-chess --features bots,presentation` (144 passed; one explicitly
ignored depth-five perft test), and
`cargo check -p tabula-game-client --target wasm32-unknown-unknown` (compilation only).
Screen-reader/native/browser visual interaction remains platform verification: the available
automation session exposed no browser surface. Render-list tests and WASM compilation do not establish font wrapping,
rendered focus contrast, or assistive-technology behavior. The supported compact width is
≥320 dp; below the modal's 206×152 dp geometric minimum it emits no controls and Escape can
still cancel. Stage C remains deferred at the phase gate described above.
