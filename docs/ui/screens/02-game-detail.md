# 02 — Game detail

Shared contracts: [discovery data, routes, and availability](discovery.md),
[foundation components and tokens](foundation.md), [Library](01-library.md), and
[new-match setup](03-new-match.md). Product location is `/games/:id` (doc 04 §2.1),
using a validated full registry identifier. Web implementation is Phase 5; native shell
implementation follows its phase. A static design does not establish module availability.

## Task and hierarchy

The user understands the game and the capabilities available on this client before choosing
a supported mode and opening setup. Lead with the localized module name, short tagline,
allowed player counts, estimated duration, complexity, and operation availability. Show a
short module-art region, then a contained mode list, then rules/resources. Keep a single
principal action labeled for its consequence, such as “Set up local match”. It navigates to
the setup substate `/games/:id?setup=1`; it does not immediately create or join a match.

The current selection and its availability reason remain beside the setup action. If there
is no supported mode, show readable information and reasons without an enabled Play/Setup
action. Mode support is the intersection of the module's declarative facts, the selected
client's actual adapter/presenter, current audience/rollout, and version/pack readiness from
the shared discovery contract. A linked module with rules or bots is not proof of a working
launcher. Never hardcode a mode list or branch on a game identifier in shell code (I-9).

## Layout at each breakpoint

| Width | Detail layout |
|---|---|
| Compact, <600 dp; verify 320/390 | 58 dp toolbar and 16 dp gutters; Back, task title/metadata, short art, mode rows and CTA, then rules/resources; unavailable reasons wrap under each row; no fixed-height hero; future bottom nav is ≥56 dp plus safe-area inset |
| Medium, 600–904 dp; verify 768 | 64 dp toolbar and rail, 24 dp gutters; one content column unless art/overview and mode panel both fit comfortably; resources follow in reading order |
| Expanded, 905–1439 dp | Persistent rail; overview/art beside a contained mode/action panel when space permits; versioned rules/resources below; principal action and its explanation stay together |
| Large, ≥1440 dp | 176 dp rail, centered content capped at 1200 dp; compact two-column task layout with a bounded art region; no extra marketing panel or enlarged empty hero |

At 200% text/zoom, fall back to one column and let labels/reasons grow. Keep the same source
order at every width. Art never displaces the task name/capabilities or clips controls.
Normal/compact density uses the foundation's spacing while preserving type sizes and
≥44 × 44 dp targets. Safe-area padding supplements the gutter. A wide CTA is 56–64 dp high
when space permits and grows for a wrapped label. A sticky action, if used by the runtime,
must not cover resources, focus, or the setup summary; ordinary scrolling is acceptable.

## Components and data ownership

| Region | Content and behavior |
|---|---|
| Toolbar / Back | Supported breadcrumb or Back-to-Library link; restore the previous query/scroll/invoking card when available; direct entry has `/games` as a usable fallback |
| Metadata | Localized name, tagline/description, categories, allowed seat counts, duration estimate, complexity, and content rating from registry metadata/capabilities; estimates are not an active clock |
| Module art | Logical icon/hero resource resolved through the pack contract; accessible fallback retains all text and controls; art is not authoritative board state or an interactive game |
| Mode list | Local same-device, bot, and network modes only when sourced by the shared contract; each supplied mode has a label, concise consequence, and availability/reason; group related choices with contained rows |
| Selection and action | Labeled single-selection control for supported modes; selected label/check independent of focus; one principal Setup CTA; unavailable choices visibly explained and non-activating |
| Capability explanation | Source-backed ranked, async turn, hidden-information, spectator, voice, or accessibility information when relevant; labels distinguish rules support from adapter readiness |
| Rules/resources | Versioned rules and other supplied resources; visible module/rules version context; hide absent links or show a clear unavailable explanation; do not link a mismatched version as this module's rules |
| Inline feedback | Loading/checking, artwork issue, mode error, stale/offline, disabled module, incompatible version, or unavailable pack; keep the context and real recovery next to the affected region |

Use `container`/`container-high`, semantic foregrounds, and foundation card/list corners.
The platform palette stays quiet; module artwork may provide identity without redefining
primary, danger, focus, or status roles. No mock purple palette, ornamental outlines/shadows,
or large serif UI hero. `primary`/`on-primary` identify the one main action/selected option;
passive capability badges have textual meaning and are not focus stops. Keep disabled reasons
at full contrast outside faded control content. All mode/field/button states use the existing
foundation state layers and focus geometry; shape changes leave hit bounds fixed.

