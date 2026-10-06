# Unfocused wrapped skip-link concealment

Fresh baseline: develop `7716b2ef91c8c2e79e49db6879de19a5742ee76a`, tree
`1f851b5fc3be2916db2408d52554777b5ebdc489`, after PR93.

## Actual finding and cause boundary

The genuine post-PR93 report run
[37444732095](https://github.com/loveoverflowcom/tabula/actions/runs/37444732095)
passed the byte budget, native Enter focus and eight font partitions. Inspecting
the original 320px English/200% Home PNG nevertheless showed the purple skip-link
tail “task” at the top before intentional Tab. Automated geometry alone did not
establish clean pixels. The old receipt did not record the link rectangle/focus;
the fixed negative offset versus wrapped height is a source-supported cause
inference, not a measured old rectangle.

## Bounded fix and regression

The link now starts at top zero and translates above the viewport by its own full
box height plus semantic spacing. `:focus` restores its normal top/transform for
keyboard and programmatic focus. The native `_self` fragment behavior, existing
main tabindex, foreground layer and focus ring are preserved. No logo, palette,
registry, gameplay, budget owner or workflow is changed.

The existing genuine harness records the unique link's whole bounding box and
native Tab/ARIA path before desktop Tab and every Home/Library font capture.
A positive bottom edge fails; removing the link, zero-sizing it, hiding it from
keyboard/accessibility or starting already focused cannot produce a pass. The
existing focused foreground/Enter-main assertions remain unchanged.

Seventeen pure oracle sensitivity tests and Python compilation pass. Local
canonical core passes all ordered gates: 1,260 tests pass, zero fail, and 18
existing documentation examples are ignored. Actual official online Trunk/Sass
output contains the full-height transform/focus reset; CSS SHA-256 is
`9225125367a313a01704720b42df34a05c8fb24484f45c89dc0e2edd8f08e393`.
The WASM remains 739,144 bytes, SHA-256
`3cf77b4726c22c1befaa42f15b1bf5697ce2b7edfb003611bed2a40811cd3462`,
below the unchanged 900,000-byte cap with five resources and zero gameplay
references. Source/oracle review is independently approved. The owner requests
ordinary merge after local checks without checking/waiting GitHub CI; workflows
and protections remain unchanged. Genuine post-fix 320px/200% unfocused/focused/
Enter verification is a separate allowed capture, not a local browser pass.

Local browser access remains denied. No alternate port, shell Chromium, tunnel,
TLS bypass or fabricated screenshot is used. Original images/report history stay
intact; actual pixel quality remains distinct from helper/build/source evidence.
