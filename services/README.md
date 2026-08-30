# services/

Server binaries. **Leaves**: nothing depends on them.

| Service | Phase | What it is |
|---|---|---|
| [`tabula-server`](tabula-server) | 4 | THE binary at Stage 0: HTTP API + WS gateway + match runtime + lobby |

## One binary, on purpose (ADR-015)

Doc 01 §2.3 rejects "separate services from day one" explicitly:

1. Matchmaking, lobby, and the match runtime all need the same room directory.
   As separate processes at Stage 0 that directory becomes a distributed-consensus
   problem for zero benefit.
2. Three binaries triple deploy, config, tracing-context, and local-dev
   complexity — for one developer.
3. The split we will actually want is **gateway ↔ match-worker**, because
   connection fan-out scales differently from CPU-bound match application.
   Matchmaking-as-a-service is a much later need, if ever.

**One binary composed of library crates that already have the right seams.** The
crates are the boundary; the process count is a deployment decision.

## The split order, when it comes (doc 06 §7.1)

```text
1. tabula-server                            Stage 0-1
2. gateway | match-worker                   Stage 2  ← the only split driven by real physics
3. + relay (spectator fan-out)              Stage 2/3, only when needed
4. + job-runner (ratings, exports, pushes)  whenever job load or isolation warrants
5. + matchmaker                             only when the matchmaker itself needs replication
```

**Explicitly never split out:** lobby, chat, auth, catalog, presence. They are
libraries inside the gateway.

Each step has a measured trigger in doc 06 §1.1. "It feels like it should be a
service" is not one.

## Build it to unwind

```bash
cargo build -p tabula-server --profile release-server   # panic = "unwind"
```

The match runtime wraps every `apply()` in `catch_unwind` so a panicking game
aborts **that match**, not the process (doc 01 §5.2). The default release profile
aborts, which would throw that away.
