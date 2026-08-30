# apps/

Client applications. All are **leaves**: nothing depends on them.

| App | Phase | What it is |
|---|---|---|
| [`game-client`](game-client) | 2 → 4 → 6 | The Macroquad gameplay runtime. Desktop, Android, iOS, and web-at-`/play/:id` from one codebase. |
| [`web`](web) | 5 | The Leptos application shell: auth, catalog, rooms, queue, profile, results. CSR. |
| [`admin`](admin) | 5 | Operator UI. Role-gated, separate bundle. |
| [`desktop`](desktop) | 5, optional | Tauri launcher/updater. **Never required for gameplay** (ADR-019). |

## The separation rule (doc 04 §1.1, ADR-011)

```text
application shell   →  Leptos, DOM, routed, SEO-able-if-we-ever-need-it
gameplay runtime    →  Macroquad, canvas, one frame loop, no DOM
```

They are **separate WASM binaries** on the web, sharing Rust crates but not WASM
memory. Two runtimes fighting over the canvas, the DOM, and the event loop is a
problem we decline to have — and two bundles means two independent caches, so a
shell deploy does not invalidate the game.

```text
/                → app.wasm    ~1.5-2.5 MB gz target
/play/:match_id  → game.wasm   ~4-6 MB gz target   (HARD CAP: < 6 MB gz)
```

`/play/:match_id` is a **real navigation to a separate document**, not a
client-side route into a canvas.

## I-15, enforced by `xtask check-deps`

**`leptos` must never appear in `apps/game-client`'s dependency graph** — native
or WASM. And per ADR-019, **Tauri is never required for gameplay on any
platform**. Gameplay in a WebView would make WebView latency the product's
ceiling.

## The handoff (doc 04 §3.4)

```text
shell:  POST /matches → { match_id, join_token }
shell:  sessionStorage["match.ctx"] = { match_id, join_token, game_id@version, pack }
shell:  prefetch game.wasm + pack manifest DURING the room screen
shell:  navigate to /play/:match_id
game:   read match.ctx → branded loader with real byte-level progress
game:   WS Hello + Attach(join_token) → Welcome { view, capabilities }
game:   ... play ... → in-canvas result → navigate to /matches/:id
```

Back/forward and deep links must work; re-entering `/play/:id` resumes.

**Native has no navigation — it swaps a scene.** The same `MatchContext` struct
is passed in-process, so the runtime code is identical everywhere.

## Shell screens are implemented twice, on purpose

Lobby and catalog UI: once in Leptos, once with `tabula-presentation` widgets for
native. About a dozen screens. The alternative is a WebView on mobile, which
ADR-019 rules out.

The *specification* lives once, in [`docs/ui/screens/`](../docs/ui/README.md), and
both implementations reference it. Two implementations of an unwritten spec
diverge within a month.
