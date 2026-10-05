# Issue #54 PR2 — isolated session and self-profile HTTP

**Status:** implemented on PR71's exact verified head in a fresh work session;
final real-database/aggregate/publication receipts are pending. See the
[bounded ledger](../verification/issue-54-isolated-http/README.md).

**Outcome:** exercise current durable session authority through a same-origin
isolated HTTP context and permitted read-only self-profile boundary.

**Why:** storage receipts cannot prove transport channel separation, CSRF,
origin, no-store or HTTP authority. Those must precede account-state UI.

**Dependencies:** PR1 exact verified head; ADR-0031/0034/0036. Use an explicit
stacked base if PR1 is unmerged. Resolve the small proposed HTTP/data contract
against actual library/storage APIs without inventing handles/statistics or
interpreting a provider fixture as verified authentication.

**Review boundary:** real database + HTTP tests for exact cookie/native channel,
ambiguous credentials, trusted Origin/content type/synchronizer context,
expiry/rotation/logout ordering, unavailable/unauthenticated dispositions and
private self reads. No production service startup. Actual outbound fencing
requires its own concrete proof before any corresponding claim.

**Risks / unknowns:** cookie races, failed or uncertain commits, stale context,
provider auth-time/reauth, and private publication remain distinct obligations.
Synthetic provider and browser controls must be labeled with their limitations.

**Non-goals:** production activation/deployment, provider setup/OAuth grants,
registration/edit/social/ratings, real native keychain proof, online gameplay,
voice, merging and broad phase exits.
