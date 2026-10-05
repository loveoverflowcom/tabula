# Issue #74 F1 — canonical trusted HTTPS Origin

**Status:** implemented; focused and full verification receipts are recorded in
[the ledger](../verification/issue-74-trusted-origin/README.md).

**Outcome:** invalid or noncanonical trusted Origin configuration fails before
an isolated router exists; canonical HTTPS browser Origins remain usable with
unchanged exact-Origin, CSRF and channel checks.

**Why:** review #74 reproduced accepted invalid ports and raw aliases which a
browser never sends, turning a successful constructor into a 403-only flow.

**Dependencies:** fresh merged `develop @ aee07128d111c0bd5c15eb2ccaacc40c880ba5c3`,
ADR-0031/0034/0036. No dependency on unmerged PR #69/#70/#75. The optional native
URL parser reuses locked url 2.5.8; its locked ICU graph requires Rust 1.88,
recorded only for `isolated`. Default/WASM DTO consumers stay separate and the
existing `postgres` 1.94 requirement/pinned 1.96 verification remain unchanged.

**Review boundary:** pure configuration validation plus literal, generated and
actual loopback HTTP regression tests. Reject aliases rather than silently
rewriting the configured allow-list. Preserve exact incoming Origin, current
session/context CSRF, no-store and all production closure gates.

**Risks / unknowns:** URL syntax/canonicality does not establish DNS, TLS, cookie
enforcement or provider/browser execution. The existing trusted-clock and
publication boundary remain unchanged.

**Non-goals:** the remaining [#74 follow-ups](backlog/issue-74-session-followups.md),
production activation, provider/native-store/UI redesign, lease optimization,
merge, deployment or closing #54/#74.
