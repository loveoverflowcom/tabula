# Issue #53 — one real engine and resource adapter

**Status:** deferred until the board/rules and approved adapter/artifact gates.

**Outcome:** Analyze and optional engine play use an explicitly provisioned
real provider with bounded search, position-bound evidence and truthful
Resources availability on each supported platform.

**Why:** no adapter/pack/provisioner exists. Asset size/hash verification
establishes bytes only; neither rights nor engine compatibility/probe is
encoded by the current asset manifest. Pikafish code terms cannot establish
permission to distribute or use a particular NNUE artifact.

**Dependencies:** [board/modes](issue-53-board-modes.md), approved host boundary,
exact binary and weights revisions/digests/rights, enforced resource policy,
platform-specific real probes and offline evidence. Follow
[Analyze](../../ui/screens/11-xiangqi-analysis.md),
[Resources](../../ui/screens/14-resources.md) and the
[shared identity lifecycle](../../ui/screens/xiangqi.md).

**Review boundary:** one adapter per change; separate doubles from actual engine
integration. Cover late progress/final responses after same/different position,
branch/viewer/provider/budget change; duplicate/retry/cancel/timeout/crash;
malformed/illegal PV; every resource failure and previous-pack preservation.
Fair-play rejects requests/output at authority; no auto-download/cloud fallback.

**Risks / unknowns:** native process and web worker support are independent;
CPU/RAM limits must be real, not `apply_budget` or a mock percent. Output
validation proves legal candidates, not global best moves or explanation truth.

**Non-goals:** new plugin OS, marketplace, bundle unapproved weights, replay
regeneration with AI, tutor directly mutating state, all-platform parity.
