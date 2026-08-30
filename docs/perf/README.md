# docs/perf/

Committed load-test results. Each file: date, commit, host class, scenario, and
the numbers.

Nightly CI compares against the most recent baseline and **fails on a >20%
regression** (doc 06 §10). A performance number without a committed baseline is
a feeling.

## Scenarios

```text
L1  steady blitz chess     L5  cold start (mass rehydration)
L2  werewolf vote burst    L6  tiles heavy state
L3  spectator flood        L7  deploy under load  ← asserts ZERO lost matches
L4  reconnect storm        L8  mixed realistic
```

## The targets that gate Stage 0 (doc 06 §3.4)

```text
500 CCU sustained, p95 ack < 60 ms, on the production host class
L7: a deploy causes zero lost matches
```

## Per-command latency budget (doc 03 §8.3)

```text
AckAfterPersist   p95 ~25 ms same-region  (dominated by one Postgres round trip)
AckAfterApply     p95 ~5 ms
```

When a command feels slow, the `match.command` span's children say which of
`decode | apply | persist | redact_project | broadcast | effects` is responsible.
That decomposition is why the span is structured that way.
