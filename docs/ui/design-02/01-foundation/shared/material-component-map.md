# Material 3 Expressive → Tabula design adapters

Sources inspected 01 October 2026. These are component references, not an installed native UI kit. No new dependency or production implementation is delivered.

- Buttons: round/square, selected and pressed shape feedback, five recommended size classes. Leptos native button semantics and RenderList hit/focus/Intent activation must remain aligned. Preview uses 48–64 dp actions and canonical 44 dp minimum.
- Connected button group: 2 dp gaps, 8 dp inner corners, round outer corners. Map to native radio/toggle semantics in DOM and typed focus nodes / four-radius Corners on canvas. No third-party React kit migration.
- Floating toolbar: context-related controls grouped in a standard or vibrant tonal surface. Use labels or explicit legend/tooltips; icon-only actions have accessible names. Rotate-left/right are distinct from undo/redo; deselect distinct from exit.
- Contained lists: positional first/middle/last/single corners, short aligned rows with supporting text and trailing controls. Keep actual off-control affordances and 44 dp hit wrappers.
- Loading indicator: only short indeterminate work; use real progress indicator for measurable loading and cancellable long operations. Reduced motion retains informative status.
- Split button: only one main action with a related menu. Do not use it for unrelated commands or hide mutually exclusive modes.

## Official references

- [Google research](https://design.google/library/expressive-material-design-google-research)
- [Buttons](https://github.com/material-components/material-components-android/blob/master/docs/components/CommonButton.md)
- [Button groups](https://github.com/material-components/material-components-android/blob/master/docs/components/ButtonGroup.md)
- [Floating toolbar](https://github.com/material-components/material-components-android/blob/master/docs/components/FloatingToolbar.md)
- [Lists](https://github.com/material-components/material-components-android/blob/master/docs/components/List.md)
- [Loading indicator](https://github.com/material-components/material-components-android/blob/master/docs/components/LoadingIndicator.md)
- [Split button](https://github.com/material-components/material-components-android/blob/master/docs/components/SplitButton.md)

Repository token source remains tokens.toml. Any missing role or component metric needs a written consumer justification and generator/schema parity. Preserve all four schemes, focus 3 dp and minimum 44 dp. Mock tones/art do not silently become product semantics.
