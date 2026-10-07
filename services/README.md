# services/

Server binaries. **Leaves**: nothing depends on them.

| Service | Phase | What it is |
|---|---|---|
| [`tabula-server`](tabula-server) | 4 | Gameplay binary: HTTP API + WS gateway + match runtime + lobby; enforces current sessions and resource permissions |
| [`tabula-auth`](tabula-auth) | 4 | Kanidm-backed account auth/session lifecycle skeleton; no listener or credential handling |

Both are compiled frames that print their phase gate and exit with failure.
Implementation TODOs are in their Rust modules; remove each as its implementation
and evidence land. [ADR-0034](../docs/adr/0034-kanidm-auth-service-skeleton.md)
records the auth exception and keeps Phase 4/5 runtime gates closed for issue #54.
Kanidm is operator-managed infrastructure; no deployment is provisioned here.

## One gameplay binary, on purpose (ADR-015/0034)

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

Lobby, chat, catalog and presence remain libraries inside the gameplay gateway.
ADR-0034 reserves account authentication separately; it does not split the room
directory or gameplay runtime. Session enforcement and match grants stay with
the gameplay server, while tabula-auth owns session lifecycle behind storage ports.
All Tabula SQL stays in tabula-storage; services do not depend on one another's crates.

Browser auth routes will share the app's trusted HTTPS origin through proxy
routing. ADR-0031 still governs cookies/native bearer, CSRF and lifetime. A real
cross-service revocation fence is required before enabling either account surface.

Each step has a measured trigger in doc 06 §1.1. "It feels like it should be a
service" is not one.

## Build it to unwind

```bash
cargo build -p tabula-server --profile release-server   # panic = "unwind"
```

The match runtime wraps every `apply()` in `catch_unwind` so a panicking game
aborts **that match**, not the process (doc 01 §5.2). The default release profile
aborts, which would throw that away.
