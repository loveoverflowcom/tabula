# Accounts/social — scope and evidence ledger

**Current status (2026-10-06):** the PR A evidence below is historical.
ADR-0036/0038 delivered durable sessions, HTTP/self-ID and invited Kanidm login.
The owner explicitly requests every original #54 criterion; [ADR-0043](../../adr/0043-isolated-account-registration-social.md)
now authorizes the bounded registration/profile/social contracts. The
[completion ledger](../../verification/issue-54-account-followup/README.md)
separates executed UI-double checks from real provider/PG/browser acceptance.
Broad phase exits and production activation remain gated.

PR A of [issue #54](https://github.com/loveoverflowcom/tabula/issues/54),
originally reviewed against `develop @ 3527b65d6643d805d6c80352d165d97f71417ccc`.
Compatibility refresh merges
`develop @ 115159cd38cba11f7fa94c3c49698b7a45d59e5f` (PR #63) and applies
[ADR-0031](../../adr/0031-browser-native-session-contract.md); original SHAs
and test receipts below remain historical.
See [shared contract](accounts-social.md), [Login](15-login.md),
[Register](16-register.md), [Profile](20-profile.md), [Friends](21-friends.md)
and [en/vi copy](accounts-social-copy.md).

## Delivered boundary

This change documents form/route/state matrices, authority/data owners,
permission/privacy, safe local escape/redirect, foundation mapping, public
errors, native input/IME/password-manager and screen-reader/focus behavior.
It adds no route, credential handling, provider, session persistence, social
mutation, schema, dependency, theme value or production UI preview.

Source inspection confirmed that auth/profile/friends and their APIs are
unimplemented, not merely disabled. Current discovery routes and bounded local
handoff stay unchanged. ADR-0031 resolves the former cookie/localStorage and
HTTP/WS policy conflict only. Session enforcement/real target evidence and
missing register/profile/social contracts remain explicit blockers. Synthetic
local roster IDs, sample statistics/presence, architecture prose and successful
existing builds cannot establish account/session authority.

## Session-policy compatibility refresh

The accepted policy is documented; account/session runtime remains
**NOT_IMPLEMENTED**. Login/shared specification, the screen index and B backlog
now defer to ADR-0031 instead of treating its policy choice as unresolved.
The work-plan index retains both PR A/B/C and the session prerequisite links.
No executable behavior, credential storage, auth route or phase gate changes.

[S01–S14](../../verification/session-contract/README.md#required-acceptance-scenarios)
refine A01/A05/A13/A15 without replacing registration, profile, social,
form/layout/IME/password-manager/keyboard/AT requirements. Actual platform,
portfolio, Phase-4 integration/exit and Phase-5 shell evidence remain required;
see the [session backlog](../../work-plan/backlog/issue-54-session-contract.md).
Fresh final-head core/feature checks, structural validation, publication SHA
and exact-head CI receipts are recorded in
[PR #62](https://github.com/loveoverflowcom/tabula/pull/62), separately from
these retained original-review receipts.

## Historical local evidence (original PR A review)

| Claim / invariant | Owner/failure mode and oracle | Check / status / evidence | Residual |
|---|---|---|---|
| Correct base and isolation | Fresh `origin/develop`, source SHA; no implicit PR #61 content | PASS — `git fetch origin develop`, worktree from `origin/develop`; source-read | Historical pre-publication boundary; later publication/check receipts are in PR #62 |
| No phase or authority invention | Original-review AGENTS/doc 00/03/04/05/07, ADR-0028/0030, actual mounted routes and PHASE scaffolds | PASS — documented/source-read, independent source audit | PR B/C need actual API and phase/security evidence |
| Design adaptation grounded | Four desktop SVGs and mobile/state SVG, three actual supplied PNGs, Register/Friends static SVG rasters | PASS — source-read and static image-inspected for task hierarchy/tonal containment | No product pixels, all-width layout, behavior or AT proof |
| Local escape grounded | Current `/games`, registry availability, `RuntimeBinding` and two-human opt-in host | PASS — source-read | No auth route/redirect implementation; actual local hosting remains separately evidenced |
| Links/copy/coverage coherent | Existing repository targets, Markdown anchors, all four screen matrices and bilingual unique keys | PASS — documentation validation: 11 Markdown files, 99 local links/anchors, 68 unique en/vi keys with argument parity, 3 referenced keys, 5 original SVG XML sources; `git diff --check` | Check structure cannot prove security or translation usability |
| Existing portable core gate | `cargo xtask check` from isolated worktree; official Rust 1.96.1, shared safe build cache | PASS — aggregate executed; counts/commands recorded below | Unchanged runtime regression evidence; not auth/social implementation evidence |
| Independent final spec review | Source-backed phase, privacy, state, data-owner, route and copy audit | PASS — no blocking findings; independently checked 99 local links/anchors and 68 copy rows; clarified five-SVG provenance wording | Review is source evidence, not integration or accessibility execution |

Aggregate: `source ../toolchain/env.sh; export CARGO_TARGET_DIR=/workspace/scratch/429f9ca90843/tabula/target; cargo xtask check`
from the isolated worktree. Authoritative sequence is fmt, workspace clippy
all-targets/all-features with warnings denied, workspace tests, dependencies, no-game-ids, manifests,
token freshness, no-raw-colors and cargo deny; retained output ends with
`xtask check: all gates passed`. Summed Rust test reports: **980 passed,
0 failed, 21 ignored** across 70 summaries (41 non-empty). Ignored and empty
targets do not establish exercised behavior. Cargo deny reports advisories,
bans, licenses and sources OK; duplicate-version warning trees are retained
in the output and did not fail the gate.
No changed Rust/wire/runtime behavior requires new executable account tests in
this specification-only slice. Feature/WASM/native/browser/mobile/AT checks for
the unimplemented account flow are **NOT_IMPLEMENTED/NOT_RUN**, not PASS.

Auxiliary repository guidance validation passed:
`python3 .agents/skills/tabula-engineering/scripts/check_skills.py`,
`python3 .agents/skills/tabula-engineering/scripts/test_check_skills.py`
(32 tests), and `python3 .agents/skills/tabula-engineering/scripts/test_ai_doc_contracts.py`
(6 tests). An initial `cargo xtask check-skills` probe exited 2 with
`unknown command`; that target is **NOT_IMPLEMENTED**, not a passing gate.
The actual documented Python commands above were then run successfully.

## Required PR B/C acceptance oracles (not existing test targets)

The later [isolated PR3 ledger](../../verification/issue-54-account-state-ui/README.md)
refines A01/A05/A06/A07/A13/A15 only within ADR-0036's bounded session/immutable
self-ID consumer. The cases below remain the full production/form/social
requirements; that narrower implementation does not pass A02/A03/A04/A08–A12
or actual A14 platform interaction by inference.

The case IDs below identify future acceptance requirements, not runnable test
names or a claim that a harness exists. Use real typed authority/adapter fixtures,
distinguish doubles from integration, confirm non-empty selections and record
exact command/features/target/result. Never use the implementation's output as
its own expected permission/state oracle.

| Case / invariant | Independent oracle and exercised partition | Gate / required evidence |
|---|---|---|
| A01 session truth | Actual service session status/subject: resolving, signed out, authenticated, expired/revoked, unavailable/offline | B — adapter/security integration; no guessed session or localStorage demo |
| A02 login public failures | Known/unknown identifier, bad credential, disallowed account, rate limit and unavailable service: permitted response/status/timing disclosures | B — server anti-enumeration/security checks; generic UI string alone insufficient |
| A03 registration disposition | Approved syntax/Unicode/length/handle rules, duplicate identity, real/missing/changed terms, accepted with/without session, unknown result | B — real create/idempotency/session and agreement contract; no invented handle or auto-login |
| A04 form semantics | Native label/name/autocomplete, autofill/paste/visibility, composition Enter, first invalid field/error links and no secret live output | B — real password-manager/IME, keyboard and screen-reader interaction; doubles separately labeled |
| A05 once-only/late completion | Double click/Enter/repeat, retry after unknown result, cancel/leave/account change/new draft before response | B/C — adapter operation identity/current generation and real mutation reconciliation; no success after departure |
| A06 safe navigation/local escape | Allowed bounded Library/detail/permitted document; external/protocol-relative/backslash/control/encoded traversal/nested/oversized/auth/admin/play reject cases | B — route parsing tests, Back/Forward/reload/deep link and current registry launch; no automatic mutation/launch |
| A07 self/other privacy | Authority subject/ID and separately allowed DTOs; forged handle/flags, private/missing/forbidden, change viewer during read | B self first; later other — service authorization integration and forbidden-field assertions |
| A08 profile facts | Named permitted metrics/source/as-of; no history vs no provider vs failure/denial; long name/handle and missing avatar | B — provider/response fixture assertions and layout/AT evidence; edit/ratings/history not inferred |
| A09 edit conflict | Approved fields, revision conflict, dirty draft, pending/error/cancel, permission narrowing and server-confirmed save | Separate approved edit slice — real mutation integration; unavailable at current base |
| A10 friend discovery | Authorized list/filter/search, no friend vs no match vs failure/denial, colliding display names, Unicode/IME, pagination/rate limits | C — actual social read/disclosure contract and search race tests; no email enumeration |
| A11 request authority | Every pending/accepted/declined/expired/cancelled/restricted state, wrong actor/recipient/resource, racing accept/decline/cancel, expiry at commit | C — server permissions/idempotency/conflicts; same-resource retry; friendship vs room invite isolated |
| A12 presence truth | Authorized snapshot/delta, duplicate/late/gap/restart, hidden/unknown/offline/stale/fresh, visibility change, missing timestamp | C — real lobby stream/resync and freshness policy tests; client clock/socket cannot manufacture Online |
| A13 private-state isolation | Logout/expiry/revocation/account switch while fetch/subscription/mutation pending; stale/cache responses | B/C — scoped cache and cleanup integration; no previous viewer data or credential persisted in prefs/URLs/logs |
| A14 foundation/UI parity | Four schemes, normal/compact, 320/390/768/1440 dp, short landscape, software keyboard, 200% zoom, long vi text/IDs/errors, reduced motion | B/C — real browser/native target-specific screenshots inspected plus keyboard/AT; ≥44 dp controls and readable functional boundaries |
| A15 core/wire gates | Final source, I-9/I-13/I-15 and unchanged gameplay isolation | Each implementation — targeted checks, `just check`, relevant feature/WASM builds and golden/version discipline |

Server expiry and permission are rechecked at commit; UI disabled controls alone
are not A07/A11 proof. Successful UI doubles cannot establish server persistence,
public timing/privacy policy, actual stream freshness, browser focus/IME/AT or
the stranger-demo phase exit. Local cancellation cannot prove server rollback.

## Remaining issue acceptance

PR A records all eight issue acceptance areas, but runtime M3 components,
desktop/mobile reflow, server session/permissions/presence, authorized statistics,
keyboard/AT/IME and duplicate/expired mutation behavior remain unimplemented.
Do not check off or close the whole issue from this spec/aggregate result.
Next: implement/evidence ADR-0031 after actual platform/portfolio gates and
resolve registration/public-error/self-profile contracts; prove real Phase-4
identity/session integration/exit and the Phase-5 shell gate, then B
login/register/self read-only.
After actual typed social/lobby contracts, C implements friends/presence/requests.
Profile editing remains a distinct approved slice; see the [work queue](../../work-plan/README.md).
