# 09 — Synthesis and Decision Register

> The one-page answers, the master decision table, and the places where this plan deliberately
> differs from or sharpens the original brief.

---

## 1. What is the real reusable product?

Not the renderer. Not "a Macroquad board game". The product is a **stack of contracts**, ordered
here by how much value they carry and how expensive they would be to rebuild:

```text
1. A deterministic rules runtime
      (State, Input, Ctx) -> (State', Events, Effects), pure and total
      + a single ordered input stream
      + canonical encoding and state hashing
      → this is what makes replay, audit, bots, and server validation possible at all

2. A game module SDK
      one trait set, one manifest, one conformance suite
      → this is what makes game number ten cost a fraction of game number one

3. An authoritative multiplayer match runtime
      single-writer match actors, ordering, idempotency, versioning, event log,
      snapshots, reconnect, spectators, timers that survive restarts
      → this is the hard engineering that every multiplayer board game platform must have
        and that almost nobody gets right the first time

4. A player projection / hidden-information model
      project() and view_event() as a security boundary, with SecretModel scanning
      → this is a *security product*: it is what lets card and social-deduction games exist

5. Replay
      exact reproduction, two flavors (canonical for audit, projected for players)
      → anti-cheat evidence, bug reproduction, product feature, and the determinism alarm

6. Cross-platform presentation primitives
      RenderList + tokens + motion, one semantic design language across DOM and canvas
      → this is the reach: web, Android, iOS, desktop from one Rust codebase

7. Platform services
      identity, lobby, rooms, matchmaking, chat, presence, ratings, assets, admin
      → table stakes; valuable but not differentiated
```

**The one-sentence version:** Tabula is a deterministic, server-authoritative runtime plus an SDK
that turns "a set of board-game rules" into "a live, secure, replayable, cross-platform multiplayer
game" without touching platform code.

### 1.1 The test of whether we built the right thing

> A developer who has never seen the codebase implements a new board game in a day, and it appears
> in the lobby, is matchmade, plays over the network on four platforms, survives disconnects,
> records replays, keeps its secrets, and updates ratings — **and we changed zero platform code.**

That is Phase 9's acceptance criterion, and it is the real product spec. Everything in these
documents exists to make that sentence true.

---

## 2. What should remain replaceable?

Ordered by how likely the replacement is.

| Component | Replaceable behind | Likelihood we replace it | Cost when we do |
|---|---|---|---|
| **Renderer** (Macroquad) | `RenderList` + `Renderer` trait | **High** — Macroquad's text/render-target ceiling is a known risk | One crate; games untouched |
| **Voice provider** | `VoiceService` trait | **High** — cost and quality will drive it | One adapter |
| **Asset CDN** | `AssetSource` + content-hashed URLs | Medium | Config change |
| **Deployment platform** | one binary + Postgres, no platform coupling | Medium | Terraform/systemd change |
| **Database deployment model** (host, replicas, pooling, partitioning) | `tabula-storage` ports | Medium | One crate |
| **Redis** (once introduced) | directory/presence/pubsub ports | Medium | Ports already exist as the in-process implementation |
| **Web UI framework** (Leptos) | `tabula-protocol` types + tokens.css | Low–Medium | The shell only; protocol and design survive |
| **Desktop/mobile shell** (Tauri or native) | gameplay never depends on it | Low | Shell only |
| **Audio backend** (Macroquad → kira) | `AudioSink` | Medium | One crate |
| **Wire codec** (Postcard → other) | `Codec` enum + golden vectors | Low | Protocol crate + a client rollout |
| **Matchmaking algorithm** | it reads only capabilities + queue entries | Medium | One module |
| **Rating algorithm** | computed from `MatchOutcome` | Medium | One job |
| **Text rendering / shaping** | `RenderCmd::Text` + `measure_text` | Medium | Renderer crate |

### 2.1 The rule that makes replaceability real

Each item above has a **named seam in the code today**, not a hypothetical one. A "replaceable"
component with no interface is just a component. When adding anything to this list, add the trait
in the same PR.

---

