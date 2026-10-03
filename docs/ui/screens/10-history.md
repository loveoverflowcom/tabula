# 10 — Match history

Issue #52; shared [authority, gates and failures](results-replay.md),
[foundation](foundation.md), [results](09-results.md) and
[replay](11-replay.md). Reviewed
`develop @ eb7803b8a304f2be4ae9890ea6484dab02688540` against the pinned
`04-replay/screens/10-history.svg`, its actual desktop PNG and mobile PNG.
The three sample rows are design data, not persisted matches.

## Task, route and data owner

The user finds an authorized recorded match, understands its result and opens
its full result or supported replay. The existing Library finds games; history
finds match records. Resume is a separate live-session action and never
substitutes for Watch or opens a completed record as playable.

Doc 04 §2.1 places history at `/u/:handle` and results/replay at `/matches/:id`.
The artwork's “My games” label does not authorize an extra `/history` route.
An exact history substate/query can be decided by the Phase-5 shell owner
within the profile route. At the review base these routes, authentication,
persisted match/history service and synchronization are unimplemented.
ADR-0028 covers discovery/setup only. Do not build an in-memory fake service,
populate demo rows, or label missing storage as an empty user's history.

The future typed history adapter authorizes the profile/viewer and returns
safe summaries, replay dispositions, paging/sort support and source freshness.
It consumes authoritative stored outcomes and permitted identity, never
canonical state or unrestricted replay headers. An imported file's declared
match ID/roster/result is untrusted and cannot become an online result, rating
or verified history row merely by successful decode.

## Row mapping and controls

| Row/toolbar field | Owner and rule |
|---|---|
| Game name/icon | Registry/module metadata for the record's supported identity; unknown game may show a safe identifier and unavailable reason |
| Participants | Permission-checked display names/pseudonyms and stable seat labels; no account IDs or hidden role/history data |
| Mode | Actual recorded mode; omit unknown mode instead of inferring ranked/local from players |
| Outcome | Validated permitted result kind/reason/rank/score; generic controller never assumes Chess victory/ply semantics |
| Count | Named module counter/notation unit; distinguish accepted transitions, game plies, turns and visible replay positions |
| Date/duration | Recorded timestamp/duration, locale-aware; missing values do not become current time or zero |
| Replay action | Real supported permitted artifact and compatibility/resource status, or readable reason; safe stored result can remain viewable without replay |
| Search/filter/sort | Actual supported fields over the authorized set; stable record identity tie-break for equal dates/order; never count hidden matches |
| Source/freshness | Local file/cache or server source, last refresh if known, stored/volatile and stale/offline distinction |

Search has a persistent label and native text/IME input. Game filter choices
come from authorized supported data; one choice per axis uses labeled select
or connected radio options. Sort labels state the actual order, such as newest
first. Filtering paged server data uses the real query contract; never describe
the current page as the whole history or invent total counts. Preserve query,
filters, scroll anchor and focused row on Back from result/replay; refresh may
reconcile against stable row identity. Unknown sort/filter support has an
explicit reason and cannot simulate a remote result set.

Desktop uses a contained readable list/table with game/participant, mode,
result, count/date and trailing action. Mobile uses task-first contained cards
with a clear ≥44 dp result/replay link. A full-card link is one activation
target and has no nested buttons; if independent trailing actions exist, use
a static row with separate named controls. Incompatible replay stays inline
with safe game/version metadata and an explanation, not a vanished record.

## Loading, empty, import and interruptions

Loading reflects an actual fetch and retains known constraints. Known-shape
skeletons reserve rows; count/percent appears only when measured. Separate
“No stored matches from this source” from “No matches for these filters”. The
former explains storage and offers supported new-game/import; the latter
offers Reset filters. Missing history adapter is Unavailable, not Empty.

A failed refresh retains permitted stale rows with persistent stale/offline
label and real Retry; it never labels them fresh. Initial service failure has
an explicit reason. Unauthorized/expired session clears forbidden rows and
provides useful sign-in/access recovery where real. Match missing, replay
expired and unsupported version have distinct dispositions when policy
allows disclosure. Changing profile/viewer/source retires old requests and
clears now-forbidden data; late results cannot refill the prior user's list.

Import appears only with a real supported parser and adapter. Select file,
validate bounded compressed/decompressed data, checksum, format/game/rules,
kind and viewer policy, then expose sanitized metadata and the actual
disposition. Invalid/corrupt/oversized/canonical or forbidden data fails
closed before board playback; reselect is useful, blind Retry is not.
Unknown/newer format may require an update. No guessed parsing, AI repair,
silent migration or “verified” badge based on mere successful decoding.

Cancel leaves the list/query usable, releases armed controls and discards
late import/fetch completion. Busy controls prevent duplicate imports/writes
while retaining labels and status. A failed save of an imported projected
artifact preserves its separate unsaved state; it does not invent a server
row. Current `.tbr` bounds and original accepted-index discipline are recorded
in [the shared contract](results-replay.md), not bypassed by a screen parser.

## Responsive, focus and accessibility

Verify 320/390 dp one-column cards, 768 dp compact rows when full labels fit,
1440 dp capped content, short landscape and 200% zoom/text reflow. Toolbar
fields wrap/stack and cards grow; no horizontal page scroll or smaller type/
targets. Persistent navigation is future shell context, with safe-area inset.
Use tonal containers and quiet metadata; selected filter and primary import/
new-game actions follow foundation states. Passive result pills are text,
never false buttons or color-only result identity.

DOM uses a labeled search/filter toolbar, semantic list or table with headers,
named links and route heading. Sorting exposes selected direction, filters
expose checked/selected state, and result-count/loading/error changes have
throttled polite announcements. Each row/action name includes sufficient
game, result and date context; decorative icons are not announced. Keyboard
can enter text, clear/reset, choose filters/sort and open each permitted row.
Do not implement arrow-key table focus without a genuine composite role.

Modal import errors retain entry and invoker focus; Escape/Cancel returns
there. Tab/focus order follows source order, ≥3 dp ring remains visible, and
disabled actions keep reasons readable outside faded controls. A row removed
after refresh moves focus to its logical neighbor/list heading. Theme, density
and reduced motion preserve constraints and row identity. Infinite/virtual
lists require an accessible real navigation/load-more strategy before use.

Required cases: unavailable adapter, truly empty source, filtered empty,
loading/partial page, latest equal-date order, refresh failure/offline cache,
unknown game/result/version, denied profile/replay, expired artifact, import
cancel/corrupt/oversize/unsupported/canonical, repeated activation, changed
viewer with late completion, Back restoration, four schemes and 200% reflow.
Real persistence/permissions fixtures establish history; sample rows do not.