Rules/resources may be simple heading sections; tabs are used only for genuine peer panels.
When tabs are justified, follow the foundation's tab semantics and keep loading/error status
inside the named panel. Configuration belongs to screen 03, so detail has no second copy of
seat, bot-level, or timer controls. Match-authoritative data comes only from projections;
detail must never read canonical `State` to determine capability or mode availability (I-5).

## Availability and failure states

| State | Presentation and action policy |
|---|---|
| Loading entry/capabilities | Named loading status and stable skeleton geometry; no fabricated metadata or active setup CTA |
| Ready with supported mode | Show sourced facts and the selected mode; enable Setup only while its current availability permits configuring |
| Known entry with no playable mode | Keep public metadata/rules; display each supplied unavailable reason; no fake Play or successful-install claim |
| Unregistered/planned entry | Planned editorial information belongs to its supplied source; it cannot fabricate a registry detail route or executable mode |
| Unknown/invalid identifier, or hidden by audience policy | A generic not-found/unavailable page with Back to Library; no inference that reveals a hidden module or its capabilities |
| Entry/capability fetch failure | Inline persistent error and real Retry; retain prior data only with a Stale label; preserve selection without treating it as current eligibility |
| Offline/stale metadata | Explain freshness and recompute real adapter availability; local setup may remain available only when its required module/pack/config adapter is actually usable |
| Disabled module or revoked audience access | Show the allowed unavailable explanation; disable setup and revalidate prior selection; an old detail page does not grant continuing startup authority |
| Incompatible module/rules version | Name the version obstacle and supported update/reload recovery; no silent version substitution or use of mismatched rules/resources |
| Pack unavailable or failed integrity/load | Distinguish optional artwork fallback from required gameplay pack failure; unavailable critical pack disables startup; real measurable loading uses progress, otherwise named busy text |
| Checking/retry busy | Disable duplicate checks/retry for the affected operation; preserve metadata/selection; no fabricated fraction or completion |
| Capability/mode disappears during reading | Retain the unavailable selection with its readable reason, or clear it; require an explicit supported selection and revalidation before continuing. Do not silently choose another mode. |

An older async response cannot overwrite the newly selected identifier, version, or mode.
Errors persist until addressed or dismissed through a supported policy, not just a toast.
Setup revalidates all required facts: an enabled detail action is navigation eligibility,
not a promise that match creation will succeed. Local/bot/network creation, validating,
pending, rejected config, and launch handoff are owned by [screen 03](03-new-match.md).

## Keyboard, announcements, and navigation

Use a main heading and a sensible section outline. Tab follows Back/supported navigation,
mode selection, Setup, then actual rules/resource links and Retry where shown. A radio mode
group has one Tab entry; arrows traverse supplied selectable choices and Space selects. If
unavailable rows use disabled controls, their text/reason remains discoverable in document
reading order. Static metadata, art, passive badges, and headings are not Tab stops. Enter
follows links; buttons activate once with Enter/Space. There is no nested row-action/control
activation or hover-only capability explanation.

After detail navigation, place focus on the task heading/main region through the shell's
route policy. Changing mode preserves focus and politely announces the selected mode and
its current explanation once. Loading, stale status, and progress announce without repeated
noise; a blocking fetch/configuration obstacle is announced once with recovery. Focus and
selection are independent; reconcile focus if an action vanishes or becomes disabled.
All runtime labels, status, and resource names use complete en/vi message keys.

Setup navigation creates a history entry so Back returns to this detail and its retained
selection. Closing setup returns focus to its invoker or a logical available successor.
Back to Library restores its search/filter/scroll context; a direct detail deep link works
without previous shell state and has a Library fallback. Web gameplay navigation remains
the separate-document handoff in doc 04 §3.4 after successful creation/authorization.

## Acceptance and evidence boundary

Runtime acceptance covers loading/ready/not-found/service-error/offline/stale; absent and
unavailable modes; module/audience revocation, changed capabilities/version/pack, response
races and resource-version mismatch; setup/Back/direct-link behavior; 320/390/768/1440 dp and
200% text/zoom; four schemes, normal/compact density, reduced motion; keyboard-only, touch,
screen-reader status, and fixed ≥44 dp hit targets. Pinned reference-pack previews and source
review are design provenance. Registry fetching, real mode adapters, shell navigation, and platform
accessibility remain deferred implementation checks in the [discovery ledger](discovery.md).
