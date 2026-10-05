# ADR-0039: isolated authoritative match actor

- **Status:** accepted narrow owner-requested implementation; production remains closed
- **Date:** 2026-10-05
- **Amends:** Phase 4 ordering for this offline slice; the unimplemented global-counter wire sketches in doc 03 §8 and doc 05 §2
- **Invariants touched:** none relaxed; I-1–I-9, I-13–I-16 preserved

## Context and authorization

The owner requested two implementation PRs from remote develop after PR76:
real Kanidm web authentication, then command validation, seat permissions,
ordering and duplicate protection in a match runtime, with normal self-merges
after checks. PR77 merged at `0245dc72c6a356e8e7cbbb1f7601c1d09c634915`;
this independent slice starts there. ADR0035/0037 and PR69/70/75 are unrelated
and remain unmerged. This decision records the already-approved bounded work,
not a Phase 3/4/5 exit or permission to activate a service.

## Decision

The native opt-in `tabula-match/isolated` runtime owns one erased game instance
in one Tokio task and one bounded FIFO mailbox. Registry's generic adapter owns
typed state and game validation/projection; the host owns admission and authority.
The default/WASM contracts remain runtime-free. There is no SQL match adapter,
listener, lobby/queue, reconnect/resume, recovery, deployment or live account grant.

Creation and every accepted input go through one atomic journal port before
success or effects. Its in-memory test implementation is an offline commit
receipt, **not disk durability**. Journal or effect uncertainty stops the match;
no background ack-after-apply policy is implemented. Rules receive only recorded
logical time, input index and deterministic RNG. The actor catches game panics,
closes its mailbox and reports a terminal exit without exposing panic details.
No automatic restart or production supervisor is claimed.

The host supplies resolved, non-serializable session-record/subject/epoch and
seat-generation bindings. A synchronous authority port encloses the actual
apply or actual output submission. After the journal await, output reacquires
current authority. This proves only the exercised in-memory ordering boundary;
it is **not** the ADR0031/0036 durable private-delivery/WS fence. Buffered output
delivery after enqueue and distributed revocation remain unimplemented gates.
Authorization at apply is not atomic with an awaited journal commit: an input
already applied may commit after revocation, while its later private output is
suppressed. Online command/revocation commit fencing remains separately owed.
Client attachments cannot select Audit or a delayed spectator tier.

Operation identity is session record, subject, epoch, assigned seat and seat
generation plus nonzero client sequence. Exact game/match/package identity and
command bytes are retained for duplicate comparison; correlation is retry
metadata. Same operation and payload replays its original Ack/Reject through
fresh output authority, while changed payload is OperationConflict. Rejected
game/malformed commands consume their sequence and retain a receipt; unauthorized
and sequence-window failures do not. Receipt count and TTL are bounded, but the
high-watermark is retained for every admitted scope for the actor lifetime.
Expired/evicted receipts yield StaleSeq and never reapply. When scope capacity is
full, new scopes are refused instead of evicting watermarks. Process restart
loses this cache; exactly-once across crashes is not claimed.
Seat operation scopes are reserved during successful authorized attachment,
before any game command. Lazily reserving on a private command would let another
seat infer that action by probing whether shared scope capacity became Busy.

The isolated generic registry bridge defensively applies to state/RNG candidates
and commits only after accepted outcome serialization. This narrowly supersedes
doc 02 §3.3's planned release-no-clone optimization for this adapter; game-owned
R2/R8 contracts and conformance remain required. Optimize only after measured
cost and equivalent transactional guarantees, without silently removing defense.

Mailbox/global attachment limits and per-session in-flight quotas bound memory
and prevent one admitted sender occupying all queued work. Admission is
nonblocking; busy/closed is returned without enqueue. FIFO determines order
across admitted commands, not client timestamps or game turn rules. Dropping a
ticket before dequeue skips the operation and consumes no sequence. Once apply
begins, caller cancellation cannot undo the journal transaction; a retry uses
the retained receipt. Host controls share the same mailbox. There are no port
callbacks that recursively await the owner; callbacks may only try-enqueue.

## Observable privacy and wire identity

Canonical state version, input index, time, seed, state hash and canonical events
stay inside the journal. Global versions leak private action existence through
gaps even if `view_event` returns None, so they are absent from this wire slice,
including Ack/Reject. Each attachment has its own output frame and visible
revision, advanced only after known-success output submission. Each explicit
attachment starts its own stream at frame1/revision0; full network connection
multiplexing/reconnect cursor continuity is outside this slice. A private-only
transition with unchanged projection and no visible events sends nothing to an
unentitled viewer. A public transition remains observable. This is semantic
stream/count/gap privacy, not constant-time execution or network traffic shaping.

Protocol version `0.1` is the first executable, explicitly isolated contract,
not a launched v1 network protocol. Dual Postcard/JSON codecs validate the same
domain, including ordinary serde paths, exact version and sequence, and early
allocation/length limits. Committed vectors and executable codec tests enforce
this first version. Future wire changes require a version bump and compatibility
fixtures. The future `xtask check-protocol`/vector-generation stubs are not
represented as executed checks. Minimal frames contain no GameCapabilities;
that unresolved full-protocol layering question is not settled by this slice.

## Evidence and remaining gates

The delivery ledger records real approved game-state actor transcripts plus
faithful authority/journal/effect/output fakes, counterexamples, hostile codecs,
bounded cache/admission/cancellation/failure cases, and independent review.
Offline fixtures do not prove real authenticated online sessions. SQL durability,
timer scheduling/recovery, real WS/private-output fencing, remote ownership
leases, network quotas/traffic shaping, load/SLOs, browser/native enforcement,
public signup and all broad phase exits remain gated. Effect ports receive every
request with a stable committed-input key; no timer/chat/voice/bot service is
advertised as active. Neither production entrypoint is changed.
Private Notify/chat/voice/bot adapters require their own current audience/seat/
session fence at the actual effect. A journal key and the separate Output guard
cannot authorize those deliveries. The host must also enforce unique logical
MatchId ownership and capability-based spectator admission; exclusive ownership
of this erased-state object is not a global directory or ownership lease.

Revisit before any network consumer, persistence/recovery adapter, durable
authority integration, delayed spectator, production listener or broad phase
claim. Require actual adapter/target evidence and review at that boundary.
