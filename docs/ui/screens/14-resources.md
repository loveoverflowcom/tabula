# 14 — Xiangqi resources

Issue [#53](https://github.com/loveoverflowcom/tabula/issues/53), following
the exact-artifact requirements in [#48](https://github.com/loveoverflowcom/tabula/issues/48).
Reviewed at `develop @ b16dd2e14a6e71a73e36d6b50e18ff526e4fd8d5`.
Use the shared [resource/platform gates and adapters](xiangqi.md),
[foundation](foundation.md) and [analysis](11-xiangqi-analysis.md).
This proposed document/native-shell surface adds no installer, host adapter,
download, provider selection or product route.

Inspected the pinned `030da25d0098e240ab2cf36dacf9892e8b320a89`
[14 SVG](https://github.com/loveoverflowcom/tabula/blob/030da25d0098e240ab2cf36dacf9892e8b320a89/docs/ui/design-02/05-xiangqi/screens/14-resources.svg)
and actual `mobile-states.png`; no desktop Resources PNG is supplied. Sample
CPU, size/version columns and Ready badge establish no installed artifact,
license permission, compatibility, offline capability or measured budget.

## Exact resource identity and ownership

The host/approved resource adapter owns I/O, storage, verification, process
execution and lifecycle. Pure game rules own none of these; the presenter
reads safe status and constructs a request. Existing logical asset refs and
pack integrity types are reusable contracts, not evidence of a concrete
engine installer or trusted executable. Phase A compile-time game modules
are distinct from Phase B loadable packages and Phase C marketplace/sandbox.

Show separate rows for engine binary, evaluation network/weights, board/media
pack and lesson/book source. A pack may require several rows before one
capability is ready. Each row identifies provider/official source, artifact
type, exact version/revision and digest/algorithm, download/installed size
when known, platform/architecture, compatibility requirement, license and
distribution/use disposition. Unknown size/hash/rights is a named limitation,
not zero bytes or assumed permission. Network/code licenses remain separate;
code GPL or subprocess/UCI use does not establish weights rights.

Installation is offered only after the exact artifact, source, rights,
compatibility and supported adapter are resolved. The user deliberately
selects the resource and sees its source, size, license/use terms, required
companion files, storage impact and CPU/RAM policy before starting. No
automatic download, unrequested provider switch or cloud fallback occurs.
If rights cannot be established, keep an Unlicensed/rights unresolved reason
and supported alternative/return action; do not bundle or execute it.

## Lifecycle and capability readiness

| State / transition | Required behavior |
|---|---|
| Unselected / unavailable | Explain absent provider/adapter; no synthetic download progress or enabled Install |
| Missing → preflight | Resolve exact requested identity, license, supported platform, storage and policy; validation failure preserves current usable resources |
| Preflight → downloading | One identified operation; actual transferred/total bytes if measurable, explicit Cancel, bounded retry policy |
| Cancel / interruption / storage-full / source error | Retire operation generation; incomplete bytes never become Ready; show persistent status and supported explicit retry/cleanup |
| Downloaded → verifying | Check exact size/digest and safe package structure before trusted storage/decoding/execution; never report Ready merely because transfer finished |
| Verification failure → corrupt | Do not probe or execute corrupt bytes; show the mismatch and supported retry/remove action |
| Verified → compatibility/license validation → probing | Recheck target/provider/resource binding and rights; run only the approved bounded provider protocol probe |
| Probe success → ready | Publish readiness scoped to exact artifact/platform/config and its real consumer; retain actual verified identity and probe result |
| Probe failure / incompatible / unlicensed | Distinct failure/reason; no engine Play/Analyze enablement, no automatic substitution |
| Ready → missing/corrupt/provider crash/policy change | Invalidate dependent readiness and evidence/jobs as needed; retain the permitted game board and explain loss |

License/compatibility preflight occurs before downloading and is checked
again before use; the table is not permission to fetch an unlicensed file
first. A transfer, integrity check and successful protocol probe establish
different facts. Probe success proves the exercised handshake/configuration,
not engine strength or all positions/platforms. “Offline ready” additionally
requires a successful offline start/use check for that exact supported
capability; installed or previously played is insufficient.

Requests bind resource identity, target, operation generation and user action.
Repeated Install/Retry cannot create duplicate operations. Leaving the
screen, changing provider/target or cancelling retires obsolete work; stale
progress/probe completion cannot mark a new row Ready. Background work is
allowed only under an explicit supported host policy. Resume uses verified
server/range metadata and final integrity, never trusts a partial download
because its filename looks complete.

Updates stage new artifacts separately and publish a validated compatible
set atomically; a failed update retains the previous usable set and reports
which version is active. A live match/job pins its resources. Remove/update
cannot silently evict or replace those active artifacts: explain the busy
dependency and supported cancel/defer choice. Removing a companion network
invalidates the dependent engine capability. Removal success is acknowledged
by the real adapter, not the button click. Saved history is never rewritten
to claim a new engine produced old evidence.

## Platform and policy presentation

Use a matrix per actual host/target and capability: native play/analysis,
browser play/analysis, mobile, install and offline startup. Status is
Supported with scoped evidence, Unavailable with reason, or Not verified.
Native subprocess support does not imply a browser worker/WASM engine or
mobile executable policy. Browser isolation/thread/storage requirements and
mobile lifecycle/thermal constraints need explicit adapter evidence.

Expose only enforced CPU/thread/time/node/RAM/storage limits, their units and
effective values. A sample “25% CPU” is not an available policy; a configured
thread limit is not a measured CPU or RAM bound. Unknown memory estimate says
Not measured. Cancel/timeout must be bounded and enforced by the host, not
only by hiding a spinner. Offline capability lists which verified local
artifacts are required and whether optional knowledge/LLM remains absent.

## Layout, actions and accessibility

Expanded layout uses a compact resource list and bounded platform/policy
summary; compact layout stacks fully labeled rows and their actions. Keep
type, identity, readiness reason and exact active version available when
details collapse. At 320/390, 768, 1440 dp, short landscape and 200% zoom,
wrap hashes/URLs and text instead of clipping or forcing page overflow.
The mock lifecycle pills illustrate states, not clickable navigation.

Use generated foundation `surface`/`container`/`container-high`, contained
list corners, readable metadata typography and `Mono*` for long digests.
Permitted Install is the filled principal action; Cancel/Retry/Details are
tonal/text with labels and at least 44 × 44 dp wrappers. Destructive removal
uses a bounded confirmation with resource/dependency consequence, busy/error
state and focus restoration. Keep focus/error/input outlines, omit ornamental
borders/shadows, and never fade the only unavailable/license explanation.

Tab order follows return/context → resource rows' named actions → policy
controls → active operation. Static rows/badges are not focus stops; details
use semantic disclosure and a heading. Enter/Space activates once; repeated
keys, pointer release outside/cancel and blur do not install/remove twice.
Dialogs trap focus, prevent underlying activation, allow safe Escape cancel
and restore invoker/logical successor. Completion does not steal focus.

Show named progress with actual bytes or honest indeterminate status; throttle
announcements and retain stationary text under reduced motion. Announce
failure once and preserve its readable reason/recovery. Four themes and
no-audio modes preserve type, readiness, stale/error and dependency meaning
without color alone. Runtime en/vi keys/localized byte/time units and native
text/IME semantics follow the owning shell; static SVG is not AT evidence.

## Future verification required

Exercise missing companion files, unknown/revoked rights, wrong target,
size/hash mismatch, malformed/oversized package, interrupted/repeated/cancelled
transfer, storage-full, stale completion, probe timeout/crash, update rollback,
active-resource removal and actual offline startup. Keep doubles separate
from real provider/install integration and measured budgets. Record each
platform independently with runtime keyboard/focus, compact/200%/four-theme
and AT checks. None of these lifecycle/platform checks is executed by this
specification.
