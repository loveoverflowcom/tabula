# Repair dashboard loading and access

## Outcome
Remove the demonstrated shell-WASM regression and restore native skip-link focus
and readable large-text discovery layout while retaining the original Design 01
composition at normal 320/390px sizes.

## Why
The current-develop report reproduced the unchanged 900,000-byte budget failure,
Enter not focusing main and cramped English 200% text. Smaller renderer dedup was
measured but insufficient; the shell's full-game vtable retained unused canonical
match factories. An additive discovery-only vtable is the minimum measured seam.

## Sequence
1. Reapply the independently reviewed discovery interface to verified remote develop,
   preserving full game APIs and current public-display/avatar changes
2. Use native same-context fragment focus and font-relative narrow layout;
   preserve actual text scaling, authored words and semantic colors
3. Publish a coherent corrective PR promptly; run exact-final portable core,
   feature/WASM, compatibility and emitted-budget checks plus independent review
4. Ordinary authorized merge after local checks; hand off the exact merged revision
   for separately requested genuine screenshots without waiting on unrelated CI

## Scope / dependencies
Web discovery/keyboard/layout and an additive registry facade only. Existing
ErasedGame/server/rules/config/handoff semantics remain the authority. No account
API, network/game repair, theme/branding redesign, native visual acceptance or
workflow/protection bypass.

## Risks / unknowns
Local browser access is denied. Source/build/unit checks cannot establish actual
focus or pixel quality; post-fix Chromium captures are a separate allowed task.
Concurrent develop changes must be preserved and reverified before merge.

## Status
In progress from develop `59ec8c62152d0b2d749f1a6ceba9982b5e8d2ac6`.
