# CMP ownership and conventions

Use for handwritten Kotlin APIs, decomposition, presentation state, navigation,
effects and host boundaries. Architecture doc 00 and current ADRs are normative;
the organization/API advice below is a default, not a new phase or lint gate.

## Find the current owner

Paths below are relative to `apps/mobile`; recheck callers at the current revision.

| Concern | Existing owner / contract |
|---|---|
| App composition, routes, preferences and shared screen mounting | `shared/.../TabulaApp.kt`, `navigation/BackStack.kt`, `shell/`; [shell contract](../../../../docs/ui/screens/mobile-shell.md) |
| Public discovery metadata | `tabula-registry` → `cargo xtask gen-mobile-catalog` → `catalog/RegistryDiscoveryCatalog.kt`; [ADR-0045](../../../../docs/adr/0045-mobile-discovery-parity.md) |
| Current account presentation and async fencing | `account/AccountState.kt`, `AccountSessionController.kt`, `AccountSessionPort.kt`; [account contract](../../../../docs/ui/screens/mobile-account.md), [ADR-0046](../../../../docs/adr/0046-mobile-account-surfaces.md) |
| Game launch/events/Back and native lifecycle model | `host/`, `session/`; [ADR-0043](../../../../docs/adr/0043-native-mobile-gamehost.md), [prototype evidence](../../../../docs/verification/mobile-native-host/adapter-prototype.md) |
| Native entrypoints and device adapters | `android/`, `ios/`, platform source sets; [mobile README](../../../../apps/mobile/README.md) |
| Desktop fixtures and tests over the shared UI | `previewApp/`; [agentic guide](../../../../apps/mobile/AGENTIC-CODING.md) |

ADR-0043 retires mobile WebView gameplay without fallback. The native lifecycle
prototype is a control model, not a Macroquad backend. Production entrypoints
retain an unavailable host and empty runtime inventory. Public catalog entries
do not authorize game launch; setup stays read-only until its native adapter can
honor configuration. Kotlin/Swift never receive canonical hidden state or
per-frame Rust drawing commands (I-5/I-6/I-10), and never branch on a named game
to implement rules or platform dispatch (I-9).

ADR-0046 admits read-only identity only from a current adapter. Native provider,
social, profile mutation and private persistence remain gated; isolated Web
capabilities do not open them. Named synthetic account/game ports belong to
preview/tests. Preserve masking and operation/lifetime fencing on background,
expiry, cancellation, port replacement and unconfirmed sign-out; bind a managed
avatar to the exact current identity instance. Save bounded public navigation
and preferences, never account facts, credentials, active matches or operations.
Voice/device changes read their own ADR and native adapter; they grant no game
or account authority.

## Shape the component around a responsibility

Prefer existing feature owners over global `utils`, `screens` or one state holder
that owns every concern. A route/container binds state and lifetime; content,
sections and dialogs receive explicit values/events. Split when responsibilities
or lifetimes differ, not at a line quota. A small cohesive feature can remain in
one file. No mandatory AndroidX ViewModel, MVI store, repository layer or module
per widget is introduced by this skill.

For a reusable layout-emitting composable, prefer
`modifier: Modifier = Modifier` as its first optional parameter and apply it
once at the component root. Trace existing callers before changing where a
modifier lands: moving a field modifier can move its focus/semantics contract.
Preserve root bounds, input focus and action ownership. Name callbacks for their
events (`onValueChange`, `onBack`) and slots for their purpose; use exclusive
variants when they prevent an invalid mode, not merely to remove booleans.

During extraction retain `remember`/saveable identity, stable list keys, caret,
focus, scroll, callback ordering and cleanup. Do not remount an input or recreate
a session/subscription to make a composable shorter. Preserve existing package,
module and application identities unless the task changes them explicitly.

## State and effects have a lifetime

Keep one observable mutable owner per presentation fact. Local menu/focus/scroll
state can stay at its UI lifetime; cheap derived facts should be projections,
not separately synchronized writable flags. `val`, read-only `List` and
`data class` alone do not make aliased mutations observable or deeply immutable.
Do not add stability annotations or memoization to hide mutable aliasing or claim
an unmeasured speedup.

`MutableStateFlow.update` transforms can be retried: keep them deterministic and
side-effect free. Resolve time/IDs outside, and execute I/O/effects through the
named owner with explicit ordering. An external effect accumulator inside the
transform can also duplicate work on retry.

For requests/listeners/platform callbacks, name owner, restart keys, cleanup,
current request/account generation and the policy for late results. Duplicate
submission and obsolete completion are different defects; cancellation alone
does not fence either external mutation or late publication. Use controlled
tests for those cases. No I/O, listener registration or runtime launch occurs in
the composable render body. A collection API's name alone proves no lifecycle
bug; inspect its producer and owner.

Back dismisses the current local confirmation/overlay before changing route, and
the active GameHost gets its Back opportunity first. Recomposition and resize
must not create another native runtime. Native callbacks/surfaces require their
own generation and stop/join contract; desktop stand-ins cannot prove it.

Before adding a dependency or platform API, inspect the pinned Gradle catalog,
source-set placement and concrete consumer. Android compatibility does not prove
`commonMain` or iOS support. Keep style changes scoped to touched handwritten
code. Enforce these decisions with relevant owner tests and manual API/call-site
review; discover actual formatter/lint tasks before claiming static enforcement.
