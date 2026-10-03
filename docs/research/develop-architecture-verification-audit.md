# Tabula `develop` — architecture, verification, and direction audit

> **Status:** independent audit, not a normative document.
> [`docs/architecture/00-architecture-principles.md`](../architecture/00-architecture-principles.md)
> remains authoritative. Where this audit says a doc is wrong, the fix is a PR against that doc,
> not a citation of this one.

| Field | Value |
|---|---|
| Repository | `loveoverflowcom/tabula` |
| Branch | `develop` |
| HEAD at audit time | `f256e34` — *Merge pull request #33 from loveoverflowcom/feat/owned-verified-asset-loading* |
| Audit date | 2026-09-02 |
| Toolchain | rustc 1.96.1, cargo-mutants 27.1.0, Kani (cargo-kani), cargo-nextest, tiny-skia, wasm32-unknown-unknown |
| Working tree | clean before and after; every experiment in §24 was reverted and verified with `git status` |

---

## 1. Executive summary

### 1.1 The short answer

Tabula's **core is real, and it is better than its documentation implies in some places and worse
in others.** The deterministic kernel, the chess rules, the replay container, and the self-play
driver are load-bearing, working, and independently checkable. The *verification story around them
is thinner than the vocabulary used to describe it*, and the **product surface has fallen behind
the abstraction surface** over the last several PRs.

Three sentences that this audit will defend with evidence:

1. **The functional core / imperative shell split is genuinely clean, and it is the reason
   everything else is cheap.** 533 tests pass, `cargo xtask check` is green end to end, 8 300
   chess self-play matches with ~3.1M inputs terminate with zero determinism failures, and
   published perft node counts match to depth 4 (depth 5 behind `--ignored`).
2. **Several claimed verification mechanisms do not exist.** There are **zero property tests in
   the repository** (§17). The projection secrecy scanner — the mechanism doc 00 names as the
   enforcement for I-5/I-6, the platform's *security* invariant — is `todo!()` (§8). Two nightly
   CI jobs invoke commands that cannot succeed (§22).
3. **The last five merged PRs built an asset subsystem of roughly 5 400 lines that ships zero
   assets, has zero runtime consumers, and is not reachable from any playable path** (§26), while
   Phase 2's own exit criteria — hot-seat chess *on the web*, with clocks, with sound — remain
   unmet. This is the single clearest misallocation in the project right now.

### 1.2 What changed during this audit

Two things worth recording because they are *evidence*, not opinion:

- **Chess is playable in a browser today, and nobody had ever seen it.** The `wasm32` target
  builds (0.20 MB gzipped, against a 6 MB budget) but the repository contains **no host page**
  for `apps/game-client`. Adding a 12-line `index.html` plus macroquad's own `mq_js_bundle.js`
  produced a working board; I played `e2–e4` and `e7–e5` by clicking, watched selection
  highlights, legal-target dots, move animation, and turn alternation. See §25.
- **Eighteen negative-control experiments were run against the detectors.** Twelve fired with
  precise, actionable messages; four found nothing; one fired only coincidentally. The single most
  instructive result: a real chess legality bug was caught **only** by perft, while the conformance
  suite, the determinism harness, and 200 self-play matches all passed. See §24. That is the
  strongest argument in this document for keeping several independent oracles rather than
  consolidating into one large harness.

### 1.3 The ten answers, up front

| # | Question | Answer |
|---|---|---|
| 1 | Is the core architecture fundamentally sound? | **Yes.** Keep it. §5–§9 find no structural defect worth reversing. |
| 2 | Highest-leverage decision? | The **single ordered `Input` stream** (ADR-003). Everything cheap downstream — replay, timers, bots, disconnect ownership, self-play — is a consequence of it. §7. |
| 3 | Most likely to become a constraint? | **`GameRules` as a `Sized` static-dispatch trait with associated types**, combined with `RULES_VERSION` as an associated *const*. Multi-version linking (doc 02 §9.2) requires two `impl`s of one trait for one game; that is possible but awkward, and it interacts badly with `RULES_HASH` being a build-script artefact of a *directory*. §12. |
| 4 | Strongest verification evidence? | **Chess move generation** (perft against published external node counts) and **`DetRng`/canonical encoding** (frozen stability vectors + documented-preimage reconstruction). §15. |
| 5 | Looks most verified, actually weak? | **Rejection transactionality (R2) claimed as "formally verified".** The Kani harnesses prove R2 for *two concrete tic-tac-toe states* over symbolic `(seat, cell)` — 65 536 combinations, exhaustively coverable by an ordinary loop test in microseconds. Also: **`State::from_parts`**, the "state is reachable" boundary, survives 35 of its mutants. §16, §18. |
| 6 | Assets now, or playable runtime? | **Playable runtime.** Freeze asset expansion at the current boundary; spend the next four PRs on the generic local runtime, TicTacToe presentation, and the web host page. §26–§27. |
| 7 | Start Phase 4? | **No.** Two contract-validation gaps remain open: the projection security model has never been exercised by a hidden-information game, and no `Effect` other than a test harness's has ever been executed. §13, §26. |
| 8 | Next five PRs? | §28. In order: generic local runtime → TicTacToe presentation on it → WASM host page → projection scanner + a hidden-information probe → replay recording from real play. |
| 9 | Which skills for those PRs? | §21 maps each PR to skills; the new hierarchy is `rust-verification-testing` (router) → `rust-property-testing`, `rust-replay-differential-testing`, `rust-mutation-testing`, `rust-kani`, `rust-fuzzing`. |
| 10 | What can we trust, and where does trust stop? | §31. |

---

## 2. Audit baseline

Everything below was executed on the audited tree unless marked otherwise.

### 2.1 Commands run and their results

| Command | Result |
|---|---|
| `cargo nextest run --workspace` | **533 passed, 2 skipped, 0 failed** (30.3 s) |
| `cargo xtask check` (fmt, clippy, test, check-deps, check-no-game-ids, check-manifests, token freshness, no-raw-colors, cargo-deny) | **all gates passed** |
| `cargo xtask check-deps` | 26 workspace crates checked — all clear |
| `cargo xtask check-no-game-ids` | 246 files scanned for 5 game ids — all clear |
| `cargo xtask check-manifests` | 28 manifests checked — all clear |
| `cargo xtask selfplay tictactoe --matches 20000` | 20 000/20 000 terminated, 160 013 inputs, 0 failures, 1.6 s |
| `cargo xtask selfplay chess --matches 5000 --clock bronstein` | 5 000/5 000 terminated, 1 872 954 inputs, 0 failures, 105 s |
| `cargo xtask selfplay chess --matches 3000 --clock none` | 3 000/3 000 terminated, 1 125 941 inputs, 0 failures, 60 s |
| `cargo xtask selfplay chess --matches 300` (fischer) | 300/300 terminated, 112 817 inputs, 0 failures |
| `cargo kani -p tabula-core` | 3 harnesses, **3 successful**, 18.8 s solver time |
| `cargo kani -Z stubbing -p tabula-game-tictactoe` | 3 harnesses, **3 successful**, 53.2 s solver time |
| `cargo mutants --package tabula-core` | 133 mutants: **69 caught, 30 missed, 31 unviable, 3 timeouts** (3 min) |
| `cargo mutants --package tabula-game-tictactoe` | 198 mutants: **75 caught, 80 missed, 43 unviable** (3 min) |
| `cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release` | success — **552 KB raw / 0.20 MB gzipped** |
| `./target/debug/tabula-game-client` (native, X11) | ran 12 s without error or panic |
| Chess in a browser via a hand-written host page | **worked** — see §25 |

### 2.2 Scale

| Area | Rust files | Lines |
|---|---|---|
| `crates/tabula-testkit` | 21 | 8 555 |
| `games/chess` | 14 | 8 316 |
| `xtask` | 18 | 7 269 |
| `crates/tabula-assets` | 5 | 3 857 |
| `crates/tabula-presentation` | 8 | 2 752 |
| `crates/tabula-game-api` | 11 | 2 657 |
| `crates/tabula-core` | 10 | 2 235 |
| `games/tictactoe` | 10 | 2 095 |
| `crates/renderer-macroquad` | 8 | 2 050 |
| `crates/renderer-headless` | 2 | 1 576 |
| `crates/tabula-design` | 2 | 1 126 |
| `apps/game-client` | 2 | 476 |
| `crates/tabula-match` | 2 | 387 |
| `crates/tabula-protocol` / `-registry` / `-storage` / `-lobby` / `-net-client` / `-voice` | 6 | 662 (doc comments) |
| `games/cards` + `werewolf` + `tiles` | 3 | 286 (doc comments) |

`xtask/src/pack_assets_cmd.rs` alone is 1 506 lines; `crates/tabula-assets/src/manifest.rs` is
2 428. Together with `integrity.rs`, `source.rs`, and `loader.rs`, the asset subsystem is
**~5 400 lines** — the second-largest body of code in the repository after the testkit, and
larger than `tabula-core` and `tabula-game-api` combined.

---

## 3. Current implemented surface

Classification is by *runtime reality*, not by directory existence or doc-comment quality. Several
crates in this table have excellent architecture documentation and no code; that is deliberate per
AGENTS.md §4, and the table records it rather than criticising it.

| Subsystem | Status | Evidence |
|---|---|---|
| `tabula-core` — ids, `LogicalTime`, `DetRng`, `MatchSeed`, `SeatRoster`, `MatchOutcome`, canonical encode/decode, `state_hash` | **IMPLEMENTED** | 2 235 lines, frozen stability vectors, 3 Kani harnesses, 40 tests |
| `tabula-game-api` — `GameRules`, `GameModule`, `Input`, `Effect`, `Ctx`, `GameMetadata`, `GameCapabilities`, `LegalCommands`, `A11yDescription` | **IMPLEMENTED** | 2 657 lines; two real games implement it |
| `tabula-testkit` — `conformance!`, determinism harness, replay container + `ReplayRunner` + diagnostics, self-play driver, render-list snapshots | **IMPLEMENTED** | 8 555 lines; drives every game's test suite |
| `tabula-testkit::projection` — `SecretModel`, `assert_no_leaks`, `assert_no_event_bypasses_redaction` | **CONTRACT ONLY** | both functions are `todo!()`; `SecretModel` has no implementor |
| `tabula-testkit::strategies` — proptest generators | **SKELETON** | `input_sequence` returns `vec![(); 0..1]`; `roster` returns an empty roster; nothing calls them |
| `games/tictactoe` rules + bots | **IMPLEMENTED** | conformance green, 20 k self-play, 3 Kani harnesses, state-machine reachability test |
| `games/tictactoe` presentation (`ui.rs`) | **CONTRACT ONLY** | 63 lines, all doc comment; no `impl GamePresentation` |
| `games/chess` rules (movegen, clocks, draws, promotion, castling, e.p.) | **IMPLEMENTED** | perft to depth 4 in CI, depth 5 `--ignored`; 868 lines of clock tests; 8 300 self-play matches |
| `games/chess` bots (Trivial, Easy) | **IMPLEMENTED** | `phase_one_bot_pairings_self_play_without_illegal_or_nondeterministic_moves` (30 s) |
| `games/chess` presentation | **IMPLEMENTED** (moves only) | 3 994 lines, 7 insta snapshots; emits only `Command::Move` — resign/draw/claim are unreachable from the UI |
| `crates/tabula-presentation` — `RenderList`, `RenderCmd`, `Camera2D`, `InputEvent`, focus graph, motion, `AudioCue`/`AudioSink`, `GamePresentation` | **IMPLEMENTED** | 2 752 lines; a `compile_fail` doctest enforces that `State` cannot be presented |
| `crates/tabula-design` — authored `tokens.toml` → generated Rust + CSS + JSON | **IMPLEMENTED** | freshness gate in CI; `check-no-raw-colors` |
| `crates/renderer-macroquad` | **IMPLEMENTED** | native verified running; wasm verified running (§25) |
| `crates/renderer-headless` — recorder + tiny-skia rasterizer | **PARTIALLY IMPLEMENTED** | 2 committed PNG goldens for the rasterizer's own subset; **no chess/tictactoe scene is rasterized** |
| `apps/game-client` — `LocalChessMatch` | **PARTIALLY IMPLEMENTED** | 409 lines; chess-specific; no `Effect` execution, no timers, no replay recording, no viewer switching, no bots |
| `crates/tabula-assets` — validated manifest, identity, resources, pixel regions, pure resolution, integrity witness, `AssetSource` port, `load_verified` | **IMPLEMENTED** (as a library) | 3 857 lines, 55 tests |
| Asset **content** — packs, textures, fonts, audio | **PLANNED** | `assets/packs/` contains only `.gitkeep`; no `.png`/`.ogg` anywhere in the tree |
| Asset **consumption** — renderer handles, decoding, cache, CDN | **PLANNED** | `MacroquadAudioSink` registry is always empty at runtime; `MacroquadRenderer` has no texture path |
| `xtask pack-assets` | **IMPLEMENTED** | 1 506 lines, deterministic content-addressed builder, 20+ tests; has never built a real pack |
| `crates/tabula-registry` — `ErasedGame`/`ErasedMatch`/`GameAdapter`, `register!` | **CONTRACT ONLY** | 130 lines, entirely `//!` doc comment; zero code |
| `crates/tabula-match` — actor, pipeline, idempotency, timers, resume | **CONTRACT ONLY** (ports drafted) | `lib.rs` is doc; `ports.rs` defines `EventLog` etc. as traits with no implementor |
| `crates/tabula-protocol`, `-storage`, `-lobby`, `-net-client`, `-voice` | **CONTRACT ONLY** | doc-comment sketches |
| `services/tabula-server`, `apps/web`, `apps/admin`, `apps/desktop` | **SKELETON** | placeholder `main.rs` |
| `games/cards`, `games/werewolf`, `games/tiles` | **PLANNED** | doc-comment sketches, zero `impl` |
| Replay container `.tbr` v1 + `ReplayRunner` + evidence diagnosis | **IMPLEMENTED** | 2 492 + 439 lines; 3 committed goldens |
| Replay **recording from live play** | **PLANNED** | the only writer is `xtask replay-goldens`, which synthesises fixtures |
| Multi-version rules linking / `migrate` | **CONTRACT ONLY** | `migrate` default returns `Unsupported`; no game overrides it |

### 3.1 The one-line version

> Layers 1 and 2 (deterministic core, game contract, testkit) are **real**. Layer 3's client half
> is **real but chess-shaped**. Layer 3's server half is **documentation**. The asset subsystem is
> **a real library with no data and no consumer**.

---

## 4. Architecture reconstructed from code

### 4.1 The local flow, as actually implemented

The flow in the brief is *almost* what the code does. The differences matter.

```text
macroquad event          renderer-macroquad/src/input.rs
    │                    normalises to InputEvent {Pointer|Key|Focus}
    ▼
InputEvent ─────────────► ChessPresentation::on_input(&InputEvent, &View, &mut ChessLocal)
                              │  reads ONLY View + Local. Cannot see State (compile_fail test).
                              ▼
                         Option<Intent<Command>>
                              │
                              ▼
                    LocalChessMatch::handle_input           apps/game-client/src/lib.rs
                         │  seat  = view.you.unwrap_or(view.turn).seat()
                         │  index = next_input_index (checked_add; None == exhausted)
                         │  rng   = DetRng::for_input(seed, index)
                         │  now   = LogicalTime::ZERO            ◄── ALWAYS ZERO
                              ▼
                    Input::Player { seat, command }
                              ▼
                    ChessRules::apply(&mut State, Input, &mut Ctx)
                              │
                    ┌─────────┴──────────┐
              Err(RuleError)         Ok(Outcome { events, effects })
                    │                     │
         DISCARDED SILENTLY               ├── events ──► ChessRules::view_event(state_after, e, Viewer::Seat(mover))
         (no cue, no motion token)        │                   └─► ChessPresentation::on_view_event ─► AudioCues
                                          │
                                          └── effects ──► DROPPED ON THE FLOOR   ◄── NOT EXECUTED
                              ▼
                    view = ChessRules::project(&state, Viewer::Seat(state.turn.seat()))
                              ▼
                    ChessPresentation::present(&view, &local, &frame) ─► RenderList
                              ▼
                    MacroquadRenderer::submit(&RenderList)
```

Three deviations from the intended flow, all in `apps/game-client/src/lib.rs`:

1. **`Outcome::effects` is never read.** `Effect::SetTimer`, `CancelTimer`, `EndMatch`,
   `RequestBotMove`, `Notify`, `Checkpoint` — none is executed anywhere outside
   `tabula-testkit::selfplay`. `Init::events` and `Init::effects` from `create` are also dropped.
2. **`ctx.now` is hard-wired to `LogicalTime::ZERO`.** Combined with (1), this means the chess
   clock — 302 lines of rules plus 868 lines of tests — is *unreachable* from the client. The
   local match uses `Config::default()`, i.e. `clock: None`.
3. **`RuleError` is discarded** (`let Ok(outcome) = result else { return Ok(AudioCues::new()) }`).
   Doc 00 §4.1 requires the client to play the `invalid-action` motion token on rejection.

The projection/viewer handling is subtler than it looks and is **correct for hot-seat**: the view
is re-projected for whoever is now on turn, so `view.you == view.turn` always holds and the
presenter's `view.you == Some(view.turn)` gate never blocks. It also means the client never
exercises the "you are not on turn" branch, which is the online case.

### 4.2 The generic runtime that already exists, in the wrong crate

`tabula-testkit::selfplay` is a **complete, game-agnostic, deterministic local match runtime**:

```text
SelfPlaySetup<R> { config, roster }
    │
    ├── R::create(config, roster, Ctx{ now: ZERO, index: 0, rng: for_input(seed,0) })
    │       └── interprets Init::effects into a TimerQueue
    │
    └── loop:
          bot_action  = GameBot::choose(&R::project(state, Viewer::Seat(s)), &mut rng)
                        └── ready_at = now + bot.think_time(view)
          timer       = TimerQueue::next()               (deadline, timer_id) ordered
          scheduled   = choose_scheduled(bot_action, timer)   ── timer wins ties
          now         = max(now, scheduled.at)
          R::apply(state, scheduled.input, Ctx{ now, index, rng: for_input(seed, index) })
          effects:
              Effect::SetTimer{id,delay}  -> timers.set(id, now + delay)
              Effect::CancelTimer{id}     -> timers.remove(id)
              Effect::EndMatch{outcome}   -> terminal; a second one is MultipleEndMatch failure
          hostile injection at `hostile_fraction`, then R2/R8 checks on every rejection
          the whole match is replayed a second time and compared byte-for-byte
```

That is **exactly the machinery `LocalChessMatch` is missing**, already written, already
game-agnostic, already exercised over 3.1M chess inputs. The only reasons it cannot be used
directly by the client are that (a) it lives in the test tier, and (b) its input source is a
`GameBot` rather than a human's `Intent`. Both are small. See §11 and §28.

### 4.3 The future server flow — what exists of it

```text
network ──────────────────► NOT IMPLEMENTED (tabula-net-client: 115 lines of doc)
protocol ─────────────────► NOT IMPLEMENTED (tabula-protocol: 159 lines of doc)
authorization/sequencing ─► NOT IMPLEMENTED (documented as pipeline steps 2–6, doc 03 §7)
registry type erasure ────► NOT IMPLEMENTED (tabula-registry: 130 lines of doc)
MatchActor ───────────────► NOT IMPLEMENTED (tabula-match: lib.rs doc + ports.rs traits)
GameRules ────────────────► IMPLEMENTED
event log / snapshots ────► ports drafted (LogBatch, EventLog, InputKind); no implementor
projection ───────────────► IMPLEMENTED per game; no dispatch layer
clients ──────────────────► native + wasm gameplay client exists, no transport
```

The **only** part of the server flow with running code is `GameRules`. The ports in
`tabula-match/src/ports.rs` are the most valuable artefact there: they encode the "one transaction
per applied input" and "take owned data, return" constraints as *signatures*, and their own doc
comment honestly flags that the architecture docs name these ports but never define them.

### 4.4 Boundary compliance — verified, not assumed

| Boundary | Mechanism | Status |
|---|---|---|
| Layer-1 crates cannot reach tokio/macroquad/sqlx/leptos/getrandom | `deps.toml` + `xtask check-deps` walking the **resolved** graph | **Enforced.** Negative control NC-1 injected `tokio` into `tabula-core`; check-deps reported 8 violations with full transitive paths. |
| No platform crate names a game | `xtask check-no-game-ids` with zone classification | **Enforced.** NC-2 caught a literal in `tabula-presentation` at file:line:col. |
| Presenter cannot see canonical `State` | `compile_fail` doctest on `GamePresentation` (`crates/tabula-presentation/src/game.rs`) plus the fact that `View`/`ViewEvent` derive only `Serialize`, never `Deserialize` | **Enforced by types.** |
| Presentation state stays out of canonical state (I-10) | dependency direction only | **Not mechanically enforced**, but structurally hard to violate: `tabula-core`/`tabula-game-api` cannot depend on `tabula-presentation`. |
| Rules never read a clock or OS entropy | dep ban + `clippy.toml` `disallowed-types` | **Enforced in `tabula-core` and `games/*` only** — see F-08, §23. |
| Rejected input is a total no-op | testkit R2/R8 checks wired into `conformance!` unconditionally | **Enforced.** NC-3b confirmed. |

### 4.5 Accidental architecture drift found

| Drift | Where | Severity |
|---|---|---|
| A chess-specific local runtime in a crate whose name and docs promise a generic one | `apps/game-client` | **P1** (F-03) |
| A per-line `xtask-allow-game-id` escape hatch used to import a game directly into the client | `apps/game-client/src/lib.rs:28`, and `allow_games = true` in `deps.toml` for `apps/game-client` | P3 (F-29) — legitimate for Phase 2, but unbounded and unexpiring |
| Three different logical-time models in three harnesses | `testkit::determinism::run` (`index * 1000 ms`), `testkit::selfplay` (timer-driven), `LocalChessMatch` (`ZERO`) | P3 (F-30) — none of them is "the" runtime, so nothing pins the semantics the Phase-4 actor must implement |
| Determinism enforcement lint present in 6 crates, absent in the other 20 | `clippy.toml` exists only under `crates/tabula-core` and `games/*` | P2 (F-08) |

---

## 5. Functional-core / imperative-shell assessment

### 5.1 Decision review

```text
Decision            GameRules is a pure, synchronous, total function; the platform is the shell.
Why it exists       Replay, server-side validation, bots, audit, and property testing all
                    reduce to one property (ADR-002).
Problem solved      Every other correctness mechanism in the product is a corollary of purity.
Complexity added    Games cannot call anything; every platform interaction becomes an
                    Input variant or an Effect variant. Authors must learn effects-as-data.
Evidence useful     8 300 chess self-play matches replayed byte-identically; 20 000 tictactoe
                    matches; committed golden replays reproduce exact final state hashes;
                    the R2 checker caught an injected surgical defect with an exact diagnostic.
Failure mode        Purity is a *contract plus tests*, not a type guarantee, because `apply`
                    takes `&mut State` (ADR-026 §1 argues this deliberately). A game that
                    mutates before validating corrupts matches silently.
Alternatives        `apply(&State) -> Result<Transition>`; rejected in ADR-026 §1 with a
                    reason this audit accepts: the cheap move-based spelling buys nothing,
                    and the rebuild-everything spelling is a permanent per-command cost.
Verdict             KEEP.
Confidence          High.
```

### 5.2 Is the boundary genuinely clean?

I looked for four specific leaks.

**(a) Side effects inside rules.** None found. `games/chess` and `games/tictactoe` import only
`tabula_core` and `tabula_game_api`. `check-deps` walks the resolved graph, so a transitive
`getrandom` would fail. Verified negatively in NC-1.

**(b) Platform concerns in games.** One borderline case: chess's `can_checkmate` /
`timeout_outcome` encodes a *FIDE* rule (a flagged player draws if the survivor cannot mate)
rather than a platform rule — correct placement. `ReconnectPolicy { grace: 60_000, notify_rules:
true }` in chess's capabilities is a *declaration*, not code, which is exactly the intended shape.
No violations found.

**(c) Rules reading a clock.** `ctx.now` only. `clippy.toml` bans `Instant`/`SystemTime` in the
six rules-tier crates that have one. `charge_completed_move` takes `now` as a parameter and does
not mutate — the state write happens in the caller, after legality. This is the cleanest part of
the chess implementation.

**(d) The shell mutating state.** `LocalChessMatch::state` is private; the only writer is
`ChessRules::apply`. `tabula-testkit::selfplay` likewise. Good.

### 5.3 Where the split is *not* clean

**The shell is missing.** `apps/game-client` implements roughly one sixth of an imperative shell:
it decides input order and assigns input indices, and it does nothing else the shell is supposed
to do. It does not execute effects, does not own a clock, does not log inputs, does not schedule
timers, does not dispatch projections to more than one viewer, and does not surface rejections.

This is not a *violation* of the split — it is an **absence**. The consequence is that the
`Effect` half of the contract has never been executed by anything a user touches, and therefore
has never been validated against a real UI's needs. That is a Phase-4 risk today (doc 03 §7 step
16 is "execute effects", and Phase 4's whole idempotency story rests on effects being right).

**Finding F-03** (see §22) is the recommended fix.

---

## 6. `GameRules` contract assessment

### 6.1 The eight rules, and what actually enforces each

| Rule | Statement | Claimed enforcement | Actual enforcement | Gap |
|---|---|---|---|---|
| R1 | `apply` is deterministic | `determinism_same_inputs` | **Real.** Two independent runs, canonical bytes compared, wired into `conformance!` unconditionally. Plus 3.1M self-play inputs. | none |
| R2 | rejection is byte-transactional | `error_is_transactional` | **Real and non-opt-in.** `det::assert_transactional_on_error` compares `canonical_encode` before/after with a diagnostic naming the input index and both hashes. Verified by NC-3b. | none |
| R3 | never panics on hostile input | *"I-1 dep ban + clippy"* in the trait doc | **Weak.** No test is named `no_panic_on_hostile_input`; the actual coverage is the self-play hostile-injection stream (`hostile_fraction = 0.05`) plus fixture-driven `invalid_command` scenarios. There is no fuzz target and no property test. | **F-01** |
| R4 | no wall clock, OS RNG, env, files | dep ban + clippy | **Real for `games/*` and `tabula-core`; absent elsewhere** (F-08) | partial |
| R5 | `project` never leaks | `projection_hides_secrets` | **Does not exist.** `assert_no_leaks` is `todo!()`. | **F-02** |
| R6 | `view_event` is the only path to a client | `view_event_never_bypasses` | **Does not exist.** `assert_no_event_bypasses_redaction` is `todo!()`. The *structural* half is real (`View`/`ViewEvent` are `Serialize`-only, `State` is not on any wire). | **F-02** |
| R7 | ordered iteration only | I-2 clippy | partial, as R4 | F-08 |
| R8 | rejection disturbs no later RNG | `rejection_does_not_disturb_rng` | **Real**, and structurally guaranteed by per-input `DetRng::for_input(seed, index)` derivation. | none |

