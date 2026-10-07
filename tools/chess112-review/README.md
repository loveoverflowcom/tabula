# Bounded PR112 actual runtime screenshots

This separate authorized screenshot workflow builds exact clean PR112 source
`c61dbc62271cd4a5554dd62bc139a06bab58fb44` and runs its own standalone setup and
real compiled Macroquad/WASM in official Chromium inside GitHub Actions. It does
not execute locally around the denied browser capability, gate a merge, inspect
unrelated CI, change authentication or exercise real accounts.

The driver uses current source-owned `BoardLayout` only to map public pointer
coordinates. Actual canvas bounds already exclude the runtime footer. The older
online-match pointer helper is not applicable to this revised layout. No FEN,
canonical state, CSS, response or UI state is injected. Opening moves are ordinary
legal public local inputs. Public original pixels and OCR are observation, not
an independent rules or performance oracle. Failed/blocked checks are retained.

Cases cover source-owned setup, four actual themes, selection and display flip,
compact Actions padding/dismissal, narrow viewports, legal capture/en-passant/
castling/promotion/cancel/checkmate sequences and full/reduced-motion observations.
Original PNGs remain unchanged; source/tree/build/browser/viewport/theme/locale/
time/action/hash receipts accompany them. Inspect pixels before claiming visible
quality. Native OS, physical mobile, CMP, online authority and frame-pacing
acceptance remain NOT_RUN. No game source or production behavior changes.