## 3. Master decision register

### 3.1 LOCK NOW

Structural. Code may depend on these. Changing one requires a superseding ADR and an enforcement
update in the same PR (doc 00 §7.1).

| Decision | ADR | Enforced by | Why it is locked |
|---|---|---|---|
| Deterministic, pure, sync game rules | 002 | I-1, I-2, conformance suite | The entire product's value derives from this |
| Single ordered `Input` stream + append-only log | 003 | I-7, I-8, replay tests | Makes replay total; resolves all AFK/disconnect/timer ownership questions |
| Server-authoritative validation | 004 | pipeline design, integration tests | Anti-cheat is not retrofittable |
| `project()` / `view_event()` as the security boundary | 005 | I-5, I-6, `SecretModel` scans | Concentrates leak risk in two reviewable functions per game |
| Platform never branches on `game_id` | — | I-9, `xtask check-no-game-ids` | The marginal-cost-per-game property depends on it |
| Opaque game payloads tagged by `(game_id, game_version)` | 008 | protocol tests | Avoids a mega-enum; keeps games typed |
| Renderer-independent canonical state and presentation | 010 | I-1, I-10, dependency matrix | Renderer is the most likely thing to change |
| Dual codec (Postcard prod / JSON debug) | 009 | golden vectors, subprotocol negotiation | Debuggability of a binary protocol is a productivity multiplier |
| One Tokio task per match, single writer | 006 | I-14, ownership leases | Ordering correctness with minimal machinery |
| Compile-time game registry (Phase A) | 007 | registry crate structure | Type safety now; Phase B/C doors kept open at near-zero cost |
| PostgreSQL as the only Stage-0 datastore; event log + snapshots | 013 | doc 03 §9 | One store, transactional, replay-friendly |
| Modular monolith, one repo, one workspace | 015 | doc 01 §2, doc 06 §7 | A small team cannot afford distribution |
| Voice on a separate plane behind a trait | 016 | `tabula-voice` | Media must never share the game socket's semantics |
| Per-game versioned, hashed asset packs | 017 | `tabula-assets`, manifest | Otherwise app size grows with the catalog |
| Design tokens defined once in Rust, adapted to CSS and Theme | 018 | `xtask gen-tokens`, no-raw-colors lint | One product feel across DOM and canvas |
| Tauri never required for gameplay | 019 | I-15, dependency matrix | Gameplay must not sit in a WebView |
| No k8s/Kafka/NATS/mesh/microservices before a measured need | 020 | doc 06 §1.1 triggers | Operational tax paid daily, benefit received rarely |
| `#![forbid(unsafe_code)]` in rules; canonical hashing | 021 | workspace lints | Determinism and audit integrity |
| Chat transport platform / chat scoping game-driven | 022 | `ChatScopes` enforcement tests | Serves both chess and werewolf with one mechanism |
| Matchmaking reads only capabilities | 023 | dependency matrix | Keeps matchmaking generic |
| Ratings computed by the platform from `MatchOutcome` | 024 | rating job | Ladder integrity uniform across games |
| `tabula-testkit` conformance mandatory per game | 025 | `register!` requires it | Determinism cannot be maintained by review |

### 3.2 EXPERIMENT

Direction chosen, details unproven. Build behind the seam; let measurement decide.

