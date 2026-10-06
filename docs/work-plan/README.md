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

The requested [Werewolf standalone](020-werewolf-standalone.md) is a bounded opt-in local
referee and isolated-seat simulator under [ADR-0035](../adr/0035-werewolf-local-simulator.md).
Its gate/evidence ledger does not activate online social gameplay, voice or rollout.

## Authorized isolated #54 implementation series

[ADR-0036](../adr/0036-isolated-durable-session-validation.md) records the owner's
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

The third slice's [design delta](../ui/screens/account-state-isolated.md) and
[evidence ledger](../verification/issue-54-account-state-ui/README.md) record the
implemented isolated consumer and its remaining target evidence. The next
production work is still the existing prerequisite/backlog work, including
actual provider/TLS/cookie/AT/BFCache evidence, bounded non-secret cross-reload
logout suppression and live deadline/revocation delivery. Do not extend the
exception into voice, invitations or provider provisioning by inference.

## Bounded native mobile voice

The owner explicitly requested [ADR-0037](../adr/0037-native-mobile-voice-client.md)
after the merged isolated account series. This separate client/loopback-dev slice
adds CMP controls and native LiveKit adapters without broad backend authority or
production activation. [Actual native audio acceptance](backlog/native-mobile-voice-acceptance.md)
remains the next target check when authorized hardware/SFU prerequisites exist;
[its ledger](../verification/native-mobile-voice/README.md) separates compilation,
controller/UI doubles and real audio. The unrelated trusted-Origin defect in
[issue #74](https://github.com/loveoverflowcom/tabula/issues/74) is not folded into
this voice change.

## Review #74 follow-up

The merged-tree review's confirmed P3 configuration finding is handled by
[canonical trusted HTTPS Origin validation](050-issue-74-trusted-origin.md),
with [its own evidence](../verification/issue-74-trusted-origin/README.md).
The [remaining review follow-ups](backlog/issue-74-session-followups.md) retain
G2→G1→G3→G4/G5 ordering and their prerequisites. This fix neither depends on
unmerged native voice work nor opens account-dependent production consumers.

## Newly authorized invited OIDC slice

After PR76 normal merge, the owner requested [real invited Kanidm web login](060-invited-kanidm-web-auth.md)
followed by a separate fresh-develop match actor/wire PR. [ADR-0038](../adr/0038-isolated-invited-kanidm-web-auth.md)
permits only the first opt-in implementation and actual disposable provider/DB
proof. Production, persistent-provider setup, public signup, native credential
stores, lobby/queue/friends/match SQL and existing phase exits stay separate.
Normal self-merge needs independent review and exact-tree terminal CI success;
a synthetic fixture or compiled provider module cannot substitute for real login.

PR77 is normally merged with all 14 pre-/post-merge jobs and actual Kanidm
acceptance passed; [060](060-invited-kanidm-web-auth.md) has the verified receipts.
The second requested PR is the [offline match actor/wire slice](070-isolated-match-actor.md)
under ADR0039, from develop0245dc72, now merged as PR78 at develop fd0f1e4.
That completed two-PR authorization did not include SQL/network/production work.
The new owner-requested sequence below is a separate bounded authorization.

## Authorized durable-to-online match series

Fresh baseline: develop `fd0f1e496251a05722de1a0891be9548a6ee7f75`, tree
`f8b10c34dcc02bb93e9568fd817bf1476fa4aab1`, after PR78. The owner requests three
sequential implementation PRs, with normal self-merges after their checks and
review; each PR is implemented in its own fresh chat/worktree from verified develop.
PR79 is merged at `60d0f1ad802c47848e4847def1705a449b51a7b7`; PR80 is merged at `e75624ae870a74f62f0f734fbcf2f12043047dd4`; PR3 is active
in its own separate chat as draft PR83. They are not one expanding PR or broad phase exit.

1. Merged PR79: [consistent PostgreSQL match commit and exact restart/write-failure recovery](080-durable-match-postgres.md), under [ADR0040](../adr/0040-isolated-durable-match-postgres.md)
2. Merged PR80: [join-by-code and two independent browsers completing Chess](090-join-code-browser-chess.md), under [ADR0041](../adr/0041-isolated-direct-match-browser-play.md)
3. Current PR3: [reconnect/resync and network/revocation/server-crash recovery](100-reconnect-resync-fault-recovery.md)

The [PR1 ledger](../verification/durable-match-postgres/README.md) owns actual
DB/process/local/CI evidence and the next-chat handoff contract. Carry exact
merged refs, reproducible commands, verified receipts, durable operation/privacy
laws and residual gates forward. PR2/PR3 need their own bounded online contracts
and real target evidence; PR1's database generation fence is not session/commit/
private socket delivery authority. Production activation, live migrations,
provider provisioning, lobby/queue/voice and existing broad phase exits remain
outside this series unless separately requested and proved.

## Issue #87 — original Design 01 dashboard

[Restore Design 01 discovery](110-restore-design01-discovery.md) is the current bounded web shell/catalog task, from fresh develop `ab0983e`. Existing work ordering and independent phase gates above are preserved.
