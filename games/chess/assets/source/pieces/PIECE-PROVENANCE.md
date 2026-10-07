# Chessnut piece source and rights

Copyright 2015 Alexis Luengas. Apache License 2.0.

The twelve SVGs are byte-identical to
[`lichess-org/lila@5820854ca42e082891d6a47ceb3b57d33ebb7fe1`,
`public/piece/chessnut`](https://github.com/lichess-org/lila/tree/5820854ca42e082891d6a47ceb3b57d33ebb7fe1/public/piece/chessnut).
Source byte counts, SHA-256 and upstream Git blob SHA-1 identities are recorded
in the adjacent `manifest.json` and checked before every atlas generation.

Original rights documents:
[COPYRIGHT.txt](https://github.com/LexLuengas/chessnut-pieces/blob/2b8eaf14a31edad7e9deb53b1473e1d4857868a9/COPYRIGHT.txt),
[LICENSE.txt](https://github.com/LexLuengas/chessnut-pieces/blob/2b8eaf14a31edad7e9deb53b1473e1d4857868a9/LICENSE.txt).
They are retained locally as COPYRIGHT.txt and LICENSE-Apache-2.0.txt.
The original repository has no NOTICE at that pin. NOTICE.txt is the attribution
notice added by the approved Tabula design package, retained unchanged.
Lichess's pinned
[COPYING.md](https://github.com/lichess-org/lila/blob/5820854ca42e082891d6a47ceb3b57d33ebb7fe1/COPYING.md)
explicitly identifies Chessnut, its author and Apache 2.0 as a third-party
exception to the application license; a copy is retained as LILA-COPYING.md.

## Tabula exports and distribution

The SVG source geometry, black/white fills, contours, internal detail and
800×800 viewBox are unchanged. The bounded PNG atlases uniformly rasterize the
whole source viewBox into 64×64 and 128×128 regions, surrounded by transparent
gutters. No gradients, decorative brass rims, theme recolors or selection/check
states are baked into these pieces. Attribution, license, notice and this
provenance document are hashed non-image files in the same versioned pack as
the atlas. Distributions must retain them, including when bundling native art.

These assets replace the old original Tabula Carved Staunton sources only on
this follow-on branch; earlier source and the independent issue-85 draft remain
in their own revisions. The existing Tabula editorial cover is independent and
unchanged. No Chess.com artwork, branding, Lichess logo or other piece set is
included. This asset license does not change the surrounding software license.
