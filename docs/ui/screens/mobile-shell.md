# CMP shell foundation — issue #101

The mobile shell adapts [the foundation](foundation.md), [Home/Library Design 01](01-library.md)
and the current web chrome in `apps/web/src/views/parts.rs`. Architecture doc 00, doc 04 §3.3,
ADR-0032, ADR-0043 and ADR-0045 govern ownership. Issue #102 extends the original
application chrome and route scaffolding with registry-backed discovery. Issue
#103 adds [bounded account task screens](mobile-account.md) under ADR-0046:
typed current state and read-only adapter-supplied identity, with production
native provider/social integration unavailable. No phase exit is claimed.

Production Android/iOS entrypoints use generated public registry discovery data and an
empty packaged runtime inventory with unavailable gameplay under ADR-0043. The WebView
hosts and web-bundle pipeline are retired; the native adapter remains
blocked. Desktop tests supply explicit catalog/game doubles to exercise the retained host seam.
Local launch behavior below describes that simulated preview, not a playable native build.

## Ownership and routes

`tokens.toml` remains the authored design authority. Generated `TabulaTokens.kt` supplies colors,
spacing, typography, shapes, state layers and focus treatment. Kotlin owns shell navigation,
copy and presentation; opaque identifiers and localized names are supplied display data.
The shell displays module-authored configuration descriptors/defaults read-only;
it does not interpret their rule meaning or reimplement rules (I-9/I-10).

| Destination | Public identity | Available behavior |
|---|---|---|
| Home | `/` | Branded discovery hero and real public catalog cards; no fabricated saved-match region |
| Games | `/games` | Localized search and AND-combined category, exact seat-count, maximum estimated duration and complexity filters |
| Detail | `/games/:id` | Registry artwork, description, metadata, mode declarations and rules-resource availability; unknown IDs never select another game |
| Setup | `/games/:id?setup=1` | Read-only module defaults; explicit launch through a supplied packaged host, otherwise disabled native start and explanation |
| Account | `/account` | Typed account state, current read-only adapter identity or visible native-adapter unavailability |
| Account tasks | `/login`, `/register`, `/me`, `/friends` | ADR-0046 task navigation; read-only self Profile; native provider/enrollment/social unavailable |
| Game host | `/play/local/` | In-memory preview `GameLaunch` through the retained `GameHost` seam; native adapter unavailable |

These are route identities, not registered universal links or authorization URLs. Unknown
paths and malformed queries cannot launch a runtime. Top-level navigation has a Home root;
nested detail/setup/game destinations preserve the caller. System Back and the visible Back
action pop shell history. In gameplay, Back first reaches `GameBackPort`, preserving the game's
leave confirmation. Exiting releases the game and native voice session through the existing
owners. Navigation cannot carry canonical state, credentials or match grants (I-5/I-10).

Saved navigation contains bounded public shell paths. Search/filter preferences and
screen scroll/expanded-filter state are public presentation state retained across Back.
An active local game restores to setup, requiring a fresh explicit launch. Launch
preferences, capabilities and runtime state are not saved.
No local continuation is promised after process death.

## Adaptive chrome and components

Phone layouts at 320 and 390 dp use a compact brand/context bar, neutral account/avatar entry,
scrollable page and labeled bottom navigation. Wider layouts use a navigation rail. Safe-area
padding belongs to the outer chrome; navigation consumes its own layout slot so content is
not placed underneath it. Large text can increase component heights and scroll content.
Compact pages and cards use 16 dp insets. Nested shell tasks use a 48 dp Back icon
target with the localized accessible name; the brand and account entry retain their
own space at 200% text. The gameplay toolbar retains its labelled Back action.
Rail width grows with measured localized navigation labels and the current text
scale, bounded to one third of the viewport. Navigation labels remain centered
and wrap between whole words when needed.
Bottom navigation shares width according to the measured longest word in each
label. When the labels cannot jointly fit in `labelMd`, they use the generated
`labelSm` role at the same OS text scale. Both phone and rail labels retain
complete words and the generated minimum target size.

The page uses `shellCanvas`; contained rows use `shellPaper`/`shellNote`; the Home hero uses
`shellHero`/`shellOnHero`. Generated primary/on-primary roles own the principal action and
selected navigation. No mobile palette is authored. The canonical T Portal brand and neutral
human avatar preserve the web identity without claiming an authenticated profile.

Reusable components own the page scaffold, top bar, compact navigation/rail, action emphasis,
surface grouping, state panel and progress affordance. Actions have accessible names and at least the
generated minimum target size; focus and selected state are visible and semantic. Lists,
headings and statuses retain their accessibility meaning. Loading components are available to
future adapters; no artificial loading sequence or speculative server data is introduced.

## Localization and evidence

The shell owns maintained vi/en message keys aligned with the web vocabulary. Locale tags
resolve by language, with English fallback. Game names remain supplied display data. Visible unavailable,
failure, retry and navigation copy uses the same localization owner.

Loading, empty, unavailable and error/retry are distinct catalog states. A ready catalog
with no matches retains constraints and offers reset; errors retain search/filter input.
Retry exists only with an actual caller-provided recovery. Public catalog entries never
mount a GameHost or preload runtime packs. Setup does not offer editable values until
the native host can validate and honor them.

The [issue #102 ledger](../../verification/issue-102-mobile-discovery/README.md) records
current discovery screenshots, web comparisons and checks. The
[issue #101 ledger](../../verification/issue-101-mobile-shell/README.md) records historical commands,
matrix assertions and captured renders. Desktop Compose screenshots test the shared shell;
the simulated game is a lifecycle double. They do not establish native Android/iOS gameplay,
TalkBack/VoiceOver, device gestures, gameplay latency or store acceptance.
