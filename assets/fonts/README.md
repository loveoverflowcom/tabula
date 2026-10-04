# Built-in standalone typefaces

Copied byte-identically from the approved standalone design's bundled fonts.
The native/WASM host loads them once before the first renderer frame. These
are the tiny shared built-in families allowed by architecture doc 04 §12.2,
not a general font-delivery adapter or game-asset cache.

- Open Sans Regular / Semibold: Apache-2.0. `OFL-OpenSans.txt` retains the input
  package's copyright/provenance filename; its actual license is Apache-2.0,
  and the complete license is included separately.
- Noto Serif Bold: SIL Open Font License 1.1, retained in `OFL-Noto.txt`.

Each encoded font is bounded to 256 KiB before decoding. Renderer's text
measure, wrap, prepare and draw paths use the same selected font and raster
size; clock digits retain equal advance. All accepted frame glyphs warm before
any drawing so growing Macroquad atlases cannot invalidate earlier text draws.
Font replacement after the first frame begins is rejected. Font handles never
cross RenderList or enter rules.

Complex shaping and actual target pixel/accessibility verification remain
separate evidence; loading these fonts does not claim those checks passed.
