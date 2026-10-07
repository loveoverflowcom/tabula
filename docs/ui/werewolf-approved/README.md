# Approved Werewolf design inputs

The approved design is a historical review input, not evidence of the Rust runtime
running or of multiplayer acceptance. The original export, preview images and their
hash/Library metadata remain available in the pinned
[design archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/werewolf-approved).
No design bundle or preview pixels remain tracked here.

Runtime assets and editable authoring inputs are preserved under
[`games/werewolf/assets/`](../../../games/werewolf/assets/README.md). Pack 0.2.0 has
eighteen exports at two densities: six role portraits, a common opaque back and
night/dawn scenes. Typography remains live code; the full role atlas is never served.
The game-owned [`budgets.json`](../../../games/werewolf/assets/budgets.json) is the
byte/hash/dimension contract checked by the generator and loading-budget tests.
The original hand-authored
[`design-provenance.json`](../../../games/werewolf/assets/source/design-provenance.json)
retains attribution, artwork-generation declarations and reference conventions.
Both files moved byte-for-byte from this directory; they are reproducibility inputs.

The original scene/atlas files and their transformations are described by
[source provenance](../../../games/werewolf/assets/source/PROVENANCE.md).
The [redesign ledger](../../verification/werewolf-redesign-84/README.md) records
its source/check/runtime scope separately. New raw evidence goes to ignored
`verification/` output or Actions Artifacts, never back into this design archive.
