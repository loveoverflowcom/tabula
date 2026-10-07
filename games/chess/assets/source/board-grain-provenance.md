# Quiet Chess wood grain

Original code-authored decorative artwork for issue #85, 2026-10-07. The
diagonal 26-unit pattern and restrained dark-grain treatment are adapted from
Tabula's original [design exporter archive](https://github.com/loveoverflowcom/tabula/blob/dac1c2940a7d1fa22f6057074618ab3377d8604f/docs/ui/issue-82-game-feel/chess/export_preview.py),
retained by merged [PR #86](https://github.com/loveoverflowcom/tabula/pull/86).
No stock artwork, font, external SVG dependency or third-party piece source is
used. The original Staunton sources remain unchanged.

`board-grain.svg` contains only transparent decorative grain. It has no board
orientation, coordinates, markers, pieces, clock, state or controls. The
presenter maps it into each authoritative square beneath semantic overlays.
The reproducible 128×128 and 256×256 exports are density-selected, hashed and
verified through Chess's managed per-game pack. Runtime uses neither SVG
parsing nor direct file/URL loading.
