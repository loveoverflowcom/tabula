# Invited Kanidm web authentication

**Status:** isolated implementation and local/source-review gates passed; real-provider CI and draft publication pending.

**Outcome:** a previously admitted person completes real Kanidm code+PKCE login,
receives a durable opaque HttpOnly Tabula cookie, reads current `/me`, logs out,
and cannot regain authority through replay, old epochs or stale completions.

**Why:** the owner requested the first of two next implementation PRs after PR76.
Existing account UI/session fixtures did not authenticate an actual provider.

**Dependencies:** fresh remote develop172f222 after PR76 normal merge and 13/13
post-merge checks. ADR-0031/0034/0036 and the narrow [ADR-0038](../adr/0038-isolated-invited-kanidm-web-auth.md).
No unmerged game/assets/voice dependency. Actual isolated provider + PostgreSQL
CI is required, along with independent security review and exact-tree checks.

**Review boundary:** provider trust/crypto/epochs; cookie-bound single-use preauth
and concurrent cancel/issuance; web generation/privacy/reload suppression; honest
capabilities and disposable real-provider evidence. [Ledger](../verification/invited-kanidm-web-auth/README.md).

**Next:** after this PR is normally merged and post-merge checks succeed, start
a fresh develop match actor/wire permissions/idempotence PR with its own gate
exception/review. No match SQL/store, lobby, friends, queue or reconnect is included.

**Remaining gates:** production/TLS/app browser/native/AT/BFCache acceptance,
provider password-change synchronization, live private output/WS fences,
capacity, deployment and full phase/#54/#74 closure. Persistent-provider setup
requires specific access/credential/account/grant approval; no live changes occur here.
