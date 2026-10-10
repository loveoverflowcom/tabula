# D05 offline presentation tooling

The bounded public source entry is
[`games/age-war/design-tools`](../../../games/age-war/design-tools/README.md).
It contains executable presentation helpers and synthetic tests only. Authored
assets, rig descriptors and detailed design review are delivered separately.

This is neither a playable game nor a production implementation. Timing and
speed affect local presentation only; canonical rules determine combat outcomes.
The original helper package has no Node dependencies. The D05 Rust pilot adds
only a native dev-dependency on the existing catalog; it does not alter the
D01 catalog, gender/ID mapping, quantized math, colliders, rules or registration.

Issue #123 acceptance remains open until its visual, coverage, listening, rights
and owner-review criteria have evidence. Native importers, mobile performance
and projected-event binding are separate gates. A successful codec check is not
proof that those gates passed.

The `runtime/` tools build private per-age runtime packs from the owner's D05
delivery and drive a presentation pilot through the existing verified-asset,
`RenderList` and Macroquad path. Their outputs stay outside Git; see the
[D05 ledger](../../verification/age-war-d05/README.md) for executed evidence
and the [D06 HUD/UX contract](D06-HUD-UX.md) for how they feed the code gate.
