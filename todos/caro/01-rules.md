# Goal

Implement a small deterministic Caro reducer with a separately reviewable win oracle. Follow the
accepted [C-D decisions](00-decisions.md); no platform behavior is needed.

## Current state

Only the skeleton exists. Its Grid type is illustrative; use a game-local dense Board. Tiles
provides useful validate/commit and source-hash precedents, not an obligation to copy its timer
policy or its representation.

## Proposed architecture

| Module/type | Meaning and invariant |
|---|---|
| rules/board.rs — BoardSize, Board, Stone | Validated odd size 9..19, row-major Vec of exactly n² cells; Stone=First/Second |
| rules/coord.rs — RawCoord, Coord, Direction | Raw x/y enters Command; validated coordinate is scoped to this board; axes horizontal/vertical/two diagonals |
| rules/win.rs — WinLine | Pure scan through newly placed stone; full maximal run with canonical axis/start tie-break |
| rules/state.rs — State | Board, two distinct SeatIds, current/last active turn, moves, status, last placement, ruleset, timeout config, optional current timer/deadline |
| Status / WinReason | Playing; Won{winner, reason: Line(WinLine) / Resignation / Timeout}; Draw; Aborted{reason}. A win need not have a line |
| Command | Place{at: RawCoord}, Resign |
| Event | Placed{seat,at,stone}; Ended{outcome}; any public timer/turn data needed by the projected view |
| View / ViewEvent | Distinct View: public board, seat bindings, turn/status/last move, authorized-to-act seat and legal commands, public deadline. ViewEvent may alias Event because all Caro events are public |
| Config | BoardSize, Ruleset::Freestyle, optional validated timeout (raw 0 disables) |
| rules/mod.rs | Complete GameRules implementation, RULES_VERSION/HASH and shared config validation |
| lib.rs | GameModule + validated metadata/capabilities; optional bot and presentation modules |

`State` fields stay private; RawState decoding validates board length, seat bindings, counts and
turn parity, in-bounds last placement, valid winner/line, terminal/deadline coherence. A structural
validator does not prove historical reachability. Prefer generated legal traces for semantic tests.
No circular module dependency: foundational types first, algorithms consume Board, reducer consumes
both. Reference detector belongs in tests/support or a cfg(test) module, without production helpers.

### Validate then commit

Compute every fallible check, deadline and result proposal before modifying State. A private
PlacementProof records actor, coordinate, stone and the next timer/result decision; construct and
consume it within one apply call. It cannot be cached and applied to a later state. This prevents
accidentally skipping the validation path; it does not formally prove R2 or predicate correctness.
Byte-before/after rejection assertions remain mandatory.

### Input decisions

| Input | Behavior |
|---|---|
| Any input after terminal | Err(MatchOver), no new EndMatch |
| Player from unknown seat | Err(NoSuchSeat) |
| Place | Require active seat, time window open, coordinate in bounds and empty. Place once; win before full-board draw; otherwise switch turn |
| Resign | Either seated player while Playing may resign, including off-turn; opponent wins with Resignation reason |
| Due current timer | Enabled timer, matching id and now≥deadline → Timeout result for opponent |
| Disabled/early/unknown/stale timer | Accepted empty outcome, unchanged game bytes; driver still advances version because result is Ok |
| Seat lifecycle | Known seated lifecycle notifications accepted with no rule-state change; timer keeps running. Unknown seat rejects NoSuchSeat |
| Admin(Cancel) | Aborted outcome, cancel timer, EndMatch exactly once |
| Admin(Pause/Resume/ForceEnd) | Err(Unsupported) |

Store absolute deadline in State. At `now == deadline`, Place is rejected (WrongPhase); the timer
input owns timeout resolution. Resign is allowed until the timer's ordered input has ended the
match. Use fresh TimerId per turn, e.g. move count + 1 bounded by n²+1, to distinguish old timer
inputs. Re-arm only when continuing, cancel on all terminal paths. Checked next-deadline overflow
rejects before mutation. Do not copy a handler that ends a match solely because TimerId matches.

### Win algorithm and independent oracle

Production scans four axes through the newly placed coordinate, in a fixed axis order, walking
both ways until a different stone/boundary. O(n) bounded work per placement (n≤19), returning the
maximal run; if multiple directions win, choose the first in the documented axis order and its
lexicographically normalized start. Board coordinates use signed intermediate deltas and checked
bounds, so negative diagonals cannot wrap a row.

