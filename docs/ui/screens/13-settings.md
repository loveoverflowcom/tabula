# 13 — User settings

Shared foundation: [layout, tokens, component states](foundation.md).
Product location: `/settings` in the Phase-5 Leptos shell. Native shell implementation follows
its own phase. Screen 22 is [a component showcase](22-component-showcase.md), not this route.

## Task and layout

The user changes appearance, contrast, motion, audio, and density and sees the effective
result immediately. Account, privacy, storage, and sync actions appear only when their real
adapters exist. No placeholder control promises an unavailable backend action.

Use a compact toolbar, a sans task title, and contained settings rows. At compact/medium
widths show the appearance controls, motion/audio/density rows, then the preview in one
column. At expanded/large widths put a smaller preview beside the control column. Normal
and compact spacing follow the foundation; labels wrap and rows grow at 200% text/zoom.
Keep the action/reason beside the row it concerns. No marketing hero or decorative borders.

## Preference model and resolution (future adapter contract)

| Preference | Choices / default | Effective result |
|---|---|---|
| Appearance | System (default), Light, Dark | System follows OS appearance while no explicit override is selected |
| Contrast | System (default), Standard, High | System follows supported OS contrast preference; otherwise Standard |
| Reduce motion | User reduction off by default; OS request always respected | Effective reduction = OS requests reduction OR user requests reduction |
| Audio | Sound on/off; volume only with a working adapter | Client playback policy; never authoritative state or turn behavior |
| Density | Normal (default), Compact | Changes grouping/padding; type scaling and 44 dp targets stay intact |

Appearance and contrast resolve together:

| Effective appearance | Effective contrast | `ThemeKind` / CSS scheme |
|---|---|---|
| Light | Standard | `Light` / `light` |
| Dark | Standard | `Dark` / `dark` |
| Light | High | `HighContrastLight` / `hc-light` |
| Dark | High | `HighContrastDark` / `hc-dark` |

Expose the resolved scheme by name in the preview. Use two labeled single-selection groups
for appearance/contrast rather than contradictory four-scheme chips plus an independent HC
toggle. Screen 22 can select all four schemes directly for review. Changes to OS settings
update only System choices; they preserve user overrides. Effective reduced motion remains
visible/explained when an OS request keeps it enabled. A preview changes no game command.

## Loading, persistence, and failures

1. While the approved `KvStore` adapter loads `prefs.v1` (doc 04 §4.5), show readable loading
   status and the OS-resolved theme. Do not briefly render a hardcoded palette or allow edits
   that a late load can overwrite. Malformed/unsupported preferences fall back to supported
   defaults and report the recovery; never construct an invalid scheme.
2. After loading, edits apply to local presentation immediately and are persisted by the
   adapter. Show saving status only while a real write is pending. Disable duplicate retry,
   not unrelated local controls. A stale completion must not overwrite a newer edit.
3. A failed write keeps the active local choice and shows a persistent, readable
   “Not saved on this device” status with Retry. Report success only after the adapter
   acknowledges the current revision. Leaving the page does not falsely imply persistence.
4. Offline server sync, if later available, is distinct from device persistence. Authentication,
   sync/conflict policy, and storage implementation remain owned by their later phases;
   this spec introduces no new network service or preference wire payload.

All status/action copy becomes en/vi i18n keys when implemented. Prototype English is design
copy. The static preview deliberately shows an adapter-unavailable explanation and makes no
save/sync calls.

## In-match overlay

The canvas overlay exposes only sound, reduced motion, and supported board appearance aids.
It shares the same client preferences and effective policy; there is no second preference
store. Keep the board snapshot/authoritative projection intact behind it. Opening, editing,
or closing the overlay sends no gameplay command and grants no new information (I-5/I-10).
Use a compact sheet on small widths and a bounded dialog on wider layouts. A supported full
settings link uses the shell handoff policy without silently abandoning the match.

Opening stores the invoking focus node and places focus on the overlay heading/first control.
Trap keyboard/pointer input inside the overlay. Escape or the labeled Close action dismisses
it and restores focus; if the invoker is gone, focus the nearest meaningful board/action node.
Adapter failures stay visible without preventing dismissal. Modal focus/pointer behavior is
runtime work, not proven by the static preview.

## Keyboard, AT, and acceptance

Appearance/contrast groups have legends, one checked option, one Tab entry, and arrow/Space
selection. Toggles are labeled native checkboxes/switches with ≥44 dp hit wrappers. Tab order
follows visible controls, then any retry action; static preview content is not focusable.
Announce changed effective scheme and persistence failure once via a polite status; do not
repeat the preview text on every pointer move. Disabled actions have full-contrast reasons.

Runtime acceptance requires all four schemes, normal/compact, OS change and override cases,
effective reduced motion, load/write failure and stale completion, 320/390/600/905/1440 dp,
200% zoom/text, keyboard-only navigation, touch hit regions, deep-link/back behavior, and
overlay focus restoration. Real adapter and platform checks are deferred to Stage C after
the gate; the [editable preview](foundation-preview.html#settings) supplies design variants.
