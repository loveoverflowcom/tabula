# Core-domain-boundary verification ledger

This ledger records the named claims for the Phase-0 core-domain-boundary
refactor. It is deliberately claim-first: every production change below has a
corresponding constructor, decoding, or state-machine test.

| Invariant | Former escape path | Proof boundary | Evidence |
| --- | --- | --- | --- |
| `GameId` is a canonical reverse-DNS name | Public tuple field and derived `Deserialize` | Private newtype; fallible constructor and serde `try_from` | Constructor partitions and postcard decode tests |
| `GameVersion` is SemVer | Public arbitrary string | Private validated newtype and serde boundary | SemVer partitions and round trips |
| A roster has no duplicate addressable seat | Public `SmallVec` and derived `Deserialize` | Validated `SeatRoster::new` and custom serde boundary | Duplicate and decode-rejection tests |
| Rating standings cover the authoritative roster | Public `MatchOutcome` fields; self-attested serialized roster | Private fields; intrinsic serde validation plus `validate_against(&SeatRoster)` scoped witness | Rank/duplicate/abort partitions and restored-outcome roster test |
| Seat capabilities have one authoritative count representation | Duplicated `min`/`max` and nested range | `SeatCounts` constructor is the sole count representation | Range/exact partition and serde tests |
| Capability combinations are meaningful | Public flag/`Option` and empty lists | Closed async enum, validated bot levels/team sizes, capability constructor | Constructor and decode-rejection tests |
| Tic-tac-toe state is reachable (historical) | Public state fields and derived `Deserialize` | Private, validated state representation; moves derived from board and terminal-mover checks | Invalid-snapshot regressions and reachable-state exploration (retired; see `docs/legacy/tictactoe.md`) |
| Tic-tac-toe seats are match-local identities (historical) | Numeric `SeatId(0/1)` assumption | State stores the two creation-roster seats | Arbitrary-seat and relabeling tests (retired; see `docs/legacy/tictactoe.md`) |
| Rejected Tic-tac-toe input is a no-op (historical) | Mutable reducer could regress | Validate-before-commit rule plus generic conformance | Canonical-byte hostile-sequence tests (retired; see `docs/legacy/tictactoe.md`) |

The document is a verification aid, not a new normative architecture source;
[`00-architecture-principles.md`](../architecture/00-architecture-principles.md)
remains authoritative.

## Follow-up identified while rebasing onto develop

Chess PR #5 merged while this work was in progress. Its direct uses of the
hardened core values have been migrated and its conformance/perft suites still
pass. The audit also found a separate, deliberately deferred Chess-only task:
canonical `State` still has public fields and derived deserialization, and the
rules require `SeatId(0)`/`SeatId(1)`. Changing that mapping would alter Chess
canonical state and needs a focused `RulesVersion`/perft compatibility review;
it is not folded into this core-boundary PR.