| Question | Decide in | How we will decide | Fallback if it fails |
|---|---|---|---|
| Macroquad's practical ceiling (text, render targets, mobile input) | Phase 2 / 6 | Build chess and tiles UI; log every workaround | `renderer-miniquad` |
| Macroquad UI widgets vs our own `RenderList` widgets | Phase 2 | Implement ~20 widgets ourselves; measure effort | Use Macroquad's UI for internal tools only |
| Postcard vs alternatives for the game payload | Phase 4 | Measure size and CPU under load L1/L2 | Protobuf for the payload only |
| Leptos ↔ Macroquad handoff UX | Phase 5 | Time-to-first-frame at `/play/:id`; user testing | Single-bundle integration spike, or a lighter shell |
| Tauri desktop value (launcher/updater/notifications) | Phase 5 | Spike; compare with `cargo-dist` alone | Ship without Tauri |
| Tauri mobile for shell screens | post-Phase 6 | Only if native shell screens prove painful | Keep native shell |
| Voice provider (self-hosted LiveKit vs managed) | Phase 8 | Cost per participant-minute, quality, ops burden | Swap adapters |
| Snapshot cadence and log compaction policy | Phase 4→10 | Measure rehydration time and storage growth | Tune per `StateSizeClass` |
| Sharded executor vs task-per-match | Phase 10 | Benchmark at 30k+ matches/process | Stay with task-per-match; add processes |
| Deck commitment scheme for provable shuffles | Phase 3 | Does anyone care? Is verification usable? | Drop it; projection remains the guarantee |
| Generated config forms from `Config` schema | Phase 5 | Try it for four games | Hand-written form per game |
| Board Reader depth (status/actions vs full regions) | Phase 5 / 9 | Screen-reader user testing | Ship status+actions only |
| Audio backend (Macroquad vs kira) | Phase 3 | Do we need buses/ducking for voice? | Adopt kira in the backend |
| `IndexMap` in rules where insertion order is semantic | ongoing | Case by case, documented per use | `Vec` + explicit index |
| Placement table (Postgres) vs Redis directory | Phase 10 | Attach p95 | Redis |

### 3.3 DEFER

Preserve the seam. Write no code. Each has a written trigger.

| Deferred | Trigger to start | Seam that keeps it possible |
|---|---|---|
| Redis | doc 06 §4.3 (both conditions) | Directory/presence/pubsub already behind ports |
| Kubernetes | Team grown **and** container orchestration is the measured constraint | Stateless-ish gateways; one binary |
| Kafka / NATS | doc 06 §5.4 | Transactional outbox already the export path |
| gRPC between services | A non-Rust internal service appears | Internal transport is versioned and framed |
| Microservice decomposition | Divergent scaling or deploy cadence, per component | Crate boundaries + ports |
| Multi-region | p95 RTT > 150 ms for >10% of sessions in a geography | `region` column from Phase 4; region-local match data |
| `wgpu` renderer | Miniquad blocks a needed capability | `Renderer` trait + `RenderList` |
| Third-party WASM game modules (Phase C) | A real third-party developer with a shipped game | `ErasedMatch` is already an ABI shape |
| Loadable first-party packages (Phase B) | First-party games need independent deploy cadence | Same |
| Dynamic native plugin loading | **Never** | — |
| Custom SFU | **Never** for MVP; only if provider economics collapse | `VoiceService` |
| ECS architecture | A game with thousands of simulated entities | Presentation-half-local choice |
| Second datastore (ClickHouse etc.) | A product analyst exists and asks | Outbox → object storage → DuckDB |
| Cross-game economy / marketplace | Product decision, post-Phase 11 | Games emit events; economy consumes them |
| Tournaments / brackets | Product decision, Phase 9+ | Rooms + matches + ratings already model it |
| SSR for the web shell | Public indexable pages needed | CSR app is separable from a marketing site |
| Gamepad support | User demand | `InputEvent` normalization |
| ML bots | A game where heuristics are embarrassing | `GameBot` takes only a projection |
| Client attestation / anti-tamper | **Never** | Server authority makes it unnecessary |

---

## 4. What should be extremely stable?

If these change, everything downstream pays. They are the reason the roadmap front-loads Phases
0–3 before any networking.

