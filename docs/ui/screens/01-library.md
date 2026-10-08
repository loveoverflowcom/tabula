# 01 — Home and game library: original Design 01

Shared contracts: [discovery data, routes, and availability](discovery.md),
[foundation components and tokens](foundation.md), and
[game detail](02-game-detail.md). Web owners are `/` and `/games` in doc 04 §2.1;
native shell implementation follows its phase. This specification and the editable reference pack
do not open the Phase-5 gate or establish a working catalog/resume adapter.

## Authoritative visual reference (issue #87)

For web Home/Library, the original 1 October 2026 **Design 01** is the visual oracle.
The later Design 02 compact/sans/short-cover interpretation is superseded for
Home/Library only. [Pinned source and provenance](https://github.com/loveoverflowcom/tabula/tree/fe6a6bac1037ea28355bd1f1192acdca6f2ce123/docs/ui/dashboard-original-design-01)
contains byte-preserved `libraryScreen`, `shell`, `gameArt`, CSS and preview PNGs
from `tabula-design-html-mjs-png.zip`. The reference's hash routes are prototype
navigation, not product deep links. Compare actual app and prototype at the same
CSS viewport, DPR, locale and scheme before assessing coordinates.

Restore the warm paper canvas, shared Tabula T Portal identity, 224px desktop sidebar,
80px context topbar and neutral account area; display-serif page/hero headings,
lavender split discovery hero, quiet white continue region and three-column
landscape-artwork catalog. Body, control labels and metadata remain sans. The
mobile shell has a native modal drawer and fixed bottom navigation with a real
safe-area/content slot. Widths are nominal at default text size; user font scale
must reflow the composition instead of clipping it.

The sample Tutor/AI, profile name/initials, opponent, clock, counters and favorites
are fixtures. Restore their visual hierarchy with supported content, never those
claims. Profile data remains the #84 contract dependency; both shell and future
game consumers can use `crates/tabula-design/assets/avatar-neutral.svg` when no
verified avatar is available. It is a silhouette, not an invented account.

`tokens.toml` owns additive `shell-canvas`, `shell-paper`, `shell-hero`,
`shell-on-hero`, `shell-note` and `shell-cover-*` / `shell-art-*` semantic roles.
All four schemes are authored and generated through existing Rust/CSS/JSON/Kotlin
adapters. Light canvas `#F9F7F4` and paper `#FFFFFF` retain the original quiet
chrome. [Issue #91](https://github.com/loveoverflowcom/tabula/issues/91) supersedes
only the conflicting old logo: [canonical T Portal sources](../../../assets/brand/README.md)
and additive semantic `brand-*` roles own identity across shell/targets.
The shared primary violet is authored in the [token contract](../tokens.md#shared-primary-violet).
Gameplay material palettes and interaction semantics
remain independent. Web typography metrics use root-relative units for font scaling.

The CMP Library's 2026-10-08 continuation follows the owner-supplied
`tabula-cmp-mobile-design` handoff instead of the original compact landscape-card
adaptation. The pack records
[`develop @ 6d31cef51f9f186e6bd43213c6ecb0d0d7a47162`](https://github.com/loveoverflowcom/tabula/tree/6d31cef51f9f186e6bd43213c6ecb0d0d7a47162)
as its metadata/token provenance. `docs/handoff.md`, `cmp-design-spec.mjs` and
the list/grid/filter/detail PNGs supply the Library reference; its Home and
Account tabs are chrome examples. [The mobile shell contract](mobile-shell.md)
owns the adaptation below. Home, Account and the web Design 01 composition
retain their existing owners. Browser QA of the supplied HTML is design
evidence, not a CMP or Android/iOS test result.

## Task and route distinction

The user finds a game they can actually play, checks its capabilities, and opens its detail
before [configuring a match](03-new-match.md). `/` is the resume-first home: show an eligible
existing match first, then a small featured selection and a labeled link to `/games`.
`/games` is the full Library: search, filters, result count, and all audience-visible entries.
Both reuse the same resume and game-summary components and availability vocabulary. A
featured entry remains subject to the same rollout and adapter checks as a catalog entry.

At `/games`, put a relevant resume section before the browse results when the adapter supplies
one. A resume is not a new-game action and does not borrow the detail/setup CTA. In the absence
of a resume adapter or eligible match, omit the action; do not invent an active match, clock,
turn, or history from the design samples. Locally cached context helps recover a session but
cannot authorize a match or replace a fresh projection (I-5, doc 04 §4.5).

Use `/games/:id` with the validated full registry identifier as the card destination. A card
opens detail; it never starts a match, joins a queue, or opens a game canvas. Planned editorial
entries, if supplied separately, are clearly labeled information and have no fabricated
registry identifier or Play action. The source pack's hash navigator is design navigation.

## Layout at each breakpoint

Source order is navigation, task heading, Home discovery hero, continue region, then featured catalog and secondary help. `/games` has a focused heading/search, continue region, labeled filters, result status and full catalog, with no repeated hero. Desktop Library search can sit beside its heading; reading and keyboard order stay coherent. Landscape lightweight artwork above compact metadata is the original desktop card hierarchy. CMP Home retains its compact landscape thumbnail adaptation; the CMP Library uses the separate square-logo composition below. An eligible Resume remains the strongest action in its own region.

| Width | Original Design 01 Home and web Library layout |
|---|---|
| Compact, <600 dp; verify 320/390 | 64 dp toolbar, 18 dp gutters, one artwork-card column; continue text wraps; labeled search and filters stack; fixed bottom navigation is ≥72 dp plus safe-area inset with matching content padding |
| Medium, 600–904 dp; verify 768 | 64 dp toolbar and drawer/bottom navigation, 24 dp gutters; two catalog columns when full text/targets fit, otherwise one; filters wrap above results; resume remains above browsing |
| Expanded, 905–1439 dp | At 992px and above, 224px sidebar and 80px topbar, 40px gutters; three artwork columns when full contents fit, otherwise two; search may sit beside the title; no hidden filter labels |
| Large, ≥1440 dp | 224px sidebar, 80px topbar and centered content capped at 1200px; three catalog columns when contents fit; resume is a contained row and browsing stays compact |

At 200% text/zoom, reduce the number of columns and let metadata and controls grow. The
reference's five entries fitting one desktop viewport is a sample composition, not a clipping
or fixed-height requirement. No horizontal page scrolling or scaled-down targets. Safe-area
padding supplements the gutter. Normal/compact density changes spacing, preserving type size,
labels, focus clearance, and ≥44 × 44 dp targets. Navigation names/links come from supported
shell destinations; the sample “My games” label does not introduce a new route.

The owner-requested CMP mobile density correction uses the additive `shell-display`
role (28sp/36sp, regular serif) for compact page/hero headings. The Home image sits
beside its heading, with body text and the browse action at full width; one complete
catalog card is visible before scrolling at 390×844dp and default text scale.
[The mobile shell specification](mobile-shell.md) records thumbnail geometry and
large-text reflow. This adaptation preserves body/control sizes and minimum targets.

The current CMP Library puts its localized title beside the canonical T Portal
mark in the compact top bar and begins the page with search; it has no duplicate
heading, hero or banner. List is the default, with an accessible list/grid
selection. Both layouts use a 72 dp square game logo, names/taglines and readable
seat-count, duration-estimate and complexity metadata. Grid uses two columns at
390 dp when contents fit, and one at 320 dp or 200% text. Heights and metadata
wrap with content; the existing 16 dp page insets and minimum touch/focus targets
remain owned by the shell foundation.

Library filters open a modal bottom sheet with labeled category, exact player
count, maximum estimated duration and complexity dropdowns. Each axis uses
generated metadata and all axes combine with AND. Opening copies the applied
values into a draft; Reset changes the draft, Apply commits all axes together,
and dismiss/Back drops unapplied edits. Search keeps the existing accent-insensitive
matching. Applied constraints and the displayed result count stay visible on
Library. Detail is another scrollable modal sheet with an 88 dp square logo and
source-backed facts. Close/Back restores the invoking Library's search, applied
filters, display choice and scroll; card selection never starts gameplay.
Registered mode declarations remain available through the Ways to play
disclosure toggle; the initial detail sheet keeps its information compact.

The generated mobile Library also receives a separate metadata-only planned
Werewolf descriptor, explicitly labeled Coming soon/Sắp có. It uses the manifest's
opaque identifier and facts (6–20 seats, 15–45 minutes, medium complexity),
without changing the disabled/staff rollout or registering a playable module.
Planned detail has no setup or launch action, modes or configuration; a restored
planned setup route shows information-only unavailability. Home
excludes planned entries. Production native start remains unavailable for all
entries under ADR-0043; the informational card adds no runtime inventory.

## Components and content

| Component | Content and behavior |
|---|---|
| Resume row | Module name, supported session description, and one labeled Resume action from eligible adapter data; show checking/busy/error beside the action; no speculative turn/clock |
| Search field | Persistent “Search games” label, search input with native text/IME support, and a labeled Clear action when populated; search localized module name/tagline and supported metadata as defined in the shared contract |
| Filters | Labeled category, allowed player count, estimated duration, complexity, and supported mode/capability choices; use native selects or connected radio options for one choice per axis; independent axes combine, with a visible Reset filters action |
| Result status | Current result count and active constraints; distinguish loading/checking, no matches, unavailable data, and stale results in words; count only displayed results, not hidden rollout entries |
| Game summary | Short module art/icon, localized name/tagline, allowed seats, duration estimate, complexity, and concise availability/capability labels from the shared data contract |
| Planned information | Explicit Planned/Unavailable label and source-backed reason; no success-colored Play claim, enabled setup action, fake supported modes, or presumed release date |
| Empty/error region | Plain task explanation with the one useful next action: clear/reset filters, or retry a failed real fetch; preserve search/filter context |

Use the authored Design 01 shell surfaces, display-serif task/hero headings, landscape covers and compact sans card metadata. Keep controls/tokenized states consistent with the foundation. Selected connected options use `primary`/`on-primary`; state layers, fixed
hit bounds, the exterior focus ring, and disabled reasons follow the foundation. Passive
badges are not buttons and must not look like actionable chips. Separate the card's detail
link from any trailing action; never nest a button inside a full-card link. Favorites appear
only with a defined preference adapter and persistence behavior, not because the sample SVG
contains a heart. Artwork load failure retains the name and usable text/detail link.

Every capability claim has a named metadata/capability or adapter source. “Has bots”, “ranked
rules”, “voice recommended”, and “spectators supported” do not by themselves mean that the
corresponding shell operation works. Do not mark a game accessible from a static drawing or
the existence of `describe()` alone (doc 04 §10.4).

## Loading, empty, error, and resume states

| State | Visible result and action policy |
|---|---|
| Initial catalog loading | Named loading status and skeletons for known card geometry; no empty-results claim or fabricated count; search input is preserved |
| Ready with results | Readable result count, source-backed summaries, detail links, and active filter selections |
| No filter matches | “No games match these filters” plus Clear search/Reset filters; keep the current search and constraints until changed |
| No audience-visible entries | Explain that no games are available for this audience; do not disclose hidden entries or substitute planned cards for playable results |
| Catalog fetch failure | Persistent error and real Retry; retain constraints and any previous data with a Stale label; no automatic reset or false success |
| Offline/stale | Keep readable cached summaries where policy permits, show offline/stale status, and recompute operation availability; cached metadata never authorizes online startup |
| Checking resume | Keep the supplied resume context visible with “Checking availability”; action is busy/disabled with a full-contrast explanation |
| Resuming | Disable duplicate Resume; identify the active request; enter gameplay only after the owning adapter acknowledges the same eligible match/context |
| Resume expired, forbidden, ended, or rejected | Keep a readable explanation; remove the unusable Resume action, offer detail/browse or the supported result destination; retain the browse/search context |
| Disabled module | Summary/detail remains only if audience policy allows; disable new startup. Resume independently checks eligibility for the recorded match; disabling new creation does not end an existing match. |
| Incompatible version or unavailable critical pack | Label the specific obstacle; new startup and continuation require their own supported version/resources. Resume must not silently switch to the latest version. |

Async completions update only the current query/context; an old request cannot replace newer
results or produce a success message for a different resume. Do not hide a persistent failure
in a disappearing toast. A module artwork failure is separate from catalog/adapter failure.

## Keyboard, announcements, and navigation

Provide a skip link to the main task. Tab follows visible source order: supported navigation,
Resume when present, search/Clear, filters/Reset, then result detail links and other actual
actions. Static art, metadata, badges, and headings are not Tab stops. Enter follows a detail
link; buttons use Enter/Space once per activation. Native select/radio semantics apply: a
connected radio group has one Tab entry, arrows traverse choices, and Space selects. Filter
changes preserve focus and do not move it to the first result.

Use a heading for the result region, a list for summaries, and descriptive link names such as
“View details for {game}”. Decorative art has empty alternative text; meaningful art gets
appropriate text without repeating the full card. Announce the settled result count and
loading/resume status politely once; throttle typing-driven announcements. Announce a blocking
resume/fetch failure once with its context and recovery, without stealing focus. Localize
counts, labels, reasons, and status with complete en/vi message keys at runtime.

Opening detail records the Library query, scroll position, and invoking card identity so Back
restores them. If that card is no longer visible, restore focus to the results heading or
nearest surviving link and explain changed availability. The compact drawer stores/restores
its invoker, traps focus/pointer input while open, and closes with Escape or a labeled Close
action. The current destination has `aria-current="page"`; no hash URL becomes a product route.

## Acceptance and evidence boundary

Runtime acceptance covers ready/loading/empty/error/offline/stale states; fresh and expired
resume; changed rollout/version/pack; query/result races; detail and Back/deep-link recovery;
320/390/768/1440 dp and 200% text/zoom; normal/compact density; four schemes and reduced motion;
keyboard-only, touch, screen-reader result announcements, and fixed ≥44 dp targets. Source
review and the pinned reference-pack previews are design provenance. Real fetching, resume authority, storage,
shell navigation, and assistive-technology verification remain implementation work at their
opened phases under the [shared discovery ledger](discovery.md).

## Current implementation evidence

The catalog is synchronous linked registry data: no async loading/fetch error is
manufactured. Search/filter results and unavailable launch modes are source-backed.
There is no saved-match list/resume adapter: the shared continue strip explicitly
labels unavailable, not empty, and has no bogus Resume button. Existing account
validation and opt-in online detail/setup controls keep their original owners.
Game-owned static SVG covers are exposed through the erased discovery accessor;
there is no shell prefetch of game WASM, atlas, model or role resources.

[Restoration verification ledger](../../verification/dashboard-design01/README.md)
separates source/unit/compile evidence, actual Chromium artifacts and remaining
native/CMP/assistive-technology scope. This is evidence for the original web
restoration, separate from the supplied mobile design pack and the current CMP
Library continuation. Screenshots alone do not prove gameplay.
