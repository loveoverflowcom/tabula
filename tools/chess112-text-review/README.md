# PR112 corrected text — bounded actual pixel recheck

Build exact clean source `94dc7df5012640e7d6909d1ab5c02eb273d1dc37`,
tree `7a2f8d0ddd9474d038ab0fd59c6de23fa219da13`, then execute its own
compiled Macroquad/WASM through actual local setup controls. Preserve seven
original screenshots: desktop initial, same capture sequence and Fool's mate
result; 390px and 320px initial/after-e2e4 wrapping states. Compare with the
unchanged original c61 PNGs in Actions artifact11468256290, run37589145897.

Keep raw OCR assertions and failures. Capture success is not visual-quality or
rules acceptance. Source-owned pointer geometry is not an independent oracle.
The two earlier OCR failures and corrected preview-reading error are preserved
in the earlier PR comment, not rewritten as successful checks.

The optional graphics probe queries only the already-created WebGL context and
samples at most15 public test pixels selected from an original PNG. It changes
no CSS, UI/game state, context configuration or framebuffer binding. A request-
animation-frame callback reads current context attributes/blend factors and
small RGBA samples. Unsupported or cleared readback is inconclusive; current
blend state does not establish the pipeline used to draw each glyph. The bright-
edge compositing cause remains a candidate until adequate evidence supports it.

This is separate authorized screenshot verification in GitHub Actions, never a
local-browser workaround, merge gate or unrelated CI check. Original generated
PNG/JSON output stays in Actions artifacts; publish only a lightweight scoped
PR comment/link. Native, physical mobile, CMP, online authority and performance
remain NOT_RUN. No production source changes belong to this harness.
