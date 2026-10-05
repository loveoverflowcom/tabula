# Isolated authoritative match actor

**Status:** implemented; focused and full local checks passed; independent
security source review has no unresolved blocker. Draft publication and exact-tree
terminal CI remain pending; normal merge is authorized only after those gates.

**Outcome:** one owner validates resolved seat/session authority, serializes
opaque game commands, applies actual approved game rules, commits one journal
record, retains bounded duplicate receipts and submits only authorized projections.

**Why:** second and last of the two owner-requested implementation PRs after
PR76 and invited Kanidm PR77. The bounded exception is [ADR0039](../adr/0039-isolated-match-actor-runtime.md).

**Dependencies:** fresh remote develop0245dc72/tree55a47eec, PR77 merged and all
14 post-merge CI jobs passed. There is no dependency on unmerged PR69/70/75.

**Review boundary:** current authority at actual apply/output, ordering across
one bounded FIFO, full operation/payload conflict identity, TTL/eviction without
watermark loss, backpressure/cancel/reentrancy, panic/commit/effect failures,
strict dual-codec serde/bounds and private-event existence/counter-gap privacy.
Actual game transcripts and faithful offline authority/journal/output/effect
ports are distinct from real authenticated network/storage evidence.

**Acceptance:** [delivery ledger](../verification/isolated-match-actor/README.md),
authoritative `cargo xtask check`, strict feature lint, native/WASM feature/build
and kernel entropy/dependency gates; independent security review; exact head/tree
and terminal CI before normal Ready/self-merge; merge ancestry/post-merge CI.

**Remaining gates:** SQL match durability/recovery/snapshots, lobby/queue,
reconnect/resume, network quotas/traffic shaping, actual provider-bound match
authority/commit/private-delivery fences, spectator delay, real sockets, target
browser/native proof, deployment/SLO/load and full Phase 3/4/5 exits.
Neither service listener is opened. Further work requires a new owner task.