| Contract | Stability requirement | Blast radius if changed |
|---|---|---|
| `GameRules` (State/Command/Event/View + apply/project/view_event) | Frozen after Phase 3; additive only after Phase 9 | Every game; possibly the protocol and stored replays |
| `Input` variants and the single-stream principle | Frozen after Phase 3; new variants are additive | Every game's `apply`; the event log format |
| `Effect` variants | Additive only | Every game; the platform's effect executor |
| Projection security semantics (`Viewer`, `Audience`, `SecretModel`) | Frozen | Every hidden-information game; a mistake is a security incident |
| Deterministic replay rules (`DetRng` algorithm, canonical encoding, `state_hash`) | **Frozen forever** — changing the RNG algorithm invalidates all historical replays | Every stored match ever |
| `rules_version` / `rules_hash` discipline | Frozen | Replay validity; audit integrity |
| Protocol versioning principles (negotiate, additive minor, golden vectors) | Frozen | Every deployed client |
| Platform/game ownership boundary (doc 00 §6) | Frozen | Every future feature argument |
| `RenderList` command set | Additive only, gated by doc 04 §5.4 | Every presenter; every renderer backend |
| Semantic design token *names* | Additive; renames require a codemod | Every UI surface |
| Database event-log shape (`match_inputs`, `match_events`, `match_snapshots`) | Additive; migrations only | All stored history |

### 4.1 Challenging the brief's assumption list

The brief proposed that "protocol/versioning principles" be extremely stable — agreed, but with a
sharpening: **the principles are stable, the protocol itself is expected to evolve additively and
frequently.** Conflating the two produces either a frozen protocol (features blocked) or an
unversioned one (clients corrupted). The golden-vector + negotiation machinery exists precisely so
that frequent additive change is safe.

Similarly, the brief listed "state/command/event semantics" as stable. Sharpened: the *semantics*
(what a command means, what an event records, that events are canonical and full-information) are
stable; the *contents* of each game's types change with every `rules_version` bump and that is
normal. The mechanism that makes it safe is `rules_hash` + multi-version linking, not restraint.

---

## 5. Where this plan differs from or sharpens the brief

Everything in the brief's "preserve the spirit of" list is preserved. These are the deliberate
refinements, each with a reason.

| Brief | This plan | Why |
|---|---|---|
| `validate_command` and `apply_command` as separate trait methods | **One `apply` returning `Result`**, plus optional non-authoritative `legal_commands` | Two functions that must agree about legality is a permanent divergence bug and an eventual exploit (doc 02 §3.2) |
| `GameCommand` as the input to the runtime | **A single `Input` enum** covering player commands, timers, seat lifecycle, and admin actions | Makes replay total, timers deterministic, and resolves every "who owns AFK/disconnect/pause" question by construction (doc 00 §3.1) |
| `services/{gateway, game-server, matchmaking, ...}` from day one | **One `tabula-server` binary** composed of crates with the right seams; the gateway/worker split is Stage 2 with a written trigger | Three binaries triple the operational cost at Stage 0 for zero benefit; the split we will actually want is gateway↔worker, not per-feature (doc 01 §2.3) |
| Games as one crate each | **One crate per game with a feature split** (`rules` / `presentation` / `bots`) | The server must compile a game without a renderer; this is I-1 in practice (doc 02 §1) |
| Presentation abstraction justified by future renderer replacement | Also justified **immediately** by `renderer-headless`, which enables `RenderList` and golden-image tests in CI with no GPU | An abstraction whose only justification is a hypothetical future gets skipped or done badly (doc 04 §6.1) |
| Poker/generic card game as reference Game B | **Tiến Lên** | Same architectural coverage (hidden hands, shuffle, projections) without building a betting economy; also fits the first market |
| "Redis optional initially" | Redis deferred with **two explicit conditions**, and a Postgres placement table as the intermediate step | "Optional" tends to become "added anyway"; a numeric trigger plus a cheaper intermediate keeps it honest (doc 06 §4.3, §4.4) |
| Accessibility as a design-system concern | Accessibility is **part of the game contract** (`describe()`), because a canvas is otherwise unreadable to assistive technology | Retrofitting a11y onto a canvas game is not possible; a per-game function is the only mechanism that works (doc 04 §10.4) |
| Snapshot/event-log strategy | Store **both inputs and events**, with a stated 2× cost and three concrete reasons | Events are read far more often than replays run, and storing both is what makes production determinism drift *detectable* (doc 03 §9.5) |
| Spectators as a capability | Spectator **delay** enforced server-side by buffering, plus a separate spectator chat channel | Live spectators are a coaching channel in ranked play; the delay must be platform-enforced, not requested |
| Bots as a platform feature | Bots consume **only projections**, by type signature | A bot with state access is an accidental cheating oracle, and projection-only bots double as the primary fuzzer (doc 00 §6.5) |
| Design system inspired by M3 Expressive | Take the token architecture, tonal palettes, state layers, and motion philosophy; **reject the component library and build a distinct "good physical game on a good table" identity**, with quiet platform chrome so game palettes dominate | A cloned component library makes a game platform feel like a productivity app (doc 04 §7.1) |

