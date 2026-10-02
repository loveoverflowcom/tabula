# ADR-0028: the discovery/setup slice crosses the Phase 4 and Phase 5 gates

- **Status:** accepted
- **Date:** 2026-10-02
- **Supersedes:** none
- **Invariants touched:** none. I-9 and I-15 are enforced unchanged, and the
  slice is built to make them *more* checkable, not less.

## Context

[Issue #50](https://github.com/loveoverflowcom/tabula/issues/50) asks for two
pull requests. The first — the discovery/setup specification — merged as #56.
The second implements screens 01–03 against registry interfaces, and the issue
makes it conditional: *"Sau foundation và khi registry Phase 4 + shell Phase 5
gate được chứng minh"*.

Those gates are not met at `develop` (merge a4ec52c):

- Phase 3 has not exited. Caro has no rules, and Werewolf has a creation-only
  slice (`docs/ui/screens/discovery-availability.md`).
- Phase 4 therefore has not started. `tabula-protocol`, `tabula-match`,
  `tabula-registry` and `tabula-server` were doc-only scaffolds.
- Phase 5's own gate is the Phase 4 exit, and `apps/web` exited with a gate
  message.

Doc 07 §7 gives the reason for the ordering: a wire and dispatch contract built
before games have validated it is a contract that can no longer move. AGENTS.md
§4 states the rule and this ADR process as the only way past it.

The repository owner asked for the implementation anyway, with the gate crossing
named and accepted rather than hidden. This ADR is that record.

## Decision

Implement the discovery/setup slice now, bounded as follows.

**What is implemented ahead of its phase**

- `tabula-registry` (Phase 4): the catalog, `ErasedGame` with its single blanket
  `Adapter<S>`, module-authored config forms, draft parsing, mode availability,
  message tables, and ADR-011 handoff resolution.
- `apps/web` (Phase 5): routes `/`, `/games`, `/games/:id` and the
  `?setup=1` substate, in Leptos CSR.

**What is explicitly NOT implemented, and still waits for its gate**

- `ErasedMatch`, codecs, match creation, snapshot restore, rollout tables and
  multi-version resolution in the registry.
- The wire protocol, the match runtime, storage, the server, and every network
  route in the shell (`/login`, `/rooms`, `/queue`, `/matches`, `/u`).
- Any online, ranked, async, resume, or asset-delivery operation. Each of those
  is rendered as unavailable with its reason, never as a disabled control with
  no explanation and never as a fabricated success.

**What does not change**

- I-9 holds: `games/` is named only inside `crates/tabula-registry/src/games/`,
  which is the one zone `xtask check-no-game-ids` exempts. The shell carries no
  game id, and no game message key, in its own source — game copy reaches it as
  data through the catalog.
- I-15 holds: `leptos` stays out of `apps/game-client` and gameplay stays a
  separate document (ADR-011).
- `deps.toml` is unchanged. The dependency the registry now uses on game crates
  was already written there as `allow_games = true`.
- Game rules, the game contract, and the protocol are untouched. The slice is
  additive above them.

**One structural consequence had to be absorbed.** `tabula-registry` linking
game crates closes a package cycle: `registry → games/* → tabula-testkit →
tabula-match → registry`. Cargo rejects that regardless of features, so
`tabula-testkit` no longer lists `tabula-registry`/`tabula-match` even
optionally. Its `runtime` feature backed no code (`src/fakes.rs` is a
`TODO(phase 4)`), so nothing was lost; the dependency returns when the match
runtime is real and the fakes are written against its ports.

## Consequences

What becomes easy: screens 01–03 exist against real typed data, and the catalog
boundary is now exercised rather than only described. The config-form and mode
availability seams the specification named as "pending decisions" now have an
implementation to argue with.

What becomes hard: the `ErasedGame` surface implemented here was validated by
two games, not by the five Phase 3 planned. Phase 4 may need to change it, and
this slice is a consumer that will have to change with it. That is the exact
cost doc 07 §7 warned about, accepted knowingly. The slice is small and
self-contained for that reason: the registry owns its own types, and the shell
owns no contract at all.

A reader must not read this slice as evidence that Phase 3, 4 or 5 is complete.
The `PHASE` banners stay; the registry's banner now names precisely what is
implemented and what is not.

## Revisit when

Phase 3 exits. At that point the `ErasedGame` surface is reviewed against every
Phase 3 game — not just the two linked here — before Phase 4 builds the match
runtime on it. If that review changes the surface, this slice is updated or
reverted rather than preserved for compatibility.
