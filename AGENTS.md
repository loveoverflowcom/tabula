# AGENTS.md — how to work in this repository

You are working on **Tabula**: a deterministic, server-authoritative runtime for board games,
plus the SDK, protocol, and clients that make adding a game cheap. Tabula is *not* a game.

**Read [`docs/architecture/00-architecture-principles.md`](docs/architecture/00-architecture-principles.md) before your first edit.**
It is the contract. When any other document, comment, or this file disagrees with doc 00, doc 00
wins and the other is a bug.

---

## 1. Where to read before you write

| Task | Read |
|---|---|
| Anything at all | doc 00 |
| Adding a crate or a dependency | doc 00 §8, doc 01, `deps.toml` |
| Writing or changing a game | doc 00, doc 02, doc 08 |
| Server, match runtime, persistence | doc 00, doc 03, doc 05, doc 06 |
| Client, renderer, design tokens | doc 00, doc 04, doc 05 (client section) |
| Wire protocol, replay, versioning | doc 05 |
| Planning a phase | doc 07, doc 09 §7 |
| "What was decided and why" | doc 00 §10 (ADRs), doc 09 §3 (register) |

Docs live in [`docs/architecture/`](docs/architecture/README.md) and are numbered 00–09.

---

## 2. The five rules that matter most

1. **Game rules are a pure, synchronous, total function.**
   `apply(&mut State, Input<Command>, &mut Ctx) -> Result<Outcome, RuleError>`.
   No I/O, no clock, no OS randomness, no panics on hostile input, and *transactional on error*
   (a rejected input must leave `State` byte-identical).

2. **Clients never receive canonical `State`.** Only `View` (from `project`) and `ViewEvent`
   (from `view_event`). A leak here is a security defect, not a gameplay bug. (I-5, I-6)

3. **No platform crate branches on a `game_id`.** Everything game-specific goes through
   `tabula-registry`'s erased interfaces. If you find yourself writing `if game == "chess"`,
   the answer is a new declarative field on `GameCapabilities` with a named consumer. (I-9)

4. **Determinism is the product.** Same seed + same ordered inputs + same rules version
   ⇒ byte-identical state, on every OS, architecture, and both native and WASM.
   `HashMap`/`HashSet` iteration, floats in canonical state, and wall-clock reads all break it.

5. **Dependency arrows point down, always.** `deps.toml` is the machine-readable law;
   `cargo xtask check-deps` enforces it. Layer 1 (core/game-api/games) knows nothing about
   Layer 3 (server/apps) or Layer 4 (Postgres/Macroquad/Leptos).

---

## 3. Repository map

```text
crates/           platform libraries — the real product
  tabula-core          deterministic kernel: ids, DetRng, LogicalTime, Viewer, hashing
  tabula-game-api      the game contract: GameRules, GameModule, Input, Effect, Ctx
  tabula-protocol      the wire: envelopes, versions, codecs, error codes
  tabula-registry      the catalog: the ONLY crate that names games; type erasure
  tabula-match         authoritative match runtime: actor, pipeline, ports
  tabula-lobby         rooms, matchmaking, presence
  tabula-storage       the ONLY crate that knows SQL exists
  tabula-presentation  View -> RenderList, input model, animation (renderer-independent)
  tabula-design        semantic design tokens, generated into CSS + a Theme struct
  tabula-assets        versioned, hashed asset packs
  tabula-net-client    client session: connect, resume, sequence, idempotency
  tabula-voice         VoiceService trait + provider adapters
  tabula-testkit       the conformance suite every game must pass
  renderer-macroquad   the first Renderer backend — deliberately replaceable

games/            one crate per game; feature-split into rules / bots / presentation
apps/             game-client (Macroquad), web (Leptos), admin, desktop (optional Tauri)
services/         tabula-server — THE binary at Stage 0
mobile/           gradle + Xcode wrappers around the game-client library
xtask/            repo automation (pure Rust, no make)
deploy/           compose (dev), systemd (Stage 0–1), terraform (Stage 2+)
tests/            integration (real Postgres), load (Rust generator), replays (golden .tbr)
docs/             architecture (00–09), adr, games (per-game info models)
```

Crate-level responsibilities, allowed deps, and forbidden deps: doc 01 §3, mirrored in `deps.toml`
and repeated in each crate's `src/lib.rs` header.

