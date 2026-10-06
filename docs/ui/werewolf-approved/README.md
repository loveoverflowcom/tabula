# Approved Werewolf design inputs

This directory preserves the source review export v1 and intact original artwork provenance.
The contact sheet and mobile reveal/conceal states were inspected from the approved Library
inputs before implementation. `design-reference-v1.zip` is the source review export, with
editable Vietnamese card markup, original-art atlas and private/game styling. It is a
**design reference**, not evidence of the Rust runtime running or a multiplayer game.

Runtime assets are the independent PNG derivatives at `games/werewolf/assets/`, generated
reproducibly from the original art atlas and #84's supplied village scenes.
Pack 0.2.0 has eighteen exports at two densities and nine logical resources:
six role portraits, a common back, village-night and village-dawn. Typography remains live code; the opaque common
back is role-independent. The full source atlas is never a served runtime texture.
`budgets.json` preserves measured per-file encoded/hash/dimension and estimated RGBA budgets.
`design-provenance.json` preserves the original attribution, commercial-layout references
and original-generated-art declarations; no publisher artwork or logo was imported.

The source review Library identity is `libfile_9dac47be9c388191a6011fcdb121ad47` v1.
The separately inspected six-card PNG export is `libfile_025c58cc8b6881918a6be3a5ac36a4d2`.
Contact/mobile/comparison design pixels are retained as design-*-reference.png with original
Library identities and hashes in design-reference-images.json. They must never be labelled
as Rust runtime screenshots.

The unpublished offline Mac code/evidence commits were not imported. The cloud game-owned
View/presenter/host is recreated from this approved design and current maintained rules.

The #84 scene originals are retained under `games/werewolf/assets/source/` from
the owner's `Downloads/tabula-redesign 3/werewolf/assets/village.png` and
`village-dawn-original.png`. Their exact supplied/retained hashes and export
transformations are recorded in
[source provenance](../../../games/werewolf/assets/source/PROVENANCE.md).
`design-provenance.json` remains the original six-role artwork record; it does
not claim these later scene inputs were part of that initial Library export.
The [redesign ledger](../../verification/werewolf-redesign-84/README.md) records
current check/runtime scope separately from this design archive.
