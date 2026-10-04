# 03 — New match setup

Stage-A screen specification for [issue #50](https://github.com/loveoverflowcom/tabula/issues/50).
Source base and gates: [discovery contract](discovery.md); current game facts:
[availability inventory](discovery-availability.md). This is intended behavior,
not an implemented launch form. It consumes the [foundation](foundation.md) and
the pinned pack's `screens/03-new-match.svg` / `previews/mobile-states.png`.

## Entry, owner, and result

Screen 02's Configure action opens `/games/:id?setup=1`; direct entry resolves
that same module through the registry. A route error, unavailable adapter,
disabled game, or incompatible version resolves to a reason state, not a default
game. Back returns to detail and its invoker; Back to Library retains filters.

The shell owns draft fields and focus. The module-authored form adapter owns
field meaning, parsing, defaults, seat labels, presets and normalized summary.
`GameModule::validate_config` and trusted creation remain authoritative. All
dispatch uses registry interfaces (I-9). Local/bot choices can proceed without
an account when their adapter confirms support; network setup names its actual
next action, such as Create room, rather than pretending an online match began.

## Layout and components

| Logical width | Form and summary |
|---|---|
| Compact <600, check 320/390 | 58 dp toolbar; 16 dp gutter; one column: title/game identity, mode selector, seat rows, game options, validation, normalized summary, primary CTA. Mode/time options wrap or stack with visible labels; summary never hidden in a collapsed disclosure. CTA is full-width in the lower task region after summary. |
| Medium 600–904, check 768 | Navigation rail and 64 dp toolbar; 24 dp gutter; form and summary in one readable column. Use two panes only if both fit without truncating values or targets. |
| Expanded 905–1439 | Form left, summary/action right when content fits; summary/action remain in source order after fields. Sticky summary is optional only when it does not obscure focus/errors. |
| Large ≥1440 | 176 dp rail, content capped at 1200 dp; compact form and tonal summary beside it. No marketing hero or oversized empty field cards. |

At 200% zoom/text scaling, fall back to a single column. Allow vertical scrolling
and growing seat/error/summary rows; never shrink typography or hide overflow to
keep all controls above the fold. Fixed navigation and safe areas must not cover
the final action. A soft keyboard leaves the active field, validation and summary
reachable; avoid a sticky footer that consumes their reading space.

| Foundation component | Consumer / meaningful treatment |
|---|---|
| Connected radio selector | Local humans / Local bots / Network, only confirmed choices; 2 dp gaps, 8 dp inner corners, rounded outer ends. Unavailable choices retain readable reasons; no split-button menu hiding modes. |
| Contained list | Seat/occupant rows with first/middle/last corners; selected seat identity has label/glyph as well as color. No invented team/color picker for games whose adapter supplies none. |
| Filled fields and radio/preset group | Game-provided options, persistent label and units, native number/text/select input on web, explicit off-control affordance. Invalid field uses danger stroke + text. |
| Tonal section / summary | `container` groups form sections; `container-high` contains fields/rows; `shape.card` for summary. No decorative outline or shadow. |
| Filled principal action / tonal Back | One strongest creation action, 56–64 dp high when space permits; secondary actions quieter and labeled. All targets ≥44×44 dp, including small checkbox marks. |
| Persistent banner and progress | Validating, pending, rejection, unsupported version/resource and offline use real result/status data. Fractional progress only when measured; reduced motion retains text. |

Use generated semantic roles and all four schemes from foundation. Selection,
focus and error are distinct. Functional field, focus and selection boundaries
remain visible in high contrast even when tonal surfaces flatten. The SVG's
purple fills and larger card corners do not add token roles or dependencies.

## Form and normalized summary

Retain screen 02's explicitly selected mode if it is still supported. For direct
entry with no prior selection, choose the adapter's explicit supported default;
otherwise require a selection. A revoked selection stays explained or is cleared,
never silently replaced with the default.
A default is never inferred from mock position, a game name, or an estimated
duration. A single legal seat count can be static labeled text rather than an
editable control. For exact count sets show only permitted counts, not every
integer between min and max. Roster labels/order and any permitted seat assignment
come from the adapter; the trusted driver/authority confirms roster ownership.
Room/queue setup shows unassigned vacancies as such: its validated seat plan is
not an occupied `SeatRoster`. The destination validates its request, resolves
participants, and performs complete-roster module validation before match
creation, as described in the shared contract and issue #55.

The mode group distinguishes people sharing one device, a local bot opponent,
and network participants. Bot choice lists only the package's declared policy
levels for a supported setup mode; unsupported levels have no selectable value.
The gameplay host must independently confirm its linked factory and bot runner
before launch. Selecting or normalizing a policy never establishes that runtime.
Network-dependent ranked/async/voice options require actual service support as
well as declarations. No generic rating, engine-install, or engine-ready UI is
introduced. Mic permission is not needed to browse or play an unrelated local
mode; voice operations belong to their later phase.

Parse localized input through the game adapter into checked integer/time units
and enum choices. Never silently clamp an invalid seat/time value to an accepted
one. `estimated_minutes` is informational only. Time controls appear only when
the module supplies config/timer meaning and the selected launch mode schedules
those timers. Until normalization succeeds, the summary says “Not validated”
and describes entered values; Ready replaces those with normalized values.

The summary stays immediately before the CTA in reading order and includes:

- Localized game name, module version, rules version and required resource
  identity/status, with longer technical detail available through disclosure.
- Selected mode, allowed seat count, ordered seat/occupant labels (or explicitly
  unassigned room seats) and confirmed bot levels where relevant.
- Module-formatted time semantics (including increment versus delay or disabled
  timer) and other normalized game options. Never abbreviate differing semantics
  into the same `10 + 5` label.
- Validation status, unavailable reason/recovery, and the exact consequence of
  the action: start this local session or request the named network destination.

Move hints, sound and motion are user/presenter preferences; if the adapter
supports these controls, bind them to [screen 13](13-settings.md)'s preferences,
outside canonical configuration (I-10). “Save replay” is displayed only with an
actual replay adapter and its limits. The sample checked glyph is not evidence
that either feature is implemented.

## Module examples and validation boundaries

These source-derived examples inform a future module-owned form. They are not
shell code branches, a launch capability catalog, or new presets chosen by this
specification. The inventory distinguishes options in rules from the current CLI.

| Module | Config meaning / oracle | Setup consequence |
|---|---|---|
| Chess | [Config and ClockControl](../../../games/chess/src/rules/state.rs): `clock: None` is untimed; timed config has `initial: Millis` and Fischer increment or Bronstein delay. [config_is_valid](../../../games/chess/src/rules/clock.rs) rejects zero initial and overflow of initial + increment/delay. [validate_config](../../../games/chess/src/lib.rs) requires exactly seats 0 and 1. | Adapter may expose Untimed / Fischer / Bronstein when the launch path supports them. Reject parse errors, negative/fractional milliseconds, overflow, duplicate/missing seats, and invalid combinations. The current client uses fixed Fischer 5 min + 2 s; the mock's other presets are unimplemented form choices. |
| Tiles | [Config and turn_deadline](../../../games/tiles/src/rules/state.rs): `turn_deadline_ms = 0` disables; nonzero must be ≥5,000 ms. [validate_config](../../../games/tiles/src/lib.rs) permits 2–5 seats and checks deadline; creation also checks timer scheduling overflow against `ctx.now`. | Adapter summary distinguishes No deadline from a per-turn deadline. Reject 1–4,999 ms and invalid counts; values 0 and 5,000 are boundary examples. Current local CLI uses 0; a timed setup requires a confirmed scheduler. Validation success cannot guarantee creation success. |
| Werewolf / Caro / Xiangqi | [Inventory](discovery-availability.md) establishes incomplete or absent playable paths. | Do not expose a start form from the artwork or incomplete metadata; explain availability and return to Library. |

## State, feedback, and keyboard path

Use the shared [creation state machine and failure matrix](discovery.md).
Editing, validating, ready, pending, rejected and unavailable have separate
visible labels. Start is disabled until the exact draft revision is Ready and
its mode/version/resources remain available; its reason stays readable. For a
room/queue action, Ready means the destination request and seat plan have passed
their checks; only eventual match creation validates the full resolved roster.
A field
edit immediately invalidates Ready. Ignore late validation responses. Pending
locks fields and duplicate submission and shows the submitted summary. Known
rejection preserves config; an unknown transport result reconciles the same
request identity instead of offering an unsafe fresh submission.

On entry announce the page title and focus its heading; normal Tab order is
Back → mode → seats → game options → preferences if supported → summary detail
controls → creation action. Static summary/seat labels are not Tab stops. Radio
groups have one Tab stop and arrow-key selection; disabled choices are skipped
while their reasons remain discoverable in surrounding text. Use native labeled
fields/IME and `aria-describedby` for units/reasons; `aria-invalid` for rejected
fields. Enter submits once only from the appropriate form context, respecting
IME composition and ignoring repeated keydown. No auto-create when changing mode.

Validation status is polite and debounced; do not announce every keystroke.
On submit failure announce the summary once and move focus to the first invalid
field; field errors are associated with their inputs. A service/resource failure
keeps focus at the action and provides a readable persistent banner. If a choice
becomes unavailable, preserve input for review, clear readiness, and reconcile
focus to the remaining choice/reason. Navigation restores the prior detail or
Library focus; any bounded dialog/sheet uses foundation's focus trap and return.

All runtime text uses complete en/vi keys and locale-aware values rather than
concatenated fragments. Contrast, keyboard completion, soft-keyboard reflow and
assistive-technology announcements require actual platform testing at Stage B;
the reference SVG and this specification establish none of those runtime passes.
