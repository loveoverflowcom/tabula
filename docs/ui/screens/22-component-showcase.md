# 22 — Foundation component showcase

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/screens); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


This is a design/development surface, not a product route or preference controller.
Use [the foundation contract](foundation.md) for all token and interaction rules, and
[screen 13](13-settings.md) for actual settings ownership.

## Layout and review controls

Open [Historical the editable preview](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/screens/foundation-preview.html#showcase). Review controls select one of
the four generated schemes, normal/compact density, and reduced motion. They affect preview
data only. At desktop widths, use two columns of tonal component groups; on mobile stack them
in source order. At 320 dp every action/label remains visible; connected selectors wrap/stack.
Keep a small toolbar and sans title, with no hero or decorative card outline/shadow.

Include the same content in every scheme so reviewers can compare default, hover, pressed,
disabled-with-reason, focused, busy, error, empty, offline/stale, and success states. Do not
simulate a working account, auth, network, game engine, or settings persistence adapter.

## Component examples and state variants

| Example group | Required variants / semantics | Intended consumer |
|---|---|---|
| Actions | Filled principal action; tonal related action; round/square anatomy; hover, press, focus, disabled reason, stationary busy label | Current gameplay action slice in Stage B; shell action in Phase 5 |
| Connected selection | Labeled mutually exclusive options; checked mark; outer/inner corner hierarchy; 2 dp gap; independent focus | Appearance/contrast in screen 13; future mode consumer supplied by its owning issue |
| Context toolbar | Named related actions; no ambiguity between rotation, undo/redo, deselect, exit; fixed targets | Tiles toolbar when selected by a runtime change |
| Contained list | First/middle/last/single shapes; main/supporting text; trailing ≥44 dp control | Settings rows |
| Field and error | Persistent label, filled tonal field, functional boundary, inline error plus danger stroke | Real Phase-5 form consumer; no canvas IME implementation |
| Status and progress | Named passive badges; real measured fraction distinct from indeterminate busy text; persistent retry/error/empty/offline | Adapter-backed status in later owning change |
| Dialog/sheet | Heading, bounded choices, labeled close, error preserving content, invoker restoration | Selected gameplay confirmation/choice or future settings overlay |

The preview implements static examples and review controls, not every inventory component.
Dialog/sheet keyboard trapping, tabs, canvas Intent dispatch, real async progress, and AT
announcements remain explicit runtime checks. A state painted by CSS is only a design sample.

## Contrast and geometry review

Use existing tonal surfaces and primary/on-primary pairs in every scheme, including HC.
Check a 3 dp focus ring with 2 dp gap, 44 dp minimum hit regions, visible labels, and
full-contrast reasons outside disabled content. Normal/compact alter spacing only. Shape
feedback leaves hit regions fixed, and reduced motion gives immediate feedback. A list's
containment may flatten in HC; headings and actual control boundaries must remain legible.

Review against the independent contrast equation in `tabula-design`, not the mock palette.
The preview imports `apps/web/style/tokens.css`; it must not contain scheme color literals.
Source is editable HTML/CSS alongside these Markdown specifications. Original SVG/PNG
provenance is linked from [the screen index](README.md).
