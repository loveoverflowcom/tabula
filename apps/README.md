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
platform**. On desktop, gameplay in a WebView would make WebView latency the
product's ceiling. On mobile the owner accepted a WebView `GameHost` under
[ADR-0032](../docs/adr/0032-compose-multiplatform-mobile-host.md), with that latency
risk still unmeasured and an evidence requirement before it ships. The mobile app
lives in [`mobile/`](../mobile/README.md), not in `apps/`.

## The handoff (doc 04 §3.4)

This is the future network flow under
[ADR-0031](../docs/adr/0031-browser-native-session-contract.md). Credentials stay
in the browser HttpOnly cookie/native secure store; public handoff hints grant
no authority. ADR-0030 local play has neither identity nor network resume.

```text
shell:  POST /matches → { match_id, public runtime metadata }
shell:  sessionStorage["match.ctx"] = { match_id, game_id@version, pack }
shell:  prefetch game.wasm + pack manifest DURING the room screen
shell:  navigate to /play/:match_id
game:   read match.ctx → branded loader with real byte-level progress
game:   authenticated HTTP + CSRF → fresh memory-only scoped attach grant
game:   cookie + Origin WS upgrade; Hello + Attach(grant) → Welcome { view, capabilities }
game:   ... play ... → in-canvas result → navigate to /matches/:id
```

Back/forward and deep links revalidate the current session and permissions
before network resume. Stored cursors/IDs cannot authorize attachment.

**Desktop has no navigation — it swaps a scene.** The same `MatchContext` struct
is passed in-process, so the runtime code is identical everywhere.

## Shell screens are implemented once per shell, on purpose

Lobby and catalog UI: once in Leptos, once with `tabula-presentation` widgets for
the desktop client; Compose Multiplatform in `mobile/` for Android and iOS (ADR-0032).
About a dozen screens.

The *specification* lives once, in [`docs/ui/screens/`](../docs/ui/README.md), and
both implementations reference it. Two implementations of an unwritten spec
diverge within a month.
