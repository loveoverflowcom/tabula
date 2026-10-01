# Tabula Design 02 / 06-accounts

A small design-only reference with sample data. No rules, backend, authentication, installation, engine or AI execute.

- index.html, prototype.css, viewer.mjs: static screen navigator
- screens/*.svg: scoped, editable desktop and mobile designs
- previews/*.png: two desktop boards and one purpose-built mobile/state board
- implementation-notes.md: per-screen layout, state, adapter and accessibility contract
- implementation-prompts.md: reviewed small sequential PR prompts
- render.mjs: reproducible SVG-to-PNG renderer using Sharp from the official npm registry

Open index.html in a browser, or open an individual SVG directly. The navigator selects static designs; it does not simulate product actions. Run node render.mjs to regenerate the three PNG previews. Source syntax and visual QA were performed; browser execution and production behavior were not tested.

Shared canonical token snapshots and the component contract are included once in 01-foundation. Product theme source remains the repository-root tokens.toml. Do not copy mock color literals into production theme code, or port gameplay SVGs into DOM.

## Approved Expressive revision

The current files use tonal containment and minimal ornamental borders/shadows. Primary actions are more prominent; routine controls are quieter. All desktop/mobile sources and the offline screen navigator were revised together. Component/state adaptation and official sources are documented in 01-foundation/shared/material-component-map.md.