---

## 6. The five things most likely to go wrong

Named so they can be watched.

1. **A projection leak in a hidden-information game.** The highest-severity, hardest-to-detect
   failure. Mitigations: `SecretModel` scans on every PR, socket-level assertions in werewolf tests,
   `View` types that cannot represent absent secrets, a second-engineer review of every
   `project`/`view_event`, and a leak bounty in the closed beta.
2. **Silent determinism rot.** A `HashMap`, a float, an unordered iteration, or a behavior change
   without a `rules_version` bump. Mitigations: lints, `rules_hash`, state hashes in the log, and the
   nightly replay job over sampled production matches. The `state_hash_mismatch` counter pages.
3. **Phase 4 ordering/idempotency bugs under load.** Correct in tests, wrong at 5k CCU. Mitigations:
   load scenarios L1/L2/L4/L7 from the start of the phase, not the end; fencing tokens before any
   multi-process work; the "must always be 0" counters.
4. **Macroquad's ceiling arriving at the worst moment** (during mobile work, Phase 6). Mitigations:
   the `Renderer` seam, a Phase 2 spike that documents every workaround, and an early "hello
   triangle" on iOS to de-risk the toolchain separately from the renderer.
5. **Scope drift into building a UI framework or an engine.** The classic failure of exactly this
   kind of project. Mitigations: the capped `RenderCmd` set with a written admission rule, the "no
   phase is only refactoring" constraint, and the fact that every phase must end in a demo a
   non-engineer can watch.

---

## 7. Starting instructions

For the first agent or developer to pick this up:

```text
1. Read 00-architecture-principles.md fully. It is the contract.
2. Read 07 §"Phase 0" and 01 §2–§6.
3. Create the workspace exactly as in doc 01 §2.2, but ONLY the Phase-0 crates
   (tabula-core, tabula-game-api, tabula-testkit, games/tictactoe, xtask).
4. Write the enforcement FIRST: deps.toml + xtask check-deps + clippy.toml + CI.
   Then deliberately add a forbidden dependency and confirm CI fails. Remove it.
5. Implement tabula-core exactly as sketched in doc 02 §2. Pin DetRng's algorithm and
   shuffle implementation; write the test that proves shuffle output is stable.
6. Implement tabula-game-api exactly as sketched in doc 02 §3–§4.
7. Implement games/tictactoe from doc 02 §10 (the code is nearly complete there).
8. Implement tabula-testkit's conformance! suite from doc 02 §11.1.
9. Run `xtask selfplay tictactoe --matches 10000`. It must pass.
10. Only then proceed to Phase 1.
```

**Do not** create `tabula-protocol`, `tabula-match`, `tabula-storage`, or any service in Phase 0.
The temptation is strong and it is the single most common way this kind of project goes wrong: the
networking gets built against a contract that has not yet been validated by real games, and then
the contract cannot move.

---

## 8. Diagram index

The twelve required diagrams and where they live.

| # | Diagram | Location |
|---|---|---|
| 1 | Overall architecture | doc 00 §9.1 (system), doc 00 §2 (layers) |
| 2 | Rust crate dependency graph | doc 01 §4 |
| 3 | Client/server topology | doc 03 §1 (Stage 0), doc 06 §4.1 / §5.1 (later stages) |
| 4 | Match actor flow | doc 03 §6.3 (loop), doc 03 §1.1 (task inventory) |
| 5 | Command sequence | doc 00 §9.2 (lifecycle), doc 03 §7 (pipeline) |
| 6 | Reconnect sequence | doc 03 §10 |
| 7 | Player projection / security model | doc 00 §9.4 |
| 8 | Game plugin / registry architecture | doc 02 §8 (erasure), doc 02 §9.3 (Phase A/B/C) |
| 9 | Renderer abstraction | doc 04 §6 |
| 10 | Design-system adapters | doc 04 §8 |
| 11 | Deployment evolution | doc 06 §3.1, §4.1, §5.1, §5.5, and the §12 summary table |
| 12 | Roadmap dependency graph | doc 07 §0.1 |

