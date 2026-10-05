# Shared screen specifications

These specifications are the shared DOM/canvas contract from doc 04 §3.3.
They describe intended behavior; implementation and platform evidence are recorded separately.

| Screen / contract | Specification | Runtime owner / gate |
|---|---|---|
| Foundation used by issues #49–#55 | [Foundation](foundation.md) | `tokens.toml`; small current gameplay widgets in `tabula-presentation`; DOM components in Phase 5 |
| Discovery/setup contract for issue #50 | [Routes, typed data and states](discovery.md); [availability](discovery-availability.md); [verification](discovery-verification.md) | Module configuration and registry adapters; runtime awaits Phase 4/5 gates |
| 01 — home and Library | [Library](01-library.md) | Leptos `/` and `/games` in Phase 5; native shell in Phase 6 |
| 02 — game detail | [Detail](02-game-detail.md) | Leptos `/games/:id` through registry interfaces in Phase 5 |
| 03 — new match setup | [Setup](03-new-match.md) | Proposed `?setup=1` detail substate; real driver/authority validates creation |
| Gameplay contract for issue #51 | [Ownership, runtime states and input boundaries](gameplay.md); [verification](gameplay-verification.md) | Existing local presenters; network/recovery Phase 4 and Board Reader status/actions Phase 5 |
| 04 — Chess gameplay | [Chess](04-chess.md) | Current Phase-2 presenter; online adapter stays gated |
| 05 — Caro gameplay | [Caro](05-caro.md) | Future Phase-3 rules/presenter; design placeholder |
| 06 — Tiles gameplay | [Tiles](06-tiles.md) | Current Phase-3 presenter; online/async operations stay gated |
| 07 — Werewolf gameplay | [Werewolf](07-werewolf.md) | Creation-only headless foundation; presentation/social Phase 7, voice Phase 8 |
| Xiangqi contract for issue #53 | [Modes, identity, proposed adapters and platform/resource matrix](xiangqi.md); [verification](xiangqi-verification.md) | Specification slice A; #48 vision/ADR and Xiangqi rules gate precede runtime |
| 08 — Xiangqi Play | [Play](08-xiangqi.md) | Proposed module/presenter; no playable Xiangqi at the pinned base |
| Results/history/replay contract for issue #52 | [Authority, index domains, permissions and failure matrix](results-replay.md); [verification](results-replay-verification.md) | Existing local completion/replay-tooling slice; persistence/protocol Phase 4, document shell Phase 5, projected scrub Phase 9 |
| 09 — match result | [Results](09-results.md) | Local compact handoff; full document `/matches/:id` awaits Phase 4/5 |
| 10 — my matches | [History](10-history.md) | Authorized persisted history at `/u/:handle` awaits Phase 4/5 |
| 11 — generic replay | [Replay](11-replay.md) | Shared projected timeline/controller; scrub/speed Phase 9; Xiangqi extension is separate |
| 11 — Xiangqi analysis extension | [Analyze](11-xiangqi-analysis.md) | Shared replay seam plus a supported branch/host adapter; no fake engine output |
| 12 — Xiangqi Learn | [Learn](12-learn.md) | Current-position evidence and structured fallback; optional LLM/knowledge provider separately gated |
| 13 — user settings | [Settings](13-settings.md) | Leptos `/settings` after Phase 4 exit; native shell at its phase |
| 14 — Xiangqi Resources | [Resources](14-resources.md) | Actual approved artifact rights, integrity, platform/budget/probe and provisioner required |
| Accounts/social contract for issue #54 | [Authority, forms, navigation and privacy](accounts-social.md); [en/vi copy](accounts-social-copy.md); [verification](accounts-social-verification.md) | Specification slice A; identity/session/security contract and Phase 4/5 gates precede B/C |
| Isolated account-state/self-profile slice | [Design delta](account-state-isolated.md); [bounded evidence](../../verification/issue-54-account-state-ui/README.md) | ADR-0036 `/account`/`/me` consumer only; provider/social/other-profile and production gates remain closed |
| 15 — login | [Login](15-login.md) | Proposed `/login`; real identity/session adapter and shell gate required |
| 16 — registration | [Register](16-register.md) | Proposed `/register`; approved form/handle/agreement/disclosure/session contract required |
| 20 — profile | [Profile](20-profile.md) | Proposed `/u/:handle`; B starts with self read-only; other/edit/history remain separately permission-checked and gated |
| 21 — friends | [Friends](21-friends.md) | Proposed `/friends`; C requires real typed social, presence, request and server-permission contracts |
| 22 — component showcase | [Showcase](22-component-showcase.md) | Design/development documentation; no product route |

Open [the editable foundation preview](foundation-preview.html) from a repository-root
HTTP server to review desktop/mobile, four schemes, density, and component states. It imports
the generated CSS adapter directly. Its controls change preview data only; it neither persists
preferences nor implements the Phase-5 application shell.

Issue #50 adds specifications for find → understand capabilities → configure →
start, with source-backed availability and an explicit runtime acceptance ledger.
The [pinned discovery artwork](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/02-discovery)
is reference provenance; this stage adds no discovery preview or executable shell.

Issue #51 adds source-grounded gameplay specifications for screens 04–07 and
the loader/reconnect/error contract. The
[pinned gameplay artwork](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/03-gameplay)
is design provenance; availability and executed checks are recorded separately.

Issue #52 specifies screens 09–11 and separates local completion from fatal
runtime stop, canonical support evidence from authorized projected playback,
and original input indices from accepted-transition/cursor counts. The
[pinned replay artwork](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/04-replay)
is reference provenance. The [verification ledger](results-replay-verification.md)
records the bounded implemented slice and remaining phase/UI gates.

Issue #53 specifies Play/Analyze/Learn/Resources on one proposed Xiangqi rules
authority. The [pinned artwork](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi)
is static reference provenance; sample clocks, arrows and PV are not evidence.
The [shared contract](xiangqi.md) records missing #48 reconciliation, rules,
reconstruction, engine and artifact-rights gates. No Xiangqi game, route,
worker, install lifecycle or AI capability is enabled by these specifications.

Issue #54 specifies compact Login/Register/Profile/Friends documents and their
shared form, route, operation and privacy states. The
[pinned account artwork](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/06-accounts)
is static provenance; credential fields, profile edits, sample statistics,
friends and online badges are not functioning adapters. The
[shared contract](accounts-social.md) applies
[ADR-0031's accepted browser/native session policy](../../adr/0031-browser-native-session-contract.md)
and names its missing enforcement/evidence and identity/profile/social APIs.
This specification does not mount account routes or infer permissions from local presentation.
ADR-0028/0030 preserve only discovery and bounded local play.
The subsequent ADR-0036 isolated slice adds the account-state documents above;
its runtime and target receipts are separate from the historical specification.

```sh
python3 -m http.server 8000 --directory .
# http://localhost:8000/docs/ui/screens/foundation-preview.html
```

The issue's original [editable SVGs and PNG references](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/01-foundation)
remain pinned design provenance. Their palette extensions and token snapshots are reference
material; [the repository token source](../../../tokens.toml) governs this preview and runtime.
