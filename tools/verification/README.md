# Reusable verification helpers

These source tools were moved out of historical evidence folders. Their raw output
and restored historical inputs belong under ignored `verification/`, never in docs.
The current retained ledgers describe what their original runs established.

- `capture-brand-identity.py --output verification/brand-identity-artifacts --chrome /path/to/chrome`
  exercises the public shell brand matrix against the actual build served on port 8191;
  requires Playwright and Chrome. It writes current-source receipts and screenshots
- `compare-mobile-discovery.py --evidence verification/issue-102-mobile-discovery`
  composes explicit `web/` and `screenshots/` inputs without altering those originals;
  requires Pillow and the captures, which are not bundled in current source
- `summarize-issue59.py --evidence verification/issue-59`
  validates and summarizes captured historical `runs/*.json`; refuses an empty selection.
  Restore inputs from the pinned archive linked by the issue59 ledger or capture the
  exact declared historical workload; do not substitute current receipts silently
- `check-werewolf-typography.py` reads current Rust labels, tokens and bundled fonts;
  requires Pillow. Output is conservative headless font-advance examples, not pixel QA

Renderer-spike summarization remains in `tools/renderer-embedding-spike/summarize.py`;
pass `--evidence` with the complete restored/captured input set. Its browser runners
and the Werewolf verifier default to ignored `verification/` output. The mobile-host
Chrome tool is historical ADR-0033 tooling, not a revived mobile gameplay path.
