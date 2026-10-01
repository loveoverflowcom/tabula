# Tabula Design02 / Xiangqi Play / Analysis / Learn / Resources

Scope: design only. Audited develop @ 44f6b74e07648abc7191363d7582efc1fceab262. Future implementation must re-pin HEAD and phase gates.

## How to use this small pack

Open index.html in a browser to select each editable SVG screen; the viewer is a static design navigator, not a runnable game. No network calls, auth, rules, payments, installs, engine or AI execute. Open individual screens/*.svg for direct editing. Run node render.mjs with Sharp installed from the official npm registry to regenerate the3preview PNGs; no screenshot/browser capture is claimed.

## Shared layout / component contract

- Desktop1440×1000: rail176px, topbar64px, task title32px, body14/16px, compact metadata12px. Sans hierarchy replaces the original44px serif/306px hero. Board view gets the first large region; ancillary content follows.
- Mobile390px: one-column task-first view, topbar58px, navigation56px, canvas before detail sheet. Never scale down actions below44logicaldp. 200%browser zoom must remain enabled in implementation.
- Existing tokens.toml is the single source. Use generated Rust/CSS/JSON. Exact four schemes are light,dark,hc-light,hc-dark; focus3dp primary; min target44dp; card16dp,board12dp,button14dp,chip8dp. Board art colors inherited from the original prototype are design/art extensions, not new product semantic tokens.
- Foundation owner: buttons/icon-buttons, tabs, fields, compact rows/cards, capabilities/badges, alerts/progress/error/empty, dialogs/sheets/toasts. Reference pack01instead of copying theme logic. Showcase22andsettings13are distinct surfaces.
- Focus/order/AT/live announcement,hover/pressed and actual keyboard actions require implementation/platform QA. Static rendered SVG is not accessibility proof. Visual circles/toggles/checkbox glyphs specify44dp wrapper hit targets, not their small drawn marks.
- Label unavailable features visibly withreason. Labels CURRENTdescribe audited module availability, not a working portal route. Auth/lobby/replaystorage/settingsshell arephase-gated.
- All names,clocks,moves,counters,roomcodes,presence andscores are sampledata. Design instructions never provide engine/network authority. i18n plan: stable en/vi message keys, no string concatenation forplural/status, locale-awaretime/date andnotation fromgame adapter.

## State matrix required by implementation

Default / hover / pressed / disabled-with-reason / keyboardfocus / loading / busy / empty / validationerror / serviceerror / stale / offline. Primaryactions cannot imply successful work before authority/adaptersacknowledge. Modal focus returns toinvoker; error announcer and boardreader follow platform contracts. Reduced-motionuses canonicalmotion preference and OSsetting; no decorative spinner dependency.

## Verification boundary

This deliverable has SVG/XML parse checks,source-syntax checks,image dimensions/sizechecks and visualinspection. No Rust build/runtime/screenshots,real engine/AI/network,screenreader audit,all-browser layout orphaseexit certification performed. PRs must pin current HEAD,read AGENTS.md andarchitecture00/04/07/09,follow repo verification skills,invariant→oracle→evidence,targeted tests then justcheck,explicitly reportwhat wasnotrun. Do not auto-regenerate goldens.

## Per-screen ownership and details

### 08-xiangqi

Board-first Play view with planned module/provider gate. Local only after rules/presenter; bot only after engine probe.

### 11-xiangqi-analysis

Xiangqi-specific extension over pack04controller: engine missing state, evidence/PV, budget and cancellation. Evaluation data must includeprovider/version, depth, nodes, elapsed and position identity.

### 12-learn

Tutor conversation grounded in position and engine evidence. No LLM in this mock; no assertions of best move when evidence missing. Explicit evidence links to board/ply/branch.

### 14-resources

Install lifecycle and metadata reference: official provider/source,license,size,checksum,compatibility; resource state cannot sayready until verify+probe.

## Specific flows / failure states

- Missing engine / no LLM → visible explanation; analysis controls gated; learning facts only from valid resources.
- Position/branch changes → stale evidence badge; cancel obsolete work; never paint prior evaluation as current.
- Worker progress has cancellable budget CPU/thread/time; error/retry idempotent; no duplicate provider jobs.
- Engine/model download is not performed here; platform compatibility must be proven before UI offers installation.

## Existing code to compare

- [docs/architecture/02-game-module-and-sdk-design.md](https://github.com/loveoverflowcom/tabula/blob/44f6b74e07648abc7191363d7582efc1fceab262/docs/architecture/02-game-module-and-sdk-design.md)
- [docs/architecture/04-frontend-and-design-system.md](https://github.com/loveoverflowcom/tabula/blob/44f6b74e07648abc7191363d7582efc1fceab262/docs/architecture/04-frontend-and-design-system.md)
- [docs/architecture/07-phases-and-implementation-roadmap.md](https://github.com/loveoverflowcom/tabula/blob/44f6b74e07648abc7191363d7582efc1fceab262/docs/architecture/07-phases-and-implementation-roadmap.md)

## Small sequential PR prompts

See implementation-prompts.md for the reviewed source-grounded issue plan. Each prompt is a separate PR; this design branch itself changes no production code.

## Hit area and mobile link details

Every drawn toggle, checkbox and icon has a separate minimum 44dp interaction wrapper; the visible mark is not the hit region. Mobile history cards use a full-card focusable link with at least a 44dp activation region around “Xem lại”. Planned lobby status pills are not actions. Screen navigation, focus restoration and assistive-technology behavior require implementation tests.

## Approved Material 3 Expressive visual revision

Group by tonal containment, contrast, hierarchy and purposeful shape. No ornamental card outlines or shadows. Retain functional board lines, focus rings, off-control affordances and validation strokes. The prominent action is larger or higher contrast; routine actions are quieter. Keep label-first scientific scanning and no marketing hero.

Reference components: filled/tonal round or square buttons, standard/connected button groups, contained lists, filled fields and context floating toolbars. Connected groups use 2dp gaps, 8dp inner corners and fully rounded outer corners; mobile selection options remain visible. Shape and spring interaction states are specified, not rendered as working animations in these static SVGs.

The exact Material component anatomy is adapted to existing Leptos DOM and Macroquad RenderList contracts. This is not a claim that Tabula already ships a native Material 3 Expressive kit. Canonical Corners has four radii and MotionTokens already includes spring families/reduced-motion; use those capabilities first. Component-specific radius/color mappings that differ from existing tokens are bounded proposals for tokens.toml after a real consumer is selected, not a parallel theme source. The purple tonal reference fill is an explicit color-extension proposal; generated four-scheme semantics remain authoritative.