R1/R2/R8 are the strongest part of the contract. R5/R6 — the *security* half — have **no
mechanical enforcement at all**, only the type-level separation of `View` from `State`.

### 6.2 Is the trait shape right?

Reviewed against the five reference games.

**Yes, with three named reservations.**

1. **`LegalCommands` lives in the wrong place for hidden-information games.** Chess puts the full
   enumeration inside `View.legal_moves`. That is harmless for chess. For Tiến Lên, "what can I
   legally play" is a function of the player's own hand, so it is still fine — but for a game
   where legality depends on *another* seat's hidden state (a werewolf night action whose legality
   depends on who is still alive-but-unrevealed), a naive `legal_commands` in the view becomes a
   side channel. This is not a defect today; it is a **hazard to name before cards is written**
   (F-16). Recommendation: when the projection scanner lands, make it scan `View` *including*
   any embedded legal-command list, and document that `legal_commands` must be computed
   per-viewer or not at all.

2. **`RULES_VERSION` and `RULES_HASH` as associated consts constrain multi-version linking.**
   Doc 02 §9.2 requires linking `ChessModuleV1` and `ChessModuleV2` simultaneously. With
   associated consts on a trait, that means two distinct types implementing `GameRules`, which
   means two copies of the rules source tree in the workspace — and `RULES_HASH` is computed by
   `build.rs` over the *directory* `src/rules`, so the two copies must be two directories, or two
   crates. That is workable (`tabula-game-chess-v1` / `-v2`) but it is a real cost that nothing in
   the docs prices. See §12.

3. **No `is_terminal`, by design (ADR-026 §1).** Terminality is `Effect::EndMatch`. This audit
   agrees, and self-play's `MultipleEndMatch` failure kind proves the single-authority rule is
   checkable. One consequence worth writing down: **a shell that does not execute effects cannot
   know a match ended.** `LocalChessMatch` learns it only indirectly, via
   `View.status == Ended`, which is a second authority sneaking in through the projection. Fixing
   F-03 fixes this too.

### 6.3 Coverage of `Input` variants by real games

| Variant | chess | tictactoe | Exercised by a client? |
|---|---|---|---|
| `Player` | yes | yes | **yes** |
| `Timer` | yes (`TIMER_CLOCK`) | yes (`TIMER_MOVE`) | **no** — no client sets or fires timers |
| `Seat` | `Outcome::empty()` (clock keeps burning) | `Outcome::empty()` | **no** |
| `Admin(Cancel/ForceEnd)` | yes | yes | **no** |
| `Admin(Pause/Resume)` | `Unsupported` | `Unsupported` | **no** |

Only `Input::Player` has ever reached rules from a human. Self-play covers `Timer`, `Seat`, and
`Admin` via hostile injection, which is genuine coverage — but it is *bot-driven*, not
*shell-driven*, and the shell is the thing Phase 4 will build.

---

## 7. Input / Event / Effect model assessment

### 7.1 The single ordered input stream

```text
Decision            All match-changing facts enter through one totally ordered
                    Input::{Player, Timer, Seat, Admin} stream (ADR-003).
Why it exists       Replay totality; deterministic timers; clean AFK/disconnect ownership;
                    bots need no special path.
Problem solved      It removes the entire class of "something happened outside the log".
Complexity added    Every platform mechanism must be expressible as an Input variant, and
                    every game must handle every variant (even by returning Outcome::empty()).
Evidence useful     Replay works: committed .tbr files reproduce exact final hashes, and one
                    of them contains an Input::Timer at LogicalTime(6000) that replays.
                    Self-play injects Timer/Seat/Admin inputs and finds no divergence.
Failure mode        A second mutation channel appears, and replay silently becomes partial.
Alternatives        Separate command/timer/lifecycle queues. Rejected: doc 00 §3.1's
                    argument is correct and this audit found nothing to add to it.
Verdict             KEEP. This is the single highest-leverage decision in the project.
Confidence          High.
```

### 7.2 Where a developer will be tempted to add a second channel

I looked hard for these. Ranked by how likely and how soon.

| Temptation | Where it bites | Is the single stream sufficient? | Mitigation |
|---|---|---|---|
| **Client-side preview / optimistic echo** | now, in the chess presenter | Yes — doc 00 §4.1 already says preview is a separate `PendingCommand` type computed from the projection. The current presenter does not preview at all; it waits for the local apply, which is instantaneous. Online, someone will add preview. | Make `PendingCommand` a real type in `tabula-presentation` **before** the online client exists, so the shape is fixed. |
| **Bots** | Phase 3+ | Yes. `Effect::RequestBotMove` out, `Input::Player` back in. Self-play already proves it. | none needed |
| **Reconnect / resync** | Phase 4 | Yes — resync is a *read* (a fresh projection at a `state_version`), not a mutation. | none needed |
| **Pause/resume** | Phase 4 | Yes, `Input::Admin(Pause)`. But note: pausing is *subtraction on the shell side* (`paused_for`), so the shell must maintain a wall-clock→logical-time mapping that survives restart. That mapping is not in any port today. | Add `paused_for` to the `Clock` port sketch before Phase 4 |
| **Moderation (mute, kick, shadowban)** | Phase 7 | **Mostly no, and that is correct.** Muting is a *chat transport* decision (platform), not a match-state change. The only part that touches state is `Effect::SetChatScopes`, which flows the other way. Kicking a seat is `Input::Seat{change: Abandoned}`. | none needed |
| **Tournament control** (start round, adjudicate, force pairing) | Phase 9+ | Partially. Adjudication is `Admin(ForceEnd)`. *Scheduling* the next round is a room-level concern above the match, and belongs in the lobby, not in `Input`. | Write it down in doc 00 §6 before someone adds `Input::Tournament` |
| **Simultaneous action games (werewolf night, drafting)** | Phase 3 | **Yes, and this is the interesting case.** Simultaneity is not a second channel; it is a phase in which several seats may act and the game buffers until a `Timer` closes the phase. Werewolf's night is exactly `Input::Player` × N followed by `Input::Timer`. The ordering the log records is the *arrival* order, which is arbitrary but deterministic-on-replay. | The hazard is a game that makes outcomes depend on arrival order in a way players can exploit by racing. Name it in the werewolf information model. |
| **Async / correspondence games** | Phase 9 | Yes. `LogicalTime` is milliseconds since match start; a 24-hour turn is just a large delta. Doc 02 §12.4's claim that "a game written for 60-second turns works unchanged for 24-hour turns" holds — verified by inspection of `LogicalTime`/`Millis`, which are plain `u64` with saturating arithmetic proven by Kani over the full domain. | none needed |
| **Hibernation / eviction of idle matches** | Phase 9 | Yes, and this is where the *single stream* pays off most: rehydrating from a snapshot plus the input suffix is total. | none needed |
| **Undo / takeback** | any time a friendly game is requested | **This is the real trap.** Undo is a mutation that is not a forward input. The only correct implementations are (a) a game-defined `Command::ProposeTakeback` + `AcceptTakeback` that *forward*-applies a state rewind computed by the rules, or (b) truncating the log, which destroys audit integrity. | **Write this down now.** Doc 00 §6 has no row for takeback, and chess is the game most likely to be asked for it. Recommend: undo is a game command, never a log operation. |

Net: the single stream survives all nine. **One documentation gap** (takeback) and **one shape to
fix early** (`PendingCommand`).

### 7.3 Command / Event / Effect separation

```text
Command = intent      (may be illegal; decoded by the module from opaque bytes)
Event   = canonical domain fact, full information, appended to the log verbatim
Effect  = idempotent request to the imperative shell, executed after persistence
```

The semantic distinction is **clean in the type system and clean in both games**. I looked for
three specific ambiguities:

**(a) Duplicated authority between `Event::Ended` and `Effect::EndMatch`.** Chess emits both, and
tictactoe emits both. Are they two authorities? No — and the code is careful about it. `Event`
is what replay and audit read; `Effect` is what the platform acts on. `tabula-testkit::selfplay`
treats `Effect::EndMatch` as the *sole* terminal authority
(`@ai.invariant end-match-is-sole-terminal-authority`) and fails on a second one. The `Event` is
redundant-by-design for clients. This is correct; it should stay documented as such.

**(b) `Effect` variants that are really events.** `Effect::Checkpoint { label }` and
`Effect::Notify { audience, notice }` are borderline: both are "record that something happened"
rather than "do something". `Checkpoint` is defensible (it asks the platform to write a durable
marker the game cannot write itself). `Notify` is a real effect (push notification, toast). No
change recommended.

**(c) Absolute-vs-delta.** `SetChatScopes`/`SetVoiceScopes` carry the whole scope map, explicitly
so re-application after crash recovery is a no-op. That is the right call and the doc comment
explains it. Nothing enforces it yet, because nothing executes effects.

**One real gap:** the idempotency table in `crates/tabula-game-api/src/effect.rs` is excellent
prose (`RequestBotMove` keyed by `(match_id, seat, state_version)`, `Notify` by
`(match_id, audience, notice_id)`) — but `Notice` has **no `notice_id` field**. The documented
dedupe key does not exist in the type. That is a small, cheap, pre-Phase-4 fix and exactly the
kind of drift that becomes expensive after the protocol freezes.

### 7.4 Effects: never executed

Worth restating as a first-class finding: **outside `tabula-testkit::selfplay`, no code in this
repository has ever executed an `Effect`.** The variant list has been designed twice (doc 00 §6.4,
`effect.rs`) and validated zero times against a real shell's needs. See F-03 and PR-1 in §28.

---

## 8. State / View security model

### 8.1 The model as designed

```text
State (server-only, full information)
  ├─ project(state, Viewer::Seat(n))     -> View
  ├─ project(state, Viewer::Spectator(t))-> View
  ├─ project(state, Viewer::Audit)       -> View
  └─ view_event(state_after, event, v)   -> Option<ViewEvent>     None hides EXISTENCE
```

Three structural properties are **real today and verified by type**:

1. `View` and `ViewEvent` derive `Serialize` but **not** `Deserialize`. `State` derives both. So
   the wire can carry a view out and cannot carry a view back in as state.
2. `GamePresentation`'s transition methods name only `View`/`ViewEvent`. A `compile_fail` doctest
   in `crates/tabula-presentation/src/game.rs` asserts that passing `State` to `present` does not
   compile. That is a genuine type-level proof, and it is the best example of "types as proofs" in
   the repository.
3. `MatchSeed` has a manual `Debug` that prints `MatchSeed(<redacted>)`, and `DetRng`'s `Debug`
   never prints its key.

### 8.2 What is missing — and it is the important half

| Mechanism doc 00 names | Reality |
|---|---|
| `SecretModel` implemented by every hidden-information game | trait exists; **zero implementors**; no game has `hidden_information = true` |
| `assert_no_leaks` token scan over project() and view_event() for every unauthorised viewer | **`todo!()`** |
| `assert_no_event_bypasses_redaction` (I-6) | **`todo!()`** |
| `no_state_type_on_the_wire` protocol test (I-5) | Phase 4; `tabula-protocol` has no code |
| Spectator projections checked explicitly, `Live` and `Delayed` as separate viewers | not runnable — the scan is the `todo!()` |
| `docs/games/<slug>.md` information model per game | `docs/games/` contains only `README.md` |

**AGENTS.md §5 instructs contributors that a game with hidden information needs "a `SecretModel`
with the projection scan passing".** Following that instruction today produces a `todo!()` panic.

This is **F-02**, and it is the highest-severity finding in the audit, not because anything is
broken now — no game has secrets — but because:

- doc 09 §6 names "a projection leak in a hidden-information game" as the **#1 most likely thing
  to go wrong**, and the named mitigation is precisely this scan;
- Phase 3's *reason for existing* is to stress the projection boundary with cards and werewolf;
- Phase 4's exit criteria include "spectator sees only projected data (asserted against
  `SecretModel` for cards)".

Writing cards without the scanner means the scanner gets written *against* an existing projection,
which is how scanners end up shaped to pass.

### 8.3 The three leak classes and how each is (not) covered

**Class 1 — wholesale leaks** (a hand serialised into a spectator's view; a role map sent
verbatim). This is what the token scan is designed for, and the design is right: canonically
encode each secret's tokens and assert absence in the encoded view. **Not implemented.**

**Class 2 — existence leaks** (the *fact* that a night action occurred reveals who acted). Handled
by `view_event -> None`. **Structurally possible, entirely untested.** Note the mutation result:
in tictactoe, `replace view_event -> Option<Event> with None` **survives** — nothing in the test
suite notices if a game hides every event. The inverse mutation (returning `Some` where it should
be `None`) would be the actual security bug, and there is no detector for it either.

