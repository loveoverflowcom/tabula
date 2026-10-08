# Select Tabula review boundaries

Choose sections by changed behavior and affected consumers. These are tracing
questions, not extra gates or a requirement to load every link.
[Doc 00](../../../../docs/architecture/00-architecture-principles.md), its ADR
register and selected owners define the rules. Recheck current implementation:
an isolated exception, compiled skeleton or historical ledger is not general readiness.

## Rust/domain, game authority and history

Trace raw input → trusted facts → pure decision → committed effects. Does the
owning game/platform boundary still decide, or has a handler, presenter, adapter
or SQL query acquired game meaning? Follow actual consumers through registry
erasure and `deps.toml`, including optional/transitive dependency edges and the
rules/presentation feature split.

Use engineering's [type barriers](../../tabula-engineering/references/types-as-proofs.md),
[construction bypasses](../../tabula-engineering/references/boundary-hardening.md)
and [functional boundaries](../../tabula-engineering/references/functional-core.md).
A validated wrapper does not protect a second unchecked ingress.

For rules/replay/projection changes, select [game-audit layers and rubric](../../tabula-game-audit/SKILL.md).
Trace rejected input/context, RNG/time/order, encoding and rules identity, then
`project`/`view_event` for actual viewers. Read implemented fixtures; same-build
replay/self-play is not an independent rulebook oracle. Shared SDK/kernel edits
consider every actual consuming game.

Review compatibility at the changed carrier: game payload, protocol envelope,
HTTP DTO, persisted snapshot/journal and their versions can differ. Preserve
canonical replays and explain expectation changes. Rules-subtree comments can
change `RULES_HASH`; inspect actual hash/build inputs before judging impact.

## Session, network, privacy and durable effects

Select [doc 05](../../../../docs/architecture/05-data-protocol-and-replay.md)
and relevant session/match ADRs from doc 00. Trace credentials and server-owned
account/session/channel/seat/match scope through admission, apply, commit and the
actual output handoff. Inspect the affected consequential boundary, not only
the first authorization read. Useful current seams when touched:

- [Session HTTP ports](../../../../crates/tabula-session/src/http_ports.rs):
  snapshot observations versus the boundary a publication guard fences
- [Actor authority/effect ports](../../../../crates/tabula-match/src/runtime_ports.rs):
  synchronous apply/submission, durable commit and eventual delivery obligations
- [HTTP body publication](../../../../crates/tabula-match-http/src/native_body.rs):
  guard lifetime and first-frame release after asynchronous work
- [SQL-free journal](../../../../crates/tabula-match-journal/src/lib.rs) and
  [storage owner](../../../../crates/tabula-storage/src/lib.rs): atomic journal,
  scoped receipts/watermarks, owner fencing and integrity-checked recovery
- [Client selection](../../../../crates/tabula-net-client/src/lib.rs) and
  [direct path](../../../../crates/tabula-net-client/src/direct.rs): real transport
  support, pending-command, duplicate/retry and reconnect/resync semantics

What invalidates a witness across `await`, queued output, cancellation, expiry,
rotation/revocation, membership or owner replacement? Is unknown commit distinct
from rejection, and is retry bound to the original operation without duplicate
effects? Do faults retire uncommitted state and suppress stale output before
recovery/resync admits input? A stable effect key is not disclosure permission.

Follow unauthorized observations: projection, event existence/order, receipt/version
metadata, errors, logs, caches and hints. Select [hidden-information criteria](../../tabula-game-audit/references/hidden-information.md)
when needed and verify its game-status statements against source. Browser/native
credential storage, origin and CSRF checks need the real carrier. Mock sessions
or in-memory journals cannot prove PostgreSQL, HTTP body or independent-browser
enforcement; name the gap without asserting a leak.

## Macroquad presentation, renderer and assets

Use [presentation criteria](../../tabula-game-audit/references/presentation-review.md)
and [doc 04](../../../../docs/architecture/04-frontend-and-design-system.md).
Trace projection/event → local state → intent/RenderList → renderer/resources.
Do reveal/conceal, animation, selection, camera and input stay local, and does
authoritative ack/reject/resync replace pending preview correctly?

Follow asset IDs through game-owned sources, manifest/hash/version, loader and
actual resource consumer. Distinguish asset-byte integrity, decoded image bounds,
ready GPU resources and successful frame flush. Inspect cache identity, origin/
size/hash checks, cold/warm load, missing-resource recovery and late completions.
A fixture asset or fake loader does not establish the packaged runtime path.

For changed visible/input behavior, inspect keyboard/pointer/touch hit geometry,
resize, focus, themes, reduced motion and interrupted private disclosure as relevant.
RenderList assertions establish commands; headless raster goldens establish only
that backend's supported subset. Actual pixels, font metrics and browser/native
event integration need separate evidence. Screenshots need artifact/scenario/
viewport/theme identity and inspected criteria, not only a capture caption.

## Leptos shell, shared tokens and CMP/native host

Follow [screen contracts](../../../../docs/ui/screens/README.md), authored
[tokens](../../../../tokens.toml), generated adapters and actual consumers. Shared
token/brand edits need a DOM/canvas/CMP consumer map; declared consumers may be deferred.
ADR-011 separates Leptos from the Macroquad gameplay document. Inspect relevant
[web source](../../../../apps/web/src/) for route/handoff/query/return validation,
state disposal and Fetch/DOM/event behavior. CSS/DOM checks cover neither gameplay
pixels nor Kotlin semantics.

For [mobile work](../../../../apps/mobile/README.md), use
[CMP engineering](../../tabula-cmp-engineering/SKILL.md) as criteria without entering
its implementation/live-edit loop. Trace shared production components, state/port
owners, `GameHost`, native lifecycle and artifact consumers. Kotlin/Swift own
UI/navigation/device services, not rules/projection/protocol decisions.
[ADR-0043](../../../../docs/adr/0043-native-mobile-gamehost.md) retires mobile WebView
gameplay without fallback; the native model is not a delivered Macroquad backend.
Review recomposition, Back, foreground/background, disposal, reload/retry and late
callbacks through the actual host. Launch facts remain untrusted; credentials
and native media stay outside the game host contract.

Use the mobile README and agentic guide to distinguish controlled host/model tests,
Desktop Compose assertions, live MCP interaction, inspected preview pixels,
APK/iOS Kotlin compilation/linking and actual Android/iOS device execution.
Public discovery and synthetic account/game ports grant no native readiness.
Native voice review selects its grant,
permission/media lifecycle and provider boundary; local mute does not prove
game audience/SFU enforcement.

## Tooling, documentation and evidence changes

Review runner/dependency/feature selection, filters/skips, environment defaults,
corpus/goldens, harness assumptions/stubs and generator output as executable policy.
Does a missing prerequisite or relevant case fail the check? Does a claimed proof
reach production or only a duplicated model? Reuse engineering's technique references;
do not introduce an unsupported proof stack.

For skills/docs-only edits use [skill validation](../../README.md#validation)
and targeted source/link review: ownership/routing, actual commands and phase/
evidence wording. Structural validation proves metadata/paths, not runtime discovery
or sound decisions. Full builds are not substitutes for these checks; report any
repository-required but unrun gate honestly.