Reference model enumerates every valid length-five window over the entire board, directly indexing
its own simple representation. It shares no scan/direction iteration/legality helper with production.
There are `2*n*(n-4)+2*(n-4)^2` windows: 140 for 9, 572 for 15 and 1020 for 19. Exhausting window
geometry is **not** exhausting all 3^(n²) boards. Compare winner existence on reachable nonwinning
prefixes plus the accepted placement; do not demand equal WinLine representations from two
algorithms with different tie-breaks. Separately check that the returned production line is real,
maximal and uses the documented tie-break. A whole-board reference can find an old win on an
arbitrary invalid board while a last-move detector correctly cannot; constrain that comparison.

### Legal commands and terminality

For a playing active seat: all empty Place commands in row-major order followed by Resign. For
an off-turn seated player: Resign only. Unknown/terminal viewers: None. Since legal_commands has
no Ctx, it describes the stored phase until its timer is applied; equality with apply is asserted
with a Ctx inside the active window, not after a due but undelivered timer. Bots filter out Resign
while placements exist; they never receive State. All public data is projected for spectators,
but their View offers no acting seat/commands.

A **progressing placement trace** ends within n² accepted placements, since occupancy increases.
This is not a claim that every arbitrary input stream or real no-clock human match terminates.
The selfplay harness has its own input bound; absence of a wall-clock cap is deliberate.

## Invariants

| ID | Invariant |
|---|---|
| I-C1 | Board length/bounds and seat bindings are valid across construction and serde |
| I-C2 | moves equals occupied count; a nonterminal placement switches turn once |
| I-C3 | Line win is backed by ≥5 same stones; full-board win takes priority over draw |
| I-C4 | Every accepted placement increases occupancy; terminal outcome is immutable |
| I-C5 | EndMatch once across line/draw/resign/timeout/admin-abort paths |
| I-C6 | Rules never draw RNG; no I/O, clock, canonical floats or unordered iteration |
| I-C7 | legal_commands matches accepted player commands for the same stored state and open time window |
| I-C8 | Rejected input leaves complete canonical State bytes unchanged; driver owns version/index |

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| I-C1/I-C8 | Invalid decode or late validation corrupts state | Constructor/DTO partitions, canonical-byte rejection properties | example-tested + property-tested (planned) | Every PR | Witness is provenance, not a formal proof |
| I-C2..4 | Wrong win/draw/turn | Window enumeration and independent board oracle over reachable sequences | differentially-tested (planned) | Every PR | Geometric enumeration alone omits occupied blockers |
| I-C5 | Phantom/double timeout/end | Deadline/id/terminal table, EndMatch counting | example-tested (planned) | Every PR | Shell scheduling reliability excluded |
| I-C7 | Off-turn resign omitted; stale hints treated as authority | Exhaustive command alphabet per reached state at valid logical time | property-tested (planned) | Every PR | Hostile bytes belong to a future codec |

## Expected file changes

`games/caro/src/rules/{mod,board,coord,state,win}.rs`, `src/lib.rs`, `game.toml`, `build.rs`,
Cargo.toml, tests/{rules,conformance}.rs and docs/games/caro.md. Rules remain independent of
presentation. Floating-point layout arithmetic is allowed in the presentation feature with a
narrow lint scope, never canonical state; a crate-wide claim of “no floats anywhere” would be wrong.

## Implementation steps

1. C1 constructor/types and decisions; no fake GameRules.
2. C2 reference window oracle, production detector, complete reducer and module.
3. Add full Input/terminal cases and baseline GameTestFixture immediately with GameRules.
4. Wire build.rs only after numeric RULES_VERSION source marker exists; validate manifest/code parity.

## Acceptance criteria

- [ ] All accepted input classes and all errors have defined behavior.
- [ ] Resign/timeout winners do not invent a win line; winning placement retains last active turn.
- [ ] Fast/reference detectors agree in their stated domain and share no algorithm helper.
- [ ] Enabled deadline, exact boundary, stale/duplicate timers and overflow covered.
- [ ] Baseline conformance green; no platform edits.

## Residual risks

Changing a derived WinLine inside State changes hashes; preserve canonical tie-break and review
rules_version. Invalid snapshot validation needs more than cell count alone. Timer generation and
logical-time policy are game-owned even though scheduling belongs to the shell.

## Next dependency

[02-verification.md](02-verification.md), then bots/replays and local presentation.
