# Recover online matches through interruptions

**Status:** owner-requested PR3; deferred to its own later chat after PR2's
verified normal merge. No network recovery or fault acceptance is claimed yet.

**Outcome:** the two-browser online slice survives reconnect/resync, network loss,
current-authority revocation and actual server crash with correct committed state,
duplicate semantics and projection privacy.

**Why:** completing a happy-path online Chess game does not prove uncertain-send,
reconnection, expired/revoked authority or process-death behavior.

**Dependencies:** [PR2](090-join-code-browser-chess.md) actual browser proof and merge
receipts; fresh remote develop; ADR0031 session/permission/commit/private-output
ordering, ADR0039 visible stream privacy, and ADR0040 exact durable recovery.

**Review boundary:** permission/grant revalidation before restored data or pending
command replay; retry identity retained across uncertain sends; explicit resume
versus full projection resync; buffered output revoked at actual delivery;
recorded-time/timer/outage policy; real process kill and fenced new owner.

**Acceptance:** real two-browser network interruption and partial/lost-frame cases;
retained/expired/evicted duplicate receipts, current-session/seat/epoch changes,
revocation ordered before/during commit and private delivery, actual server kill
with committed/uncommitted outcomes, correct Chess state/clocks and no private
counter-gap or restored-frame leakage; checks/review and exact-tree CI before
normal self-merge. Name any unavailable browser/native/AT/load evidence precisely.

**Non-goals:** production activation/live migration, public signup, lobby/queue,
friends/voice, native secure-store acceptance, distributed placement/leases,
backup/PITR/load/SLO or broad phase exits unless separately requested and proved.

**Risks / unknowns:** the online interruption policy must not be inferred from
offline actor recreation. Recovery must resolve durable truth and fresh authority
before showing state; timeout is not proof that a command failed to commit.
