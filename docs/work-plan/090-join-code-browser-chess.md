# Connect two browsers by join code

**Status:** PR2 incomplete; three partial recovery branches integrated by owner request.
[ADR0041](../adr/0041-isolated-direct-match-browser-play.md) records the bounded
contract; the [ledger](../verification/join-code-browser-chess/README.md) records
actual implementation/check status. Browser acceptance is not yet claimed.

**Outcome:** two independent browser sessions join the same server-owned Chess
match by code and complete a game through the actual projection/command path.

**Why:** durable isolated authority is valuable infrastructure, but the next
independently useful user capability is actual two-player browser gameplay.

**Dependencies:** [PR1](080-durable-match-postgres.md) merged with exact-tree/post-merge
CI receipts; fresh remote develop; explicit narrow online contract/phase exception
before implementation. ADR0031's browser cookie, origin/CSRF, upgrade and match
permission rules and ADR0039/0040 privacy/durability laws remain authoritative.

**Review boundary:** bounded join-code issuance/lookup and seat admission, one
generic match dispatch path, real authenticated transport and projection-only
output, isolated browser handoff and actual two independent browsers. Resolve
required current command/commit/output authority fences before exposing private
online output; a synchronous test port is insufficient evidence.

**Acceptance:** real browser-enforced cookies/origin and full Chess transcript
in two independent contexts, current seat/session authority, hostile code/seat/
match attempts, exact durable final state/result and no canonical-state disclosure;
applicable portable/feature/target checks, independent review and exact-tree
terminal CI before the authorized normal self-merge.

**Non-goals:** PR3 reconnect/resync/fault campaign, public signup, lobby/queue,
friends, voice, deployment/live migration or broad phase completion.

**Risks / unknowns:** strict composed migrations and current durable commit/body
publication authority need real PG proof; independent HTTPS rendered gameplay
needs the dedicated actual-browser CI fixture. Do not invent
routes/schemas or weaken wire privacy to make browser integration easier.

**Next handoff:** once PR2's actual browser evidence and normal merge are verified,
carry its reproducible launch/transport/browser instructions and remaining fault
partitions to [PR3](100-reconnect-resync-fault-recovery.md) in another chat.
