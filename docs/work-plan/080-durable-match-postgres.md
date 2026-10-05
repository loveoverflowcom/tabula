# Persist match commits and recover safely

**Status:** current authorized PR1; implementation and verification in progress.
No executed checks or merge receipt are asserted by this planning entry.

**Outcome:** the isolated actor's committed canonical input/events/version/hash,
snapshot and complete bounded operation ledger survive real PostgreSQL writes,
actor/process restart and write uncertainty without partial state or double apply.

**Why:** PR78's in-memory journal loses duplicate protection and canonical state
on process death. Independent receipt/snapshot persistence cannot protect the
consistent operation result. [ADR0040](../adr/0040-isolated-durable-match-postgres.md)
records the narrow implementation exception and durable laws.

**Dependencies:** fresh remote develop `fd0f1e4` after PR78; existing exact approved
rules bridge and isolated wire 0.1. No unmerged game/assets/native-voice dependency.

**Review boundary:** SQL-free journal DTO/port separated from optional registry;
native opt-in match-postgres; one transaction for committed head/input/events/
snapshot/whole bounded ledger; persistent monotonic scope watermarks; reservation
at authorized attach; expected-version and durable owner-generation fence;
fail-stop on known/indeterminate write; exact deterministic corruption-rejecting
reopen; no global canonical metadata in client output.

**Acceptance:** [claim/evidence ledger](../verification/durable-match-postgres/README.md);
non-empty real PostgreSQL 16 transaction/fault/reopen/stale-owner and process-crash
fixtures; faithful actor faults; authoritative `cargo xtask check`, feature/target/
dependency/entropy checks and unchanged startup closure; independent review;
exact published head/tree with all required terminal CI before normal self-merge,
followed by merge ancestry and post-merge CI verification.

**Non-goals:** network listener or join codes, two-browser Chess, reconnect/resync,
online revocation/commit/socket fence, production effect services/outbox, automatic
supervision/timer scheduling, lobby/queue, live migrations, deployment and phase exits.

**Risks / unknowns:** DB commit acknowledgment loss is an uncertain outcome and
must be resolved by reopen; snapshot acceleration cannot bypass log consistency.
Production seed encryption and real output/authority fences remain prerequisites.
Execution statuses stay pending until the ledger receives actual run receipts.

**Next handoff:** after verified normal merge, start
[PR2](090-join-code-browser-chess.md) in its own later chat from fresh develop.
Carry the merged commit/tree, passing CI links, migration/test commands, exact
journal/recovery and privacy contracts, and unresolved activation gates.
