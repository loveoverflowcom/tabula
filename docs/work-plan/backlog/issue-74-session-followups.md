# Issue #74 — remaining session review follow-ups

**Status:** deferred to their actual consumer/prerequisite. F1 is handled by
[the canonical Origin fix](../050-issue-74-trusted-origin.md).
[#74](https://github.com/loveoverflowcom/tabula/issues/74) owns the acceptance
criteria; this short queue preserves their recommended sequence, not a new
production-activation exception.

**Outcome:** account-dependent production consumers have current authority,
transport/lifecycle evidence and measured capacity before activation.

**Why:** PR #71–#73's isolated implementation does not establish whole-issue
production security. F1's configuration validation has no dependency on these
larger contracts.

**Dependencies and recommended boundaries:**

1. **G2:** define lifetime/effect/private-output and owned-connection fencing
   before account-dependent gameplay/voice/production. Cover actual TCP/WS,
   queued output, idle expiry, revoke/epoch change and process/backend loss;
   snapshots are not permanent authority. Provider/TLS/native secure-store
   prerequisites remain explicit
2. **G1:** bounded non-secret logout suppression across document reload,
   then offline/reload/reconnect/replacement-context browser lifecycle tests.
   Persist no credential or private profile; do not clear intent on ambiguity
3. **G3:** real browser/native pixels, first restored frame/tree, focus/IME/AT,
   cookie/TLS/storage and real provider acceptance. Build/headless/loopback
   receipts cannot substitute for these targets
4. **G4:** measure sequential context→profile, refresh/logout, multiple tabs,
   pool pressure and cancellation before changing the conservative ~2-second
   committed lease. Preserve backend-loss/suspended-callback exclusion; no SLO
   violation or performance improvement is claimed by the Origin fix
5. **G5:** before a provider/preauth consumer activates, invalidate server
   preauth state on successful logout and test old-cookie/token replay. Keep
   that locking/error-ordering change separately reviewable. Add deterministic
   UNIQUE-wait crossing-expiry and acquisition-abort/runtime-shutdown/stalled
   close fault partitions when their trusted-port/consumer contract needs them

**Risks / unknowns:** absence of a new P0/P1/P2 finding in the isolated review
is not a production acceptance result. Browser/display access blockers must be
respected. No enabled login/register consumer currently gains account authority
from replayed preauth state; G5 is not represented as an auth bypass.

**Non-goals:** infer phase exits, implement all provider/WS/native UI machinery
inside a configuration fix, weaken fencing for latency, merge/deploy or close
#54/#74 before their acceptance is satisfied.
