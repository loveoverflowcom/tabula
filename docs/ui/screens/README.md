# Shared screen specifications

These specifications are the shared DOM/canvas contract from doc 04 §3.3.
They describe intended behavior; implementation and platform evidence are recorded separately.

| Screen / contract | Specification | Runtime owner / gate |
|---|---|---|
| Foundation used by issues #49–#55 | [Foundation](foundation.md) | `tokens.toml`; small current gameplay widgets in `tabula-presentation`; DOM components in Phase 5 |
| 13 — user settings | [Settings](13-settings.md) | Leptos `/settings` after Phase 4 exit; native shell at its phase |
| 22 — component showcase | [Showcase](22-component-showcase.md) | Design/development documentation; no product route |

Open [the editable foundation preview](foundation-preview.html) from a repository-root
HTTP server to review desktop/mobile, four schemes, density, and component states. It imports
the generated CSS adapter directly. Its controls change preview data only; it neither persists
preferences nor implements the Phase-5 application shell.

```sh
python3 -m http.server 8000 --directory .
# http://localhost:8000/docs/ui/screens/foundation-preview.html
```

The issue's original [editable SVGs and PNG references](https://github.com/loveoverflowcom/tabula/tree/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/01-foundation)
remain pinned design provenance. Their palette extensions and token snapshots are reference
material; [the repository token source](../../../tokens.toml) governs this preview and runtime.
