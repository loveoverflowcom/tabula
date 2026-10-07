# Issue #54 C — friends, presence and request actions

**Status (2026-10-06):** the owner now authorizes real social/lobby contracts
in the isolated composition under [ADR-0044](../../adr/0044-isolated-account-registration-social.md).
The [completion ledger](../../verification/issue-54-account-followup/README.md)
records actual acceptance. [#100](https://github.com/loveoverflowcom/tabula/issues/100)
tracks the same work; #54 cannot close merely by transferring it.

## Original production proposal (historical)

The original production prerequisites below remain outside the bounded exception.

**Outcome:** the signed-in viewer can read/search permitted friends, understand
fresh/unknown/offline/stale presence and handle real requests with clear
permission, pending and terminal status.

**Why:** no friend graph/search/request API, presence stream or approved
freshness/expiry contract exists. Sample rows, green badges and local toggles
would fabricate social state and conceal privacy/idempotency gaps.

**Dependencies:** B's actual identity/session integration, Phase-5 shell gate,
typed authorized social reads/mutations, request kind/actor/recipient/revision/
expiry/conflict/idempotency contracts, and the real single shell lobby socket
with snapshot/delta ordering/resync and visibility/freshness policy. See
[#54](https://github.com/loveoverflowcom/tabula/issues/54),
[Friends](../../ui/screens/21-friends.md) and
[acceptance oracles](../../ui/screens/accounts-social-verification.md).

**Review boundary:** one real viewer-scoped list/presence/request vertical
slice, existing foundation and en/vi. Test empty/filter/no-match vs denied/
failed data, stable-ID ambiguity/IME search, all pending/accepted/declined/
expired/cancelled/restricted states, duplicate/conflicting/unknown mutation,
expiry at commit, late/gapped/restarted presence, permission/account change,
safe navigation and keyboard/AT focus. Record doubles vs service integration,
then targeted checks, `just check` and applicable target evidence.

**Risks / unknowns:** a friend relationship never grants seat/room/spectate/
voice permissions; room/game invites are not friend requests. Hidden presence
must not become Offline or disclose blocked-by status. Old callbacks cannot
resurrect private data or overwrite newer authoritative terminal revisions.

**Non-goals:** matchmaking/voice, notifications, graph extensions, security/
privacy settings or a second match/per-friend socket. Remove/block/unblock
requires a separately approved real mutation contract; safe denial remains
required even when those controls are absent.
