# Invited Kanidm web authentication

**Status:** completed and normally merged as [PR77](https://github.com/loveoverflowcom/tabula/pull/77)
at develop `0245dc72c6a356e8e7cbbb1f7601c1d09c634915`, tree
`55a47eec8b1872c1f20e6c48085eaa30d2cf3402`. Independent review and all 14
pre-/post-merge jobs passed. Actual Kanidm 1.11.2 MFA/code+PKCE/ES256 and
PostgreSQL session/profile/logout/epoch lifecycle executed with 1 passing,
0 failed, 0 ignored provider case. Receipts: [main 12 jobs](https://github.com/loveoverflowcom/tabula/actions/runs/37292012677),
[PostgreSQL](https://github.com/loveoverflowcom/tabula/actions/runs/37292012767),
[real provider](https://github.com/loveoverflowcom/tabula/actions/runs/37292012776).

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

**Next:** the separately approved [offline match actor/wire slice](070-isolated-match-actor.md)
starts from this verified fresh remote develop with its own gate exception and
review. No match SQL/store, lobby, friends, queue or reconnect is included.

**Remaining gates:** production/TLS/app browser/native/AT/BFCache acceptance,
provider password-change synchronization, live private output/WS fences,
capacity, deployment and full phase/#54/#74 closure. Persistent-provider setup
requires specific access/credential/account/grant approval; no live changes occur here.
