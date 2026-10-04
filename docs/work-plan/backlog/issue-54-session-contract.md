# Issue #54 — implement the accepted session contract after gates

**Status:** policy reconciled in [ADR-0031](../../adr/0031-browser-native-session-contract.md);
runtime deferred. [PR #62](https://github.com/loveoverflowcom/tabula/pull/62)
is separate specification A and remains independently reviewable.

**Outcome:** real, revocable browser/native sessions support sign-in and
read-only self-profile with trustworthy transport/lifecycle, then viewer-scoped
social data. Account-free supported local play stays reachable.

**Why:** the former cookie/localStorage/Hello-bearer disagreement is resolved,
but server, persistence, protocol and client session enforcement are not
implemented. A frontend flag would conceal that missing authority.

**Dependencies:** [Phase-2 actual platform evidence](../010-standalone-chess-platform-evidence.md),
Phase-3 portfolio conformance/projection/benchmark/freeze exit, Phase-4 real
authority/server/protocol/persistence integration and exit, then Phase-5 shell
gate under [doc 07](../../architecture/07-phases-and-implementation-roadmap.md).
ADR-0028/0030 allow only their bounded discovery/local-play slices. Do not
duplicate unpublished local Caro/Werewolf work or infer its completion.

**Review boundaries:**

1. When Phase 4 is actually open, implement one coherent identity/session
   boundary: ports and real persistence, channel-bound cookie/native adapters,
   CSRF/context, upgrade authentication, expiry/rotation/revocation fencing.
   Run [S01–S14](../../verification/session-contract/README.md#required-acceptance-scenarios)
   against real services/DB/targets, documenting doubles and missing receipts
2. B additionally needs registration fields/handle/normalization/password/
   agreement/disclosure/disposition and actual self-profile API. Start with
   login/register/self read-only after the shell gate; preserve A's safe return,
   generic errors, form/IME/focus/password-manager/AT and local escape
3. C additionally needs real typed social reads/mutations and lobby stream,
   viewer permissions, presence freshness/visibility/order/resync and request
   actor/recipient/revision/expiry/conflict/idempotency. Reuse one shell lobby
   socket; session validity never manufactures social permission

The pinned [B backlog](https://github.com/loveoverflowcom/tabula/blob/8e62f1c7be54d429a5362495388d53ac2e67f9e9/docs/work-plan/backlog/issue-54-auth-profile.md),
[C backlog](https://github.com/loveoverflowcom/tabula/blob/8e62f1c7be54d429a5362495388d53ac2e67f9e9/docs/work-plan/backlog/issue-54-friends-presence.md)
and [shared specification](https://github.com/loveoverflowcom/tabula/blob/8e62f1c7be54d429a5362495388d53ac2e67f9e9/docs/ui/screens/accounts-social.md)
remain the UI sources; ADR-0031 supersedes their unresolved session-policy
choice only. Later A integration updates those links/wording, not their scope.

**Risks / unknowns:** chosen conservative lifetime may require reauth during
long waits; cross-tab cookie races can force safe sign-out; browser session
restore retains cookies; offline logout cannot prove server revocation;
queued commands/output need a commit-order fence. Native secure-store and real
browser behavior need receipts. Public registration disclosure, private
profile and social policies remain unresolved by the session ADR.

**Non-goals:** new OAuth/reset/verification providers, remember-me, JWT-only
accounts, credential plaintext fallback, private account caching, security
settings, full profile editing, new online game implementation, voice,
cross-origin auth/embedding, multi-process services, merge or deployment.