---

## 4. Phase discipline

The roadmap is doc 07. Crates exist in this repository as directory + doc-comment skeletons
**ahead of their phase, deliberately**, so the shape of the system is visible. That is not
permission to implement them early.

| Phase | Crates that become real |
|---|---|
| 0 | `tabula-core`, `tabula-game-api`, `tabula-testkit`, `games/tictactoe`, `xtask` |
| 1 | `games/chess` |
| 2 | `tabula-design`, `tabula-presentation`, `renderer-macroquad`, `renderer-headless`, `apps/game-client` |
| 3 | `tabula-assets`, `games/cards`, `games/tiles`, `games/werewolf` (rules only) |
| 4 | `tabula-protocol`, `tabula-registry`, `tabula-match`, `tabula-storage`, `tabula-net-client`, `services/tabula-server` |
| 5 | `tabula-lobby`, `apps/web`, `apps/admin` |
| 6 | `mobile/android`, `mobile/ios` |
| 7 | werewolf + social |
| 8 | `tabula-voice` |
| 9+ | SDK stabilisation, scaling, third-party ecosystem |

Every not-yet-real crate's `lib.rs` starts with a `PHASE N` banner naming the gate. **Do not
implement past your phase.** Doc 09 §7 explains why: networking built against a contract that
games have not yet validated is a contract that can no longer move.

If you believe a phase gate is wrong, write an ADR — do not quietly cross it.

---

## 5. Before you open a pull request

```bash
just check          # cargo xtask check: fmt, clippy, test, check-deps, check-no-game-ids,
                    # check-manifests, cargo deny check — in that order, stops at the first failure
```

Or individually:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask check-deps            # the deps.toml matrix (I-1, I-15)
cargo xtask check-no-game-ids     # I-9
cargo xtask check-manifests       # game.toml/Cargo.toml schema and feature-shape validation
cargo nextest run --workspace
cargo deny check
```

A change to a game crate additionally needs its conformance suite green
(`tabula_testkit::conformance!(YourFixture)` against a `GameTestFixture` impl — doc 02 §11.1)
and, if the game has hidden information, a `SecretModel` with the projection scan passing.

---

## 6. Things that will get a PR rejected

| You wrote | Why it fails | Do instead |
|---|---|---|
| `std::time::Instant` in rules | I-3, breaks replay | `ctx.now` |
| `rand::thread_rng()` in rules | I-4, unreplayable | `ctx.rng.stream(DOMAIN)` |
| `HashMap` in `State` or in any output-affecting iteration | I-2 | `BTreeMap` / `Vec` |
| `f32`/`f64` in canonical state | Cross-arch divergence | scaled `i32`/`i64` |
| `View { secret: Option<T> }` set to `None` | One refactor fills it in | model the *knowledge*: `Summary { count }` |
| `if game_id == "chess"` in a platform crate | I-9 | a `GameCapabilities` field with a named consumer |
| Mutating then validating in `apply` | Violates R2 | validate fully, then mutate |
| Animation/camera/selection state in `State` | I-10 | `GamePresentation::Local` |
| A wire type changed without a version bump | I-13 | `xtask gen-protocol-vectors --bump` |
| Raw hex colours outside `tabula-design` | Breaks theming | semantic tokens |
| A new crate not in `deps.toml` | check-deps fails | add the row, with the allow-list |

Full anti-pattern table for game authors: doc 02 §13.

---

## 7. Adding a game

```bash
cargo xtask new-game <slug> --seats 2 --category abstract
```

Then work the checklist in doc 02 §14. The target is a playable, networked, spectatable,
replayable game in **one crate, under 300 lines**, with **zero platform changes**. If adding
your game requires editing anything under `crates/` or `services/`, that is a platform bug —
report it rather than working around it.

---

## 8. Writing style for code in this repo

- Doc-comment every public type with *what it is for*, and cite the doc section
  (`doc 03 §7`) rather than restating the design.
- Cite invariants by id (`I-5`) and decisions by id (`ADR-011`). They are defined once,
  in doc 00 §7 and §10.
- Prefer a `TODO(phase N):` marker over a plausible-looking wrong implementation.
- `#![forbid(unsafe_code)]` is workspace-wide. There is no exception process yet; if you need
  one, that is an ADR.
