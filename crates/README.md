# crates/

The platform libraries. **This is the product** — `services/` and `apps/` are
thin shells around these.

Responsibilities, allowed deps, and forbidden deps for each are normative in
[doc 01 §3](../docs/architecture/01-stack-and-repository-plan.md), machine-readable
in [`deps.toml`](../deps.toml), and repeated in each crate's `src/lib.rs` header.

| Crate | Phase | Responsibility |
|---|---|---|
| [`tabula-core`](tabula-core) | 0 | Deterministic kernel: ids, `DetRng`, `LogicalTime`, `Viewer`, canonical hashing |
| [`tabula-game-api`](tabula-game-api) | 0 | The game contract: `GameRules`, `GameModule`, `Input`, `Effect`, `Ctx` |
| [`tabula-testkit`](tabula-testkit) | 0 | The conformance suite every game must pass |
| [`tabula-design`](tabula-design) | 2 | Semantic tokens, generated into CSS and a `Theme` |
| [`tabula-presentation`](tabula-presentation) | 2 | `View` → `RenderList`, input model, animation, a11y |
| [`renderer-macroquad`](renderer-macroquad) | 2 | The first `Renderer` backend — deliberately replaceable |
| [`tabula-assets`](tabula-assets) | 3 | Versioned, hashed per-game asset packs |
| [`tabula-protocol`](tabula-protocol) | 4 | The wire: envelopes, versions, dual codec, error codes |
| [`tabula-registry`](tabula-registry) | 4 | The catalog — the **only** crate that names games |
| [`tabula-match`](tabula-match) | 4 | Authoritative match runtime: actor, pipeline, ports |
| [`tabula-storage`](tabula-storage) | 4 | The **only** crate that knows SQL exists |
| [`tabula-net-client`](tabula-net-client) | 4 | Client session: connect, resume, sequencing, idempotency |
| [`tabula-lobby`](tabula-lobby) | 5 | Rooms, matchmaking, presence |
| [`tabula-voice`](tabula-voice) | 8 | `VoiceService` trait + provider adapters |

Crates beyond Phase 0 exist here as **doc-comment skeletons**: the responsibility,
the sketched contracts, the phase gate, and the doc references, with no
implementation. That is deliberate — see [`AGENTS.md`](../AGENTS.md) §4 and doc
09 §7.

## Two facts to read off the dependency graph (doc 01 §4)

1. **Games touch only `tabula-game-api`.** They cannot reach the network, the
   database, or the renderer. That is I-1 and I-11 made structural rather than
   asked for politely.
2. **Everything game-specific funnels through `tabula-registry`.** The match
   runtime, the net client, and the server see games only through erased
   interfaces. That is I-9 made structural.

```text
                    games/*        (rules: pure, no I/O)
                       │
                  tabula-game-api
                       │
                   tabula-core     ← everything depends on this; keep it tiny
                       │
     ┌─────────────────┼──────────────────┐
  protocol          design            assets
     │                 │                  │
  registry        presentation ───────────┘
     │                 │
   match         renderer-macroquad
     │                 │
   lobby          apps/game-client
     │
services/tabula-server
```

## Crates that could merge, and the trigger

Only one. `tabula-game-api` could merge into `tabula-core` if the trait set
stabilises. Trigger: zero changes to either crate for two consecutive phases
**and** no third-party game authors. Low priority (doc 01 §3).

Everything else is "never" — each split earns its keep by being the seam that
makes something replaceable.
