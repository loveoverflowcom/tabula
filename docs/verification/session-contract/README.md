# Session contract — acceptance and evidence

Date: 2026-10-04. Source baseline:
`develop @ 7f3aad749cd1ed1140a32d5f629f14a66711868e`.
[ADR-0031](../../adr/0031-browser-native-session-contract.md) resolves the
storage/transport contradiction for [#54](https://github.com/loveoverflowcom/tabula/issues/54).
This is documentation and behavior-neutral scaffold-comment reconciliation.

## Source and delivered boundary

| Claim | Owner, failure mode and oracle | Evidence/status | Residual |
|---|---|---|---|
| Current source/PR isolation | Fresh develop and PR #62 API records; separate worktree | PASS — source-read: develop SHA above, PR #62 draft at `8e62f1c7be54d429a5362495388d53ac2e67f9e9`, mergeable clean; 11/11 exact-head checks successful | PR #62 is separate; its passing baseline checks do not implement session security |
| Storage/attachment policy reconciled | ADR register, docs 01/03/04/05, apps handoff, net-client/protocol/server comments | Documented — cookie/native channel separation, credential-free Hello, public-only handoff, CSRF and lifecycle | Runtime authentication/security enforcement NOT_IMPLEMENTED |
| Phase truth preserved | PHASE-4 protocol/net-client/storage/server, actual server gate exit, Phase-5 lobby and shell | PASS — source-read; no executable change or PHASE removal | Platform/portfolio/authority exits remain unestablished by this change |
| Security choices grounded | OWASP/WHATWG/MDN primary mechanisms listed in ADR | PASS — source review, independent security audit | Chosen timeout/grant bounds are product policy, not measured safe production limits |
| Documentation/check receipts | Final links, whitespace, comments-only diff and repository gates | See publication/check results in the PR description, pinned to its exact head | Structural checks and existing game tests cannot prove future session behavior |

No authentication harness exists here. Required scenarios below are **acceptance
specifications**, not runnable test names and not PASS. Record real service/DB,
native secure-store and browser evidence separately from doubles. Use an
independent expected policy, non-empty selected cases and fault/clock/race
controls; do not derive permissions from the implementation under test.

## Required acceptance scenarios

| ID / claim | Adversarial setup and independent oracle | Required evidence / gate |
|---|---|---|
| S01 credential creation/storage | Two fresh logins create different canonical 32-byte OS-random bearers; public UUID/record ID rejected. Server rows contain only digest. Inspect actual browser cookie flags/host/path and verify document.cookie/JS/WASM/localStorage/sessionStorage/IndexedDB/CacheStorage/URL/logs contain no session bearer | Phase 4 real DB+HTTPS browser integration and entropy-source review; statistically different values alone do not prove cryptographic entropy |
| S02 channel/ambiguity | Valid browser-cookie/native-bearer controls succeed; wrong-channel replay, both together, duplicate cookies/headers, malformed encoding, missing native bearer and spoofed platform/User-Agent fail with no alternate identity selection | Phase 4 real HTTP+WS gateway policy; inspect side effects/private bytes, not only status |
| S03 CSRF/pre-auth | Valid same-origin JSON+matching synchronizer control succeeds. Cross-site and same-site sibling origin, scheme/port change, missing/null Origin, simple form/text, missing/wrong/stale context token and hostile preflight fail. Login CSRF with attacker's account fails; pre-auth context cannot read private data or upgrade | Phase 4 server+real browser boundary. Assert zero auth/session/mutation effect and credentialed CORS not enabled; verify no-store and pre-auth destruction |
| S04 upgrade/Hello | Valid cookie+Origin browser/native bearer controls reach HelloAck; origin bypass, stolen browser token in native header, revoked/suspended/expired record and mixed credentials fail before 101/private output. Missing/late Hello, malformed/non-negotiation first frame, codec/version mismatch and 5-second deadline fail; Hello/subprotocol/query contain no bearer | Phase 4 real upgrade tests and wire vectors. Browser generic WS failure stays transport failure until context resolves; negotiation tests are not authentication proof |
| S05 match/grant authority | Positive exact session/account/match/seat and permitted spectator controls. Missing grant, wrong issuer/type/audience/key/algorithm/nbf, altered claims/signature, other match/subject/session/seat/viewer, expired token, removed membership, wrong/stale authorization epoch and Audit reject. Exercise just-before/equal/after exp/nbf; reject expiry beyond 10 minutes or current session deadline. Replay remains valid only for same currently authorized binding within TTL; revoked session fails despite valid signature | Phase 4 signed-grant+real session/match authorization and I-5/I-6 projections; an authentic signature alone is insufficient |
| S06 handoff/reconnect | Navigate/reload/deep-link/back into separate game document using public-only hints. Expired/missing grant reacquired only with current session/permission; forged cursor/sequence/seat hint gives no authority. Revoked session/changed occupant fails; no old-account pending replay, local play needs no auth | Phase 4 adapters/match integration, Phase 5 real browser handoff; no sessionStorage token, hidden state, duplicate game input or clock reset |
| S07 expiry/activity | Controlled server clock at just-before/equal/after 30-minute idle and 24-hour absolute limits. Advance with only heartbeats/context/poll/asset/refresh and show no extension; reads/rejected/duplicate/control operations never extend idle; only accepted protected mutation/game command updates idle bounded by absolute. Silent open WS expires and stops private output. Wrong client clock grants nothing | Phase 4 fake-clock policy plus actual timer/gateway integration; long spectator wait requires reauth, not invented activity |
| S08 refresh race/fault | Two requests with same old generation race: exactly one CAS winner emits new bearer/Set-Cookie; loser returns no replacement/deletion cookie. Old verifier fails immediately; stable session CSRF and ordinary live WS remain valid; subject/deadline unchanged. Drop response, retry stale token and deliver old response after logout/new login | Phase 4 transactional real DB+HTTP fault injection and browser cookie inspection; no stale resurrection or DB plaintext recovery; indeterminate recovery may require sign-in |
| S09 revocation fence | Queue move/read/private output, pause before authorization/commit/send, then commit logout/revoke/password/privilege epoch change and release. Ordered-after work never commits/sends; all linked sockets close 4401. Already committed/sent control remains recorded, not falsely rolled back. Another device survives current-device logout | Phase 4 storage/session/actor/gateway ordering integration with observed commit/fence, not socket-close assertions alone |
| S10 offline logout/switch | Lose network while signed in, choose logout and keep surviving HttpOnly cookie. Private UI clears, server revocation remains pending; refresh/BFCache/reopen/late callbacks cannot silently restore account. Reconnect reconciles before switch; native keychain deletion with lost revoke ability reports unresolved server revocation | Phase 4/5 real failure/reload lifecycle; local cleanup is not durable revocation or logout everywhere |
| S11 cross-document isolation | Two tabs/shell-game documents, one misses broadcast or is BFCache restored; account switch/revocation/private permission narrowing during reads/mutations/subscriptions. Every resumed document masks/revalidates; stale responses and grants/projections/AT output remain absent | Phase 5 real pageshow/focus/back/forward/tab lifecycle plus service permissions; event mocks alone insufficient |
| S12 public errors/unavailability | Existing/unknown identifier, bad password, duplicate identity, disallowed account, rate limit, DB fault and uncertain mutation result. Compare approved public status/body/timing/disclosure and recovery; no auth fallback from cached profile/private data | Phase 4 anti-enumeration/error policy and Phase 5 UI disposition; generic copy alone does not establish timing privacy |
| S13 native secure store | Each shipping OS keychain scope works; wrong server/account, access failure, locked device, missing store and deletion/restart tested. Failures do not store plaintext or claim durable login/logout; redacted transport/log/backup inspection | Phase 4 adapter and actual native/mobile target receipts; builds/mocked store are not secure-store proof |
| S14 private caching | Protected context/profile/login/native-credential responses no-store, not cached by service worker/CDN; public asset requests omit cookies. Account/server/permission change rejects previous cache entries and strips private telemetry/error bytes | Phase 4 HTTP integration, Phase 5 real browser/network/storage inspection |

Every implementation boundary also runs its targeted non-empty checks and
`cargo xtask check`; changed executable wire types require I-13 version/golden/
compatibility evidence. Feature/WASM builds, actual IME/password-manager/
keyboard/AT and UI layouts remain necessary for #54 B/C; S01–S14 do not replace
PR A's full UI/form/profile/social acceptance set.

## Executed local checks

Toolchain: official Rust 1.96.1; isolated worktree with shared build cache.
`source ../toolchain/env.sh; export CARGO_TARGET_DIR=/workspace/scratch/429f9ca90843/tabula/target; cargo xtask check`
passed the authoritative ordered fmt, workspace all-targets/all-features clippy,
workspace test, deps, no-game-ids, manifests, token freshness, raw-colors and
cargo-deny gates. **994 passed, 0 failed, 21 ignored**, 72 Rust summaries
(43 non-empty). Ignored/empty targets prove no behavior; this is unchanged
runtime regression evidence, not session integration. Cargo-deny advisories,
bans, licenses and sources passed with non-failing duplicate-version warnings.
An initial aggregate attempt failed `doc_markdown` for two unquoted technical
words in the changed server comments; those comments were corrected and the
full aggregate rerun passed.

Final-content structural validation passed: 15 files (12 Markdown, 3
comment-only Rust), 73 local links/anchors, 21 external references and 14 unique
S01–S14 scenarios. Pinned PR A links were checked against its exact-head
checkout; Rust executable content matches baseline. `git diff --check`,
repository skill drift validation, its 32 tests, and six AI-doc-checker tests
passed; annotation checker found zero annotated items (syntax check only).
Publication's exact remote commit/tree and CI are recorded in the PR description.

The session HTTP/WS/CSRF/secure-store/DB/browser lifecycle acceptance harness,
real account UI/IME/password-manager/AT evidence and runtime phase exits are
**NOT_IMPLEMENTED/NOT_RUN**. Local feature-matrix results, remote CI and any
publication-specific receipts must be reported separately at their actual SHA.

## Relationship to specification A and remaining work

PR #62 is independently reviewable documentation. Its historical unresolved
cookie/localStorage bullet is superseded **only on that policy point** by
ADR-0031; its remaining registration/profile/social blockers still apply.
Use these pinned cross-links until A merges (do not duplicate its documents):

- [Shared account/social contract](https://github.com/loveoverflowcom/tabula/blob/8e62f1c7be54d429a5362495388d53ac2e67f9e9/docs/ui/screens/accounts-social.md): apply ADR-0031 to session/transport/cleanup
- [A01/A05/A13/A15 and other acceptance oracles](https://github.com/loveoverflowcom/tabula/blob/8e62f1c7be54d429a5362495388d53ac2e67f9e9/docs/ui/screens/accounts-social-verification.md): S01–S14 refine these session-related cases; retain every other case
- [B auth/self-profile backlog](https://github.com/loveoverflowcom/tabula/blob/8e62f1c7be54d429a5362495388d53ac2e67f9e9/docs/work-plan/backlog/issue-54-auth-profile.md) and [C friends/presence backlog](https://github.com/loveoverflowcom/tabula/blob/8e62f1c7be54d429a5362495388d53ac2e67f9e9/docs/work-plan/backlog/issue-54-friends-presence.md): policy choice is resolved; implementation/phase/API evidence is not

The [session prerequisite backlog](../../work-plan/backlog/issue-54-session-contract.md)
states the next bounded work and gates. Do not close #54 or mark B/C ready from
a documentation review, the existing aggregate test count or PR #62's CI.