**Class 3 — derived leaks** (public deck count + public discards + own hand ⇒ opponent's hand).
No scanner can find these; `docs/games/README.md` says so honestly and requires a written
information model. **The mechanism is a required document that does not exist for either shipped
game.** For chess and tictactoe that is harmless (no secrets). It becomes load-bearing at cards.

### 8.4 Additional safeguards this audit recommends

Beyond finishing the scan:

1. **A noninterference property, not just a containment scan.** Containment ("the secret's bytes
   do not appear in the view") is coarse. The stronger and equally cheap property is
   *noninterference*: mutate only the secret part of the state and assert the unauthorised
   viewer's projection is **byte-identical**. That catches derived leaks that a token scan cannot
   — a length, an ordering, a count that shifts. This is a property test over reachable states and
   belongs in `rust-property-testing`. It requires games to expose a `scramble_secrets(state, rng)`
   in the `SecretModel` — a small addition with a large payoff.
2. **`Viewer::Audit` should not be freely constructible** (already a `TODO(phase 4)` in
   `viewer.rs`). Today any code can write `Viewer::Audit` and get full information. The type-level
   fix — an `AuditGrant` capability token — is cheap and should land before any code path can be
   handed a `Viewer` from a network message.
3. **`legal_commands` must be part of the projection audit** (F-16), because chess has already
   established the pattern of embedding it in `View`.
4. **Delayed spectators need a platform-side buffer, and there is nowhere to put it yet.**
   `SpectatorTier::Delayed { by: Millis }` exists in the type; nothing consumes it. Fine for now;
   flag it as a Phase-4 port.

---

## 9. Determinism and replay architecture

### 9.1 Deterministic RNG

```text
Decision            MatchSeed (32B, OS entropy, server-only) →
                    DetRng::for_input(seed, index) = ChaCha8(blake3(seed ‖ b"input" ‖ index_le))
                    DetRng::stream(&self, domain)  = ChaCha8(blake3(key ‖ b"stream" ‖ domain_le))
                    below() = rejection sampling; shuffle() = pinned Fisher-Yates
Why it exists       Per-input derivation makes the number of draws inside one input invisible to
                    every later input. That is what makes a rejected input a *total* no-op
                    with no rollback machinery (contract R8, ADR-026 §5).
Problem solved      RNG drift when a rule adds a draw; replay invalidation on refactor.
Complexity added    Two blake3 hashes per input; substream discipline authors must learn.
Evidence useful     Frozen stability vectors; two "documented preimage" tests that rebuild the
                    derivation by hand; substream-independence test; 3.1M self-play inputs.
Failure mode        A dependency bump or an "optimisation" silently changes the stream.
Alternatives        One match-long stream (rejected — breaks R8); a counter-mode PRNG without
                    domain separation (rejected — same reason).
Verdict             KEEP, with two concrete hardening items below.
Confidence          High on the design; medium on the implementation's test coverage.
```

**Investigation results against the brief's checklist:**

- **Rejected commands** — safe by construction. Each index derives an independent stream, so a
  rejected input at index *i* cannot shift index *i+1*. The testkit checks this
  (`assert_rejection_does_not_disturb_rng`) and NC-3b confirmed the check fires.
- **Optional random draws** — safe within one input, because a draw inside input *i* cannot shift
  input *i+1*. **Not safe within one input across rules versions**: adding a draw before an
  existing one at the same index changes the later draw. That is correct and expected (it is a
  `RULES_VERSION` bump), and the `state_hash` preimage includes `RULES_VERSION` so old and new
  cannot collide. Good.
- **Domain reuse** — a game that uses the same `domain` for two purposes gets a shared substream.
  Nothing detects it. `domain::GAME_BASE = 1000` is a convention, not a registry. Low severity;
  worth a doc note in the game checklist.
- **Replay** — verified: `ReplayRunner` re-derives `DetRng::for_input(seed, frame.input_index)` per
  frame, and the committed goldens reproduce exact final hashes.
- **Bot RNG** — bots receive `&mut DetRng` derived from the same seed in self-play, so bot choices
  are part of the deterministic trace. Correct.
- **Parallel computation inside rules** — forbidden by doc 00 §5.1, unenforced mechanically. No
  game does it. Acceptable.

**Two hardening items, both discovered by mutation testing (§18):**

**(i) The rejection zone has no direct assertion.** `below(n)` computes
`zone = 2^32 - (2^32 % n)`. Mutating `-` to `+` **survives the entire test suite**. I confirmed the
semantic effect with an independent program: for `n = 6`, the original rejects 4 of 2^32 draws;
the mutant rejects **zero**, restoring exactly the modulo bias the doc comment says must never
exist ("small but real in a card game is a cheating accusation we cannot disprove"). The frozen
24-draw vector cannot hit a 1-in-a-billion case, and the 60 000-sample chi-eyeball test cannot see
a bias of that magnitude. **The stated rationale for the rejection loop has no evidence behind it.**
Fix: a direct unit test that `zone` is the largest multiple of `n` not exceeding 2^32, plus a Kani
harness over all `n: u32` (§16.5). Cost: ~15 lines.

**(ii) `shuffle` of a 2-element slice is not covered.** The guard
`if slice.len() < 2 || u32::try_from(len).is_err() { return; }` survives mutation to `len == 2`.
I confirmed: with that mutant, length 0 and 1 still behave identically (`(1..len).rev()` is
empty), but **a 2-element shuffle becomes the identity**. `shuffle_is_a_permutation` iterates
lengths 0..40 and only checks that the result is *a* permutation — the identity is one. A 2-card
deck, a 2-role assignment, or a coin flip implemented as `shuffle(&mut [a, b])` would silently
never shuffle. Fix: assert that across seeds both orders of `[0, 1]` occur. Cost: ~8 lines.

Neither is a bug today. Both are **absent evidence for an explicitly claimed property**, which is
precisely what this audit is for.

### 9.2 Canonical encoding and state hashing

```text
canonical(x) = ENCODING_VERSION_le(u16) ‖ postcard(x)
StateHash    = blake3( b"tabula.state.v1" ‖ RULES_VERSION_le(u32) ‖ canonical(state) )
```

**This is coherent.** Specifically:

- Both prefixes are fixed-width, so the preimage is unambiguous without length prefixing — the
  doc comment says so and it is true.
- `canonical_decode` *checks* the version prefix and fails loudly rather than decoding a plausible
  wrong state. That distinction ("unreplayable is honest; a fake replay is not") is the right call
  and is tested.
- `RulesVersion` is a **typed parameter**, not a free-form tag, so it cannot be omitted. ADR-026 §2
  explains that the previous shape allowed exactly that mistake. Good.
- `blake3_dependency_is_standard` asserts the published empty-input vector. Cheap, and it is the
  kind of check that catches a supply-chain swap.

**Migration hazards found:**

1. **`ENCODING_VERSION` is checked; `RULES_VERSION` is not, on decode.** `canonical_decode` rejects
   a foreign `ENCODING_VERSION`. Nothing prevents decoding a `RulesVersion(2)` snapshot with
   `RulesVersion(3)` code if the postcard layout happens to be compatible — and postcard is
   non-self-describing, so a field reorder or a same-width type change *will* decode into garbage.
   The replay header carries `rules_version` and `rules_hash`, and `ReplayIdentity` compares them,
   so the *replay* path is safe. The **snapshot restore path** (`GameModule::restore_match`, Phase
   4) has no such guard yet. Write the guard into the port signature before implementing it.
2. **`RULES_HASH` is a hash of a source directory.** `games/chess/build.rs` hashes every `.rs`
   under `src/rules`, sorted, length-prefixed, domain-separated. This is genuinely good: a
   comment-only change bumps the hash, which is conservative in the right direction. But it means
   `RULES_HASH` changes on **formatting**, and the replay compatibility matrix (doc 05 §6.2) treats
   a hash mismatch as `CompatibleVersion` rather than `Exact`. Expect a long tail of
   "CompatibleVersion" replays after any `cargo fmt` sweep. That is acceptable and documented; it
   should be *expected* rather than investigated each time.
3. **The frozen constants have a single point of defence.** NC-8b changed `STATE_HASH_DOMAIN` from
   `tabula.state.v1` to `v2`. Only **one** test failed (`state_hash_is_stable`, the captured
   literal). The companion test `state_hash_matches_its_documented_preimage` rebuilds the preimage
   using the same constant, so it is an independent oracle for *composition and order* but not for
   *the constant's value*. Same for `DetRng`'s `b"input"` / `b"stream"` tags — NC-8 changing
   `b"input"` to `b"inpuT"` failed 4 tests, which is better. Recommendation: spell the domain
   strings as byte literals inside the preimage tests too, so the constant is asserted twice from
   two directions. Cost: 2 lines. (F-21)

### 9.3 Replay

**What works.** `.tbr` v1 is a well-designed hostile-input-aware container: magic, fixed-width
length prefixes, a CRC32 trailer, explicit caps on decompressed size (4 MiB), compressed size,
header size, frame size, frame count (100 000), config size, and the zstd back-reference window
(4 MiB). It rejects a projected replay that carries a seed. `ReplayRunner` re-runs the ordinary
typed `create`/`apply` path with recorded logical times and input indices, compares every recorded
checkpoint and the final hash, and `ReplayDiagnosis` classifies evidence strength as
exact / windowed / final-only / terminal-outcome. That last part is unusually honest engineering
and should be preserved.

**What is missing.**

| Gap | Detail |
|---|---|
| **No replay is derived from real gameplay.** | The only writer is `xtask replay-goldens`, which synthesises three fixtures directly from `GameRules`. The corpus is a 5-input tictactoe game, a 4-move chess mate, and a 2-frame chess clock timeout. `games/README.md` requires "≥ 3 golden replays (a normal game, an edge case, a timeout)" **per game**; chess has 2, tictactoe has 1. (F-26) |
| **The client records nothing.** | `LocalChessMatch` keeps no input log, so a human game cannot be turned into a `.tbr`. This is the single cheapest way to get a *real* replay corpus and it is ~20 lines once the runtime is generic. |
| **Self-play produces no replays.** | The driver explicitly "never writes files or mutates Git history" — correct as a library policy — but `xtask selfplay` does not offer `--write-failing-replay` either, and the nightly workflow's comment claims "any failing seed is auto-committed to `tests/replays/<game>/regressions/`", which no code does. |
| **`xtask replay --all` is referenced by nightly CI**; the usage string documents `replay <file> [--verify] ...` and `--all` is not in the parser's documented flag list. Nightly's `cargo xtask replay --all --verify` is at best untested. (F-10) |
| **No cross-target replay comparison.** | Doc 00 §5.1 requires byte-identical state "on x86-64 and aarch64, native and WASM". The WASM build is `cargo check`ed in CI and never *run*. Nothing has ever compared a state hash produced by the wasm32 build against the native one. Given that the whole determinism claim is cross-platform, this is the largest *untested* part of the strongest claim. |

The last row deserves emphasis: **"determinism is the product", and the cross-target half of that
claim has zero evidence.** It is also cheap to obtain — see PR-6 in §28.

---

## 10. Presentation / renderer architecture

```text
Decision            View + Local -> RenderList -> Renderer trait -> Macroquad (first backend)
Why it exists       ADR-010: the renderer is #7 in the value stack and the most likely component
                    to be replaced.
Problem solved      Games never call a renderer; a second backend costs one crate.
Complexity added    A capped render-command set, a hit-testing/layout layer we own, a focus
                    graph, a motion engine, and a text-measurement port.
Evidence useful     `renderer-headless` exists and rasterizes — the abstraction has a second
                    consumer today, which is the only honest justification for it.
                    The chess presenter emits only RenderList commands; `check-no-raw-colors`
                    is enforced (NC-10b) and `xtask gen-tokens` freshness is a CI gate.
Failure mode        The command set grows to please one visual idea; or Macroquad's text
                    ceiling arrives during mobile work (doc 09 §6 risk #4).
Verdict             KEEP.
Confidence          High on the abstraction; medium on Macroquad's ceiling (unmeasured).
```

### 10.1 Can Macroquad still be replaced?

**Yes.** Checked directly:

- `grep` for `macroquad`/`miniquad` outside `crates/renderer-macroquad` and `apps/game-client/src/main.rs`: none. `deps.toml` forbids it everywhere else and `check-deps` walks the resolved graph.
- `renderer-headless` implements the same `Renderer` trait with tiny-skia and **rejects** every
  command outside its documented subset rather than silently dropping pixels
  (`RasterError::UnsupportedCommand(RenderCmdKind)`). That is the correct shape for a second
  backend and it is what makes the abstraction real rather than aspirational.
- `FrameCtx`, `Viewport`, `Dpi`, `TextMetrics`, `PointerPosition` are all proof-carrying newtypes
  with fallible constructors, so a backend cannot hand the presenter a NaN viewport.

### 10.2 Leakage found

| Leak | Severity | Note |
|---|---|---|
| **Text is unmeasured at the contract level in practice.** `Renderer::measure_text` exists; `renderer-macroquad`'s implementation maps a token to a size and uses the *default* font. `text.rs` says font family, weight, tracking and shaping "need the Phase 3 font asset path". | P3 | Observed live: the status line renders `Your turn — White` with a **tofu box** where the em dash should be, because the default font has no U+2014 (F-23). This is the first concrete instance of Macroquad's text ceiling and it arrived exactly where doc 09 predicted. |
| **`View.clock` is never rendered.** 302 lines of clock rules + 868 lines of clock tests produce a field that the presenter ignores. | P2 | Part of F-11. |
| **Animation timing is driven by `frame.now_ms()` from `mq::get_time()`** — correct (presentation clock, never canonical), but the piece-move animation observed in the browser took well over one second, and mid-flight the glyph renders offset below the square centre. | P3 | Cosmetic; recorded because it is the kind of thing only running the app finds. |
| **Hard-coded English strings in the presenter.** `format!("Your turn — {}")`, `"Empty square"`, `"White rook"`, `"Game over — {summary}"`. Doc 04/doc 07 Phase 5 require i18n keys with no literals. | P3 (F-24) | Cheap now, expensive after four games. |
| **No theme toggle, no reduced-motion mode.** `main.rs` hard-codes `Theme::by_kind(ThemeKind::Light)`; `MotionMode::Full` is hard-coded in `on_view_event`. Both are Phase-2 exit criteria. | P2 | Part of F-11. |
| **Light-theme board contrast is very low.** In the running client, `surface_container` and `surface_container_high` are nearly indistinguishable, so the checkerboard reads as a flat white grid (see §25 screenshots). The token contrast test doc 07 requires is for text, not for adjacent surfaces. | P3 | A design-token issue, not an architecture issue. |
| **Asset coupling: none.** `AssetPackRef` is returned by `GamePresentation::asset_pack()` and used by nobody. The renderer has no texture path. | — | Not a leak; an absence. |

### 10.3 WASM / mobile implications

- The wasm32 build **works and is tiny**: 552 KB raw, **0.20 MB gzipped**, against doc 01 §7's
  6 MB budget. The CI job that would check the budget is commented out
  (`# - run: cargo xtask check-bundle-size`) and the command does not exist. Not urgent — there is
  a 30× headroom — but the comment implies a gate that is not there.
- **There is no host page.** `apps/game-client` has no `index.html`, no `Trunk.toml`, no
  `mq_js_bundle.js`. `apps/web` has all three, for the (empty) Leptos shell. So the wasm artefact
  has never been loaded. See §25 for what happened when I loaded it.
- Mobile is untouched; `mobile/android` and `mobile/ios` are empty directories. Fine for Phase 2.

---

## 11. Local game runtime assessment

This is §O of the brief and the most actionable part of this audit.

### 11.1 What `LocalChessMatch` is

409 lines in `apps/game-client/src/lib.rs`, of which ~250 are tests. Its non-test surface:

```rust
pub struct LocalChessMatch {
    state: State,                       // chess State
    view: View,                         // chess View
    local: ChessLocal,                  // chess presentation-local state
    seed: MatchSeed,                    // fixed [0; 32]
    next_input_index: Option<InputIndex>,
}
```

Everything is chess. Three of the five fields name a chess type. `new()` calls
`ChessRules::create`, `handle_input` calls `ChessPresentation::on_input` and `ChessRules::apply`,
`present` calls `ChessPresentation::present`.

**What it does well** — and this deserves saying, because the answer is not "rewrite it":

- The input-index discipline is genuinely careful. `next_input_index: Option<InputIndex>` uses
  `None` as the explicit exhausted state rather than a sentinel, with the comment "`InputIndex(u64::MAX)`
  is a valid final attempt", and there are two dedicated tests for the `MAX - 1` / `MAX` boundary.
  That is Phase-4-quality thinking about the RNG domain root.
- Rejected commands still consume an index, matching the replay contract.
- The presenter is fed only `View`; canonical state is private.
- Audio-sink failure cannot undo an accepted move, and there is a test for it.

**What it does not do:** execute effects, run a clock, fire timers, record inputs, drive bots,
switch viewers, surface rejections, or work for any other game.

### 11.2 Is it becoming unnecessarily chess-specific?

**Yes — and the second game will make it obvious.** Writing `LocalTicTacToeMatch` would duplicate
the index discipline, the viewer selection, the cue plumbing, and the render call, and the two
copies would drift on exactly the parts that matter (index-on-rejection, viewer choice).

But note the more important point: **the generic version already exists** in
`tabula-testkit::selfplay` (§4.2). It has a `TimerQueue`, effect interpretation, logical-time
advancement, terminal-authority checking, and a determinism re-run. The client has none of that
and the testkit has no human input. Neither is complete; together they are.

### 11.3 Recommendation — the minimum abstraction justified by two games

**Do not create a new crate yet.** Do this, in `apps/game-client`, in one PR:

```rust
/// Generic, deterministic, single-process match driver.
/// Owns the imperative shell responsibilities that `GameRules` deliberately lacks.
pub struct LocalMatch<R: GameRules, P: GamePresentation<Rules = R>> {
    state: R::State,
    view: R::View,
    local: P::Local,
    seed: MatchSeed,
    next_index: Option<InputIndex>,
    now: LogicalTime,          // advanced from the presentation clock, monotone-clamped
    timers: TimerQueue,        // SetTimer / CancelTimer / next()
    ended: Option<MatchOutcome>,
    log: Vec<RecordedInput>,   // (index, logical_time, canonical Input bytes)
    viewer: Viewer,            // hot-seat: follows the turn; later: fixed per seat
    bots: BTreeMap<SeatId, Box<dyn GameBot<R>>>,
}
```

Responsibilities it must take, in priority order:

1. **Execute every `Effect`** — `SetTimer`/`CancelTimer` into the queue; `EndMatch` into `ended`
   and stop accepting player input; `RequestBotMove` into the bot scheduler; `Notify` into a
   presentation toast; `Checkpoint` and the scope effects can be no-ops with a `TODO(phase 4)`
   *and a test that asserts they are ignored deliberately*.
2. **Own logical time.** Advance `now` from the frame clock, clamped monotone. This is the one
   place wall time is allowed to touch the match, and it must be a single, reviewable function.
   Timers fire by comparing `now` against the queue before draining input.
3. **Surface rejections.** Return the `RuleError` so the presenter can play `invalid-action`.
4. **Record the input log** so any local game can be written as a `.tbr`.
5. **Viewer selection.** Hot-seat = follow the turn (today's behaviour, made explicit); an added
   "spectator view" toggle is then three lines and is the cheapest possible smoke test of the
   projection boundary.
6. **Bots**, driven by `Effect::RequestBotMove` and answering through `Input::Player` — the same
   path a human takes.

**What must stay separate from the future server runtime, and why.** The local driver is
*allowed* to be simple in exactly the places the server cannot be: no idempotency cache (there is
no network, so no duplicate delivery), no durability, no mailbox or backpressure, no
`catch_unwind` supervision, no ownership lease, no resume. Those are the five hardest parts of
`tabula-match` and none of them has a local analogue. Trying to share them would produce a
premature abstraction of exactly the kind doc 09 §7 warns about.

**What should be shared.** The *transition semantics*: input index assignment, `Ctx` construction,
effect interpretation, timer ordering, and terminal authority. The right way to keep local and
server honest without coupling them is **a shared conformance test in `tabula-testkit`** that both
drivers must pass — a `MatchDriverContract` fixture asserting, for a scripted sequence:

```text
same (config, roster, seed, ordered inputs)
  => same input indices assigned
  => same timer firing order
  => same effect execution order
  => same final canonical state hash
```

Then `selfplay`, `LocalMatch`, and later `MatchActor` are three implementations of one checkable
contract. That is a genuine abstraction earned by three consumers, not by speculation.

### 11.4 Sequencing

Extract to a crate (`crates/tabula-local`) only when a **third** consumer appears — the most likely
being a headless replay/inspection tool or the web shell. Until then the generic driver lives in
`apps/game-client` as a library type, costs no `deps.toml` change, and crosses no phase gate.

---

## 12. Registry and type-erasure assessment

`crates/tabula-registry` is 130 lines of doc comment and zero code. The design sketch is:

```text
ChessRules: GameRules  →  GameAdapter<ChessModule>  →  Box<dyn ErasedGame> / Box<dyn ErasedMatch>
```

### 12.1 Object safety

The sketched `ErasedGame`/`ErasedMatch` traits are object-safe as written: every method takes
`&self`/`&mut self` and concrete or boxed types, with no generics and no `Self` in return position
except behind `Box`. `GameRules` itself is `Sized + Send + Sync + 'static`, which is fine because
the adapter is the thing that gets boxed. **No object-safety problem found.**

One detail the sketch gets right and should not lose: `ErasedMatch::view_events(&self, viewer,
codec)` returns "per-viewer redactions of the events from the LAST successful apply", i.e. the
adapter caches the typed events between `apply` and redaction. Re-decoding canonical bytes per
viewer would risk decode/encode asymmetry. Keep that.

### 12.2 Codec ownership and serialization ambiguity

`Codec` is a parameter on every erased method, not a global — correct, and it is what lets one
match serve a Postcard client and a JSON debug client. But there is a real ambiguity the docs have
not resolved:

- **Canonical bytes are always Postcard** (`canonical_encode`), because the state hash and the log
  depend on it.
- **Wire bytes are Postcard or JSON**, negotiated.

So `ErasedOutcome.canonical_events: Vec<Bytes>` and `ErasedMatch::view_events(.., codec)` are two
different encodings of related data, and the adapter must never confuse them. Recommend: make them
different newtypes (`CanonicalBytes` vs `WireBytes`) in the registry crate on day one. This is a
five-line application of `rust-types-as-proofs` that prevents a whole class of "we hashed the JSON"
bugs.

### 12.3 Version resolution and multiple simultaneous `RulesVersion`

This is the part with real cost, and it is the answer to brief question 3.

`RULES_VERSION` and `RULES_HASH` are **associated consts on `GameRules`**. To link chess v1 and v2
simultaneously you need two types implementing `GameRules`. Because `RULES_HASH` is produced by
`build.rs` hashing the `src/rules` **directory**, the two versions need two directories, which in
practice means two crates (`tabula-game-chess-v1`, `tabula-game-chess-v2`) or one crate with two
build-script-hashed subtrees and a feature flag.

None of that is impossible. But it means:

- the workspace grows a crate per live rules version per game;
- `check-manifests`, `check-no-game-ids`, and `deps.toml` all need to understand versioned game
  packages;
- `cargo mutants` / `cargo kani` / conformance run twice per version;
- the `game.toml`↔code cross-check (already missing, F-09) gets harder.

**Recommendation:** do not change the trait now. Do write an ADR *before* Phase 4 that states how
multi-version linking will be spelled, because the answer constrains the registry's `register!`
macro and the resolution API. The cheapest answer is probably "one crate per game, one
`RULES_VERSION` at a time; a version transition links a frozen archived crate", and archiving is
cheap if it is planned.

### 12.4 Runtime cost and testing difficulty

- **Runtime cost:** one vtable call per input plus one encode per viewer per event. At board-game
  rates (tens of commands per minute per match) this is irrelevant. Doc 02's assessment is correct.
- **Testing difficulty:** the erased layer is where "the platform must not know about games" is
  either true or false, and it is hard to test *negatively*. The existing `xtask check-no-game-ids`
  is a grep; it caught NC-2, but it cannot catch a semantic branch (`if caps.hidden_information &&
  caps.seats.max() == 4 { ... }` is a chess/cards branch in disguise). Recommend a registry-level
  test asserting that swapping two games' entries in `register!` changes nothing except the
  catalog ordering.

### 12.5 Simpler designs considered

- **One `GameState` mega-enum.** Correctly rejected (doc 00 §12): it makes every game a
  compile-time dependency of every other. No reason to revisit.
- **`Any`-based downcasting instead of an erased trait.** Worse: it moves the type check to
  runtime with no benefit.
- **Serialising across the boundary instead of erasing** (the platform holds only bytes and calls
  a per-game function pointer table). This is what a Phase-B dynamic-loading design would need
  anyway, and it is arguably *simpler* than a trait object with eleven methods. Worth one
  paragraph in the Phase-4 ADR; not worth changing course for.

**Do not implement Phase 4 to answer these questions.** The one experiment worth doing early is
tiny and is listed as PR-8 in §28: write `GameAdapter<M>` for the two existing games behind a
`#[cfg(test)]` module in a scratch branch, confirm it compiles and that `Box<dyn ErasedMatch>` can
drive a full tictactoe game, then throw it away and keep the notes. That is a day of work that
de-risks the largest unknown in Phase 4.

---

## 13. Match-actor / multiplayer assessment

```text
Decision            One Tokio task + one bounded mpsc mailbox + one exclusive
                    Box<dyn ErasedMatch> owner per live match (ADR-006, I-14).
Status              CONTRACT ONLY. `tabula-match` is a doc comment plus a ports module.
```

Evaluated against the brief's ten dimensions. This is a **design review of documentation**, since
there is no code.

| Dimension | Assessment |
|---|---|
| **Ordering** | Correct by construction. Single writer expressed as Rust ownership (the actor holds the box; there is no second handle) is the right mechanism and is stronger than a lock. |
| **Fairness** | Unaddressed. One mailbox per match means one match cannot starve another *within* a match, but a Tokio task that spends 50 ms inside `apply` starves the executor for every match on that worker thread. `Ctx::budget` is explicitly "observability, not enforcement". For chess (`max_apply_micros: 2000`) that is fine; for tiles' incremental scoring it is the risk doc 07 Phase 3 already names. **Recommendation: keep the soft budget, but add a per-apply p99 metric from day one and a documented threshold at which a game moves to a dedicated runtime.** |
| **Backpressure** | Bounded mailbox (1024) with `Reject{BUSY}` is right. "No unbounded channels anywhere" is the correct absolute rule. |
| **Crash recovery** | Rehydrate from snapshot + input suffix. Sound *because* of the single ordered input stream. The weak point is the **in-memory-only idempotency cache**, which the docs honestly flag: a crash can let a duplicate re-apply. That is a real correctness hole traded for hot-path latency, and it should be re-examined with a measurement rather than accepted permanently. |
| **Duplicate commands** | `(seat, client_seq)` with a 64-slot forward window and `STALE_SEQ`/`SEQ_TOO_FAR` rejections. Reasonable. The three-counter separation (`client_seq` / `state_version` / `input_index`) into three distinct types is exactly right and is the sort of thing that is impossible to retrofit. |
| **Long-running apply** | Watchdog metric at 5 s, deliberately not auto-killed. Correct: killing loses ordering. |
| **Actor migration between processes** | Deferred behind "ownership lease" (doc 03 §20). Nothing in the current design blocks it. Fencing tokens are named as a Phase-10 prerequisite. |
| **Async game hibernation** | Deferred to Phase 9. The `durable_timers` table with `FOR UPDATE SKIP LOCKED` is the documented bridge, and doc 03 §12.2 correctly notes it is "the only place a database poll drives gameplay". Good discipline. |
| **Process shutdown** | 15 s drain, flush, final snapshot, close 4411. Fine. |
| **Distributed ownership** | Explicitly deferred (ADR-014/015/020 with numeric triggers). Correct for the team size. |

### 13.1 Is task-per-match still the right Phase-4 model?

**Yes.** Nothing found in this audit argues against it, and one thing argues for it more strongly
than the docs do: because `apply` is *synchronous and pure*, the actor's run loop is
`recv().await` → pure call → `append().await`. There is no interior await inside the transition,
which means no cancellation-safety hazard inside the domain logic, which is the single biggest
source of subtle async bugs. That property is a direct dividend of ADR-002 and should be stated as
a reason for ADR-006, not just as a consequence.

### 13.2 Two things to fix in the design before writing code

1. **`Notice` has no `notice_id`** but the documented idempotency key for `Effect::Notify` is
   `(match_id, audience, notice_id)`. Add the field now (§7.3).
2. **Nothing pins the shell's logical-time semantics.** Three harnesses use three different models
   (F-30). Before `MatchActor` exists, the `MatchDriverContract` proposed in §11.3 should fix:
   how `now` is derived, whether `now` is monotone-clamped, whether a rejected input consumes an
   index, and the tie-break between a timer and a player command at the same logical instant
   (self-play currently says **timer wins**; nothing says the actor must agree).

### 13.3 Is Phase 4 ready to start?

**No.** Two contract-validation gaps, both from Phase 3's own exit criteria:

- **The projection security model has never been exercised by a game with secrets** (§8). Phase 3
  exists to find contract flaws here at the cost of a crate change rather than a protocol change.
- **No `Effect` has ever been executed by a shell a user touches** (§7.4). The effect list is a
  Phase-4 load-bearing contract that has been designed twice and validated zero times.

Both are addressable in 3–4 PRs (§28) without starting Phase 4.

---

## 14. Game-by-game architecture stress test

For each: does the generic contract absorb the problem cleanly, and if not, where does the fix
belong?

### 14.1 TicTacToe — the smoke test

**Absorbed cleanly.** State is a validated private struct, seats are match-local (`state.seats`,
not `SeatId(0)`/`SeatId(1)`), `legal_commands` is five lines and unlocks a free bot and self-play.
The `place` → `validate_place` / `commit_place` split makes R2 structural rather than remembered,
and it is the shape every game should copy.

Two observations:

- Its `move_timeout` timer is the only place in the repo where `Input::Timer` has a *game* meaning
  and a default (`MIN_MOVE_TIMEOUT = 5_000`, `0` selects the default). Good. But the mutation run
  shows the `timer == TIMER_MOVE` guard survives mutation **in both directions** — so neither "an
  unknown timer is ignored" nor "TIMER_MOVE forfeits" is tested at the package level (§18).
- `State::from_parts` — the reachability validator that makes "TicTacToe state is reachable" true —
  has **35 surviving mutants**. That documented boundary is much less verified than the ledger in
  `docs/verification/core-domain-boundaries.md` implies.

### 14.2 Chess — the correctness benchmark

**Absorbed cleanly, and it is the strongest evidence in the project.** Specifically:

- Complex legality: pseudo-legal generation filtered by "does not leave my king attacked", checked
  against **published external perft counts** — the only true independent oracle in the repository.
- Clocks: Fischer and Bronstein, with `charge_completed_move` evaluated *after* legality and
  *before* the first mutation, preserving R2. Bounded exhaustive reference-model tests over
  (remaining × elapsed × increment) triples. Explicit `u64::MAX` saturation policy.
- Draw rules: fivefold automatic / threefold claimable, 75-move automatic / 50-move claimable,
  insufficient material, and a genuinely subtle timeout rule (a flagged opponent draws if the
  survivor has no mating material, *including* helpmate material on the flagged side).
- Promotion, castling, en passant: all verified live in §25.
- Replay: two committed goldens, one containing a recorded `Input::Timer`.

**Where the contract creaks:**

| Issue | Where it belongs |
|---|---|
| `State` has public fields and derived `Deserialize`; a decoded snapshot can be an unreachable position (two white kings, castling rights inconsistent with the board). `from_fen` validates; `Deserialize` does not. | **Game state.** Give chess a validated `try_from` boundary as tictactoe has. Flagged as deliberately deferred in `docs/verification/core-domain-boundaries.md`; it becomes load-bearing at Phase-4 `restore_match`. (F-15) |
| Colour↔seat is hard-wired (`White = SeatId(0)`). | Game state, low priority; noted in the same ledger. |
| Five of six `Command` variants are unreachable from the UI (resign, offer/accept/decline/claim draw). | **Presentation contract** — the presenter needs a non-board action surface. This is the first real demand for the "~20 shared widgets" doc 04 promises. |
| The clock exists in rules and in `View` and is invisible everywhere else. | **Presentation + local runtime.** |

**No platform change required.** That is the important result.

### 14.3 Tiến Lên / cards — hidden hands, shuffle, 4 seats

Not implemented. Analysed against the contract:

| Stress | Absorbed by | Verdict |
|---|---|---|
| Hidden hands | `project` + `View` modelling knowledge (`HandSummary { count }`) | Contract is sufficient; **the enforcement is not** (§8). |
| Random shuffle | `ctx.rng.stream(DOMAIN_SHUFFLE)` + pinned Fisher-Yates | Sufficient. **But see §9.1(ii)** — a 2-element shuffle is untested, and a 52-card shuffle's uniformity rests on the untested rejection zone. Cards is the game that makes both of those matter. |
| 4 players | `SeatSpec`, `SeatCounts::range` | Sufficient. |
| Variable legal combinations (singles/pairs/straights/bombs) | `LegalCommands::Enumerated` may be large; `Hints` exists for that case but its shape is an explicit `TODO(phase 3)`. | **Cards is the game that should settle `CommandHint`'s shape** — not tiles, as the TODO says, because cards will hit enumeration size first. |
| Finishing order → standings | `MatchOutcome` with ranks and ties | Sufficient; `new_for_seats` already validates coverage. |
| Derived information leaks | `docs/games/cards.md` information model + human review | **Mechanism is a required document that has never been written for any game.** |
| Deck commitment (hash at start, salt at end) | doc 00 §9.4 rule 3, EXPERIMENT | Fine as an experiment. It needs `Effect`-free implementation (it is just state), so it costs nothing structurally. |

**One capability gap:** cards wants a *per-viewer* `legal_commands`. Today `legal_commands(state,
seat)` already takes a seat, so the signature is fine; the hazard is chess's precedent of copying
the result into `View` (F-16).

### 14.4 Werewolf — simultaneity, phases, event non-existence

Not implemented. The hardest case, and the contract mostly holds.

| Stress | Absorbed by | Verdict |
|---|---|---|
| Simultaneous night actions | Game-owned phase state; `Input::Player` × N; phase closed by `Input::Timer` | Sufficient. Arrival order is recorded and replayed, so it is deterministic. **Hazard:** if an outcome depends on arrival order, players can race. Must be stated in the information model. |
| Phases rather than turns | `TurnModel` + game state | Sufficient. |
| Secret roles | `project` | Sufficient contract, **no enforcement** (§8). |
| Event non-existence | `view_event -> None` | Sufficient contract; **the mutation run shows nothing detects a `view_event` that returns `None` for everything**, and by symmetry nothing detects one that returns `Some` where it should hide. This is the exact failure werewolf exists to stress and it currently has zero detectors. |
| Dynamic chat/voice permissions | `Effect::SetChatScopes` / `SetVoiceScopes`, absolute not delta | Sufficient contract; nothing executes effects (§7.4). |
| Eliminated players | `Viewer::Seat` for a dead player ≠ `Viewer::Spectator` — the enum exists precisely for this, and `viewer.rs` documents it as the motivating case | **Well handled.** This is the clearest example in the codebase of a type shaped by a real requirement. |
| Moderator / system behaviour | `Input::Admin` + game-owned phase logic | Sufficient. |
| No bot substitution | `SubstitutionPolicy` | Sufficient (werewolf declares it forbidden because a seat carries secret knowledge). |

**Werewolf needs one thing the contract does not have:** a way for the *platform* to know that a
phase deadline exists so it can show a countdown, without the game telling it in prose.
`Effect::SetTimer { id, delay }` reaches the platform but never reaches the *client*, because
timers are not events. Chess dodges this by putting the clock in `View`. Werewolf would too. That
is fine — **the answer is "game state / View", not a new `Effect` variant** — but it is worth
recording the rule: *anything the player must see is `View` or `ViewEvent`; `Effect` is for the
platform only.*

### 14.5 Carcassonne-like tiles — growing state, expensive legality

Not implemented.

| Stress | Absorbed by | Verdict |
|---|---|---|
| Growing state | `StateSizeClass` → snapshot cadence | Contract sufficient. |
| Expensive legality | `LegalCommands::Hints` | Shape undecided (`TODO(phase 3)`). |
| Incremental derived structures (union-find feature graph) | `&mut State` reducer + `state_hash` override | **This is the reason ADR-026 kept `&mut State`.** The override contract is right: "the incremental structure must itself be in the hash, or divergence stops being caught." |
| Camera / presentation separation | `Camera2D` in `RenderList`, camera state in `Local` | Sufficient; `Camera2D` already exists and the headless rasterizer honours it. |
| Snapshots | ports drafted | Fine. |
| Async turns (24 h) | `LogicalTime` as `u64` ms + `Effect::SetTimer` | Sufficient — and proven total by the Kani harnesses over the full `u64` domain. |

**One real risk:** `apply_budget.max_apply_micros` is soft. Tiles' incremental scoring is where
that bites. Measure early; do not make the budget hard.

### 14.6 Summary across five games

| Question | Answer |
|---|---|
| Does the generic contract absorb the problems? | **Yes, in every case examined.** No game-ID special case is needed anywhere. |
| Where do the gaps live? | Almost entirely in **enforcement**, not in **contract**: the projection scanner, the effect executor, and `CommandHint`'s shape. |
| Any new `Input` variant needed? | **No.** |
| Any new `Effect` variant needed? | **No** (but `Notice` needs a `notice_id` field). |
| Any presentation-contract gap? | **Yes** — a non-board action surface (resign/draw/claim), and a countdown/clock widget. |
| Any platform-subsystem gap? | **Yes** — the local runtime (§11) and the projection scanner (§8). |

---

## 15. Verification evidence matrix

Evidence levels, weakest to strongest, are **not** interchangeable:

```text
documented              a sentence in a doc or a code comment
type-enforced           the compiler refuses the alternative
statically checked      a lint, a grep, or an xtask gate refuses it
example-tested          fixed inputs, hand-written expectations
property-tested         generated inputs against a law, with shrinking
differentially-tested   compared against an INDEPENDENT implementation or published data
mutation-tested         plausible defects are demonstrably killed by the assertions
bounded-model-checked   exhaustive over a symbolic domain, under stated assumptions and bounds
cross-target-tested     identical results on another architecture/target
production-observed     seen to hold on real traffic
```

`—` means no evidence at that level. **Nothing in this table is described as "verified".**

| Invariant / claim | doc | type | static | example | property | differential | mutation | BMC | cross-target | prod |
|---|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| **I-1** Layer-1 crates cannot reach tokio/macroquad/sqlx/leptos/getrandom | ✔ | — | ✔ (`check-deps`, resolved graph; NC-1) | ✔ | — | — | — | — | — | — |
| **I-2** `apply` is deterministic | ✔ | — | partial (clippy in 6 of 26 crates; F-08) | ✔ (`determinism_same_inputs`) | — | — | — | — | — | — |
| I-2 at scale (chess) | ✔ | — | — | ✔ | pseudo-random self-play (8 300 matches / 3.1M inputs) | — | — | — | — | — |
| **I-3** no wall clock in rules | ✔ | — | ✔ (`disallowed-types`, rules crates only) | ✔ | — | — | — | — | — | — |
| Logical-time arithmetic is total (no wrap) | ✔ | — | — | ✔ | — | ✔ (`checked_mul`/`checked_add` oracle) | ✔ (caught) | **✔ full `u64` / `u64²` domain, unbounded** | — | — |
| **I-4** all randomness from `DetRng` | ✔ | — | ✔ (dep ban) | ✔ | — | — | — | — | — | — |
| `DetRng` stream stability (frozen) | ✔ | — | — | ✔ (vectors + documented preimage) | — | — | ✔ (NC-8 caught) | — | — | — |
| `DetRng::below` is unbiased (rejection zone correct) | ✔ | — | — | **—** | — | — | **✘ mutant survives** | — | — | — |
| `DetRng::below` terminates | ✔ ("# Panics Never") | — | — | — | — | — | **✘ 3 mutants hang** | — | — | — |
| `DetRng::shuffle` is a uniform permutation | ✔ | — | — | partial (permutation-ness only) | — | — | **✘ len-2 identity mutant survives** | — | — | — |
| Substreams are independent | ✔ | — | — | ✔ | — | — | ✔ | — | — | — |
| **R2** rejection is byte-transactional | ✔ | — | — | ✔ (non-opt-in, all games; NC-3b) | — | — | ✔ (caught) | ✔ **2 concrete tictactoe states × symbolic (seat, cell)** | — | — |
| **R8** rejection disturbs no later RNG | ✔ | ~ (structural: per-index derivation) | — | ✔ | — | — | ✔ | — | — | — |
| **R3** no panic on hostile input | ✔ | — | — | partial (fixture `invalid_command`) | — | — | — | — | — | — |
| R3 at scale | ✔ | — | — | — | self-play hostile injection @5% | — | — | — | — | — |
| **R5/I-5** projection hides secrets | ✔ | partial (`View` is `Serialize`-only; `compile_fail` doctest) | — | **—** | **—** | — | — | — | — | — |
| **R6/I-6** `view_event` is the only path | ✔ | partial | — | **—** | — | — | **✘ `view_event -> None` survives** | — | — | — |
| **I-7** `state_version` +1 per applied input | ✔ | — | — | ✔ (testkit `run`) | — | — | — | — | — | — |
| **I-8** replay reproduces state hashes | ✔ | — | — | ✔ (3 goldens) | — | ✔ (independent final-hash literals) | — | — | **—** | — |
| **I-9** no platform crate names a game | ✔ | — | ✔ (`check-no-game-ids`; NC-2) | ✔ | — | — | — | — | — | — |
| **I-10** presentation state never enters canonical state | ✔ | ~ (dependency direction) | — | — | — | — | — | — | — | — |
| **I-13** wire change ⇒ version bump | ✔ | — | — | — | — | — | — | — | — | — |
| **I-14** one owner per match | ✔ | — | — | — | — | — | — | — | — | — |
| **I-15** no leptos in `apps/game-client` | ✔ | — | ✔ (`check-deps` + `forbid`) | — | — | — | — | — | — | — |
| **I-16** versioned/migratable state | ✔ | — | — | ✔ (`Unsupported` default; `ENCODING_VERSION` decode check) | — | — | — | — | — | — |
| Chess legal-move generation | ✔ | — | — | ✔ | — | **✔ published perft, 4 positions; depth 5 `--ignored`** | ✔ (NC-9: perft was the *only* detector) | — | — | — |
| Chess clock arithmetic | ✔ | ~ (`Millis` newtype) | — | ✔ | — | ✔ (bounded exhaustive reference model) | — | — | — | — |
| Canonical encoding stability | ✔ | — | — | ✔ (byte-literal vector) | — | ~ (preimage rebuild; not for the constants — F-21) | ✔ (NC-8b) | — | — | — |
| `state_hash` domain separation by rules version | ✔ | ✔ (typed `RulesVersion` param) | — | ✔ | — | — | — | — | — | — |
| Asset byte integrity (size + BLAKE3) | ✔ | ✔ (`VerifiedAssetBytes` witness, private ctor) | — | ✔ | — | — | ✔ (13 caught, 0 missed on `integrity.rs`) | — | — | — |
| Asset density resolution is deterministic | ✔ | ✔ (private variant repr) | — | ✔ | — | — | ✔ (NC-6 caught) | — | — | — |
| Asset pack build is reproducible | ✔ | — | — | ✔ (`repeated_builds_have_identical_observable_output`) | — | — | — | — | — | — |
| `.tbr` container rejects hostile input | ✔ | ✔ (`ValidatedReplay` after `ReplayDraft::validate`) | — | ✔ (corruption, truncation, seed-in-projected) | — | — | — | — | — | — |
| Design tokens are the only colour source | ✔ | — | ✔ (`check-no-raw-colors`; NC-10b caught, NC-11 missed) | ✔ | — | — | — | — | — | — |
| Generated artefacts are current | ✔ | — | ✔ (CI diff gate) | ✔ | — | — | — | — | — | — |
| WASM builds | — | — | ✔ (CI `cargo check`) | — | — | — | — | — | ~ (**builds and now demonstrably runs**, §25) | — |
| WASM produces identical state hashes | ✔ (doc 00 §5.1) | — | — | — | — | — | — | — | **✘ never attempted** | — |
| aarch64 produces identical state hashes | ✔ | — | — | — | — | — | — | — | **✘ never attempted** | — |

### 15.1 Reading the matrix

**Strongest columns:** *example-tested* and *statically checked*. The repository is unusually good
at both, and the static gates are genuinely mechanical rather than aspirational (NC-1, NC-2, NC-6,
NC-7a, NC-8, NC-10b all fired).

**Empty columns:** *property-tested* (entirely empty — F-01), *cross-target-tested* (entirely
empty), *production-observed* (no production, correctly).

**The two rows that matter most and have the least:** projection secrecy (R5/I-5) and cross-target
determinism. Both are load-bearing product claims.

---

## 16. Kani proof audit

### 16.1 Inventory

Six harnesses, in two crates, all passing:

| Harness | Crate | Symbolic domain | Bounds | Stubs | Solver time |
|---|---|---|---|---|---|
| `millis_from_secs_is_exact_or_saturates` | `tabula-core` | `seconds: u64` — **2⁶⁴** | none | none | 18.4 s |
| `logical_time_plus_is_exact_or_saturates` | `tabula-core` | `(LogicalTime, Millis)` — **2¹²⁸** | none | none | 0.11 s |
| `logical_time_since_never_wraps` | `tabula-core` | `(LogicalTime, LogicalTime)` — **2¹²⁸** | none | none | 0.24 s |
| `concrete_opening_place_is_accepted` | `tictactoe` | **none — fully concrete** | n/a | none | 16.0 s |
| `rejected_initial_place_preserves_state` | `tictactoe` | `(SeatId(u8), cell u8)` — **2¹⁶**, over **one concrete state** | implicit loop unwinds | `commit_place` → verifier-only stub | 14.5 s |
| `rejected_second_place_preserves_state` | `tictactoe` | same, over **one concrete state after one concrete move** | implicit | same stub | 22.7 s |

### 16.2 Per-harness interrogation

**`millis_from_secs_is_exact_or_saturates`.**
*Proposition proved:* for every `u64` `s`, `Millis::from_secs(s) == Millis(s*1000)` when
`s.checked_mul(1000)` is `Some`, and `Millis(u64::MAX)` otherwise.
*Domain:* full `u64`. *Bounds:* none — the function is branchless. *Production code reached:* yes,
the real `from_secs`. *Stubs:* none. *Vacuity risk:* nil — there are no `assume`s.
*Oracle independence:* `checked_mul` is a different operation from `saturating_mul`; adequate.
**Verdict: genuinely valuable.** No test can cover 2⁶⁴ inputs; this is the correct tool.

**`logical_time_plus_is_exact_or_saturates`** and **`logical_time_since_never_wraps`.**
Same shape over 2¹²⁸ pairs, plus a monotonicity postcondition (`result >= original`, `result.0 <=
now.0`). **Genuinely valuable**, and they cost 0.35 s combined.

**`concrete_opening_place_is_accepted`.**
*Proposition:* one concrete move from one concrete state succeeds and produces the expected board
and turn. *Symbolic domain: empty.* **This is an ordinary unit test running under CBMC at 16 s
instead of under `cargo test` at 3 µs.** It buys nothing Kani-specific. It is not harmful (it does
sanity-check that the real `Outcome`/`SmallVec` path is CBMC-tractable, which the stubbed harnesses
avoid), but it should be labelled as a *tractability canary*, not as a proof.

**`rejected_initial_place_preserves_state` / `rejected_second_place_preserves_state`.** These are
the interesting ones and they need careful reading.

- *Proposition actually proved:* "For the concrete state `State::new([SeatId(7), SeatId(42)],
  5000)` (respectively, that state after `place(SeatId(7), 0)`), for **all** `seat: u8` and
  **all** `cell: u8`, if `place` returns `Err` then every canonical field of `State` is unchanged."
- *Symbolic domain:* 2⁸ × 2⁸ = **65 536** combinations, over **one** state.
- *Bounds:* none declared; loops inside `State`/`has_line` are unwound and CBMC reported unwinding
  progress for `slice::Iter::fold` and `any` up to 8–9 iterations, consistent with a 9-cell board.
  No `--default-unwind` is set, so Kani's unwinding assertions are the guard.
- *Stubs:* `commit_place` is replaced by `commit_place_verification_stub`. **This does not weaken
  the stated proposition**, because on the rejected path `validate_place` returns before
  `commit_place` is ever called. The stub only removes CBMC work from the accepted path, which the
  proposition does not constrain. The code comment says exactly this, correctly.
- *Vacuity:* low. There is no `kani::assume`, so the harness cannot be over-constrained that way.
  But the *reachability* question is unaddressed: nothing asserts that the `Err` branch is
  reachable at all. If a refactor made `place` infallible, both harnesses would pass **vacuously**.
  **There is no `kani::cover` anywhere in the repository.** Adding
  `kani::cover!(result.is_err())` and `kani::cover!(result.is_ok())` to these two harnesses is a
  two-line change that converts "the assertion never fired" into "the assertion fired and the
  other branch also exists".
- *Drift:* the field-by-field comparison `canonical_state_fields_equal` destructures `State`
  exhaustively, so **adding a field breaks compilation until someone reviews it**. That is a
  deliberate and excellent anti-drift device and should be copied wherever a proof mirrors a type.

### 16.3 The honest scope statement

The tictactoe R2 harnesses should be described as:

> Harnesses `rejected_initial_place_preserves_state` and `rejected_second_place_preserves_state`
> prove that, for two specific reachable TicTacToe states, `place` leaves all five canonical
> `State` fields unchanged on every rejected `(seat, cell)` pair in `u8 × u8`, with `commit_place`
> replaced by a verifier-only model that is unreachable on the rejected path. They do **not** prove
> R2 for arbitrary reachable states, for `apply` as a whole, for other command variants, or at the
> canonical-byte level.

The code comments already say most of this. What is missing is the same discipline **outside** the
code: nothing in `AGENTS.md`, `justfile`, or the docs restates the scope, so "we have Kani proofs
of R2" is the sentence that will survive into a summary.

### 16.4 Is Kani the right tool for the tictactoe harnesses?

**Partly no.** A 65 536-case exhaustive loop test over one concrete state runs in microseconds:

```rust
#[test]
fn rejected_place_preserves_every_canonical_field_exhaustively() {
    for seat in 0..=u8::MAX {
        for cell in 0..=u8::MAX {
            let mut state = initial_state();
            let before = canonical_encode(&state).unwrap();
            if place(&mut state, SeatId(seat), cell).is_err() {
                assert_eq!(before, canonical_encode(&state).unwrap());
            }
        }
    }
}
```

That version is **stronger** than the Kani harness in one respect (it compares *canonical bytes*,
which is the actual R2 statement, not five hand-listed fields) and **weaker** in another (it fixes
the state). It also does not need the stub, does not need `-Z stubbing`, and runs in CI.

**The Kani harness earns its keep only if the state itself becomes symbolic.** That is the version
worth building: `State` has a 9-cell board of `Option<Mark>` (3⁹ = 19 683 boards), two seats, a
turn, a status, and a `u64` timeout — small enough that a symbolic board plus a symbolic
`(seat, cell)` is plausibly tractable and would prove R2 over *representable* states, and (with an
added reachability predicate as `kani::assume`) over *reachable* ones. That is a real proof no test
loop can replace.

**Recommendation:**
- Keep the three `tabula-core` time harnesses unchanged; they are exemplary.
- Add `kani::cover!` to the two R2 harnesses.
- Add the exhaustive byte-level loop test as the *cheap* R2 evidence, running in ordinary CI.
- Upgrade one Kani harness to a symbolic board, and state the assumption set explicitly.
- Retire or relabel `concrete_opening_place_is_accepted`.

### 16.5 Two Kani harnesses that should exist and do not

1. **`below_rejection_zone_is_the_largest_multiple`** in `tabula-core`:
   for every `n: u32` with `n >= 2`, `zone = 2^32 - (2^32 % n)` satisfies `zone % n == 0`,
   `zone > 0`, and `2^32 - zone < n`. This is the property the doc comment claims, it is currently
   **unasserted at any level**, and mutation testing shows it is broken by a one-character edit
   (§9.1(i)). It is also the guarantee that makes the unbounded rejection loop terminate — three
   mutants **hung the test runner**, which is a liveness hazard for a server that runs `apply`
   inside a Tokio task. Full `u32` domain, no bounds needed, no stubs.
2. **`state_version_increments_exactly_once`** — deferred to Phase 4 when a driver exists.

Priority 1 is the single highest-value Kani harness not yet written in this repository.

### 16.6 Cost

Total solver time for all six harnesses: **~72 s**, plus compilation. That is nightly-scale, not
per-PR scale, and `justfile` correctly keeps Kani out of `cargo xtask check`.

---

## 17. Property testing audit

### 17.1 The finding

**There are zero property tests in this repository.**

```text
$ grep -rn "proptest!\|prop_assert" --include='*.rs' . | grep -v ./target | wc -l
0
$ grep -rn "strategies::" --include='*.rs' . | grep -v ./target
(no matches)
```

`proptest 1.11` is a dependency of `tabula-testkit`. It is used only inside
`crates/tabula-testkit/src/strategies.rs`, whose three public functions are placeholders:

```rust
pub fn input_sequence(_cfg: SeqCfg) -> impl Strategy<Value = Vec<()>> {
    // Placeholder so the module type-checks; replace with the real generator.
    proptest::collection::vec(proptest::strategy::Just(()), 0..1)
}
pub fn roster(_min: u8, _max: u8) -> impl Strategy<Value = SeatRoster> {
    proptest::strategy::Just(SeatRoster::new(SmallVec::new()).expect("..."))
}
pub fn config<T: Default + Debug>() -> impl Strategy<Value = T> {
    proptest::strategy::LazyJust::new(T::default)
}
```

`input_sequence` yields a vector of zero or one unit values. `roster` yields an empty roster.
Nothing calls any of them.

### 17.2 What claims this contradicts

| Source | Claim |
|---|---|
| doc 00 §7, I-2 enforcement column | "`proptest` determinism harness in `tabula-testkit`, run for every game crate" |
| doc 00 §7, I-7 enforcement column | "Assertion in the match actor + property test" |
| `.github/workflows/ci.yml`, `test` job | "Unit + property + replay" |
| `crates/tabula-testkit/src/lib.rs` module map | "`strategies` — `proptest` generators for inputs, rosters, configs" |
| doc 07 Phase 1 tests-required | "clock arithmetic property tests" |

None of these is true today.

### 17.3 What actually plays the role property tests would

Two things, and they are genuinely good — which is why the gap has gone unnoticed:

1. **`tabula-testkit::selfplay`** is a hand-rolled, deterministic, pseudo-random sequence
   generator with hostile injection, invariant checking after every step, and a full replay-and-
   compare. Functionally it *is* a state-machine property test. What it lacks: **shrinking**. When
   a 400-input chess match fails, self-play reports `(base_seed, match_index, input_index)` — good
   reproduction coordinates, but the developer still has a 400-input sequence, not the 3 inputs
   that mattered. `strategies.rs`'s own doc comment names shrinking as the reason proptest was
   chosen over quickcheck, and then does not use it.
2. **Bounded exhaustive reference-model loops**, e.g.
   `fischer_charge_matches_bounded_reference_model` iterating 32 × 41 × 9 triples against an
   independently written expected value. This is *differential* testing done by hand, and for
   small domains it is strictly better than a property test (no sampling, no flakes). Keep it.

### 17.4 Where property tests would actually pay, ranked

The point is not "add proptest because it is missing" — it is that five specific laws in this
codebase have no adequate oracle at any other level.

| # | Property | Why nothing else covers it | Generator |
|---|---|---|---|
| 1 | **Projection noninterference.** For a reachable state `s` and a secret-scramble `s'` that differs only in data viewer `v` may not see, `project(s, v)` and `project(s', v)` are byte-identical. | This is the *only* mechanical way to catch derived leaks. A containment scan cannot. §8.4. | reachable states from a self-play prefix; `SecretModel::scramble` |
| 2 | **Rejected apply leaves canonical bytes unchanged, over reachable states.** | Today R2 is checked on fixture rejections and self-play rejections; neither explores the state space with shrinking. Kani covers two concrete states. | state-machine: legal prefix + arbitrary hostile input |
| 3 | **Ordered input streams yield identical state hashes.** Two independent runs of the same generated sequence. | `determinism_same_inputs` does this for **one** fixture script per game. | generated sequences via `legal_commands` |
| 4 | **Replay equivalence.** live-evolve(seq) == replay(record(seq)) for generated `seq`. | The three golden `.tbr` files are the entire evidence today. | as above |
| 5 | **Deterministic resource resolution.** `resolve(asset, density)` agrees with a naive reference resolver over generated manifests. | The two hand-written laws caught NC-6, but the manifest space (multi-variant, mixed density modes, duplicate detection) is large. | generated manifests |
| 6 | **`DetRng::below` uniformity bound.** Over generated `n`, the rejection zone is the largest multiple of `n` ≤ 2³². | Nothing at all today; a mutant survives (§9.1). Kani is even better here (§16.5). | `n: u32` |
| 7 | **`legal_commands` ⊆ accepted-by-`apply`, over reachable states.** | Checked at exactly two states per game (initial and post-script). | state-machine |

### 17.5 `arbitrary State` vs `reachable State` — the distinction that matters here

Both are useful, for **different invariants**:

- **Arbitrary (representable) state** is the right generator for *robustness* claims: `apply` must
  not panic; `canonical_decode` must reject garbage; `project` must not index out of bounds. Chess
  `State` today has public fields and derived `Deserialize`, so arbitrary states are *constructible
  from the wire* — which is exactly why the robustness claim matters (F-15).
- **Reachable state** is the right generator for *semantic* claims: R2, invariant preservation,
  projection noninterference, `legal_commands` soundness. Asserting a semantic law over an
  unreachable state (three white kings) proves nothing and produces false failures.

**The cheap way to generate reachable states in Tabula is already built:** run a self-play prefix
of generated length and take the resulting state. That gives reachable states for free for any game
that implements `legal_commands`, which both shipped games do. `strategies.rs`'s own TODO says
exactly this ("drive the legal fraction through `legal_commands`") and it is the right plan.

### 17.6 Properties to avoid

Two shapes that would look productive and prove nothing:

- **Re-implementing the transition in the test.** A property that computes the expected board by
  calling the same move generator is not an oracle. Chess already avoids this correctly by using
  perft.
- **`state_hash(s) != state_hash(s')` for generated distinct `s`, `s'`.** That is a BLAKE3 property,
  not a Tabula property.

### 17.7 State-machine testing

`proptest` 1.11 is in the tree; `proptest-state-machine` is **not**. Its `ReferenceStateMachine` /
`StateMachineTest` / `prop_state_machine!` trio maps almost exactly onto Tabula's shape
(`Transition` = `Input<Command>`, `check_invariants` = the conformance assertions, shrinking
removes transitions from the end and simplifies them from the front). Adding it is a `deps.toml`
row plus a dev-dependency.

**Recommendation:** do **not** add it in the first property-testing PR. Write properties 2, 3, and
7 with plain `proptest!` over generated `Vec<Input<Command>>` first, because that reuses
`testkit::determinism::run` unchanged. Reach for `proptest-state-machine` only if the hand-rolled
sequence generator's shrinking proves inadequate — and record that decision, because "we added a
framework" is exactly the failure mode doc 09 §6 risk #5 names.

---

## 18. Mutation testing audit

Two real campaigns were run for this audit, on the two crates where the tests are supposed to be
strongest.

### 18.1 `tabula-core` — 133 mutants, 3 minutes

```text
69 caught · 30 missed · 31 unviable · 3 timeouts
```

Survivors classified (the classification scheme is the one required by the brief):

| Survivor | Class | Note |
|---|---|---|
| `rng.rs:190` `replace - with + in DetRng::below` | **REAL TEST GAP** | Reintroduces modulo bias. Confirmed by independent computation: for `n = 6` the original rejects 4 of 2³² draws, the mutant rejects 0. The doc comment's entire rationale for rejection sampling is unasserted. |
| `rng.rs:193` `replace < with <= in DetRng::below` | **REAL TEST GAP** | Admits `x == zone`, adding one extra `0` outcome. Same root cause. |
| `rng.rs:208` `replace < with ==` / `<=` in `shuffle` guard | **REAL TEST GAP** | Makes a **2-element shuffle the identity**. Confirmed independently: lengths 0 and 1 are unaffected (`(1..len).rev()` is empty), length 2 returns early. `shuffle_is_a_permutation` cannot see it — the identity *is* a permutation. |
| `rng.rs:208` `replace \|\| with &&` in `shuffle` guard | **EQUIVALENT / UNREACHABLE** | The `u32::try_from` arm requires a slice longer than 2³², unreachable. |
| `seat.rs:137` `replace == with != in SeatRoster::get` | **REAL TEST GAP** | `get(seat)` would return a *different* seat. Used by `ChessModule::validate_config`. |
| `seat.rs:122/127/132` `as_slice`/`len`/`is_empty` accessors | **REAL TEST GAP (low)** | `len` feeds seat-count validation in both games. |
| `outcome.rs:229` `standings -> empty` | **REAL TEST GAP** | The accessor ratings will read. |
| `viewer.rs:64` `Viewer::seat -> None` | **REAL TEST GAP, cross-package** | `Viewer::seat()` sets `View.you` in tictactoe's `project`. Downstream tests would catch it; `--package tabula-core` does not run them. A textbook example of package-scoped blindness. |
| `ids.rs:218/240/255` semver validation predicates | **REAL TEST GAP (low)** | `1.0.0+a+b` would be accepted. |
| `ids.rs:128/184/206/212` `as_str`/`Display`/`From` | **LOW-VALUE** | Pure accessors. |
| `rng.rs:101` `MatchSeed::fmt -> Ok(())` | **REAL TEST GAP (security-relevant), weak mutant** | The mutant still redacts, so it proves little — but its survival reveals that **no test asserts `format!("{:?}", seed)` does not contain the seed bytes**. That redaction is a stated security property ("a seed in a log line is a leaked deck"). Worth a 3-line test. |
| `time.rs:142/155/170/174` (4 mutants) | **VERIFIER-ONLY** | Inside `#[cfg(kani)] mod verification`. Not compiled in a test build, so any mutation there survives trivially. |
| `rng.rs:62` `<< → >>`; `rng.rs:193` `< → ==` / `< → >` (3 timeouts) | **TOOL SIGNAL, worth acting on** | All three make `DetRng::below`'s rejection loop non-terminating. This is evidence that `below` contains an **unbounded loop whose termination depends on an unproven invariant** (`zone > 0`), inside a function documented "# Panics — Never." On a server this is a hung Tokio task, and doc 03's watchdog explicitly does *not* auto-kill. |

**Adjusted survival rate excluding verifier-only noise: 26 / 129 ≈ 20 %.**

### 18.2 `games/tictactoe` — 198 mutants, 3 minutes

```text
75 caught · 80 missed · 43 unviable · 0 timeouts
```

| Cluster | Count | Class | Note |
|---|---:|---|---|
| `verification::*` (`#[cfg(kani)]` module) | **20** | **VERIFIER-ONLY** | Includes nonsense like `Outcome::new()` and `Outcome::from(Default::default())` "surviving" — they are never compiled. 25 % of all survivors. |
| `State::from_parts` predicates (lines 81–147) | **~35** | **REAL TEST GAP** | Every `\|\|`↔`&&`, `==`↔`!=`, `+`↔`-`/`*` in the reachability validator survives. This is the function that makes `docs/verification/core-domain-boundaries.md`'s claim "Tic-tac-toe state is reachable" true. **The claimed boundary has almost no predicate-level evidence.** |
| `bot.rs` (`choose`, `mark_for_turn`, `other`, `completion`, `think_time`, `level`) | **19** | **REAL TEST GAP** | Bots have no unit tests. `choose -> None` survives. Bots are the primary fuzz driver: a degenerate bot silently weakens self-play everywhere. Caught only in `xtask`, not in the package. |
| `apply`'s `timer == TIMER_MOVE` guard (both directions) | 3 | **REAL TEST GAP** | Neither "unknown timer ignored" nor "TIMER_MOVE forfeits" is tested at package level. |
| `view_event -> None` | 1 | **REAL TEST GAP (security-adjacent)** | Nothing notices a game that hides every event. The inverse — revealing what should be hidden — has no detector either (§8.3). |
| `describe -> Default::default()` | 1 | **LOW-VALUE** | Already returns `unsupported()`. |
| `GameModule::bot -> None` | 2 | **REAL TEST GAP (low)** | |
| `move_timeout` delete match arm 0 | 1 | **REAL TEST GAP (low)** | The documented "0 selects the default" behaviour. |

**Adjusted survival rate excluding verifier-only noise: 60 / 178 ≈ 34 %.**

### 18.3 The `cfg(kani)` blind spot — and the verified fix

cargo-mutants parses source, so `#[cfg(kani)]` blocks are mutated even though they are never
compiled into the test build. Every such mutant survives trivially. Combined across the two
campaigns: **24 of 110 survivors (22 %) are pure noise**, and they cluster in exactly the modules
an engineer is most likely to think are well verified.

Verified fix, measured during this audit:

```toml
# .cargo/mutants.toml
exclude_re = ["verification::"]
```

`cargo mutants --package tabula-game-tictactoe --list` drops from 198 to 179 mutants and reports
zero `verification::` entries. Recommended, with the caveat that the regex is a naming convention:
it works because both crates put Kani harnesses in a module literally called `verification`. Keep
that convention and document it.

### 18.4 Existing campaign artefacts

`mutants.out/` and `mutants.out.old/` are present in the working tree and correctly gitignored.
The last committed-era campaign covered **20 mutants** in `tabula-assets/src/integrity.rs`
(13 caught, 0 missed, 7 unviable) — a well-scoped, high-value run against the newest security-
relevant code. That is exactly the right way to use the tool, and it is the pattern the new skill
should teach.

### 18.5 Highest-value regression tests to write from these survivors

Ranked. Together these are roughly 60 lines.

1. `below_rejection_zone_is_the_largest_multiple_not_exceeding_2_32` (kills 2 survivors + 3
   timeouts; also the Kani harness in §16.5).
2. `shuffle_of_two_elements_produces_both_orders_across_seeds` (kills 2 survivors).
3. `seat_roster_get_returns_the_requested_seat` and `len`/`is_empty`/`as_slice` assertions (kills 4).
4. `match_seed_debug_never_contains_seed_bytes` (kills 1; security-relevant).
5. `tictactoe_from_parts_rejects_each_unreachable_class` — a table test with one row per predicate
   (kills ~35, and is the correct evidence for the documented reachability boundary).
6. `tictactoe_bot_chooses_a_legal_cell_and_prefers_a_win_then_a_block` (kills ~19).
7. `unknown_timer_is_ignored` / `move_timer_forfeits_the_side_on_turn` (kills 3).
8. `view_event_returns_some_for_every_event_variant` (kills 1; the tictactoe contract is "nothing
   is secret", so this is the correct assertion for *this* game).

### 18.6 What mutation testing is and is not evidence for

It measures **assertion strength**, not correctness. A 100 %-killed crate can still be wrong about
its specification — NC-9 is the proof: removing chess's castling-through-check guard is a *real
bug* that conformance, determinism, and 200 self-play matches all missed, and only perft caught.
Mutation testing on `games/chess` would likewise not have caught it unless perft were in the test
set. Use it to find *unasserted behaviour*, then choose the right oracle for that behaviour.

---

## 19. Fuzzing opportunities

### 19.1 Current state

There is **no `fuzz/` directory**. `.github/workflows/nightly.yml` nevertheless contains:

```yaml
- run: cargo fuzz run protocol_decode -- -max_total_time=600
- run: cargo fuzz run command_decode  -- -max_total_time=600
```

Neither target exists, and `tabula-protocol` has no code. That job cannot have succeeded. See
F-10.

### 19.2 Where fuzzing is the *right* tool today

The test is: does fuzzing give a **different oracle** than a property test could, and does the
target consume **untrusted bytes**?

| Target | Untrusted? | Different oracle? | Verdict |
|---|---|---|---|
| **`ValidatedReplay::read` / the `.tbr` decoder** | **Yes.** doc 05 treats replays as distributable artefacts; a support engineer will open a user-supplied `.tbr`. | **Yes** — the interesting failures are resource exhaustion (a zstd bomb, a declared 100 000-frame count), not logical mismatch. A round-trip property cannot generate a *malformed* container. | **DO IT NOW.** The decoder already has every cap a fuzz target wants to attack: `MAX_DECOMPRESSED_REPLAY_BYTES`, `MAX_COMPRESSED_REPLAY_BYTES`, `MAX_HEADER_BYTES`, `MAX_FRAME_BYTES`, `MAX_FRAME_COUNT`, `MAX_CONFIG_BYTES`, `MAX_ZSTD_WINDOW_SIZE`, plus a CRC32 trailer. Those caps are asserted by hand-written tests; a fuzzer is how you find the eighth one nobody wrote. |
| **`AssetPackManifest::from_toml`** | **Yes** — a pack manifest is served by a CDN and doc 04 §12 treats it as data. | **Yes** — TOML parsing plus 15 validated newtypes plus duplicate/uniqueness checks is a grammar, and grammars are what libFuzzer is for. | **DO IT** (second priority). |
| **`canonical_decode::<T>`** for each canonical type | **Yes** — snapshots and log payloads at Phase 4. | Partially. Postcard is non-self-describing, so most garbage fails fast; the interesting cases are length-prefixed collections claiming huge lengths. | **DO IT at Phase 4**, targeted at `State`/`Input`/`Event` per game. |
| **`AssetPath` / `AssetRef` validation** | Yes | No — the input space is short strings; a property test with a naive reference validator is a *better* oracle because it checks acceptance as well as non-panic. | **Property test, not fuzz.** |
| **`ChessRules::apply` with arbitrary `Command`** | Yes at Phase 4 | No — `Command` is a typed enum with `u8` fields; the reachable input space is small and self-play already injects hostile commands with a state-aware generator. | **Property/self-play, not fuzz.** |
| **Protocol decoder** | Yes | Yes | **Phase 4**, exactly as the nightly job intends. |

### 19.3 Recommended first fuzz PR (small)

```text
fuzz/
  Cargo.toml                 # cargo-fuzz workspace, excluded from the main workspace
  fuzz_targets/
    replay_container.rs      # ValidatedReplay::read(bytes) -> must not panic, must bound memory
    asset_manifest.rs        # AssetPackManifest::from_toml(&String::from_utf8_lossy(data))
  corpus/replay_container/   # seeded from tests/replays/*.tbr
  corpus/asset_manifest/     # seeded from the manifests in tabula-assets' tests
```

With:

- **Corpus strategy:** seed from the three committed `.tbr` files and every manifest literal in
  `tabula-assets`' tests. Commit the *minimised* corpus, not the raw one.
- **Dictionary:** worth it for the manifest target (`pack`, `version`, `game`, `files`, `path`,
  `hash`, `bytes`, `priority`, `density`, `resources`, `variants`, `region`). Not worth it for the
  binary container — the magic `TBR1` and the little-endian length prefixes are enough structure
  that libFuzzer finds them quickly.
- **Structure-aware generation:** *not* for these two targets. The whole point is malformed input.
  (`arbitrary`-based generation would be right for a future `Input<Command>` target.)
- **Crash minimisation:** `cargo fuzz tmin`, then commit the minimised input as an ordinary
  `#[test]` fixture under `crates/tabula-testkit/tests/` — **the regression must live in the
  deterministic suite, not in the fuzz corpus**, so it runs on every PR.
- **Resource bounds:** set `-rss_limit_mb` and `-max_len`; "does not panic" is insufficient when a
  200-byte input can allocate 4 MiB.
- **Placement:** nightly only, `-max_total_time=600` per target, plus `cargo fuzz run <t> -runs=0`
  in PR CI to confirm the targets still *build*. A fuzz target that stops compiling is the most
  common way fuzzing dies.

### 19.4 What not to fuzz

Do not fuzz `DetRng`, `state_hash`, `LogicalTime`, or the chess move generator. They are pure
functions over small typed domains with better oracles available (frozen vectors, Kani, perft).
Fuzzing them would burn nightly minutes to rediscover nothing.

---

## 20. Differential and model-based testing opportunities

This is where Tabula's largest *unclaimed* verification value sits. The pattern already works in
the repository — perft and the clock reference models are both differential — and it generalises.

### 20.1 TicTacToe — exhaustive, not sampled

TicTacToe's entire reachable game tree is **5 478 positions** and 255 168 complete games. Both are
trivially enumerable.

**Recommended:** a tiny reference model in `games/tictactoe/tests/` — a plain `[Option<Mark>; 9]`
with `winner()` and `moves()` written in the most naive way possible — and an exhaustive
cross-check of `TicTacToeRules::apply` against it over the **whole game tree**:

```text
for every reachable position p (DFS from empty, ~5 478):
    assert legal_commands(p) == naive_free_cells(p)  when playing
    for every cell 0..=8 (legal and illegal):
        real = apply(clone(p), Place{cell})
        model = naive_apply(p, cell)
        assert same acceptance, same resulting board, same terminality
        on rejection: assert canonical bytes unchanged
```

This costs a few hundred milliseconds and **subsumes** both Kani harnesses, the state-machine test,
and most of the 35 `from_parts` mutation survivors — with a genuinely independent oracle. It is the
single highest value-per-line verification artefact available in this repository.

*Do not use Kani where a finite exhaustive model is possible.* TicTacToe is that case.

### 20.2 Chess — perft, and one more oracle

**Perft is already the strongest evidence in the project**, and NC-9 proved it is the *only*
detector for a legality regression. Current coverage: 4 positions, depths 3–4, plus depth 5 for the
initial position behind `--ignored`.

Improvements, in value order:

1. **Run the `--ignored` depth-5 perft in nightly.** It exists and nothing schedules it.
2. **Add the two remaining standard positions** (CPW positions 5 and 6) and raise Kiwipete to
   depth 4 (4 085 603) in nightly. Cheap, and Kiwipete depth 4 is where most castling/EP bugs die.
3. **Divide-perft on mismatch.** When a count differs, per-move subtotals localise the bug in one
   run instead of a bisect. ~30 lines in `xtask perft`.
4. **An independent legality oracle in dev-dependencies only.** The brief asks whether an external
   chess library can be used as an oracle without entering production deps. It can:
   `[dev-dependencies]` of `games/chess` is not in the server's or client's graph, and
   `check-deps` reads the resolved graph for normal deps. **But this audit recommends against it
   for now**, for two reasons: (a) perft *is* an independent oracle already — the node counts come
   from an external authority, not from our algorithm; (b) adding a chess crate to the workspace
   invites `cargo deny` and MSRV churn for a marginal gain over deeper perft. Revisit only if a
   legality bug ever escapes perft.
5. **FEN round-trip property.** `from_fen(to_fen(state)) == state` over reachable states. `to_fen`
   does not currently exist; adding it costs ~40 lines and pays for itself in debugging.

### 20.3 Asset resolution — a naive reference resolver

`BoundAssetPack::resolve` selects a density variant by `min_by_key((|d - target|, Reverse(d)))`.
The two hand-written laws caught NC-6. A reference resolver — sort all variants, linear scan,
pick by explicit comparison — plus a property test over generated manifests would cover the
combinatorics (mixed density modes, 1/2/3 variants, duplicate detection, density-independent
resources) that two examples cannot. ~60 lines. Good second property-testing PR.

### 20.4 Replay — from real gameplay, not fixtures

The brief's target:

```text
play live → save ordered inputs/events → replay → identical final canonical state hash
```

Today the loop is closed only over synthetic fixtures generated by the same code path that
verifies them. Two upgrades, in order:

1. **Record from the local client** (depends on the generic runtime, §11.3). A human plays chess in
   the hot-seat client, presses a key, and a `.tbr` lands in `tests/replays/manual/`. That single
   feature converts every manual play session into a permanent regression artefact and gives the
   corpus the "normal game / edge case / timeout" spread `games/README.md` already demands.
2. **Record from self-play failures.** `xtask selfplay --write-failing-replay <dir>` (the driver
   correctly refuses to write files itself; the CLI is the right layer). The nightly workflow's
   comment already promises this.

### 20.5 Cross-target differential — the biggest missing oracle

Doc 00 §5.1: *"byte-identical final state ... on every OS, architecture, and both native and
WASM."* Evidence: **none**.

The cheap version, which this audit strongly recommends as PR-6:

```text
xtask determinism-vectors            # native: run N scripted matches, emit (game, seed, script, final StateHash) as JSON
                                     # commit the JSON
wasm32 test harness                  # wasm-bindgen-test: load the same JSON, re-run, compare
CI matrix                            # additionally run the native emitter on aarch64 (GitHub arm64 runners)
```

That closes the strongest claim in the product with roughly 150 lines and one CI job. Until it
exists, "determinism is the product" is example-tested on exactly one architecture.

### 20.6 Where a second implementation beats more assertions

| Domain | Second implementation | Beats assertions because |
|---|---|---|
| TicTacToe rules | naive board model, exhaustive | the whole space is enumerable |
| Chess legality | published perft counts | an external authority computed them |
| Chess clocks | bounded loop reference model (already done) | the arithmetic has boundary cases at every increment |
| Asset resolution | naive resolver | the selection rule is a one-liner that is easy to write twice, differently |
| Canonical encoding | hand-built preimage (already done, partially) | it ties the vector to the *spec*, not the code |
| State hash across targets | the other target | no assertion can observe a different architecture |

---

## 21. Verification skill improvements

### 21.1 What existed before this audit

```text
.agents/skills/
  rust-verification-testing/   175 lines + 158-line strategy catalog   — strategy + everything
  rust-types-as-proofs/        189 lines + 154-line boundary hardening — invalid states
  rust-functional-core/        158 lines + 156-line extraction recipes — architecture
  rust-ai-doc-contracts/       164 lines + schema + a Python checker   — @ai.* metadata
.claude/skills/                duplicates of two of the above, drifted (different lengths)
draft-skills/                  three 1 000–1 600 line essays, unreferenced
```

`rust-verification-testing` is a good document. Its problem is that it is **one skill covering
eight disciplines**: it names Kani, Flux, Verus, Creusot, Aeneas+Lean, Loom, Miri, fuzzing,
mutation, property, metamorphic, and differential testing in a single 175-line file. An agent
picking a technique gets a paragraph per technique — enough to choose, not enough to execute. And
the paragraph it gets on Kani ("record the bounds in the property name; a proof for length ≤ 4 is
not a proof for arbitrary length") is correct but did not prevent this repository from shipping a
harness whose scope is much narrower than the sentence "we have Kani proofs of R2" implies.

### 21.2 The hierarchy created by this PR

```text
rust-verification-testing        ROUTER — choose the cheapest adequate oracle; the ledger;
                                 the evidence-level vocabulary; the escalation table;
                                 explicit "do not use X here" rules
   ├── rust-property-testing     generative, state-machine, metamorphic, noninterference;
   │                             reachable-vs-arbitrary generators; shrinking discipline
   ├── rust-replay-differential-testing
   │                             independent oracles: reference models, published data,
   │                             replay equivalence, cross-target hashing, golden corpora
   ├── rust-mutation-testing     assertion strength; survivor classification; cfg-gated
   │                             blind spots; scoping campaigns; converting survivors
   ├── rust-kani                 bounded model checking; propositions; vacuity; cover;
   │                             stubbing; honest scope statements
   └── rust-fuzzing              untrusted bytes; corpora; minimisation; resource bounds;
                                 where NOT to fuzz
rust-types-as-proofs             prevention: make the invalid state unrepresentable
rust-functional-core             architecture that makes all of the above cheap
rust-ai-doc-contracts            durable law → evidence links
```

Each specialised skill: ≤ ~170 lines, opens with a "use this when / do not use this when" gate,
cross-references its siblings, and contains **Tabula-specific worked examples drawn from this
audit** rather than generic advice.

### 21.3 Skills deliberately **not** created, with reasons

| Candidate | Decision | Reason |
|---|---|---|
| `rust-miri` | **No** | The workspace is `#![forbid(unsafe_code)]` with no FFI. Miri's own documentation states that for pure safe Rust "the compiler already guarantees safety through its type system", and Miri only observes executed paths. A Miri skill here would be ceremonial. The router records the trigger: *if an `unsafe` ADR is ever approved, or a C dependency enters the graph.* |
| `rust-loom-concurrency` | **Not yet** | There is no concurrent code in the repository today. `tokio` appears in zero compiled crates; `tabula-match` is a doc comment. Loom becomes correct at Phase 4 for exactly three things: the idempotency cache, the mailbox/drain interaction, and the ownership lease. The router records that trigger. |
| `rust-model-based-testing` | **Folded in** | State-machine modelling belongs in `rust-property-testing` (that is where the generator and shrinker live); reference-model comparison belongs in `rust-replay-differential-testing`. A third skill would split one decision across two files. |

### 21.4 Overlap removed

- `rust-verification-testing` keeps: the ledger, the evidence-level vocabulary, the escalation
  ladder, the edge-case partition list, oracle-independence rules, and the completion-report
  format. It **loses** the per-tool paragraphs, which move into the specialised skills.
- `references/strategy-catalog.md` is retained but repointed: its per-tool sections now say
  "see `rust-<tool>`" rather than duplicating guidance.
- `.claude/skills/` contains drifted duplicates of `rust-functional-core` and
  `rust-types-as-proofs`. **Recommendation (not done in this PR, because it is a repository policy
  question):** pick one location and make the other a symlink or delete it. Two copies of a skill
  that disagree is the same failure mode as two copies of an architecture rule.

---

## 22. Technical-debt ledger

Every entry is repository-specific and carries a location. Findings use the format required by
Part W; short entries are compressed to one row where the full form would add nothing.

### F-01 — There are no property tests
```text
ID:            F-01
Severity:      P1
Confidence:    Certain (mechanically verified: 0 occurrences of `proptest!`/`prop_assert`)
Category:      Verification / false confidence
Claim:         The repository has zero property tests. `tabula-testkit::strategies` is a
               placeholder module that nothing calls.
Evidence:      crates/tabula-testkit/src/strategies.rs:48 returns `vec![(); 0..1]`;
               :58 returns an empty roster; grep for `proptest!` across the tree = 0.
Counterevidence: `tabula-testkit::selfplay` is a genuine randomized state-machine harness with
               hostile injection and full invariant checking; 3.1M chess inputs passed. It is
               real evidence — it is just not property testing, and it does not shrink.
Why it matters: doc 00 §7 names proptest as the enforcement for I-2 and I-7; CI's `test` job is
               commented "Unit + property + replay". Five specific laws (§17.4) have no other
               adequate oracle, and one of them is projection noninterference — the only
               mechanical defence against derived secret leaks.
Failure mode:  A rule bug that self-play's fixed generator never reaches ships silently; when a
               400-input self-play match does fail, there is no shrinking, so the reproduction
               is 400 inputs instead of 3.
Current detector: none.
Why current verification may miss it: example tests only explore the examples someone thought of.
Recommended verification: PR-7 in §28 — properties 2, 3, 7 first (reuse `determinism::run`),
               then 1 and 5.
Recommended engineering action: implement `strategies::input_sequence` on top of
               `legal_commands` as its own TODO already specifies; delete the placeholders that
               cannot be implemented yet rather than leaving them callable.
When to fix:   Next 2 sprints; property 1 (noninterference) is a Phase-3 blocker.
```

### F-02 — The projection security scanner does not exist
```text
ID:            F-02
Severity:      P1
Confidence:    Certain
Category:      Security / verification
Claim:         I-5 and I-6 have no mechanical enforcement. `assert_no_leaks` and
               `assert_no_event_bypasses_redaction` are `todo!()`.
Evidence:      crates/tabula-testkit/src/projection.rs — both functions are `todo!()`;
               `SecretModel` has zero implementors; `docs/games/` contains only README.md.
Counterevidence: The structural half is real and type-enforced: `View`/`ViewEvent` are
               `Serialize`-only, `State` has no wire representation, and a `compile_fail`
               doctest prevents presenting canonical state. No shipped game has secrets.
Why it matters: doc 09 §6 names a projection leak as the #1 most likely failure. AGENTS.md §5
               instructs contributors to run a scan that panics. Phase 3 exists to stress this
               boundary; Phase 4's exit criteria assert spectator safety "against SecretModel".
Failure mode:  Cards or werewolf ships with a leak that no gate can see; after launch it is
               unfixable, because the information is already out.
Current detector: none.
Why current verification may miss it: `selfplay --check-projections` checks projection
               *determinism*, not secrecy (F-14).
Recommended verification: implement the token-containment scan, and add the stronger
               *noninterference* property (§8.4) which catches derived leaks a scan cannot.
Recommended engineering action: PR-4 in §28 — implement both functions and prove them with a
               deliberately leaky fixture game in `tabula-testkit/tests/`, exactly as
               `conformance_catches_violations.rs` already does for four other invariants.
When to fix:   Before any line of `games/cards` is written.
```

### F-03 — `LocalChessMatch` is a chess-shaped runtime that executes no effects
```text
ID:            F-03
Severity:      P1
Confidence:    Certain
Category:      Architecture
Claim:         The client's local runtime is chess-specific, drops every `Effect`, hard-wires
               `LogicalTime::ZERO`, discards `RuleError`, records no input log, and cannot
               drive bots or switch viewers. A generic equivalent already exists in the test
               tier (`tabula-testkit::selfplay`) and the two will diverge.
Evidence:      apps/game-client/src/lib.rs — `state: State`, `view: View`, `local: ChessLocal`;
               `outcome.effects` is never read; `now: LogicalTime::ZERO` at two sites;
               `let Ok(outcome) = result else { return Ok(AudioCues::new()) }`.
Counterevidence: The input-index discipline is careful and well tested (MAX-1/MAX boundary
               tests). For a hot-seat chess demo, the omissions are invisible.
Why it matters: `Effect` is a Phase-4 load-bearing contract that has never been executed by a
               shell a user touches. Chess's 1 170 lines of clock code are unreachable. The
               next game will duplicate the file and the two copies will disagree about
               index-on-rejection and viewer choice.
Failure mode:  Phase 4 discovers the effect contract is wrong after the protocol is frozen.
Current detector: none — there is no test that any effect is executed.
Recommended verification: a `MatchDriverContract` fixture in `tabula-testkit` that `selfplay`,
               the local runtime, and later `MatchActor` must all pass (§11.3).
Recommended engineering action: PR-1 in §28.
When to fix:   Next PR.
```

### F-04 — The asset subsystem has no data and no consumer
```text
ID:            F-04
Severity:      P1 (direction, not correctness)
Confidence:    Certain
Category:      Development direction
Claim:         ~5 400 lines across `tabula-assets` (3 857) and `xtask pack_assets_cmd` (1 506)
               produce zero packs, ship zero asset bytes, and are reachable from no runtime path.
Evidence:      `assets/packs/` contains only `.gitkeep`; no `.png`/`.ogg`/`.ttf` anywhere in the
               tree; `MacroquadAudioSink` starts with an empty registry and
               `apps/game-client/src/main.rs` comments "Phase 3 binds preloaded pack sounds
               here"; `MacroquadRenderer` has no texture path; `AssetPackRef` returned by
               `GamePresentation::asset_pack()` has no caller.
Counterevidence: The code quality is high (55 tests, a private-constructor integrity witness,
               deterministic content-addressed builds, 13/13 mutants killed on integrity.rs),
               and ADR-017 is a correct decision. None of this work is wrong; it is early.
Why it matters: The last five merged PRs (#29–#33) were all asset work, while Phase 2's own
               exit criteria — chess on the web, clocks, sound, themes — remain unmet. The
               project is accumulating unexercised abstraction faster than validated behaviour.
Failure mode:  When real art finally arrives, the manifest shape will be wrong in some way that
               a real pack would have revealed in an afternoon, and 5 400 lines will need
               revision.
Current detector: none — every asset test passes because every asset test is self-contained.
Recommended verification: build ONE real pack for chess (six placeholder piece SVG→PNG sprites
               and two .ogg cues), load it through `load_verified`, register the cues in
               `MacroquadAudioSink`, and render the pieces. One end-to-end pack is worth more
               than the next 1 000 lines of manifest validation.
Recommended engineering action: freeze new asset-library work; do PR-9 in §28 after the
               runtime PRs.
When to fix:   Freeze now; the one-real-pack PR after PR-1..PR-5.
```

### F-05 · F-06 — `DetRng::below` has unasserted uniformity and unproven termination
```text
ID:            F-05 / F-06
Severity:      P2
Confidence:    High (mutants survive; semantic effect confirmed by independent computation)
Category:      Determinism / fairness / liveness
Claim:         (F-05) The rejection zone `2^32 - (2^32 % n)` has no assertion; mutating `-` to
               `+` survives and restores modulo bias. `shuffle`'s `len < 2` guard mutated to
               `len == 2` survives and makes a 2-element shuffle the identity.
               (F-06) `below` contains an unbounded loop whose termination depends on
               `zone > 0`; three mutants made it hang the test runner.
Evidence:      `cargo mutants -p tabula-core`: MISSED at rng.rs:190, :193, :208 (×3);
               TIMEOUT at rng.rs:62, :193 (×2). Independent program: for n=6 the original
               rejects 4/2^32 draws, the mutant rejects 0. Guard table: len 0/1 unaffected,
               len 2 returns early under the mutant.
Counterevidence: The frozen stability vectors pin the *current* behaviour, so an accidental
               change is caught for the sampled draws; and no game shuffles two elements yet.
Why it matters: `below` and `shuffle` are the fairness primitives for every card, dice, tile-bag
               and role-assignment game Tabula will ever ship, and the doc comment states the
               anti-bias rationale as the reason for the design. `below` also runs inside
               `apply`, inside a Tokio task, on a server whose watchdog deliberately does not
               kill stuck actors.
Current detector: `below_is_not_visibly_biased` (60 000 samples) cannot see a 1-in-10^9 bias;
               `shuffle_is_a_permutation` accepts the identity.
Recommended verification: Kani harness over all `n: u32` (§16.5) + two unit tests (§18.5 items
               1–2).
Recommended engineering action: ~25 lines. Do it in the next core-touching PR.
When to fix:   Before `games/cards`.
```

### The remaining ledger

| ID | Sev | Category | Finding | Location | Detector today | Action |
|---|:--:|---|---|---|---|---|
| **F-07** | P2 | Verification | `State::from_parts` — the tictactoe reachability validator — has **35 surviving mutants**; the documented "state is reachable" boundary has almost no predicate-level evidence. | `games/tictactoe/src/rules/state.rs:81–147`; `docs/verification/core-domain-boundaries.md` | none | Table test, one row per rejected class (§18.5 item 5). Better: the exhaustive model in §20.1. |
| **F-08** | P2 | Enforcement | `clippy.toml` (`disallowed-types`: HashMap/HashSet/SystemTime/Instant) exists only in `crates/tabula-core` and `games/*`. `tabula-game-api`, `-protocol`, `-registry`, `-presentation`, `-testkit`, `-assets`, `-match` have none. Verified: a `HashMap` added to `tabula-game-api`, `-protocol`, and `-presentation` passes clippy silently; the same in `games/chess` fails with the I-2 reason. | `crates/*/clippy.toml` (absent) | none in 20 of 26 crates | Add the file to every crate whose types can appear in canonical output — at minimum `tabula-game-api` (its `Effect`/`ChatScopes` payloads are canonically encoded by the determinism harness) and `tabula-protocol`. |
| **F-09** | P2 | Duplicate source of truth | `game.toml` and the compiled `GameMetadata`/`GameCapabilities` are two independent declarations. Only `rules_version` is cross-checked (by `build.rs`). Verified: changing `complexity` from `heavy` to `light` and `max_match_duration` from 14400000 to 999 passes `check-manifests`, the full test suite, and `cargo xtask check`. | `games/*/game.toml` vs `games/*/src/lib.rs`; `xtask/src/manifest_policy.rs` | build.rs (rules_version only) | Either implement the `metadata_from_manifest!` proc macro doc 02 §10.2 specifies, or add a `check-manifests` mode that constructs the statics and compares field-by-field. The second is much cheaper and removes the drift class entirely. |
| **F-10** | P2 | CI hygiene | `nightly.yml` runs four commands that cannot succeed: `cargo fuzz run protocol_decode` / `command_decode` (no `fuzz/` directory), `cargo xtask load --scenario L1` (`future_command`, exits 2), and `cargo udeps` (needs a nightly toolchain the pinned `rust-toolchain.toml` does not install by default). `cargo xtask replay --all --verify` uses an `--all` flag absent from the documented parser surface. | `.github/workflows/nightly.yml` | the job itself, if anyone reads it | Split nightly into jobs that exist today (replay corpus, self-play, depth-5 perft, Kani, mutation) and a clearly-labelled `phase-4-placeholder` job that is `if: false`. A permanently red nightly trains people to ignore nightly. |
| **F-11** | P2 | Phase drift | Phase 2's exit criteria are unmet — no web host page, clocks not rendered, no sound assets, no theme toggle, no reduced-motion mode, no bundle-size gate — while Phase 3 asset work has merged five PRs. | `docs/architecture/07` Phase 2 vs the tree | none | Either meet the criteria (PR-1..PR-3, PR-9) or amend doc 07 in a PR that says why. Do not leave the doc claiming a gate that was walked past. |
| **F-12** | P2 | Product | The wasm artefact has never been loaded, because `apps/game-client` has no `index.html`/`mq_js_bundle.js`. Adding 12 lines made chess playable in a browser (§25). | `apps/game-client/` | none | PR-3 in §28 — ~15 lines plus a `just wasm-serve` recipe. |
| **F-13** | P2 | Contract / UX | The client discards `RuleError`. doc 00 §4.1 requires the `invalid-action` motion token on rejection. Observed live: clicking an illegal destination silently clears the selection with no feedback. | `apps/game-client/src/lib.rs` | none | Part of PR-1. |
| **F-14** | P2 | False confidence | `tabula-testkit`'s module doc says self-play checks "determinism, projection safety, and termination". `check_projections` compares canonical `View`/`Option<ViewEvent>` **bytes across two runs** — projection *determinism*. It performs no secrecy check. | `crates/tabula-testkit/src/lib.rs`; `selfplay.rs` `@ai.invariant projection-output-determinism` | — | One-line doc fix. The `@ai.*` annotations in `selfplay.rs` are already precise; the prose is not. |
| **F-15** | P2 | Architecture | Chess `State` has public fields and derived `Deserialize`; a decoded snapshot can be an unreachable position. TicTacToe has a validated `from_parts` boundary; chess does not. | `games/chess/src/rules/state.rs:158` | `from_fen` validates; `Deserialize` does not | Give chess a `try_from` serde boundary before Phase-4 `restore_match`. Requires a `RULES_VERSION` review (already flagged in `docs/verification/core-domain-boundaries.md`). |
| **F-16** | P2 | Security hazard (future) | Chess embeds the full `legal_commands` enumeration in `View`. For a hidden-information game this is a side channel the (unimplemented) scanner must cover. | `games/chess/src/rules/mod.rs` `project`; `View.legal_moves` | none | Document the rule now: *`legal_commands` is per-viewer or absent*; make the scanner cover embedded command lists. |
| **F-17** | P3 | Stale docs | `cargo xtask new-game` is `unimplemented_command`, but AGENTS.md §7 presents it as the way to add a game and `xtask/README.md` describes what it scaffolds. | `xtask/src/main.rs`; AGENTS.md §7 | its own test asserts the *unimplemented message* | Either implement it (it is a template copy) or mark it clearly in both docs. |
| **F-18** | P3 | Stale docs | MSRV drift: doc 01 §5 line 494 says `rust-version = "1.82"`; the workspace says `1.85`. `rust-toolchain.toml` documents the deviation in detail and doc 01 was never amended. | `docs/architecture/01-...md:494` | none | One-line doc fix; the reasoning is already written. |
| **F-19** | P3 | Stale docs | Root `README.md` describes a different architecture vocabulary (`Action`, `PlayerView`, `Visibility`) from the normative one (`Command`, `View`, `Viewer`), in a different language, and is a third source of truth alongside doc 00 and AGENTS.md. | `README.md` | none | Rewrite as a pointer to `docs/architecture/README.md` plus a build/run quickstart. |
| **F-20** | P3 | Vacuity | `assert_deterministic` begins `let Ok(a) = run::<R>(scenario) else { return };` — if `create` fails, the check silently passes. | `crates/tabula-testkit/src/determinism.rs` | none | `panic!` instead of `return`. A conformance check that can no-op is a green tick that means nothing — the exact failure mode `harness_catches_violations.rs` exists to prevent. |
| **F-21** | P3 | Verification | The "documented preimage" tests rebuild the hash preimage using the *same constants* they are meant to defend. NC-8b (changing `STATE_HASH_DOMAIN`) failed exactly one test — the captured literal. | `crates/tabula-core/src/hash.rs`, `rng.rs` | one literal each | Inline the byte strings (`b"tabula.state.v1"`, `b"input"`, `b"stream"`) in the preimage tests. 3 lines. |
| **F-22** | P3 | Tooling | cargo-mutants mutates `#[cfg(kani)]` code; 24 of 110 survivors across two campaigns are verifier-only noise concentrated in the modules that look most verified. | `.cargo/mutants.toml` | none | `exclude_re = ["verification::"]` — verified during this audit to drop tictactoe from 198 to 179 mutants with zero `verification::` entries. |
| **F-23** | P3 | Product | The running client shows a tofu box in `Your turn — White`: the default Macroquad font has no U+2014. First concrete instance of the text ceiling doc 09 §6 predicted. | `games/chess/src/presentation/mod.rs` `status_text` | none | Use `-` until a font asset exists; record the incident in the Macroquad-ceiling log doc 09 §3.2 asks for. |
| **F-24** | P3 | i18n | The chess presenter builds English string literals (`"Your turn — "`, `"White rook"`, `"Empty square"`, `"Game over — {summary}"`). doc 04/doc 07 require i18n keys with no literals. | `games/chess/src/presentation/mod.rs` | none | Cheap now, expensive after four games. Convert `status_text`/`chess_a11y` to keys + args. |
| **F-25** | P3 | Enforcement gap | `check-no-raw-colors` misses `0x3E7B5AFF`-style integer literals. (It deliberately ignores hex inside string literals; the integer form is not deliberate.) | `xtask/src/colors_cmd.rs` | none | Low priority — a colour must still be constructed, and the constructor check fires. Note it in the tool's docs. |
| **F-26** | P3 | Corpus | 3 synthetic golden replays total; `games/README.md` requires ≥ 3 per game covering a normal game, an edge case, and a timeout. None derives from real play or self-play. | `tests/replays/` | none | Falls out of PR-5 (record from the client). |
| **F-27** | P3 | Coverage | Bots have no unit tests (19 surviving tictactoe mutants, including `choose -> None`). Bots are the primary fuzz driver, so a degenerate bot silently weakens self-play. | `games/*/src/bot.rs` | `xtask selfplay` only, out of package | 20 lines of table tests per bot. |
| **F-28** | P3 | Escape hatch | `Viewer::Audit` is a freely constructible enum variant with full information. Already a `TODO(phase 4)` in the source. | `crates/tabula-core/src/viewer.rs` | none | Introduce the `AuditGrant` token before any `Viewer` can be derived from a network message. |
| **F-29** | P3 | Escape hatch | `xtask-allow-game-id` is an unbounded, unexpiring per-line suppression; used once legitimately (`apps/game-client` importing chess) and once for a genre name. `deps.toml` additionally grants `allow_games = true` to `apps/game-client`. | `apps/game-client/src/lib.rs:28`; `deps.toml` | the suppression is visible in review | Require a `TODO(phase N)` alongside every suppression and grep for suppressions without one. |
| **F-30** | P3 | Test-helper divergence | Three logical-time models: `determinism::run` uses `index * 1000 ms`; `selfplay` advances by timer/bot deadlines; `LocalChessMatch` uses `ZERO`. None is normative, so nothing pins what `MatchActor` must do. | testkit + client | none | Fix the semantics in the `MatchDriverContract` (§11.3) before Phase 4. |
| **F-31** | P3 | Contract drift | `Effect::Notify`'s documented idempotency key is `(match_id, audience, notice_id)`, but `Notice` has no `notice_id` field. | `crates/tabula-game-api/src/effect.rs` | none | Add the field now; it is additive and free before Phase 4. |

---

## 23. False-confidence risks

Ranked by how likely a reader is to over-trust the claim.

| # | The sentence someone will write | What is actually true |
|---|---|---|
| 1 | "R2 is formally verified." | Two Kani harnesses prove field-level R2 for **two concrete tic-tac-toe states** over `(u8, u8)` — 65 536 cases, exhaustively coverable by a loop test. R2 for chess, for arbitrary reachable states, for `apply` as a whole, and at the canonical-byte level rests on example tests and self-play. §16. |
| 2 | "Projection safety is checked on every PR." | The scanner is `todo!()`. What runs is projection *determinism*. §8, F-14. |
| 3 | "We have property tests." | Zero. §17. |
| 4 | "Determinism is verified cross-platform." | Never attempted on any second target. §9.3. |
| 5 | "`tabula-assets` is verified." | It is well tested *as a library*, and 13/13 mutants died on `integrity.rs`. It has never processed a real asset, and no runtime consumes it. §3, F-04. |
| 6 | "The conformance suite proves a game is correct." | It proves determinism, transactionality, ordering, round-tripping, and terminality. NC-9 shows it does **not** catch a chess legality bug: removing the castling-through-check guard passed conformance, determinism, and 200 self-play matches, and was caught only by perft. §24. |
| 7 | "Self-play is the fuzzer, so hostile input is covered." | Self-play injects hostile inputs at 5 % against a *state-aware* generator; it has no shrinking, no byte-level malformed input, and does not touch the `.tbr` decoder or the manifest parser, which are the two real untrusted-input surfaces today. §19. |
| 8 | "The frozen constants are protected." | `DetRng`'s tags are protected by 4 tests; `STATE_HASH_DOMAIN` by exactly one captured literal, because the companion "documented preimage" test uses the same constant. §9.2, F-21. |
| 9 | "`game.toml` is the source of truth and CI checks it." | CI validates each side's schema and compares exactly one field (`rules_version`, via `build.rs`). Changing `complexity` or `max_match_duration` in `game.toml` passes every gate. F-09. |
| 10 | "Nightly verification is running." | Four of its jobs invoke commands that do not exist. F-10. |
| 11 | "The clock is implemented and tested." | The *rules* are, thoroughly (1 170 lines). No client sets a clock, no client renders one, no client fires a timer. §4.1. |
| 12 | "`xtask new-game` scaffolds a game." | It prints "not yet implemented". F-17. |
| 13 | "Mutation testing shows the core is well covered." | 30 survivors in `tabula-core` and 80 in tictactoe; 22 % of them are `cfg(kani)` noise that inflates the *apparent* gap while hiding the real one. §18. |
| 14 | "`@ai.evidence` annotations link laws to tests." | They do, and they are unusually accurate — I spot-checked a dozen and every cited test exists and tests what the tag claims. This is the one place where the documentation is *stronger* than the prose around it. Keep the practice; consider running the bundled `ai_doc_contracts.py` checker in CI, which nothing currently does. |

---

## 24. Architecture negative-control experiments

Eighteen negative-control experiments (seventeen distinct injected defects; NC-3c re-runs NC-3b's
defect under Kani) were injected and reverted. `git status` was verified clean after each.
This is the most important table in the audit: **a verification mechanism that has never been
observed to fail should not be trusted.**

| ID | Defect injected | Expected detector | Actual detector | Result |
|---|---|---|---|---|
| **NC-1** | `tokio` added to `tabula-core`'s `[dependencies]` | `xtask check-deps` | `check-deps: 8 violation(s) across 26 workspace crates`, each with the transitive path (`tabula-game-tiles -> tabula-core -> tokio`) | **DETECTED** — and the path output is genuinely useful |
| **NC-2** | `pub const AUDIT_TEMP: &str = "com.tabula.chess";` appended to `crates/tabula-presentation/src/audio.rs` | `xtask check-no-game-ids` | reported `crates/tabula-presentation/src/audio.rs:98:42`, game id `chess`, with the offending line and the rule | **DETECTED** |
| **NC-3a** | Mutate-then-validate in `tictactoe::place` (turn flipped around `validate_place`) | conformance R2 | 13 test failures across conformance + determinism | **DETECTED** (over-broad defect) |
| **NC-3b** | **Surgical**: on the rejected path only, `state.move_timeout_ms = state.move_timeout_ms.wrapping_add(1)` — a canonical field validation never reads | conformance R2/R8 | exactly 3 failures: `tabula_conformance_invalid_command_safety`, `rejected_inputs_leave_state_byte_identical`, `a_rejection_does_not_disturb_the_next_input`, with a diagnostic naming input index 1, the rejection code, both hashes, and both canonical byte strings | **DETECTED** — best diagnostic in the repository |
| **NC-3c** | Same surgical defect, run under Kani | the two R2 harnesses | `rejected_second_place_preserves_state` FAILED (22.5 s), `rejected_initial_place_preserves_state` FAILED (14.1 s), `concrete_opening_place_is_accepted` SUCCESSFUL | **DETECTED** — confirms the harnesses are not vacuous *for this defect class* |
| **NC-4** | `game.toml`: `complexity = "heavy"` → `"light"`, `max_match_duration = 14400000` → `999` | `xtask check-manifests` | `check-manifests: 28 manifest(s) checked — all clear`; full chess suite 63/63 passed | **NOT DETECTED** (F-09) |
| **NC-4b** | `game.toml`: `rules_version = 3` → `4` | `build.rs` assertion | build failed: *"game.toml rules_version must match the rules module RULES_VERSION — left: 3, right: 4"* | **DETECTED** (one field of ~30) |
| **NC-5** | Duplicate `GameId`: tictactoe's metadata id changed to `com.tabula.chess` | registry uniqueness check | no registry exists. `check-no-game-ids` fired (**coincidentally** — it saw the literal `chess` outside chess's package) and `committed_tictactoe_replay_reproduces_its_independent_final_hash` failed (the replay header carries `game_id`) | **PARTIAL / COINCIDENTAL** — two games sharing a *novel* id would pass both |
| **NC-6** | Asset density selection: `Reverse(variant.density)` → `variant.density` (flip the equal-distance tiebreak) | asset resolution law tests | 2 failures: `resolution_obeys_density_selection_law`, `resolution_is_declaration_order_independent_and_never_infers_file_names` | **DETECTED** |
| **NC-7a** | `pub audit_marks: HashMap<u8,u8>` added to chess `State` | clippy `disallowed-types` | 3 errors, each with `note: I-2: nondeterministic iteration order; use BTreeMap` | **DETECTED**, with the invariant id in the message |
| **NC-7b** | `pub fn audit_map() -> HashMap<u8,u8>` in `tabula-game-api` | clippy `disallowed-types` | clean build, no warning | **NOT DETECTED** (F-08) |
| **NC-7c** | Same in `tabula-protocol` and `tabula-presentation` | clippy `disallowed-types` | clean build, no warning in either | **NOT DETECTED** (F-08) |
| **NC-8** | `DetRng::for_input` domain tag `b"input"` → `b"inpuT"` | frozen stability vectors | 4 failures: `for_input_stream_is_stable`, `for_input_matches_its_documented_preimage`, `shuffle_is_stable`, `below_is_stable` | **DETECTED** (defence in depth) |
| **NC-8b** | `STATE_HASH_DOMAIN` `b"tabula.state.v1"` → `b"tabula.state.v2"` | frozen stability vectors | **1** failure: `state_hash_is_stable`. The companion `state_hash_matches_its_documented_preimage` rebuilds the preimage from the same constant and passed. | **DETECTED, single point of defence** (F-21) |
| **NC-9** | **Chess legality**: removed `&& !is_attacked(state, Square(home.0 + 1), color.other())` — i.e. allow castling through an attacked square | perft | perft Kiwipete depth 3: **98 069 vs expected 97 862**; plus `illegal_moves_are_byte_identical_noops`. **`tabula_conformance_*` (all 11), `determinism`, `replay`, `clocks`, `bot`, and 200 self-play matches ALL PASSED.** | **DETECTED ONLY BY PERFT** |
| **NC-10** | `pub const AUDIT_COLOR: &str = "#3E7B5A";` in chess presentation | `check-no-raw-colors` | clean — hex inside a *string literal* is a deliberate, tested exemption | **NOT DETECTED (by design)** |
| **NC-10b** | `tabula_design::Color::rgb(62, 123, 90)` in chess presentation | `check-no-raw-colors` | `games/chess/src/presentation/mod.rs:3997 raw Color constructor; use tabula-design semantic tokens` | **DETECTED** |
| **NC-11** | `pub const AUDIT_RGBA: u32 = 0x3E7B5AFF;` in chess presentation | `check-no-raw-colors` | clean | **NOT DETECTED** (F-25, low severity) |

### 24.1 What the table says

- **The static architecture gates are real.** NC-1, NC-2, NC-6, NC-7a, NC-8, NC-10b all fired with
  precise, actionable messages. This is unusual and it is the project's biggest quality asset.
- **The conformance suite's R2/R8 checks are real and non-opt-in.** NC-3b's diagnostic is a model
  of what a failing invariant check should say.
- **Kani is not vacuous for the defect class it targets.** NC-3c.
- **Three gates are narrower than they appear.** `check-manifests` (NC-4), clippy determinism bans
  (NC-7b/c), and duplicate-identity detection (NC-5).
- **NC-9 is the headline.** A genuine chess rule violation — one that would produce illegal games
  and diverging replays against any other engine — was invisible to *every* generic mechanism and
  visible only to the one oracle that comes from outside the codebase. **Keep several independent
  oracles. Do not consolidate verification into one harness.**

---

## 25. Manual / playable validation

The brief asks for the distinction between "tests say this works" and "a person actually played
this flow". Here is that distinction, made explicitly.

### 25.1 Native desktop

`cargo run -p tabula-game-client` **should** and **does** launch local hot-seat chess.

```text
$ ./target/debug/tabula-game-client      # X11 display :0, Intel iGPU, direct rendering
(ran 12 s under timeout; exit 124 = still running; no stderr, no panic)
```

The window opens at 720×720 titled "Tabula — local hot seat". I could not drive the native window
programmatically, so gameplay was exercised two other ways.

### 25.2 Driven through the real presenter (reproducible automation)

A temporary integration test drove `LocalChessMatch` through `ChessPresentation::on_input` with
synthesised `InputEvent::Pointer` events at real square centres — i.e. through the *actual* click
path, not through `ChessRules::apply` directly. Results:

| Flow | Result |
|---|---|
| normal move (e2–e4) | ✔ board updated, turn → Black, cue `["move"]` |
| out-of-turn attempt | ✔ board byte-identical, no cue, no feedback of any kind (F-13) |
| capture (exd5) | ✔ cue `["capture"]` |
| **check → checkmate** (Scholar's mate: e4 e5, Bc4 Nc6, Qh5 Nf6??, Qxf7#) | ✔ `Status::Ended { Decisive, standings [seat0 rank0 score1, seat1 rank1 score0], summary "checkmate" }`, cues `["capture", "game-end"]` |
| post-terminal input | ✔ total no-op |
| **castling** (O-O after e4 e5 Nf3 Nc6 Bc4 Nf6) | ✔ king on g1, rook on f1 |
| **en passant** (e4 a6 e5 d5 exd6 e.p.) | ✔ pawn on d6, captured pawn removed from d5 |
| **promotion** (g4 a6 g5 a5 g6 a4 gxh7 a3, then h7×g8) | ✔ tap opened the promotion chooser instead of moving; `Interaction::Promotion { from: 55, to: 62, selected: Queen }`; keyboard `Enter` produced `Move { from: 55, to: 62, promotion: Some(Queen) }`; board shows a white **Q** on g8 |
| initial `View.legal_moves` | 20 — correct |
| `View.clock` | `None` — the local match is untimed (F-11) |
| a11y description | full 64-square region with correct piece names, positions, and a `move-square` action gated on `you == turn`; status `"Your turn — White"` |

Draw-related paths (resign, offer/accept/decline, claim) **cannot be reached from the UI at all**
— the presenter emits only `Command::Move`. Stalemate and the 50/75-move and repetition rules are
reachable in principle but were not driven manually; they are covered by `games/chess/tests/rules.rs`.

### 25.3 A person actually played it — in a browser

This is the part that had never happened. `apps/game-client` compiles to wasm32 but the repository
contains no host page. I created one (12 lines of HTML plus macroquad 0.4.16's own
`mq_js_bundle.js`) and served it locally.

**First attempt panicked immediately:**

```text
PanicHookInfo { location: apps/game-client/src/main.rs:29 }
RuntimeError: unreachable
```

Line 29 is `.expect("Macroquad supplies a finite non-empty viewport")`. With a `<canvas>` that has
no explicit `width`/`height`, macroquad reports a zero-sized viewport on the first frame and
`Viewport::new` correctly rejects it — so the proof-carrying newtype did its job and the shell
turned a rejected fact into a panic. **Giving the canvas explicit dimensions fixed it.** That is
worth recording as a real robustness note for the host page PR: the shell should skip the frame
rather than `expect` on a transient zero viewport.

**Second attempt worked.** Observed, with screenshots:

1. The board renders: 8×8 grid, ASCII glyphs for pieces (uppercase = White, lowercase = Black),
   status line "Your turn ▯ White" — with a **tofu box** where the em dash should be (F-23).
2. Clicking **e2** highlighted the square with a selection border and drew **legal-target dots on
   e3 and e4** — the legal-move highlighting Phase 2 promises.
3. Clicking **e4** played the move: mid-flight the animated pawn was still near e2 with
   last-move highlights on e2/e4; ~2 s later the pawn had arrived on e4 and the status read
   "Your turn ▯ Black".
4. Clicking **e7** selected Black's pawn with dots on e6/e5 — **hot-seat turn alternation works
   in the browser**.
5. Clicking **e5** played it.
6. Clicking an illegal destination (d5 from e7) produced **no visible response at all** — the
   selection silently cleared. This is F-13, observed rather than inferred.

Two further observations only running it could produce:

- **Board contrast is very low in the light theme.** `surface_container` and
  `surface_container_high` are nearly identical, so the checkerboard reads as a flat white grid.
- **The piece-move animation is slow** (visibly > 1 s) and mid-flight the glyph renders below the
  square centre.

**Bundle size:** 552 KB raw / **0.20 MB gzipped** — 30× under doc 01 §7's 6 MB cap.

### 25.4 Shortest path to a visually playable TicTacToe

Measured against the existing code, not guessed:

1. **Generic local runtime** (§11.3) — otherwise `LocalTicTacToeMatch` duplicates
   `LocalChessMatch`. ~200 lines net, mostly moved.
2. **`games/tictactoe/src/ui.rs`: replace the doc-comment sketch with a real
   `impl GamePresentation`.** The chess presenter is the template and tic-tac-toe needs a
   fraction of it: a 3×3 grid of `RenderCmd::Rect`, X/O as `RenderCmd::Text` or two `Rect`s,
   `Local { hover, selected }`, `on_input` mapping a click to `Command::Place { cell }` via a
   layout helper, `on_view_event` emitting a `"place"` cue, `a11y` delegating to
   `TicTacToeRules::describe` (currently `unsupported()` — 15 lines to fill in). **Realistically
   150–250 lines**, versus chess's 3 994, because there is no drag, no promotion chooser, no
   animation timeline, and no focus graph beyond nine cells.
3. **A game selector in `main.rs`** — three lines, or a `--game` argument.

**Total: one PR of ~300 lines to have two games running through one runtime**, which is exactly
the evidence the architecture needs and does not have.

### 25.5 Tests say vs a person played

| Flow | Tests say | A person (or a driven presenter) actually did it |
|---|---|---|
| chess normal move / capture / check / checkmate | ✔ | ✔ (both automation and browser) |
| castling / en passant / promotion | ✔ | ✔ (driven presenter) |
| illegal move rejected | ✔ | ✔ — **and produces no feedback** |
| turn alternation, hot seat | ✔ | ✔ (browser) |
| legal-move highlighting | ✔ (snapshot) | ✔ (browser) |
| move animation | ✔ (snapshot at midflight) | ✔ — visibly slow |
| audio cues emitted | ✔ | cues are produced; **no sound has ever played** (empty registry) |
| clocks | ✔ (1 170 lines) | **never** |
| resign / draw offer / claim | ✔ (rules tests) | **unreachable from any UI** |
| chess in a browser | CI `cargo check` only | ✔ **for the first time, during this audit** |
| tictactoe with a UI | — | **never — no presentation exists** |
| replay of a real game | — | **never — nothing records** |
| asset pack loaded at runtime | ✔ (library tests) | **never** |

---

## 26. Roadmap reassessment

### 26.1 Is the documented ordering still right?

The documented chain is:

```text
P0 architecture → P1 rules/chess → P2 presentation/renderer → P3 local games/assets
              → P4 multiplayer → P5 web shell → …
```

**The ordering is correct and should not change.** Doc 07 §0.2's rationale survives scrutiny:
rules before rendering, local play before networking, networking before the shell. Nothing in this
audit argues for reordering phases.

**What has gone wrong is not the order — it is that Phase 2 was never closed.** The repository is
doing Phase 3 work (assets) with Phase 2 exit criteria unmet:

| Phase 2 exit criterion (doc 07) | Status |
|---|---|
| Chess playable hot-seat **on desktop** | ✔ verified |
| …**and web from one codebase** | ✘ builds, no host page, never loaded until this audit |
| with **clocks** | ✘ rules only; `Config::default()` has none; `View.clock` unrendered |
| legal-move highlighting | ✔ verified in browser |
| drag **and** tap input | ✔ both implemented; tap verified live |
| capture / check / checkmate animations | partial — move animation verified; no capture/check-specific motion observed |
| **sound** | ✘ cue plumbing exists; zero sounds; the sink always errors |
| light/dark themes | ✘ `ThemeKind::Light` hard-coded in `main.rs` |
| reduced-motion mode | ✘ `MotionMode::Full` hard-coded |
| zero Macroquad references outside `renderer-macroquad` | ✔ verified |
| golden `RenderList` **and image** tests green | partial — 7 `RenderList` snapshots for chess; the 2 committed PNGs test the rasterizer's own subset, not a game scene |
| WASM bundle < 6 MB gzipped | ✔ 0.20 MB — but the CI gate is commented out |
| 60 fps on mid-range hardware | unmeasured |

That is **5 unmet of 13**, and the unmet ones are precisely the *visible* ones.

### 26.2 Should the next work be asset expansion?

**No.** The argument, in one paragraph:

The asset subsystem is now the second-largest body of code in the repository and has produced zero
observable behaviour. Meanwhile the two things that would most reduce architectural risk —
executing an `Effect` and running a second game through one runtime — cost roughly 500 lines
between them and have not been done. Doc 09 §7's warning about building against an unvalidated
contract applies to `Effect` exactly as it applies to the protocol: *the effect list has been
designed twice and validated zero times*. Continuing to deepen `tabula-assets` before a single
real pack exists risks the same class of rework it is meant to prevent.

The correct asset move is **one real pack, end to end**, after the runtime work: six placeholder
piece sprites and two `.ogg` cues for chess, built by `xtask pack-assets`, loaded through
`load_verified`, registered in `MacroquadAudioSink`, drawn by `MacroquadRenderer`. That single PR
will teach more about whether the manifest shape is right than the next 1 000 lines of validation,
and it closes two Phase 2 criteria (sound, and real board art) at the same time.

### 26.3 Evaluating the brief's proposed sequence

The brief proposes:

```text
1 manually validate chess slice → 2 harden generic local driver → 3 local Effect execution
→ 4 TicTacToe presentation → 5 both games on one runtime → 6 local bots
→ 7 replay from real gameplay → 8 integrate verified assets → 9 verify WASM/browser
→ 10 Phase 4
```

**This is close to right. Three amendments:**

| Change | Why |
|---|---|
| **Merge 2 and 3.** | Effect execution is not a follow-on to the generic driver; it is 60 % of what makes the driver generic. Splitting them produces a first PR whose only content is renaming chess types to generics. |
| **Move 9 (WASM/browser) to position 3, right after the runtime PR.** | It costs ~15 lines, it closes a Phase 2 exit criterion, and it is the highest visible-progress-per-line item available. This audit did it in ten minutes. Doing it early also means every subsequent presentation PR is verified on both targets. |
| **Insert projection scanner + a hidden-information probe before 8.** | It is the last unvalidated *contract*, it is the #1 named risk, and it must exist before cards. A "probe" can be tiny: a fixture game in `tabula-testkit/tests/` with one secret and one deliberately leaky projection, in the style of `conformance_catches_violations.rs`. |

Position 6 (local bots) is more valuable than its position suggests: bots turn hot-seat into
single-player, which is the difference between "a developer can demo it" and "a person can play
it". And the bots already exist — `ChessBot` and `Heuristic` are implemented and exercised by
self-play; the client simply cannot reach them because it does not execute
`Effect::RequestBotMove`. **Local bots are almost free once PR-1 lands.**

### 26.4 Recommended sequence

```text
PR-1  generic local runtime + effect execution + logical clock + rejection feedback + input log
PR-2  TicTacToe presentation on that runtime (two games, one driver)
PR-3  WASM host page + `just wasm-serve` + zero-viewport robustness
PR-4  projection scanner (containment + noninterference) + a leaky fixture game that proves it
PR-5  local bots (free) + replay recording from real play + corpus expansion
PR-6  cross-target determinism vectors (native vs wasm32, and aarch64 in CI)
PR-7  first property tests: R2 over reachable states, replay equivalence, legal_commands soundness
PR-8  verification hardening: below/shuffle tests + Kani zone harness + mutation regressions
      + exhaustive TicTacToe reference model
PR-9  ONE real chess asset pack, end to end (closes Phase 2 sound + art)
PR-10 Phase 4 spike: GameAdapter<M> for both games behind #[cfg(test)], thrown away, notes kept
```

Phase 4 proper begins after PR-10's notes exist and PR-4's scanner is green against a
hidden-information game — which is Phase 3's own exit criterion, honoured rather than skipped.

---

## 27. Conservative / balanced / aggressive development plans

### 27.1 Conservative — optimise correctness and architecture stability

**Next 8 PRs**

1. Projection scanner (containment + noninterference) + leaky fixture game.
2. First property tests: R2 over reachable states; determinism over generated sequences; `legal_commands` soundness.
3. Exhaustive TicTacToe reference model (subsumes the Kani R2 harnesses).
4. `DetRng` hardening: zone unit test, 2-element shuffle test, Kani `below` harness, `MatchSeed` Debug test.
5. `clippy.toml` for every crate whose types reach canonical output; `game.toml`↔code cross-check.
6. Cross-target determinism vectors (native/wasm32/aarch64).
7. Chess `State` validated deserialization boundary; `Viewer::Audit` capability token; `Notice::notice_id`.
8. Fuzz targets for the `.tbr` decoder and the asset manifest; nightly CI cleaned up.

**Visible milestone:** none. Everything is invisible to a player.
**Verification required:** each PR is itself verification.
**Debt introduced:** none. **Debt repaid:** F-01, F-02, F-05..F-10, F-15, F-20..F-22, F-31.
**Risk:** *high, and not the kind that looks risky.* Four to six weeks with no observable
progress, on a project whose only playable artefact is a hot-seat chess board with no clock, no
sound and no second game. Doc 07's own rule — "no phase is only refactoring; every phase ends with
something a person can look at" — exists to prevent exactly this. It also front-loads work whose
requirements the runtime PRs would clarify (the scanner's `Secret` token granularity is an open
`TODO` that cards, not a fixture, will settle).

### 27.2 Balanced — progress with verification kept honest

**Next 10 PRs:** PR-1 … PR-10 from §26.4.

**Visible milestones**
- After PR-3: **chess playable in a browser**, linkable, on any machine.
- After PR-2+PR-5: **two games, one runtime, playable against a bot**, and every session recordable as a replay.
- After PR-9: **a chess board with real pieces and sound.**

**Verification required per PR:** stated in §28. In summary: PR-1 gets the `MatchDriverContract`;
PR-2 gets conformance + render-list snapshots + a shared-runtime test; PR-4 gets the leaky-fixture
meta-test; PR-5 gets live→replay hash equality; PR-6 gets the cross-target matrix; PR-7/PR-8 are
verification.

**Debt introduced:** small and named — no i18n keys yet (F-24), no theme toggle until PR-9, the
`.tbr` fuzz target slips to the PR-8/PR-9 window, `xtask new-game` stays unimplemented.
**Risk:** moderate. The main one is PR-1 being done badly — over-abstracted into a crate before
three consumers exist. §11.4 is the guard.

### 27.3 Aggressive — optimise time-to-multiplayer

**Next 8 PRs**

1. Generic local runtime + effects (PR-1, unchanged — it is a prerequisite for everything).
2. WASM host page (PR-3, unchanged — it is 15 lines).
3. `tabula-registry`: `ErasedGame`/`ErasedMatch`/`GameAdapter<M>` + `register!` for the two games.
4. `tabula-protocol`: envelopes, dual codec, golden vectors, `PROTOCOL_VERSION 1.0`.
5. `tabula-match`: actor + mailbox + the doc 03 §7 pipeline against in-memory fakes.
6. `tabula-net-client` + a minimal `tabula-server` with join-by-code, no auth, no database.
7. Two browsers playing chess over the network.
8. `tabula-storage` + event log + reconnect.

**Visible milestone:** networked chess between two devices in ~8 PRs. That is a genuinely
compelling demo and it is achievable.

**Debt introduced, stated clearly:**
- The wire protocol is frozen against a contract validated by **one and a half games**, neither of
  which has hidden information. Doc 09 §7 identifies this as "the single most common way this kind
  of project goes wrong".
- The projection security boundary ships to a network with **no scanner**. The first
  hidden-information game then either finds a leak in production or forces a protocol change.
- Effects get their first real executor and their first network consumer in the same phase, so a
  wrong effect shape costs a protocol bump.
- TicTacToe still has no UI, so "adding a game is cheap" remains unproven at the presentation
  layer — which is half the claim.

**Risk:** high, and it is the *specific* risk the architecture documents were written to avoid.

### 27.4 Recommendation — Balanced, and not by default

I am not choosing Balanced because it is the middle option. Three concrete reasons:

1. **The two cheapest PRs in the plan are also the two highest-value ones.** PR-1 (~200 net lines)
   removes the largest architectural drift and unlocks bots, clocks, timers, replay recording and
   the second game. PR-3 (~15 lines) closes a Phase 2 exit criterion and makes the product
   *shareable*. Conservative delays both for a month; Aggressive keeps PR-1 and skips everything
   that would validate the contract it is about to freeze.
2. **The unvalidated contracts are cheap to validate now and expensive later.** `Effect` and
   `project` are the two contracts Phase 4 depends on and neither has been exercised. Validating
   them costs PR-1 and PR-4. Doc 09 §7's warning is not abstract — NC-9 shows how invisible a
   contract-level bug can be to generic harnesses.
3. **Visible progress is real evidence, not a morale exercise.** Running the wasm build found a
   startup panic on a zero-sized viewport, a missing glyph, a low-contrast board, and a slow
   animation — four defects in ten minutes, none of which any test would ever have reported.
   Tabula is early enough that shipping something a person can click is a *verification technique*.

**Balanced. Ten PRs. Phase 4 begins after PR-10.**

---

## 28. Recommended next PRs

### The four answers first

| Question | PR |
|---|---|
| **The next PR** | **PR-1 — generic local match runtime with effect execution.** Highest value per line; unblocks five other PRs. |
| **The next architecture-critical PR** | **PR-1** again. It is the only PR that validates the `Effect` contract before Phase 4 freezes it, and it stops `LocalChessMatch` being copied per game. |
| **The next verification-critical PR** | **PR-4 — the projection scanner.** It is the missing enforcement for the #1 named risk and a Phase-3 blocker. (If you can only do one *cheap* verification thing instead, do PR-8's `DetRng` items — 25 lines that close a confirmed hole in the fairness primitive.) |
| **The next visible-product PR** | **PR-3 — the WASM host page.** ~15 lines for a shareable link. Runner-up: PR-2, which makes "adding a game is cheap" visibly true. |

---

### PR-1 — Extract a generic, effect-executing local match runtime

```text
Goal            Replace LocalChessMatch with LocalMatch<R, P> that owns the imperative-shell
                responsibilities GameRules deliberately lacks: effect execution, logical time,
                timers, rejection surfacing, and an input log.
Why now         `Effect` is a Phase-4 load-bearing contract that has never been executed by a
                shell a user touches (§7.4). The second game would otherwise duplicate the file
                (§11.2). Chess's 1 170 lines of clock code are currently unreachable.
Scope           - LocalMatch<R: GameRules, P: GamePresentation<Rules = R>> in apps/game-client
                - execute SetTimer / CancelTimer / EndMatch / RequestBotMove / Notify;
                  Checkpoint + scope effects are explicit, tested no-ops with TODO(phase 4)
                - a monotone-clamped logical clock advanced from FrameCtx::now_ms
                - a TimerQueue ordered by (deadline, TimerId); timer wins ties with player input,
                  matching testkit::selfplay
                - return Result<Applied, RuleError> so the presenter can play `invalid-action`
                - Vec<RecordedInput> input log (index, logical_time, canonical bytes)
                - viewer selection made explicit (hot-seat = follow turn)
                - enable a clock in the chess Config and render it (View.clock already exists)
Non-goals       No new crate. No idempotency cache, no durability, no mailbox, no supervision,
                no resume — those are server concerns with no local analogue (§11.3).
                No networking. No registry. No changes to GameRules or Effect (except F-31's
                additive `notice_id`).
Crates          apps/game-client, games/chess (presentation: a clock widget + an action bar),
                crates/tabula-testkit (the new MatchDriverContract)
Skills          rust-functional-core, rust-types-as-proofs, rust-verification-testing
Verification    - a `MatchDriverContract` fixture in tabula-testkit asserting that, for one
                  scripted sequence, a driver assigns the same input indices, fires timers in the
                  same order, executes effects in the same order, and reaches the same final
                  state hash. `selfplay` must pass it too, so the two cannot drift (F-30).
                - a test that a timer set by `create` actually fires and ends a tictactoe match
                - a test that a rejected command surfaces a RuleError and consumes an index
                - existing game-client tests, ported
Exit criteria   Chess hot-seat runs with a real Fischer clock that visibly counts down and can
                flag. `grep -c "Chess" apps/game-client/src/lib.rs` is 0 outside the wiring in
                `main.rs`. Every Effect variant has an executor or an asserted deliberate no-op.
Risk            Over-abstraction. Guard: no new crate, no trait beyond what two games need,
                and delete `LocalChessMatch` in the same PR rather than leaving both.
```

### PR-2 — TicTacToe presentation on the shared runtime

```text
Goal            A second game, visually playable, through exactly the same driver.
Why now         "Adding a game is cheap" is the product thesis and it is unproven at the
                presentation layer. Two games is also the minimum that justifies PR-1's
                abstraction — the brief's own rule.
Scope           games/tictactoe/src/ui.rs: a real `impl GamePresentation` (~200 lines).
                3x3 grid, X/O marks, hover + legal-target highlight, click → Command::Place,
                a "place" cue, a11y via TicTacToeRules::describe (fill in the current
                `unsupported()` stub). A --game selector in main.rs.
Non-goals       No animation timeline, no drag, no focus graph beyond nine cells.
Crates          games/tictactoe, apps/game-client
Skills          rust-functional-core (Local vs State), rust-verification-testing
Verification    - render-list insta snapshots for initial / mid-game / won / drawn
                - input→intent unit tests for every cell, including out-of-bounds clicks
                - a test that BOTH games run through one LocalMatch instantiation
                - headless rasterization of one tictactoe scene, committed as a PNG golden
                  (this gives renderer-headless its first real game scene)
Exit criteria   `cargo run -p tabula-game-client -- --game tictactoe` is playable; zero lines
                of runtime code are game-specific.
Risk            Low. The chess presenter is the template.
```

### PR-3 — WASM host page

```text
Goal            Chess playable in a browser from a checked-in page.
Why now         ~15 lines closes a Phase 2 exit criterion. Verified achievable during this audit.
Scope           apps/game-client/web/{index.html, mq_js_bundle.js (vendored, version-pinned)};
                a `just wasm-serve` recipe; make the frame loop skip a zero-sized viewport
                instead of `expect`ing (the observed startup panic, §25.3);
                re-enable `xtask check-bundle-size` or delete the commented CI line.
Non-goals       No Leptos shell, no routing, no deployment.
Crates          apps/game-client, justfile, .github/workflows/ci.yml
Skills          rust-verification-testing (cross-target), rust-types-as-proofs (the viewport fix)
Verification    - CI already `cargo check`s wasm32; add a build of the wasm-release profile
                - assert the gzipped artefact is under the doc 01 §7 budget
                - a manual smoke checklist committed to docs/ui/
Exit criteria   A documented two-command local flow produces a playable board in a browser.
Risk            Vendoring `mq_js_bundle.js` couples us to a macroquad version. Pin it next to
                the Cargo dependency and note it in the file header.
```

### PR-4 — Projection scanner and a fixture that proves it works

```text
Goal            Make I-5/I-6 mechanical.
Why now         Last unvalidated contract; #1 named risk; Phase-3 blocker; must exist before
                `games/cards` so the scanner is not shaped to pass an existing projection.
Scope           - implement `assert_no_leaks::<R: SecretModel>` (token containment over
                  project() and view_event() for every unauthorized viewer, including
                  Spectator(Live), Spectator(Delayed) and every non-owning Seat)
                - implement `assert_no_event_bypasses_redaction`
                - add `SecretModel::scramble_secrets(&mut State, &mut DetRng)` and a
                  *noninterference* property: scrambling only secret data must not change an
                  unauthorized viewer's canonical projection bytes (§8.4)
                - wire both into `conformance!` behind `capabilities.hidden_information`
                - settle the `Secret::tokens` granularity TODO
                - a deliberately leaky fixture game in tabula-testkit/tests/, in the style of
                  conformance_catches_violations.rs
Non-goals       No real hidden-information game. No cards.
Crates          crates/tabula-testkit
Skills          rust-property-testing (noninterference), rust-types-as-proofs, rust-verification-testing
Verification    The meta-test IS the verification: a fixture whose projection leaks must make
                the scan fail, and a fixture whose view_event returns Some where it must return
                None must fail the bypass check.
Exit criteria   `todo!()` is gone from projection.rs; both scans fail on the leaky fixture and
                pass on chess/tictactoe; AGENTS.md §5's instruction becomes executable.
Risk            Token granularity guessed wrong without a real game. Mitigate by keeping the
                granularity a parameter of `Secret`, not a hard-coded rule.
```

### PR-5 — Local bots and replay recording from real play

```text
Goal            Single-player chess and tic-tac-toe, and a .tbr from every session.
Why now         Both are nearly free once PR-1 executes effects. Turns hot-seat into a product
                and turns every manual test into a permanent regression artefact (F-26).
Scope           - bind Effect::RequestBotMove to ChessBot / Heuristic; bot answers via
                  Input::Player through the same apply
                - a seat-occupancy selector (human / bot / bot level)
                - write the recorded input log as a .tbr on demand, into tests/replays/manual/
                - `xtask selfplay --write-failing-replay <dir>` at the CLI layer
Non-goals       No new bot strength. No matchmaking.
Crates          apps/game-client, games/*, xtask, crates/tabula-testkit
Skills          rust-replay-differential-testing, rust-verification-testing
Verification    - live→record→replay→identical final canonical state hash, as a test over a
                  scripted session
                - commit at least 3 replays per game: a normal game, an edge case (en passant or
                  promotion for chess; a draw for tictactoe), and a timeout
                - `cargo xtask replay --all` runs the whole corpus (implement `--all` properly)
Exit criteria   A human plays a bot, saves a replay, and `cargo xtask replay <file> --verify`
                reproduces the exact final hash.
Risk            Low.
```

### PR-6 — Cross-target determinism vectors

```text
Goal            Give "determinism is the product" its first cross-target evidence.
Why now         It is the strongest claim in the architecture and has zero evidence (§9.3).
                Cheap while there are only two games.
Scope           `xtask determinism-vectors` emits committed JSON of
                (game, seed, scripted inputs, per-checkpoint StateHash, final StateHash);
                a wasm-bindgen-test harness replays the same JSON on wasm32 and compares;
                a CI matrix job runs the native emitter on aarch64.
Non-goals       No new game. No protocol.
Crates          xtask, crates/tabula-testkit, .github/workflows
Skills          rust-replay-differential-testing
Verification    Byte-equal hashes across x86-64 native, aarch64 native, and wasm32.
Exit criteria   A red CI job if any target diverges.
Risk            Discovering a real divergence. That is the point, and finding it now with two
                games is enormously cheaper than finding it with five and a live database.
```

### PR-7 — First property tests

```text
Goal            Close F-01 with the five laws that have no other oracle.
Scope           Implement strategies::input_sequence on top of legal_commands as its own TODO
                specifies (reachable states from a generated legal prefix + hostile injection).
                Then: (a) rejected apply leaves canonical bytes unchanged over reachable states;
                (b) two runs of a generated sequence yield identical state hashes;
                (c) every legal_commands entry is accepted by apply, over reachable states;
                (d) live-evolve == replay for generated sequences; (e) asset resolution against
                a naive reference resolver.
Non-goals       proptest-state-machine (defer until hand-rolled shrinking proves inadequate,
                §17.7). No new verification framework.
Crates          crates/tabula-testkit, crates/tabula-assets, games/*
Skills          rust-property-testing, rust-replay-differential-testing
Verification    Every failure must shrink to a minimal counterexample and be committed as a
                deterministic regression test.
Exit criteria   Non-placeholder strategies; ≥ 5 laws under proptest; doc 00 §7's I-2/I-7
                enforcement column becomes true.
Risk            Slow suites. Cap PR-level cases (64–256) and run large runs nightly.
```

### PR-8 — Verification hardening from this audit's mutation and negative-control results

```text
Goal            Convert confirmed gaps into permanent tests.
Scope           - DetRng: zone-is-largest-multiple unit test; 2-element shuffle test;
                  Kani harness over all n: u32 (§16.5); MatchSeed Debug redaction test
                - tictactoe: from_parts rejection table (one row per predicate);
                  bot unit tests; unknown-timer / TIMER_MOVE tests;
                  view_event returns Some for every variant
                - the exhaustive TicTacToe reference model (§20.1) — subsumes both Kani R2
                  harnesses; retire or relabel `concrete_opening_place_is_accepted`
                - add `kani::cover!` to the two remaining R2 harnesses
                - `.cargo/mutants.toml`: `exclude_re = ["verification::"]` (verified)
                - `assert_deterministic` must panic instead of returning on create failure (F-20)
                - inline the domain byte-strings in the preimage tests (F-21)
                - clippy.toml for tabula-game-api and tabula-protocol (F-08)
                - game.toml ↔ compiled metadata cross-check (F-09)
                - split nightly CI into jobs that exist and an `if: false` Phase-4 placeholder (F-10)
Skills          rust-mutation-testing, rust-kani, rust-replay-differential-testing
Verification    Re-run both mutation campaigns; survivors must drop and every remaining one must
                be classified in a committed note.
Exit criteria   tabula-core survivors < 12 (from 30) and tictactoe < 20 (from 80), with the
                remainder classified EQUIVALENT / UNREACHABLE / LOW-VALUE.
Risk            Low.
```

### PR-9 — One real chess asset pack, end to end

```text
Goal            Prove the asset contract against real bytes, and close two Phase 2 criteria.
Scope           Six placeholder piece sprites (one 2x atlas) + two .ogg cues (move, capture);
                `xtask pack-assets chess`; load through AssetSource + load_verified;
                register cues in MacroquadAudioSink; draw sprites via RenderCmd::Sprite;
                a light/dark theme toggle and a reduced-motion toggle while in the file.
Non-goals       No CDN, no cache eviction, no HTTP source, no font assets.
Crates          crates/tabula-assets, crates/renderer-macroquad, games/chess, xtask, assets/packs
Skills          rust-types-as-proofs, rust-verification-testing
Verification    - the pack round-trips through the integrity boundary at runtime
                - a corrupted byte in the pack is rejected at load, not at draw
                - a headless golden image of the board WITH sprites
                - a test that a missing cue is a non-authoritative failure that cannot stall play
Exit criteria   A chess board with real pieces and audible move/capture sounds, in the browser.
Risk            The manifest shape needs revision. That is the point of doing it now.
```

### PR-10 — Phase 4 erasure spike (throwaway)

```text
Goal            De-risk the largest Phase-4 unknown for one day's work.
Scope           On a scratch branch: write GameAdapter<M> and the ErasedGame/ErasedMatch traits
                sketched in tabula-registry's doc comment; instantiate them for ChessModule and
                TicTacToeModule; drive a full tictactoe match through Box<dyn ErasedMatch>.
                Answer, in writing: does it compile object-safely; where does the Codec parameter
                actually need to appear; do CanonicalBytes and WireBytes need to be distinct
                types (§12.2); what does multi-version linking cost in practice (§12.3).
Non-goals       Do not merge the code. Merge the notes and an ADR.
Skills          rust-types-as-proofs, rust-functional-core
Verification    The spike compiles and drives one match. That is the whole test.
Exit criteria   An ADR answering the four questions above, so Phase 4 does not start by
                discovering them.
Risk            None — it is thrown away.
```

---

## 29. PR / nightly / release verification pyramid

The governing rule: **per-PR feedback must be cheap, deterministic, and attributable.** Anything
that samples, anything that takes minutes, and anything research-grade goes to nightly.

### 29.1 Every PR (target: under 10 minutes wall clock — doc 01 §6.1)

Already there and correct:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace                       # 533 tests, 30 s today
cargo xtask check-deps                              # I-1, I-15  (proved live: NC-1)
cargo xtask check-no-game-ids                       # I-9        (proved live: NC-2)
cargo xtask check-manifests
cargo xtask gen-tokens + git diff --exit-code       # generated artefacts current
cargo xtask check-no-raw-colors                     # proved live: NC-10b
cargo deny check
cargo check --workspace --no-default-features / --all-features
cargo check -p tabula-game-client --target wasm32-unknown-unknown
```

**Add:**

| Addition | Cost | Why per-PR |
|---|---|---|
| `cargo nextest run -p tabula-game-chess --test perft` | 1.3 s | NC-9: perft is the *only* detector for a legality regression. It is already in the default suite; keep it there and never move it to nightly. |
| Small property suites (64–256 cases, fixed `PROPTEST_CASES`) | seconds | Deterministic if the case count and seed policy are pinned. |
| The exhaustive TicTacToe reference model | < 1 s | Full-tree differential coverage for the price of a unit test. |
| The `MatchDriverContract` fixture | ms | Stops local/selfplay/actor semantics drifting. |
| `cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release` + gzip size assertion | ~15 s | It is the only cross-target signal that exists; today CI only `cargo check`s. |
| `cargo fuzz build` (once targets exist) | ~30 s | A fuzz target that stops compiling is how fuzzing dies. Build only; never run in PR CI. |
| `xtask check-ai-doc-contracts` (the bundled `ai_doc_contracts.py`) | ms | The `@ai.evidence` links are accurate today; keep them that way mechanically. |
| Compile-fail tests (the `GamePresentation` doctest already exists) | included | Type-level proofs deserve a named home. |

**Never per-PR:** Kani (72 s solver time and growing), mutation campaigns (3 min per package),
fuzzing runs, 100 k self-play, depth-5+ perft.

### 29.2 Nightly / scheduled

Rewrite `nightly.yml` so every job runs a command that exists (F-10):

| Job | Command | Notes |
|---|---|---|
| replay corpus | `cargo xtask replay --all --verify` | implement `--all`; today it is referenced and undocumented |
| self-play (matrix) | `cargo xtask selfplay {tictactoe,chess} --matches 100000` and chess × {fischer, bronstein, none} | measured: 5 000 bronstein chess matches ≈ 105 s, so 100 k ≈ 35 min per clock mode — **shard it or reduce to 25 k per mode per night** |
| deep perft | `cargo nextest run -p tabula-game-chess --test perft -- --ignored` | the depth-5 test exists and nothing schedules it |
| Kani | `cargo kani -p tabula-core` and `cargo kani -Z stubbing -p tabula-game-tictactoe` | ~72 s + compile |
| mutation | `cargo mutants -p <one package per night, rotating>` with `--in-diff` on PR branches for touched code | 3 min per package today |
| fuzzing | `cargo fuzz run replay_container / asset_manifest -- -max_total_time=600` | once PR-8/PR-9 land |
| large property runs | `PROPTEST_CASES=10000 cargo nextest run` | |
| cross-target hashes | native x86-64 vs aarch64 vs wasm32 | PR-6 |
| `cargo udeps` | needs an explicit nightly toolchain step | currently silently broken |
| Phase-4 placeholders | `if: false`, clearly labelled | so nobody is trained to ignore red |

### 29.3 Phase exit / release

| Check | Why it belongs here |
|---|---|
| **Manual gameplay scenario script**, committed under `docs/ui/`, executed and signed off | §25 shows manual play finds a class of defect no test reports. Make it a checklist, not folklore. |
| Full golden replay corpus re-verified, including manual recordings | I-8 over everything, not a sample |
| Cross-platform determinism on every supported target | the product claim |
| Full mutation campaign across all packages, with every survivor classified in a committed note | assertion strength as a gate, not a metric |
| **Security projection audit**: scanner green + a human review of every `project`/`view_event` + the per-game information model in `docs/games/<slug>.md` | doc 02 §7.1 requires the document; no game has one |
| Migration check: every stored `rules_version` either replays exactly or is explicitly marked unreplayable | I-16; the honest-failure rule |
| Load tests (Phase 4+) | doc 06 §10 |
| WASM bundle budget | doc 01 §7 |

### 29.4 One rule to keep

> **Do not move perft to nightly.** It is 1.3 s, and NC-9 proved it is the only per-PR detector
> for the defect class that matters most in the only complex game we have.

### 29.5 Choosing the tool by risk, not by fashion

Pick the **lightest** tool that gives strong evidence for *that* failure mode.

| Domain | Failure impact | Best verification | Why that one |
|---|---|---|---|
| **Time arithmetic** (`LogicalTime`, `Millis`) | A wrapped duration produces a nonsense clock that looks plausible; replay diverges months later | **Kani** (already done, exemplary) | The domain is 2⁶⁴/2¹²⁸, branchless, and has an independent `checked_*` oracle. No test can cover it; the proof costs 0.35–18 s. |
| **RNG rejection zone / termination** | Modulo bias in every shuffle and dice roll — an unprovable cheating accusation; a hung actor | **Kani** over all `n: u32`, plus 2 unit tests | Full `u32` domain, one arithmetic invariant, and the loop's termination *is* the invariant. Sampling cannot see a 1-in-10⁹ bias (§9.1). |
| **RNG stream stability** | Every stored replay invalidated silently | **Frozen vectors + documented-preimage reconstruction** (done) | The value must not change; a captured literal is exactly the right shape. Add the constant to the preimage test (F-21). |
| **Input ordering / index assignment** | Replay diverges; RNG domain reuse | **A shared driver conformance fixture** (`MatchDriverContract`) | Three implementations must agree; a contract test is the only thing that keeps them aligned. Kani would be overkill; a property test would not compare implementations. |
| **Transactional rejection (R2)** | Silent match corruption surfacing weeks later as a replay divergence | **Canonical-byte comparison in the conformance suite** (done, non-opt-in) + **exhaustive model for tictactoe** + **property test over reachable states for chess** | Byte comparison is the actual statement. Kani adds little beyond a loop test unless the state is symbolic (§16.4). |
| **Projection secrecy** | Unfixable post-launch information leak; the #1 named risk | **Property-based noninterference** + token containment scan + human information model | A scan finds wholesale leaks; only noninterference finds derived ones; only a human finds the ones that are *intended* to be derivable (§8.3). |
| **Chess legality** | Illegal games; divergence against any other engine; ratings corrupted | **perft against published counts** — nothing else | NC-9: conformance, determinism and 200 self-play matches all passed with a real legality bug. The oracle must come from outside the codebase. |
| **Chess clocks** | Wrong flags, wrong outcomes, unfair ranked games | **Bounded exhaustive reference model** (done) | The domain has boundary cases at every increment; a small triple loop is exhaustive and needs no framework. |
| **Asset integrity** | Corrupted or substituted art/audio silently rendered | **Type-level witness (`VerifiedAssetBytes`) + mutation testing** (done: 13/13 killed) | The invariant is "bytes were checked"; a private-constructor witness makes it unforgeable and mutation proves the check is asserted. |
| **Asset path validation** | Path traversal / cache poisoning at Phase 4 | **Property test against a naive reference validator**, not fuzzing | The input space is short strings; a property test checks acceptance *and* rejection, which a no-panic fuzzer does not. |
| **Asset manifest parsing** | A hostile CDN response crashes or exhausts the client | **cargo-fuzz** + a dictionary | TOML + 15 validated newtypes + uniqueness rules is a grammar with untrusted input. |
| **Replay container decode** | A user-supplied `.tbr` crashes or exhausts a support tool | **cargo-fuzz** with resource bounds | Same reason; and the caps already in the code are exactly what a fuzzer attacks. |
| **Protocol decode** (Phase 4) | Remote crash from a hostile client | **cargo-fuzz** + golden vectors + version negotiation tests | Untrusted bytes from an assumed-hostile source. |
| **Match actor idempotency** (Phase 4) | Double-applied commands under load; ordering corruption | **Loom** for the cache/mailbox interaction + integration tests + load scenarios | Schedule-sensitive; Loom is the only tool that enumerates interleavings. Not before the code exists. |
| **Reconnect / resume** (Phase 4) | Wrong position after reconnect; lost clock | **Integration tests against fakes**, then a real database | It is an I/O protocol, not an algorithm. |
| **State migration** | Fake replays — the one thing doc 05 forbids outright | **Golden replays per `rules_version` + explicit unreplayable marking** | The correct behaviour is *refusing*, and only stored artefacts from the old version can test that. |
| **Cross-target determinism** | The core product claim is false and nobody knows | **Cross-target hash comparison** | No amount of single-target assertion can observe another architecture. |
| **Anything with `unsafe`** | UB | **Miri** | Not applicable: `#![forbid(unsafe_code)]` workspace-wide, no FFI. |

**Two rules that follow from the table:**

- *Do not use Kani where perft or a finite exhaustive model is stronger.* TicTacToe's whole tree is
  5 478 positions; enumerate it (§20.1).
- *Do not fuzz where a finite or typed domain makes a property test a better oracle.* Fuzzing
  proves "does not crash"; a property test proves "accepts exactly the right things".

---

## 30. Residual risks

### 30.1 Risks this audit could not close

| Risk | Why it stays open | Cheapest way to close it |
|---|---|---|
| **Cross-target determinism is unverified.** | No second architecture was available; the wasm build was run but no hashes were compared. | PR-6. |
| **Macroquad's real ceiling is unknown.** | One data point found (missing U+2014 glyph, §25.3). Text shaping, render targets, and mobile input remain unmeasured. | Keep the Phase-2 workaround log doc 09 §3.2 asks for — it currently has zero entries and now has at least one to record. |
| **Performance is entirely unmeasured.** | No frame-time measurement, no `apply` p99, no 60 fps evidence, no bundle-budget gate. `Ctx::budget` is soft and never reported. | Emit `apply` micros and frame time in a debug HUD during PR-1; it is nearly free once the runtime is generic. |
| **Nightly CI's actual state is unknown.** | The workflow contains jobs that cannot succeed (F-10); I could not observe run history. | Fix the workflow, then look at the history. |
| **`tabula-assets`' manifest shape is unvalidated against real content.** | No real asset exists to validate it with. | PR-9. |
| **Phase-4 designs are documentation only.** | Nothing to audit but prose; the prose is good. | PR-10's throwaway spike. |

### 30.2 Looking beyond the roadmap — irreversible traps

The purpose here is not feature design; it is to check whether today's architecture *forecloses*
anything expensive. Result: **it forecloses very little, and the two real constraints are both in
the versioning story.**

| Future requirement | Does today's architecture block it? | Note |
|---|---|---|
| **Go, Shogi, Xiangqi** | No. Larger boards, different pieces; `State` is game-owned. Go's ko/superko is repetition detection, which chess already models with a Zobrist `PositionKey` ring. | Go's 19×19 board with scoring at the end pushes `StateSizeClass`, not the contract. |
| **Mahjong** | No. Hidden hands + a wall + a discard pile = cards' shape with more seats. | Needs the projection scanner first. |
| **Drafting / simultaneous card selection** | No. A phase in which all seats may act, closed by a timer (§7.2). | The hazard is arrival-order-dependent outcomes; document it per game. |
| **Deck-building (persistent deck across rounds within a match)** | No — it is match state. **Across matches** would be cross-game state, which doc 00 §1.2 forbids by design and routes through platform services. | Correct call; keep it. |
| **Cooperative games (all-win / all-lose)** | No. `MatchOutcome` with equal ranks already expresses it; `OutcomeKind::Draw` may need a `Cooperative` sibling. | Additive. |
| **Tournaments / brackets** | No, and correctly *above* the match. Rooms + matches + ratings already model it (doc 09 §3.3). | Risk: someone adds `Input::Tournament`. Write the rule down (§7.2). |
| **Matchmaking** | No. ADR-023 keeps it reading only `GameCapabilities`. | |
| **Spectators, delayed spectators** | No. `Viewer::Spectator(SpectatorTier)` exists; the buffering is a platform port that does not exist yet. | |
| **Async correspondence (24 h turns)** | No. Proven by the `LogicalTime` Kani harnesses over the full `u64` domain. | |
| **Resumable mobile sessions / offline play** | No. Snapshot + input suffix is total. Offline single-player already works today (the local runtime). | |
| **Dedicated tournament servers with a game subset** | No. `register!` is specified to support per-game cargo features. | |
| **Downloadable third-party games** | Deferred (ADR-007 Phase C). `ErasedMatch` is already an ABI-shaped boundary. Doc 02 §9.3 explicitly forbids Phase C concerns from shaping Phase A signatures — a discipline worth keeping. | |
| **Mods / custom rule packs** | Partially blocked, and this is the interesting one. `Config` is per-game and typed, so *parameterised* variants are fine. *Structural* rule changes are a new `RulesVersion`, i.e. a new build. A user-authored rule pack would need Phase C. | Acceptable; not a trap, a decision. |
| **Multiple rules versions live simultaneously** | **This is the one real constraint** (§12.3). `RULES_VERSION`/`RULES_HASH` as associated consts, with `RULES_HASH` derived from a source *directory*, means one crate per live version per game. Workable, unpriced. | Write the ADR before Phase 4. |
| **A second language implementing a game** | Blocked by ADR-008's tagged-opaque-payload design being Rust-typed inside the module. That is intentional (ADR-001) and should stay. | |

**No irreversible trap found.** The two things to write down before Phase 4 are the
multi-version-linking ADR and the "undo/takeback is a game command, never a log operation" rule.

### 30.3 Risks introduced by acting on this audit

| Recommendation | Risk if done badly | Guard |
|---|---|---|
| PR-1 generic runtime | Premature abstraction — a crate and a trait hierarchy before three consumers | No new crate; delete `LocalChessMatch` in the same PR; the abstraction must be justified by chess **and** tictactoe, both merged. |
| PR-4 projection scanner | Token granularity guessed wrong without a real hidden-information game; the scanner gets shaped to pass | Keep granularity a parameter of `Secret`; prove the scanner with a *deliberately leaky* fixture, as `conformance_catches_violations.rs` already does for four other invariants. |
| PR-7 property tests | Flaky or slow suites; properties that re-implement the transition | Pin case counts per PR; forbid oracles that call the code under test; every failure becomes a committed minimal regression. |
| PR-9 real assets | Reopening asset-library work indefinitely | Scope it to *one* pack and *one* renderer path. Anything the pack reveals becomes a follow-up issue, not a rewrite. |
| Adding `exclude_re` to mutants config | Hiding real mutants behind a naming convention | Only `verification::` (the Kani-harness module name), documented, and re-checked whenever a new harness module is added. |

---

## 31. Final recommendation

### 31.1 The ten questions, answered explicitly

**1. Is Tabula's current core architecture fundamentally sound?**

**Yes.** After reading every crate and running eighteen negative controls, I found no structural decision
worth reversing. The functional core is genuinely pure, the single ordered input stream genuinely
buys everything doc 00 claims for it, the projection types are genuinely separated, the dependency
matrix is genuinely enforced, and the determinism primitives are genuinely frozen and tested. The
problems are **gaps and misallocation**, not design errors. Preserve the invariants; do not
introduce flexibility nobody needs.

**2. Which architectural decision currently provides the most leverage?**

**The single totally-ordered `Input` stream (ADR-003).** Replay totality, deterministic timers,
disconnect/AFK ownership, bots-as-ordinary-seats, hostile-input self-play, and the fact that
rejection is a total no-op with no rollback machinery are all *consequences* of it, not separate
features. Its runner-up is the per-input `DetRng::for_input(seed, index)` derivation, which is what
makes the "rejection is a no-op" property free.

**3. Which decision is most likely to become a future constraint?**

**`RULES_VERSION` and `RULES_HASH` as associated consts on a `Sized` trait, with `RULES_HASH`
derived from a source directory.** Doc 02 §9.2 promises simultaneous multi-version linking; the
current shape makes that one crate per live version per game, with knock-on cost in `deps.toml`,
`check-manifests`, `check-no-game-ids`, and every verification campaign. Not a defect — an unpriced
commitment. Write the ADR before Phase 4.

**4. What part of the system currently has the strongest verification evidence?**

Two, in order:

- **Chess move generation.** Perft against published external node counts is the only oracle in the
  repository that comes from outside the codebase, and NC-9 demonstrated it catches a real legality
  bug that nothing else does.
- **`DetRng` + canonical encoding.** Frozen stability vectors, plus tests that rebuild the
  documented preimage by hand, plus 3.1M self-play inputs replaying byte-identically.

Honourable mention: `tabula-assets::integrity` — a private-constructor witness type with 13/13
mutants killed. The best-verified module in the repository, verifying data that does not exist yet.

**5. What looks most verified but actually has weak evidence?**

**Rejection transactionality described as "formally verified", and the "state is reachable"
boundary.**

- The Kani R2 harnesses prove field-level R2 for **two concrete tic-tac-toe states** over
  `(u8, u8)` — 65 536 cases, coverable exhaustively by an ordinary loop in microseconds, and
  comparing five hand-listed fields rather than canonical bytes. The comments are honest; the
  summary sentence people will carry away is not.
- `docs/verification/core-domain-boundaries.md` lists "Tic-tac-toe state is reachable" as a proof
  boundary. Its validator, `State::from_parts`, has **35 surviving mutants** — nearly every
  predicate in it is unasserted.

Two runners-up: "projection safety is checked" (it is projection *determinism*), and "we have
property tests" (there are none).

**6. Should we continue investing in assets now, or prioritise a fully playable local runtime?**

**Prioritise the runtime. Freeze asset-library expansion immediately.** The asset subsystem is
~5 400 lines producing zero observable behaviour and has no runtime consumer; the runtime work is
~500 lines that unlocks clocks, timers, bots, replay recording, a second game, and the first
execution of the `Effect` contract. Return to assets with **one real pack, end to end** (PR-9),
which will teach more than the next thousand lines of manifest validation.

**7. Should Phase 4 multiplayer begin soon, or is there still a contract-validation gap?**

**There is still a gap, and it is exactly the gap doc 07 Phase 3's exit criteria describe.** Two
Phase-4 load-bearing contracts have never been exercised: `Effect` (no shell executes one) and
`project`/`view_event` (no game has secrets and the scanner is `todo!()`). Both close in 3–4 PRs.
Starting Phase 4 first means freezing a wire protocol against contracts validated by one and a
half games — the failure doc 09 §7 names as "the single most common way this kind of project goes
wrong".

**8. What should the next five PRs be?**

1. **Generic local match runtime with effect execution** (§28 PR-1) — architecture-critical.
2. **TicTacToe presentation on that runtime** (PR-2) — proves the product thesis at the
   presentation layer.
3. **WASM host page** (PR-3) — ~15 lines, closes a Phase 2 exit criterion, makes the product
   shareable.
4. **Projection scanner + a leaky fixture that proves it works** (PR-4) — verification-critical.
5. **Local bots + replay recording from real play** (PR-5) — turns hot-seat into a product and
   every manual session into a permanent artefact.

**9. Which verification skills should agents consult for each?**

| PR | Skills, in order |
|---|---|
| PR-1 runtime + effects | `rust-functional-core` → `rust-types-as-proofs` → `rust-verification-testing` |
| PR-2 tictactoe UI | `rust-functional-core` (Local vs State) → `rust-verification-testing` (render-list snapshots, input→intent partitions) |
| PR-3 wasm host page | `rust-verification-testing` (cross-target) → `rust-types-as-proofs` (the zero-viewport fix) |
| PR-4 projection scanner | `rust-property-testing` (noninterference) → `rust-types-as-proofs` → `rust-verification-testing` |
| PR-5 bots + replay recording | `rust-replay-differential-testing` → `rust-verification-testing` |
| PR-6 cross-target vectors | `rust-replay-differential-testing` |
| PR-7 property tests | `rust-property-testing` → `rust-replay-differential-testing` |
| PR-8 verification hardening | `rust-mutation-testing` → `rust-kani` → `rust-replay-differential-testing` |
| PR-9 real asset pack | `rust-types-as-proofs` → `rust-verification-testing` → `rust-fuzzing` (manifest target) |
| PR-10 erasure spike | `rust-types-as-proofs` → `rust-functional-core` |

The router (`rust-verification-testing`) should be consulted first in every case; it exists to send
the agent to the right specialised skill rather than to contain all of them.

**10. What can we currently trust about Tabula — and exactly where does that trust stop?**

**Trust these, and say why:**

| Claim | Trust level | Because |
|---|---|---|
| Chess move generation is correct through practical depths | **High** | Published perft, external oracle, 4 positions; NC-9 proved it is a live detector |
| The same ordered inputs reproduce byte-identical state **on this architecture** | **High** | 3.1M self-play inputs + golden replays + two independent runs per conformance check |
| A rejected input is a total no-op **for the paths tested** | **High** | Non-opt-in canonical-byte comparison; NC-3b and NC-3c both fired |
| `DetRng` and the canonical encoding will not change silently | **High** | Frozen vectors + documented-preimage reconstruction; NC-8 fired |
| Layer-1 crates cannot reach a renderer, a runtime, or a database | **High** | Resolved-graph enforcement; NC-1 fired with transitive paths |
| No platform crate branches on a game | **Medium-high** | NC-2 fired; but the check is a grep and cannot see a semantic branch |
| The renderer is genuinely replaceable | **Medium-high** | A second backend exists and rejects what it cannot draw |
| Asset bytes cannot be used without integrity verification | **Medium-high** | Private-constructor witness; 13/13 mutants killed — but zero real bytes have passed through it |
| Chess clocks are correct | **Medium** | Excellent rules tests and bounded reference models; **no client has ever run one** |
| The conformance suite makes a new game safe | **Medium** | It makes it *deterministic and transactional*. NC-9 shows it does not make it *correct* |

**Trust stops, precisely, here:**

1. **At the projection boundary.** The types separate `View` from `State`; nothing checks what is
   *inside* the view. Zero mechanical secrecy evidence exists.
2. **At the architecture boundary.** Determinism is verified on x86-64 native only. The
   cross-platform half of the product's central claim has never been tested.
3. **At the shell boundary.** No `Effect` has been executed by anything a user touches; no timer
   has fired in a client; no clock has run; no replay has been recorded from real play.
4. **At the word "formally".** Six Kani harnesses exist. Three are excellent and unbounded over
   `u64`/`u64²`. Three are narrow — two concrete states over `(u8, u8)` with a stub, and one
   concrete unit test. Say *"harness H proves proposition P over domain D under assumptions A with
   bound B"*, never *"crate X is formally verified"*.
5. **At the manifest.** `game.toml` and the compiled metadata are two sources of truth and only one
   field of about thirty is compared.
6. **At assets.** Everything about the asset subsystem is verified except that it works.

### 31.2 The one-paragraph direction

Tabula's core is sound and its enforcement is unusually real — eighteen negative controls, twelve
caught with precise, actionable messages. Stop deepening the asset library. Spend the next five PRs turning
the chess vertical slice into a **generic local runtime that executes effects**, put **TicTacToe on
it**, **ship the browser build** (fifteen lines), **implement the projection scanner** before cards
exists, and **record replays from real play**. Then close the verification gaps this audit measured
— property tests, the `DetRng` zone, the TicTacToe exhaustive model, cross-target hashes — and only
then begin Phase 4, with the erasure spike's notes in hand. Do not choose the conservative plan;
running the browser build for the first time found four defects in ten minutes, and at this stage
of the project **shipping something a person can click is itself a verification technique.**

---

## Appendix A — Reproducing this audit

```bash
# baseline
cargo nextest run --workspace
cargo xtask check

# determinism at scale
cargo build --release -p xtask
./target/release/xtask selfplay tictactoe --matches 20000
./target/release/xtask selfplay chess --matches 5000 --clock bronstein
./target/release/xtask selfplay chess --matches 3000 --clock none

# formal + mutation
cargo kani -p tabula-core
cargo kani -Z stubbing -p tabula-game-tictactoe
cargo mutants --package tabula-core
cargo mutants --package tabula-game-tictactoe

# deep chess oracle
cargo nextest run -p tabula-game-chess --test perft -- --ignored

# the browser (until PR-3 lands, this is manual)
cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release
mkdir -p /tmp/tabula-web && cd /tmp/tabula-web
cp ~/.cargo/registry/src/*/macroquad-0.4.16/js/mq_js_bundle.js .
cp <repo>/target/wasm32-unknown-unknown/wasm-release/tabula-game-client.wasm .
cat > index.html <<'HTML'
<!DOCTYPE html><html><head><meta charset="utf-8"><title>Tabula</title>
<style>html,body{margin:0;background:#111}canvas{display:block;margin:0 auto}</style></head>
<body><canvas id="glcanvas" width="720" height="720" tabindex="1"></canvas>
<script src="mq_js_bundle.js"></script><script>load("tabula-game-client.wasm");</script>
</body></html>
HTML
python3 -m http.server 8731     # then open http://127.0.0.1:8731/
```

**The canvas must have explicit `width`/`height`.** Without them macroquad reports a zero-sized
viewport on the first frame and `main.rs:29` panics — see §25.3 and PR-3.

## Appendix B — Finding index

| ID | Sev | Summary | Section |
|---|:--:|---|---|
| F-01 | P1 | No property tests exist; `strategies` is a placeholder | §17, §22 |
| F-02 | P1 | Projection secrecy scanner is `todo!()`; I-5/I-6 unenforced | §8, §22 |
| F-03 | P1 | `LocalChessMatch` is chess-shaped and executes no effects | §11, §22 |
| F-04 | P1 | Asset subsystem: ~5 400 lines, zero data, zero consumers | §3, §26, §22 |
| F-05 | P2 | `DetRng::below` zone and 2-element `shuffle` are unasserted | §9.1, §18, §22 |
| F-06 | P2 | `DetRng::below`'s unbounded loop has unproven termination | §9.1, §18, §22 |
| F-07 | P2 | `State::from_parts` — 35 surviving mutants under a documented proof boundary | §18, §22 |
| F-08 | P2 | Determinism clippy bans exist in 6 of 26 crates | §22, §24 |
| F-09 | P2 | `game.toml` vs compiled metadata: 1 of ~30 fields cross-checked | §22, §24 |
| F-10 | P2 | Four nightly CI jobs invoke commands that cannot succeed | §19, §22 |
| F-11 | P2 | Phase 2 exit criteria unmet while Phase 3 proceeds | §26, §22 |
| F-12 | P2 | No WASM host page; the browser build had never been run | §25, §22 |
| F-13 | P2 | Client discards `RuleError`; no rejection feedback | §4.1, §25, §22 |
| F-14 | P2 | "projection safety" in self-play is projection *determinism* | §23, §22 |
| F-15 | P2 | Chess `State` has public fields + derived `Deserialize` | §14.2, §22 |
| F-16 | P2 | `legal_commands` embedded in `View` is a future side channel | §6.2, §22 |
| F-17..F-31 | P3 | Stale docs, vacuity holes, tooling gaps, escape hatches, corpus gaps | §22 |

## Appendix C — Negative-control ledger

See §24 for the full table.

| Outcome | Count | IDs |
|---|---:|---|
| **DETECTED** | 12 | NC-1, NC-2, NC-3a, NC-3b, NC-3c, NC-4b, NC-6, NC-7a, NC-8, NC-8b, NC-9, NC-10b |
| **NOT DETECTED** | 4 | NC-4, NC-7b, NC-7c, NC-11 |
| **PARTIAL / COINCIDENTAL** | 1 | NC-5 |
| **NOT DETECTED, BY DESIGN** | 1 | NC-10 (hex inside a string literal is a tested, deliberate exemption) |
| **Total** | **18** | seventeen distinct defects; NC-3c re-runs NC-3b's defect under Kani |

All reverted; `git status` clean before and after.
