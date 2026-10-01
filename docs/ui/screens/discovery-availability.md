# Discovery availability at the implementation base

Stage A evidence for [issue #50](https://github.com/loveoverflowcom/tabula/issues/50),
pinned to `develop @ cdff554de327cf8d23893c7b606fc09670371b74`.
That commit was verified against live remote `develop` on 2026-10-01.
This inventory records source reads; it does not claim tests, playtests, builds, or deployments.
Use it with [the discovery contract](discovery.md) and [screen 03](03-new-match.md).
Architecture [doc 00](../../architecture/00-architecture-principles.md) remains authoritative.

## Availability is scoped to a launch path

A manifest entry, compiled rules, a presenter, a bot factory, a working launcher, and an
online service establish different facts. An enabled manifest default does not publish a game
or make a shell route work. The current game client is a local developer launcher; screens
01–03 are specifications for later adapters, not an implemented catalog or setup controller.

| Game | Source at the pinned base | Current local launcher | Reason when an action is unavailable |
|---|---|---|---|
| Chess | `ChessRules`, `ChessModule`, presenter, Trivial/Easy bots; 2 seats | Two-human hot-seat, fixed 5 min + 2 s Fischer; no bot launcher | Online services and discovery/setup routes are not implemented; current launcher has no Chess solo mode |
| Tiles | `TilesRules`, `TilesModule`, presenter, Trivial/Easy bots; 2–5 seats | Hot-seat or solo; solo keeps seat 0 human-controlled and dispatches Easy bot commands for other seats; no turn deadline | Online/async operations and discovery/setup routes are not implemented |
| Caro | Documentation scaffold; no rules/module/presenter/bot or manifest | None | Game implementation pending |
| Werewolf | Validated configuration, state/event models, initial role assignment; compiled metadata/capabilities independent of the incomplete module | None | Rules benchmark in progress; playable presentation arrives in Phase 7 |
| Xiangqi | No workspace game crate, manifest, module, presenter, or launcher | None | Not implemented |

Sources: [Chess module and factory](../../../games/chess/src/lib.rs) (`ChessModule`),
[Tiles module and factory](../../../games/tiles/src/lib.rs) (`TilesModule`),
[Caro scaffold](../../../games/caro/src/lib.rs),
[Werewolf package](../../../games/werewolf/src/lib.rs) (`metadata`, `capabilities`),
[Werewolf creation kernel](../../../games/werewolf/src/rules/mod.rs) (`create_initial_state`),
[workspace members](../../../Cargo.toml), and
[local launcher](../../../apps/game-client/src/main.rs) (`SelectedGame`, `run_chess`, `run_tiles`).
Xiangqi artwork in the reference pack is design provenance, not game availability evidence.

## Module options and launcher options

The future setup adapter may offer the confirmed module options below only after it implements
their construction, validation, and launch paths. The existing CLI does not expose every option.
Do not advertise a choice solely because a capability declaration mentions it.

| Concern | Confirmed game contract | Existing local launcher | Proposed setup dependency |
|---|---|---|---|
| Chess time | `Config.clock = None` is untimed; `Some(ClockConfig)` uses positive initial milliseconds and Fischer increment or Bronstein delay | Fixed Fischer 300,000 ms + 2,000 ms | Adapter converts labeled time choices into `Config` and calls game validation; no named preset registry exists |
| Chess seats/bots | Exactly seats 0 and 1; factory supplies Trivial/Easy with `bots` enabled | Always two humans; client dependency does not enable Chess `bots` | Linked factory, seat plan, projection-only bot runner; no current Chess solo button |
| Tiles time | `turn_deadline_ms = 0` disables; nonzero values must be at least 5,000 ms | Always zero | Adapter validates the selected integer; rules creation remains authoritative for deadline overflow |
| Tiles seats/bots | 2–5 distinct seats; factory supplies Trivial/Easy | Default 3 seats; `--seats` clamps to 2–5; `--solo` drives seats 1 onward with Easy bots while the current roster is constructed as human occupants | Explicit validated seat selection, bot choice, and visible normalization; no silent form clamping |
| Ranked/online/async | Chess declares Elo; Tiles declares placement ranking and async rules; both declare durable acknowledgement | No network, rating, matchmaking, or durable server path | Platform service evidence and permitted phase before any executable online/ranked/async choice |
| Werewolf setup | Pure validated config/preset and 6–20-seat creation slice; no completed module/presentation | None | Complete rules/security benchmark, module and Phase-7 playable presentation before launch |

Sources: [Chess config](../../../games/chess/src/rules/state.rs) (`Config`, `ClockControl`),
[clock validator](../../../games/chess/src/rules/clock.rs) (`config_is_valid`),
[Tiles config](../../../games/tiles/src/rules/state.rs) (`Config`, `MIN_TURN_DEADLINE_MS`,
`turn_deadline`), [client dependency features](../../../apps/game-client/Cargo.toml), and
[CLI wiring](../../../apps/game-client/src/main.rs) (`parse_options`, `run_chess`, `run_tiles`).
Chess validates representability of initial time plus increment/delay; frontend range checks
must preserve that law. Game-owned config is separate from shell-owned mode and seat planning.
Local play requires no account service or unnecessary sign-in step.

## Data ownership and absent adapters

| Fact | Existing owner | Limit at this base |
|---|---|---|
| Identity and description | [GameMetadata](../../../crates/tabula-game-api/src/metadata.rs): ID, package/rules versions, localization keys, category/tags, estimated duration, complexity/content rating, logical icon/hero refs, optional rules URL key | No rating, engine-ready, offline, accessibility, config schema, or launch-readiness field |
| Rules/platform declarations | [GameCapabilities](../../../crates/tabula-game-api/src/capabilities.rs): seats, turn model, hidden information, spectators, chat/voice, ranked, async, reconnect, substitution, pause, durability, preview, state size/budget/duration | Declarations do not prove linked services, bot factories, valid presets, or runtime readiness |
| Configuration validation | [GameModule](../../../crates/tabula-game-api/src/module.rs)::`validate_config(&Config, &SeatRoster) -> Result<(), ConfigError>` | Game-owned validator returns no normalized config; normalization needs a proposed form/adapter boundary |
| Catalog and version dispatch | [Registry scaffold](../../../crates/tabula-registry/src/lib.rs): proposed erasure, `catalog`, registration, rollout/version resolution | Rustdoc sketches only; no callable runtime catalog or rollout adapter |
| Manifest defaults | `game.toml` plus [check-manifests](../../../xtask/README.md) | Schema validation exists; general cross-check with compiled metadata/capabilities and generated manifest boundary do not |
| Generated forms | [Doc 02 §10.3](../../architecture/02-game-module-and-sdk-design.md#103-what-the-developer-did-not-write) | `ConfigForm` is an experiment, not an implemented schema or generator; handwritten game forms are the documented fallback |

Discovery availability, supported creation modes, setup presets, linked bot levels, service
readiness, and cache/accessibility evidence therefore require proposed typed adapters with named
owners. They are not additions to `GameCapabilities` in this change. A future shell consumes
registry interfaces rather than branching on `game_id` (I-9); game-specific config conversion
belongs behind its adapter. Server authorization and game validation still decide creation.

## Package, rules, and resource identity

| Game | Package version | Rules version | Pack identity and evidence |
|---|---|---|---|
| Chess | 0.1.1 | 3 | Presenter declares `chess@0.1.0`; catalog icon/hero are logical `AssetRef`s |
| Tiles | 0.1.0 | 1 | Presenter declares `tiles@0.1.0`; catalog icon/hero are logical `AssetRef`s |
| Werewolf | 0.1.0 | 1 | Manifest declares `werewolf@0.1.0`; there is no presenter or delivered pack evidence |
| Caro / Xiangqi | No playable package metadata | None | No pack availability evidence |

Sources: [Chess manifest](../../../games/chess/game.toml) and
[presenter](../../../games/chess/src/presentation/mod.rs) (`asset_pack`),
[Tiles manifest](../../../games/tiles/game.toml) and
[presenter](../../../games/tiles/src/presentation.rs) (`asset_pack`),
[Werewolf manifest](../../../games/werewolf/game.toml).
Package version, rules version, and asset-pack version are separate identities; equal-looking
values are not a rule. Neither an `AssetRef` nor a manifest `size_kb` proves fetched or decoded
art. A missing resource uses a labeled placeholder and preserves the textual game identity.

[Doc 04 §12](../../architecture/04-frontend-and-design-system.md#12-asset-system) records the
implemented pure identity/binding/resolution/integrity layer and pack builder. Concrete asset
sources, cache management, CDN/retry policy, decoding, and renderer handles remain future work.
Previously played packs staying cached for local/bot use is an offline target, not evidence for
an “Offline ready” badge. Local rules execution alone does not establish browser offline startup.

## Accessibility and phase gates

[Chess presentation](../../../games/chess/src/presentation/mod.rs) (`chess_a11y`) emits status,
board regions, actions, and promotion descriptions. [Tiles presentation](../../../games/tiles/src/presentation.rs)
(`describe`) emits status/actions with empty regions; its rules mirror is chiefly status.
The DOM/native mirror and `ActionId` activation bridge remain later work under
[doc 04 §10.4](../../architecture/04-frontend-and-design-system.md#104-accessibility).
These structures do not establish screen-reader completion or justify an “Accessible” badge.
Ratings, strong-engine readiness, runtime budgets, and persisted/offline capability likewise
need scoped evidence; metadata or planned acceptance criteria cannot supply it.

[Doc 07 Phase 3](../../architecture/07-phases-and-implementation-roadmap.md#phase-3--local-games-and-rules-benchmarks)
requires Caro conformance, complete Werewolf rules/secrecy checks, all benchmark suites, and
contract stability. Caro's empty implementation and Werewolf's creation-only slice leave that
gate unresolved. [Phase 4](../../architecture/07-phases-and-implementation-roadmap.md#phase-4--authoritative-multiplayer)
depends on the stable contract; [protocol](../../../crates/tabula-protocol/src/lib.rs),
[match](../../../crates/tabula-match/src/lib.rs), registry, and server remain scaffolds.
[Phase 5](../../architecture/07-phases-and-implementation-roadmap.md#phase-5--web-application-shell)
depends on Phase-4 exit; [the web shell](../../../apps/web/src/main.rs) still exits with its gate
message, and [lobby](../../../crates/tabula-lobby/src/lib.rs) is a future seam.

Stage A specifies screens and dependencies. The separate implementation stage requires the
owning phase gates and real adapters; this evidence document grants no phase completion.
