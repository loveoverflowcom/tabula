# tests/

Test suites that do not belong to a single crate.

```text
integration/   server + Postgres + real WebSocket, multi-client scenarios   (Phase 4)
load/          Rust load generator speaking the real binary protocol        (Phase 4)
replays/       committed golden .tbr files, per game                        (Phase 0)
```

Per-crate unit, property, and conformance tests live with their crates. A game's
conformance suite is one line in `games/<slug>/tests/conformance.rs`.

## integration/ — Phase 4

Real Postgres via `sqlx::test` (per-test transactional databases) or
`testcontainers` in CI. Real WebSocket. Multiple clients.

The scenarios that matter are the ones unit tests structurally cannot reach:

```text
two clients, one match, interleaved commands, assert per-seat projections
reconnect mid-match: ResumeOk path AND the Resync path (crossing 200 versions)
duplicate client_seq → the STORED ack is re-sent, the input is NOT re-applied
spectator attach; delayed spectator sees nothing newer than the window
                  — ASSERTED AT THE SOCKET, not at the projection function
hostile client: attach to another seat, resync-spam, oversized frames
drain + restart under load → zero lost matches
```

## load/ — Phase 4

A **Rust** load generator, reusing `tabula-net-client` and the real registry. Not
k6 or Gatling: the harness has to speak our binary protocol and our game
commands, and reusing the client crate means the load test exercises the same
reconnect logic players do.

Scenarios (doc 06 §10):

```text
L1  steady blitz chess        N matches, 0.25 cmd/s/player, AckAfterPersist
L2  werewolf vote burst       N matches of 12 seats, 3 s bursts of simultaneous votes
L3  spectator flood           1 match, 5,000 spectators attaching over 60 s
L4  reconnect storm           drop 30% of connections at once, measure resume success
L5  cold start                all matches hibernated, mass rehydration on attach
L6  tiles heavy state         Medium state class, snapshot pressure, large Welcome frames
L7  deploy under load         drain + restart while L1 runs; assert ZERO lost matches
L8  mixed realistic           weighted blend at target CCU
```

Results are committed to `docs/perf/` with date and commit. Nightly CI fails on a
>20% regression. A performance number without a committed baseline is a feeling.

Note that `tests/load` is excluded from the workspace (see the root `Cargo.toml`)
so its dependencies never reach a shipped binary.

## replays/ — Phase 0

```text
replays/<game>/*.tbr                 golden replays: normal, edge case, timeout
replays/<game>/regressions/*.tbr     auto-committed failing self-play seeds
replays/<game>/divergence/*.tbr      auto-committed from nightly replay verification
```

**Committing these is what makes determinism rot visible.** A rules change that
alters historical behaviour fails CI with a precise evidence report and a failing
checkpoint coordinate (or a bounded interval when checkpoints are sparse), forcing
an explicit `rules_version` bump and a migration decision — rather than surfacing
months later as an unexplainable replay failure.

Every game needs **at least three** before it ships (doc 02 §14): a normal game,
an edge case, and a timeout.

```bash
cargo xtask replay tests/replays/chess/normal-01.tbr
cargo xtask replay --all --verify        # what the nightly job runs
```
