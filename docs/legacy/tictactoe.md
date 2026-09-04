# Legacy Record: Tic-Tac-Toe Prototype

> **Status:** Retired from active codebase at commit [`e325ccc335df11d55e82e6fd9d495aa00736d42c`](https://github.com/loveoverflowcom/tabula/commit/e325ccc335df11d55e82e6fd9d495aa00736d42c).
> **Historical Crate:** `games/tictactoe` (`tabula-game-tictactoe`)
> **Historical Role:** Phase 0 bootstrap vehicle, SDK smoke test, and new-game template.

---

## 1. Executive Summary and Retirement Rationale

Tic-Tac-Toe was introduced in Phase 0 as the minimal reference implementation of Tabula's game contract. Its purpose was to validate that the foundational kernel traits (`GameRules`, `GameModule`, `Input`, `Effect`, `Ctx`) could produce a playable, deterministic, testable game in under 300 lines of code with zero platform changes.

As the platform evolved:
- **Chess (`games/chess`)** became the primary benchmark for complex legality move generation, deterministic clocks (Fischer and Bronstein), draw claims, and rating integrations.
- **Tiles (`games/tiles`)** became a comprehensive Carcassonne-like vertical slice featuring dynamic spatial grid layout, deterministic tile-bag RNG streaming, follower placement/scoring, hidden bag-order projection verification, camera navigation, and local Macroquad presentation.
- **Caro (`games/caro`)** remains reserved as the simple-product / SDK-friction benchmark on a larger fixed grid.

Once Chess and Tiles were fully operational across the generic client runtime (`apps/game-client`), `xtask` self-play, replay verification, and the conformance testkit, Tic-Tac-Toe became redundant. Retaining it incurred maintenance overhead without adding novel contract coverage. It was retired cleanly from active code while preserving its foundational design and verification lessons in this document.

---

## 2. Core Architectural Lessons

### 2.1 Pure, Deterministic Rules Kernel (R1, R2, R4)
Tic-Tac-Toe was the first proving ground for Tabula's core rule function signature:
```rust
fn apply(
    state: &mut State,
    input: Input<Command>,
    ctx: &mut Ctx<'_>,
) -> Result<Outcome<Self>, RuleError>
```
Key contract requirements validated:
1. **Purity:** Absolute isolation from operating system randomness, wall clocks, and external I/O.
2. **Transactionality on Error (R2):** If an invalid input is submitted (e.g. placing out of turn or in an occupied cell), `apply` returns an error and leaves `State` byte-identical to its pre-call state.
3. **Effects as Data:** State mutations emit pure data descriptions of platform actions (`Effect::SetTimer`, `Effect::CancelTimer`, `Effect::EndMatch`) rather than executing side effects.

### 2.2 Structural "Validate Fully, Then Mutate" Pattern
To guarantee R2 transactionality by construction rather than memory, Tic-Tac-Toe established the two-phase validation idiom:
- `validate_place(state, seat, cell) -> Result<PlaceWitness, RuleError>`
- `commit_place(state, witness) -> Outcome<TicTacToeRules>`

Because `commit_place` requires a `PlaceWitness` proof value that can only be constructed by a successful `validate_place` call, invalid inputs cannot mutate state or leak partial writes. This pattern was subsequently adopted by Tiles and future games.

### 2.3 State vs. View Projection (I-5, I-6)
Even for a simple perfect-information game where all board marks are public:
- Clients never receive canonical `State`.
- Rules project a distinct `View` struct for each `Viewer` (`Viewer::Seat(SeatId)` or `Viewer::Spectator(SpectatorTier)`).
- Events are transformed into `ViewEvent`s before delivery.

This enforced the strict architectural boundary between authoritative server state and client-visible projections from Day 1.

### 2.4 Deterministic Replay and Format Verification
Tic-Tac-Toe provided the initial golden corpus for Tabula's binary replay format (`.tbr`):
- Canonical recording of input streams, monotonic `LogicalTime`, and `InputIndex`.
- Exact state hash checkpoints calculated with BLAKE3.
- Tooling support in `cargo xtask replay` and `cargo xtask replay-goldens`.

---

## 3. Verification & Formal Methods Lessons

### 3.1 Bounded Model Checking with Kani
Tic-Tac-Toe served as the testbed for integrating CBMC/Kani bounded model checking:
- **Transactionality Proofs:** Kani harnesses verified that for all symbolic `(SeatId, cell)` pairs (over the complete $2^{16}$ symbolic space of `u8` values), every rejected placement preserved all canonical fields (`board`, `turn`, `status`, `move_timeout_ms`).
- **CBMC Stubbing:** Demonstrated that stubbing out downstream `SmallVec` outcome destruction in verifier mode prevents CBMC solver explosion while proving the core domain transition invariants.
- **Rules Hashing Interaction:** Illustrated that `#[cfg(kani)]` harnesses located within rules source trees alter the compiled `RULES_HASH`, necessitating an understanding of `Exact` vs. `CompatibleVersion` replay classifications.

### 3.2 Mutation Testing with `cargo-mutants`
Tic-Tac-Toe calibrated the repository's mutation testing baseline:
- Exercised ~198 mutants, identifying missed assertion mutants and equivalent mutations (e.g. boundary guards on already validated fields).
- Established the practice of classifying mutants into real test gaps, equivalent mutations, and verifier-only artifacts.

---

## 4. Generic Client Integration (`LocalMatch`)

Tic-Tac-Toe was one of the initial drivers for `tabula-game-client`:
- Proved that `LocalMatch<R, P>` can drive gameplay hot-seat, advance monotonic frame clocks, dispatch normalized pointer events (`PointerPosition`, `PointerButton`), and handle timer expirations without game-specific branching.
- Validated that UI gestures (hover, drag, selection) consume zero canonical input indices when they do not produce legal game commands.

---

## 5. Transfer of Verification Responsibilities

All verification and test responsibilities previously held by Tic-Tac-Toe have been transferred to active games and platform suites:

| Responsibility | Historical Tic-Tac-Toe Role | Successor / Active Verification |
|---|---|---|
| **Move Legality & Clocks** | Basic cell vacancy check | `games/chess`: full move generation, check/mate, repetition, Fischer & Bronstein clocks |
| **Spatial State & RNG** | None (fixed 3×3 grid, no RNG) | `games/tiles`: dynamic 2D board, ChaCha8 bag shuffle, draw streaming |
| **Hidden Information** | None (perfect information) | `games/tiles`: secret bag-order scan, projection noninterference suite |
| **Replay Golden Corpus** | `tictactoe-golden.tbr` | `chess-golden.tbr`, `chess-clock-golden.tbr`, `tiles-golden.tbr` |
| **Self-Play Campaigns** | `xtask selfplay tictactoe` | `xtask selfplay chess`, `xtask selfplay tiles` (with `--seats`) |
| **Local Client Driver** | `apps/game-client/tests/local_match.rs` | Driven end-to-end via Chess and Tiles |
| **Conformance Suite** | `conformance!(TicTacToeFixture)` | `conformance!(ChessFixture)` in `games/chess`, `TilesRules` conformance suite |