Additional diagrams beyond the required set: input/effect boundary (doc 00 §9.3), game module
structure (doc 02 §1), version lifecycle (doc 02 §9.2), presentation pipeline (doc 04 §5.1), client
architecture (doc 04 §1), asset pipeline (doc 04 §12.1), chat flow (doc 03 §16), voice signaling
(doc 03 §17), scaling seams (doc 03 §20), CI pipeline (doc 01 §6.1), deployment pipeline
(doc 06 §11.1), dimension coverage (doc 08 §1).

---

## 9. Final check: does this plan answer the brief?

| Brief requirement | Where |
|---|---|
| Rust-first cross-platform platform for many game types | doc 00 §1, doc 01 §7 |
| No `if game == "chess"` in platform code | I-9, doc 02 §8, `xtask check-no-game-ids` |
| Renderer- and network-independent rules | I-1, doc 00 §5, doc 01 §3 |
| Macroquad first, Miniquad escape hatch, wgpu later | ADR-010, doc 04 §6.3 |
| Leptos shell + Macroquad gameplay, separate runtimes | ADR-011, doc 04 §3 |
| Optional Tauri, never required for gameplay | ADR-019, doc 04 §3.3 |
| Mobile native Macroquad first | doc 01 §7, doc 07 Phase 6 |
| Axum + Tokio + Postgres + SQLx backend, Redis optional | doc 01 §1.2, doc 03, ADR-014 |
| Flexible per-game networking semantics | doc 02 §12, doc 00 §6.3 |
| Platform vs game ownership fully resolved | doc 00 §6 (including the contested list) |
| Match actor architecture with a recommendation | doc 03 §6.1 (task-per-match, with alternatives assessed) |
| Protocol design with a codec recommendation | doc 05 §2–§4 (Postcard + JSON dual) |
| Game registry / plugin model, Phase A/B/C | doc 02 §8–§9 |
| Frontend architecture with dependency direction | doc 01 §4, doc 04 §1 |
| Presentation contract, MVP-scoped | doc 04 §5 (nine commands, capped) |
| Design system, M3-Expressive-inspired, own identity, motion | doc 04 §7–§9 |
| Repository architecture, challenged where needed | doc 01 §2 (incl. §2.3 challenges) |
| Persistence: canonical state, log, snapshots, recovery | doc 03 §9, §13 |
| Scaling with measurable triggers, not CCU dogma | doc 06 §1.1, §12 |
| Voice separate, WebRTC, no custom SFU | ADR-016, doc 03 §17, doc 04 §11 |
| Asset system with packs, CDN, cache | ADR-017, doc 04 §12 |
| Testing strategy incl. property + replay | doc 02 §11, doc 06 §10 |
| Security / anti-cheat, hostile clients | doc 00 §4, doc 03 §21, doc 05 §9 |
| Developer experience, hello-world game | doc 02 §10 |
| Phases with goals, tests, demos, exit criteria | doc 07 |
| 3–4 reference games stressing different dimensions | doc 08 |
| 12 diagrams | §8 above |
| Opinionated: Recommended / Alternative / Reconsider | doc 01 §1, doc 03 §6.1, doc 05 §3 |
| LOCK NOW / EXPERIMENT / DEFER classification | §3 above, doc 00 §11 |
| Optimized for a small team, with seams preserved | ADR-015, doc 03 §20, doc 06 §7 |
| Final synthesis | this document |

---

**Back to:** [`README.md`](./README.md) · [`00-architecture-principles.md`](./00-architecture-principles.md)
