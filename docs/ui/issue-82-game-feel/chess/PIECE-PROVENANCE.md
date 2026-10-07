# Tabula Carved Staunton · Piece provenance

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/issue-82-game-feel/chess).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


This is an original, code-native SVG set designed specifically for the Tabula chess design study on 2026-10-04. All silhouettes and detailing were hand-authored as SVG path and circle geometry for this task. There are no stock icons, Unicode chess symbols, emoji, font glyphs, copied SVGs, external imagery or external dependencies.

## Design

- Six conventional Staunton subjects: king, queen, bishop, knight, rook and pawn
- Two matching palettes: ivory #F7F1E4 with dark #273638 contour; midnight ink #243E42 with warm #C0A671 rim
- A shared carved foot, consistent 2.8-unit outline and restrained brass collar treatment
- Transparent 96 × 96 viewBox; intended board rendering at 36–64 CSS pixels
- All pieces face front except the left-facing horse-profile knight
- White and black variants share precisely the same geometry

## Files

- wK.svg, wQ.svg, wB.svg, wN.svg, wR.svg, wP.svg and corresponding b-prefixed black variants
- pieces.svg: optional reusable SVG symbols, referenced by #wK, #bK, etc.
- manifest.json: asset names, palettes and metadata
- piece-sheet.html: self-contained visual review sheet at large and 36 px scale on all three board states
- piece-sheet.png: static review sheet rendered from the actual SVG files
- build_piece_set.py: editable geometry and deterministic generation source
- render_sheet.py: reproducible static review sheet using Inkscape and Pillow

## Use

These assets are supplied for the requested design handoff. No production repository or prototype was changed by this asset task. All SVGs use standard paths and circles and are safe to embed as image sources. The SVG sprite is optional; the standalone files are the canonical assets.
