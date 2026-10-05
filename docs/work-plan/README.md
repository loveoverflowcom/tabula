# Upcoming work

Numeric prefixes, when present, are the current recommended sequence and may
be renumbered; they are not permanent issue IDs. GitHub issues own acceptance
and the screen specifications own UI contracts.

The authorized queue #51 → #52 → #53 → #59 → #60 has a scoped delivery for
**#60**, based on verified `develop @ 2de46d27efafd078ba2d27b027a54de565e2fbe5`.
The [#59 ledger](../verification/issue-59/README.md) remains the asset/workload
baseline and retains its outstanding target evidence. The
[#60 RFC](../rfcs/issue-60-renderer-embedding.md),
[ADR-0029](../adr/0029-renderer-embedding-spike.md), and
[execution ledger](../verification/issue-60/README.md) contain the isolated
Macroquad document/iframe and Pixi canvas prototype, without a production route
or engine migration. ADR-011 remains in effect; the production decision is deferred.

The [remaining renderer/embedding evidence](backlog/issue-60-renderer-embedding-evidence.md)
is the next narrow investigation when its target prerequisites are available.
Do not infer that a tool-only Leptos example, native loopback authority, or passing
baseline checks opens Phase 4/5 implementation. Preserve target-specific status
and measure renderer and wrapper effects separately.

The [#52 history/replay slice](backlog/issue-52-history-replay.md) remains gated.
#53's [board/modes](backlog/issue-53-board-modes.md),
[engine/resources](backlog/issue-53-engine-evidence.md) and
[Learn](backlog/issue-53-learn.md) follow their explicit prerequisites.
Passing existing baseline checks does not open these gates or close #52/#53.

The standalone Chess local vertical slice retains ADR-010/011/029. Its next
specific check is [real platform evidence](010-standalone-chess-platform-evidence.md).
The [implementation ledger](../verification/standalone-chess/README.md) separates
executed rules/presentation/build checks from unavailable real runtime pixels.
Observed coordinate history does not close the gated #52 replay slice.

The requested fresh-session Chess integration is implemented locally under
[ADR-0030](../adr/0030-local-discovery-gameplay-handoff.md), reusing the standalone
runtime in an opt-in separate document. Its
[ledger](../verification/chess-integration/README.md) records core/host/config/HTTP
checks and release bundles; new publication is not authorized. The same next
platform-evidence task now includes discovery/launch/return and real BFCache/
resource behavior. Native catalog, server resume, online/rated/AI and phase exits
remain gated.

Issue #54's [account/social specification](../ui/screens/accounts-social.md)
and [acceptance ledger](../ui/screens/accounts-social-verification.md) deliver
PR A, originally reviewed against
`develop @ 3527b65d6643d805d6c80352d165d97f71417ccc` and now aligned with
[ADR-0031](../adr/0031-browser-native-session-contract.md).
Its next slices are deferred, not new active phase crossings:
[B auth and self-profile](backlog/issue-54-auth-profile.md), then
[C friends and presence](backlog/issue-54-friends-presence.md).

[#54 session policy](../adr/0031-browser-native-session-contract.md) is now
reconciled in a separate prerequisite contract from
[PR #62 specification A](https://github.com/loveoverflowcom/tabula/pull/62).
The [session backlog](backlog/issue-54-session-contract.md) and
[adversarial acceptance matrix](../verification/session-contract/README.md)
retain actual platform/portfolio/Phase-4/server/Phase-5/API gates for B/C.
B still needs approved registration fields and public-error/self-profile
contracts; C needs real typed social/lobby authority. The ADR resolves the
policy choice only; PR A is independently reviewable and no auth/social service
or online match is activated. Existing discovery/local-play ADRs do not authorize
account services. Passing aggregate checks does not close #54 or open Phase 4/5.

## Authorized isolated #54 implementation series

[ADR-0035](../adr/0035-isolated-durable-session-validation.md) records the owner's
bounded exception without opening production or claiming phase exits. These are
three new implementation PRs; the merged specifications/ADR/service frames are
prerequisites and are not counted again:

1. [Durable session policy and real PostgreSQL authority](020-issue-54-session-foundation.md)
2. [Isolated session/context and self-profile HTTP](030-issue-54-isolated-http.md), after PR1 completes
3. [Isolated account-state/profile UI](040-issue-54-account-state-ui.md), after PR2 completes

The historical B/C production backlog remains valid outside this exception.
Whole-issue login/register/friends acceptance, actual provider and browser/native
proof, output fences and Phase 2/3/4/5 exits remain separate gates. Native voice
is outside this series.
