# 09 — Match results

Issue #52; shared [authority, gates and failures](results-replay.md),
[foundation](foundation.md) and [live gameplay](gameplay.md).
Reviewed `develop @ eb7803b8a304f2be4ae9890ea6484dab02688540` against the
pinned `04-replay/screens/09-result.svg`, its actual desktop PNG and mobile
PNG. Artwork is a sample result, not an authority fixture or runtime capture.

## Task, owner and handoff

The user understands the actual terminal outcome and chooses a supported next
action. A live game's compact terminal summary remains with its presenter;
the full web result is the future shell `/matches/:id` document (doc 04
§3.4), reached by real navigation from `/play/:match_id`. Native swaps scene
with validated context. Neither route nor a stored result service exists at
the review base. ADR-0028 does not open this route.

A local result consumer may use an honest volatile terminal receipt from
the existing authority. It must distinguish “This local game ended” from
“Saved to history”. It has no fabricated server match ID, save state, rating,
rematch room or downloadable replay. “New local game” creates fresh state;
it does not resume a fatal session or turn its failure into an aborted match.

At future entry, resolve match identity and viewer permission, validate the
recorded result and roster, then render permitted summary. A stale gameplay
handoff is a hint, not permission or a substitute for the stored authority.
Direct/deep-link entry has the same validation. Browser Back returns to the
invoking history/game context with its filter/cursor/focus restored; it must
not reopen a completed live game as commandable or resend the terminal input.

## Content and visual hierarchy

Source order is navigation, task heading, outcome/reason, game/mode and known
timing, primary next action, permitted participants/standings, then persistence
or replay availability details. The inspected artwork uses a wide tonal result
container and two supporting panels. At compact width the outcome leads,
followed by its reachable action and readable source/storage state. Do not
inflate the result into a marketing hero or shrink a desktop composition.

| Content | Actual source / display rule |
|---|---|
| Result heading | Rules-authorized `MatchOutcome` or projected terminal summary; explain decisive/draw/aborted in words |
| End reason | Public-safe game summary/i18n key: checkmate, resignation, timeout, tie or game-specific ending; never inferred from clock/board |
| Participants / standings | Permitted safe roster labels and roster-validated ranks/scores; support teams, ties, bots and arbitrary seat IDs |
| Duration/date | Recorded metadata with localized formatting; omit unavailable wall-clock date/duration |
| Count | Explicit game notation/counter unit; accepted input count is not automatically move count or ply |
| Mode / source | Real local/online/ranked mode and volatile/stored receipt status; no unsupported capability claim |
| Ratings/accuracy/rewards | Omit until an actual authorized provider exists; never derive them in the screen |
| Replay availability | Compatible permitted artifact/adapter disposition; a captured canonical trace alone does not enable Watch/export |

`Aborted` has no standings, winner or rating claim. Draws and tied placements
retain all permitted tied participants rather than assuming a two-player
winner/loser layout. Missing player artwork retains a labeled text row.
An invalid or conflicting outcome shows a safe authority error, not the first
plausible winner. A local fatal stop has a separate warning/error treatment
and keeps the wording “session stopped”; it does not use victory motion.

## Next actions and persistence

One primary action follows available facts: Watch replay when a real permitted
viewer works; otherwise New supported local game or the available return
action. The reference's Watch button is not permission to enable a missing
adapter. Rematch requires the actual lobby/room flow and authorization;
otherwise use the precise New game label. Back to history/lobby appears only
for supported destinations. Do not nest a result-card link and its buttons.

Save/export is an independent operation. Pending retains the outcome and
shows its real stage; error retains the result and entered destination/options,
with a safe reason and useful Retry. Repeated Enter/Space/pointer activation
cannot produce duplicate writes or new matches. A “Saved” label appears only
after actual durable completion, not file dialog opening or animation end.
Export declares the real format/version and authorized projected viewer;
canonical trace or `.tbr` support tooling is not a user export adapter.

Opening a new-game dialog/sheet stores its invoker and traps focus. Cancel or
Escape returns to the invoker or logical successor and releases held keys/
pointer presses. A result-rendering failure or save failure cannot erase a
previously trusted outcome. If the adapter is unavailable, present its visible
reason with no fake Retry or success. Full shared failure states are in
[the boundary matrix](results-replay.md#compatibility-resources-and-failure-matrix).

## Responsive, keyboard and accessibility

At 320/390 dp use one column, wrapped participant rows and full-width primary
action above safe-area insets. At 768 dp use two supporting panels only when
labels/targets fit. At 1440 dp cap content at 1200 dp beside shell navigation;
no fixed viewport-height assumption. At 200% zoom/text size panels stack and
grow. Routine controls are 48 dp where possible and never below 44 × 44 dp.

Use `headline`/`title` for the task and outcome, `body` for the reason, `mono`
tabular figures for scores/counters, semantic success/warning/danger plus
words/glyphs, and foundation tonal/filled button states. All four schemes and
high contrast retain readable result and operation reasons. Celebration is
skippable and reduced motion keeps the final outcome and actions immediately
available; no animation delays authority or keyboard activation.

DOM uses a real heading, participant list and labeled buttons/links. Announce
the actual outcome once on arrival/transition; persistence has a separate
polite status and failures a persistent accessible reason. Avoid announcing
the result again on each frame, focus change or retry. Initial route focus
lands on the task heading; Tab follows source order. A live in-canvas summary
uses the presenter focus/descriptions; its real Board Reader bridge remains
Phase 5, full regions Phase 9. A string description is not an AT dispatcher.

Required cases: decisive checkmate/resign/timeout, draw/tied standings,
aborted, invalid/conflicting outcome, fatal stop without outcome, known
outcome plus later storage/renderer failure, busy/retry/cancel/new game,
repeated activation, Back/deep-link denial, four schemes, 200% reflow and
no-audio/reduced motion. Test real outcome sources and roster validation;
compile or mock/sample result cards are not authority/rendered proof.
