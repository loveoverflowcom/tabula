# 07 — Werewolf local gameplay

Issue [#84](https://github.com/loveoverflowcom/tabula/issues/84), following the
game-feel review in #82. The [shared gameplay contract](gameplay.md) and
[foundation](foundation.md) apply. Rust presentation commands and Macroquad own
gameplay; HTML/SVG design samples are references, not runtime screenshots.

## Gate and source authority

[ADR-0035](../../adr/0035-werewolf-local-simulator.md) authorizes the complete
ClassicV1 pure referee and opt-in isolated-seat local simulator. The
[rules](../../../games/werewolf/src/rules/mod.rs),
[projection](../../../games/werewolf/src/rules/projection.rs),
[presenter](../../../games/werewolf/src/presentation/mod.rs), and separate
[host](../../../apps/game-client/src/bin/werewolf.rs) are implemented. The operator
explicitly selects one seat or the public outsider view; this is local testing
on one device, not authenticated social play or a secure human hot-seat mode.

Online/social UX, chat transport and enforcement, voice, authenticated seats,
persisted resume, CMP WebView embedding and rollout remain gated. The maintained
[Werewolf decisions](../../games/werewolf.md) own role, phase, vote, lifecycle and
secrecy semantics. This redesign introduces no new role, rule or player authority.

## Village, roster and actions

The public village is the primary surface. Desktop places up to twelve portraits
around the village fire, with a smaller private-card/narrator region on the right
and a separate action dock. Compact portrait uses a four-column, three-row roster
instead of shrinking the desktop ellipse. Its default surface is the table; the
private card opens deliberately in a contained drawer. Low landscape puts the
roster on the left and secondary information/actions on the right. Larger rosters
use pages of twelve while preserving the 44 dp minimum hit target.

At the 390×844 reference size, the twelve-seat roster and primary action must
remain within the first screen. Short portrait/landscape layouts retain the same
actions with secondary content contained separately. Host leave/help controls
occupy their own layout slot; they cannot cover the canvas dock or last roster row.
These are acceptance requirements, not a claim that every browser, text scale or
safe-area configuration has been exercised.

Selecting a portrait changes only local selection. The selected ring includes a
text label; it never auto-submits. The phase-specific dock constructs an explicit
`Intent<Command>` from the projected legal commands. Witch heal/poison selection
is separate and displays only permitted potion inventory. Public ballots and
revealed deaths come from the projection, not local animation. Simulator seat,
phase-deadline and restart controls live behind one “Tùy chọn” button in the
footer. The effects toggle also lives in that panel; the main screen has no
duplicate simulator shortcut or effects toggle. The compact labeled trigger uses
a slider mark. Its bounded panel groups motion separately from simulator actions;
the enabled effects state is highlighted, and short landscape retains every
control without extending below the viewport.
Advancing the simulator fires the real projected deadline through the authority;
it does not grant a player power to shorten a window.

## Public identity and private knowledge

Public portraits are independent of roles, night choices, selected viewer and
phase. Alive/dead, current-seat and selection markers are separate from the image.
The account/dashboard/game avatar source is currently unavailable: the isolated
profile response contains an account ID, not an approved avatar or display label.
The local simulator therefore uses neutral fallback identities. Design fixture
names/avatars are not real accounts and do not prove profile integration. An
eventual account resolver belongs to the host/resource boundary; it must update
or clear the seat display on occupant changes without putting profile data into
canonical game state or accepting arbitrary image URLs in game rules.

The presenter receives only authorized `View` and `ViewEvent` (I-5/I-6).
Opening a card does not broaden knowledge. Its reveal is bound to seat, role,
phase, round and lifecycle. Conceal/Escape, blur, viewer replacement, permission
change and restart remove private art, text, selection and accessibility labels
immediately, including during a flip. A held activation key cannot reopen a card
after conceal or viewer replacement.

| Viewer / fact | Disclosure contract |
|---|---|
| Living seated player | Own role/resources and authorized reports, public roster/phase/ballots |
| Living wolf | Permitted teammate identities, without teammates' live night choices or submission progress |
| Dead seated player | Permitted full current knowledge, without gameplay commands |
| Outside spectator | Public facts; roles only after permitted death/end disclosure |
| Internal Audit | Never a reachable gameplay-view switch |
| Unauthorized private night event | Omitted entirely; no counter, token, sound or timing cue |

Do not infer a save from no death, publish private ready counts or use role-coded
portrait styling. Accessibility descriptions obey the same deliberate disclosure
guard as canvas output; hidden private text cannot remain in a DOM/native mirror.

## Motion and accessibility

Motion belongs to `GamePresentation::Local` (I-10) and uses semantic profiles:
common-back deal, deliberate own-card reveal, selection lift, public ballot
feedback, day/night scene change and bounded ambient embers. Scene/ballot/death
feedback is driven by permitted projected events. No private cross-seat progress
is inferred. Motion never holds an intent, acknowledgement, phase or deadline;
interruption and skipping preserve the current view. Ambient motion is disabled
in reduced mode or an unfocused window. Timelines are bounded and late sampling
resolves completed motion directly. The synchronous local host has no replay
animation queue; `ViewEvent` carries no general arrival timestamp, so this is
not evidence for stale-event handling by a future network client.

Keyboard focus and portrait activation share the same hit geometry as pointer
input. Focus/selection have rings and labels, and all four semantic themes retain
functional typography and controls. `a11y(View, Local)` describes the public
phase/roster, permitted private region and available actions. Full Board Reader
dispatch, assistive-technology play, physical touch, mobile safe areas and 200%
text scaling require their own exercised evidence.

## Acceptance and evidence

The [#84 verification ledger](../../verification/werewolf-redesign-84/README.md)
records exact final commands, selected tests, retained artifacts and remaining
scope. The earlier [standalone ledger](../../verification/werewolf-standalone/README.md)
is historical evidence for its named source, not pixel acceptance of this change.

Before accepting the redesign, exercise all six roles, public/dead perspectives,
reveal/conceal, pointer/keyboard selection, submit/pass, replace/unvote, dawn/death,
terminal composition and viewer/focus interruption. Inspect real Macroquad pixels
at 1200×880, 1100×850, 390×844, 320×640 and 844×390, DPR1/2; check clip orientation,
Vietnamese baseline/labels and host/dock bounds. Check four themes and reduced
motion separately. Render-list tests and WASM/native compilation establish their
bounded claims, not visual quality, frame pacing, memory or online privacy.
