# Goal

Choose a concrete Phase-3 Caro ruleset before writing its kernel. Recommendations below are
**proposed defaults**, not settled product policy or changes to normative docs. C1 records which
are accepted; a rejected recommendation blocks its dependent implementation until replaced.

## Current state

Caro is a doc/Cargo/lint skeleton. No domain types, manifest, build identity or tests exist.
Doc 08 §3 and `docs/games/caro.md` deliberately leave variant/size open. Doc 00 governs constraints;
current code determines which APIs actually exist.

## Decisions

| ID / question | Recommended default | Alternatives | Why it matters / dependent implementation and tests |
|---|---|---|---|
| C-D1 Variant | Named `Ruleset::Freestyle`, shared equally by both seats | Exactly-five; Vietnamese blocked-end variant; Renju-style restrictions | Pins the actual predicate; C2 win/legality, C3 oracles, all replay fixtures |
| C-D2 Board | Private BoardSize, odd 9..=19, default 15; dense row-major board | Fixed 15×15; other explicitly bounded set | Cheap small/large fixtures without unbounded allocation; C1 constructors, C2 indexing, C3 size, C5 layout |
| C-D3 Winning length | At least five contiguous stones | Exactly five | A run of six wins under default; C2 detector, C3 length/overline tests |
| C-D4 Overline | Legal and winning, for either seat | Legal nonwinning overline; forbidden for first/both seats | Forbidden moves would affect apply and legal_commands; C2/C3 mutual consistency. Do not treat “exactly five” as automatically meaning “overline illegal” |
| C-D5 Blocked ends | Ends do not affect victory | Both opponent-blocked ends negate win; decide whether board edge also counts as a block | Changes the local win predicate and boundary tests, not terminal finality. Even an alternative variant stops at the first accepted win; a later opponent move cannot undo an ended match |
| C-D6 Opening | No center restriction, swap, double-three/double-four or asymmetric forbidden patterns | Named restricted variant later; tournament openings excluded by doc 08 | Empty cells remain legal; C2 enumeration, C4 bots and C5 input |
| C-D7 Draw | Full board with no win; check win before draw | Early impossible-win detection; move cap | C2 termination, C3 dense fixtures and C4 draw golden |
| C-D8 Resign | Either seated player may resign while Playing, including off-turn | On-turn only; no resignation | Requires victory reason without a geometric win line; C2 terminal model and C3 legality equality |
| C-D9 Timer | Optional per-turn timeout; default off (`0`), enabled durations 1s..24h; due expiry loses for seat on turn | No timer; automatic move/pass (larger ruleset) | Timer deadline must be stored, checked and replayed; C2 timer and C4 timeout golden. A timeout fixture does not justify arbitrary timer behavior |
| C-D10 Capabilities | Two occupied seats, asymmetric opening; BotOnly Trivial/Easy, live spectators, no voice, pausable=false, client_preview=false, AckAfterPersist; ranked=No and async=Disabled initially | Elo and long-turn capability after explicit product decision | Avoid declaring product features from timer support alone. State size measured, not guessed; C2 metadata, C3 parity tests, C5 current-view rendering |
| C-D11 Version boundary | Keep the one-variant Ruleset in Config and semantic state; version every behavior/encoding change | Hard-code freestyle; prematurely declare unsupported variants | Makes chosen variant explicit, but does **not** make future migrations free; C2 versioning and C4 goldens |

BoardSize validates through a private constructor and serde raw→TryFrom. Board validates its own
length against the size. A coordinate validated for 19×19 is not automatically valid on 9×9:
Command uses raw x/y; apply validates against this board before producing a local placement witness.
Use stable SeatId bindings, never seat-number parity to choose a stone.

Full-board draw fixtures are practical at every proposed size: color `(x + 2*y) mod 4 < 2` as
First and the rest Second. This has one extra First on odd n=9..19 and no length-five run in any
axis. C3 must independently assert counts/no-win and build an alternating legal trace from it;
never trust a hand-filled terminal State alone. Removing stones cannot create a freestyle win.

## Scope

C1 owns decisions and primitive types only; C2 adds complete rules, identity and mandatory baseline
conformance together. This sequencing fixes a dependency in the draft: Tiles' build.rs reads
`src/rules/mod.rs` and a numeric RULES_VERSION marker, so copying it before that source exists
would break the build. Do not fabricate a GameRules implementation solely to satisfy build.rs.

## Proposed architecture

`Config + roster + Input/Ctx → validate → private immediate witness → commit → events/effects`.
All canonical behavior lives under `games/caro/src/rules/`; bot/presentation are outside that
hashed subtree. Pure rules need no registry, server, renderer or testkit API change.

## Invariants

I-2/I-3/I-4, R2/R8: deterministic iteration, no wall clock/OS RNG, no canonical floats, all fallible
checks before mutation. Rules never draw RNG; bots may use their supplied DetRng. I-7 is enforced
by the generic driver, not a second state-version field in the game. Terminal result is immutable.

## Platform-change guardrail

| Candidate | Current contract / reusability | Disposition |
|---|---|---|
| New rule-error code | Existing IllegalMove and public-safe detail can express rejected placements, including future restrictions | No new core enum required merely for Caro; a demonstrated generic error requirement needs separate versioned contract review |
| Registry registration | Registry is a Phase-4 skeleton | Use existing app/tooling leaf wiring now; do not build registry early |
| Shared grid/render command | Dense board and current RenderList suffice | Keep game-local; any actual generic insufficiency becomes a documented separate PR/ADR, not a game-id branch |
| Dependencies | Root game alias and games/* deps policy already exist | Add only needed workspace dependencies to this crate/xtask/client; no deps.toml change expected |

## Verification ledger

| Claim | Failure mode | Cheapest oracle | Evidence level | Tier | Residual gap |
|---|---|---|---|---|---|
| Variant is unambiguous | Different detector/bot interpretations | C-D1..11 accepted/replaced in docs/games/caro.md | documented | Manual/security review | Tests later pin the selected behavior |
| Size is valid across serde | Constructor bypass creates ragged board | Boundary/invalid DTO/round-trip tests | example-tested (planned) | Every PR | State coupling needs its own validator |
| Manifest and source identity agree | Zero hash or stale version | Current manifest gate + build hash/version checks in C2 | statically checked (planned) | Every PR | Compiled capability parity is not automatic |
| No platform change needed | SDK benchmark defeated by special case | Diff accounting and dependency/game-id gates | statically checked (planned) | Phase exit | String scan cannot establish architectural correctness alone |

## Expected file changes

C1: `docs/games/caro.md`, `games/caro/src/rules/{mod,board,coord}.rs`, `src/lib.rs`, Cargo.toml and
constructor tests. C2: `game.toml`, `build.rs`, complete rules/module and tests. See PR sequence.

## Acceptance criteria

- [ ] C-D1..11 accepted or replaced, with alternatives and affected tests recorded.
- [ ] No implicit Renju/blocked-end/opening rules; terminal state includes resign/timeout.
- [ ] Board bounds are private and serde-validated; no new platform behavior.
- [ ] Every intermediate PR builds; identity introduced with a real canonical rules tree.

## Residual risks

Freestyle is a benchmark default, not a final market/tournament rules decision. Doc 02 §12.0's
symmetric Caro row conflicts with this recommended asymmetric opening capability; record amendment
request separately. A future variant requires behavior/version/migration review, not just an enum arm.

## Next dependency

[01-rules.md](01-rules.md), then [C1–C6](04-pr-sequence.md).
