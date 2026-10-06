# CMP shell foundation — issue #101

The mobile shell adapts [the foundation](foundation.md), [Home/Library Design 01](01-library.md)
and the current web chrome in `apps/web/src/views/parts.rs`. Architecture doc 00, doc 04 §3.3,
ADR-0032 and ADR-0043 govern ownership. This is application chrome and route scaffolding;
subsequent parity changes own discovery and account content. No phase exit is claimed.

Production Android/iOS entrypoints select an empty catalog and unavailable gameplay under
ADR-0043. The WebView hosts and web-bundle pipeline are retired; the native adapter remains
blocked. Desktop tests supply explicit catalog/game doubles to exercise the retained host seam.
Local launch behavior below describes that simulated preview, not a playable native build.

## Ownership and routes

`tokens.toml` remains the authored design authority. Generated `TabulaTokens.kt` supplies colors,
spacing, typography, shapes, state layers and focus treatment. Kotlin owns shell navigation,
copy and presentation; opaque identifiers and localized names are supplied display data.
The shell does not inspect game configuration or reimplement rules (I-9/I-10).

| Destination | Public identity | Available behavior |
|---|---|---|
| Home | `/` | Native-gameplay unavailability and discovery navigation; fixture local entry in preview |
| Games | `/games` | Empty native catalog or fixture entries; explicit full-catalog unavailability |
| Detail | `/games/:id` | Supplied display name and preview setup entry; unknown IDs can return through shell navigation |
| Setup | `/games/:id?setup=1` | Simulated local launch; native gameplay, additional configuration and online services remain unavailable |
| Account | `/account` | Neutral account entry and visible native-account unavailability |
| Game host | `/play/local/` | In-memory preview `GameLaunch` through the retained `GameHost` seam; native adapter unavailable |

These are route identities, not registered universal links or authorization URLs. Unknown
paths and malformed queries cannot launch a runtime. Top-level navigation has a Home root;
nested detail/setup/game destinations preserve the caller. System Back and the visible Back
action pop shell history. In gameplay, Back first reaches `GameBackPort`, preserving the game's
leave confirmation. Exiting releases the game and native voice session through the existing
owners. Navigation cannot carry canonical state, credentials or match grants (I-5/I-10).

Saved state contains bounded public shell paths only. An active local game restores to setup,
requiring a fresh explicit launch. Preferences, capabilities and runtime state are not saved.
No local continuation is promised after process death.

## Adaptive chrome and components

Phone layouts at 320 and 390 dp use a compact brand/context bar, neutral account/avatar entry,
scrollable page and labeled bottom navigation. Wider layouts use a navigation rail. Safe-area
padding belongs to the outer chrome; navigation consumes its own layout slot so content is
not placed underneath it. Large text can increase component heights and scroll content.

The page uses `shellCanvas`; contained rows use `shellPaper`/`shellNote`; the Home hero uses
`shellHero`/`shellOnHero`. Generated primary/on-primary roles own the principal action and
selected navigation. No mobile palette is authored. The canonical T Portal brand and neutral
human avatar preserve the web identity without claiming an authenticated profile.

Reusable components own the page scaffold, top bar, compact navigation/rail, action emphasis,
surface grouping, state panel and progress affordance. Actions have labels and at least the
generated minimum target size; focus and selected state are visible and semantic. Lists,
headings and statuses retain their accessibility meaning. Loading components are available to
future adapters; no artificial loading sequence or speculative server data is introduced.

## Localization and evidence

The shell owns maintained vi/en message keys aligned with the web vocabulary. Locale tags
resolve by language, with English fallback. Game names remain supplied display data. Visible unavailable,
failure, retry and navigation copy uses the same localization owner.

The [issue #101 ledger](../../verification/issue-101-mobile-shell/README.md) records commands,
matrix assertions and captured renders. Desktop Compose screenshots test the shared shell;
the simulated game is a lifecycle double. They do not establish native Android/iOS gameplay,
TalkBack/VoiceOver, device gestures, gameplay latency or store acceptance.
