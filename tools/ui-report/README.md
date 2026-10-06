# Current dashboard / Werewolf / Chess UI snapshot

This evidence-only branch builds the exact checked-out current application and
captures real Leptos and Macroquad/WASM pixels in authorized GitHub-hosted
Chromium. It makes no production or game behavior changes. Public shell APIs
remain absent; local game personas are anonymous disposable fixtures.

The strict existing shell raw-WASM budget runs after screenshot preservation,
without overriding its limit or converting a failure to success. The new issue
must report actual remaining problems and distinguish capture from UI acceptance.
Physical devices, CMP/native hosting, real accounts, Werewolf online/voice and a
new online Chess fault campaign are not covered by this screenshot-only task.

The Werewolf driver is copied from the current maintained
`games/werewolf/tests/verify-redesign.mjs` into temporary space solely to use the
official CI Chromium with software WebGL flags. Its original and executed-copy
hashes and resulting original PNG hashes are recorded. It still uses ordinary
public controls and actual input, never a canonical state export or fabricated
RenderList. Public dashboard measurements reuse the maintained independent
`tools/dashboard-acceptance/run.py` helper. No fake screenshots, mock APIs,
injected layout CSS, ignored TLS warning, tunnel or local-browser denial bypass.

Original capture manifests and PNGs are retained unchanged. A separate pixel
inspection receipt is added before publishing one replacement issue and only
then closing superseded screenshot reports with a direct replacement link.
