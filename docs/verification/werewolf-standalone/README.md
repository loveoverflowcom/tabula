# Werewolf standalone verification

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/werewolf-standalone).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


Scope: ADR-0035's opt-in deterministic referee and isolated-seat local simulator.
Implementation began at develop 115159c and publication is reconciled with the later
CMP foundation / ADR-0032, first-party host / ADR-0033, renderer renames and
Kanidm auth scaffold / ADR-0034. No mobile implementation is added.
The common dashboard remains M3; Werewolf owns its in-game card material.

## Executed evidence

| Claim / boundary | Check / actual scope | Result |
|---|---|---|
| Complete ClassicV1 / timers / outcome / lifecycle | `cargo test -p tabula-game-werewolf` | PASS: 116 tests, 0 failed/ignored/filtered |
| Projection and private event existence | 20 typed/paired noninterference tests; live/dead/outsider/Audit at 6/12/20 seats | PASS; no network cursor/transport claim |
| Canonical reconstruction | Witch consumption, Doctor repeat, Hunter mark/history coupling; hostile/overflow R2 | PASS focused semantic/state suite |
| Game conformance | 11 Werewolf checks; generic probe-clock positive/negative regressions; 11 each Chess/Tiles conformance | PASS |
| Deterministic bounded simulation | 102,000 final-source matches, 7,182,074 attempted inputs; every match run twice; 5% hostile; projections/events compared | PASS: all terminated, 0 failures |
| Simulation boundaries | 6/12/20 seats, max 3 rounds, base seed [64;32]; p99 apply 8/16/32µs | Sampled local evidence, not production performance |
| Long-window history size | 20 seats, 100 rounds, all active-role submissions; 1,400 inputs; roundtrip every phase | PASS: history ≤1,802 events, state <30KiB |
| Pinned canonical replay | 3 normal/tie/timeout `.tbr` fixtures, 128/128 checkpoints; byte-identical regeneration | EXACT / VERIFIED |
| Card/privacy/input presentation | 15 focused tests: all 6 concealed equality, only authorized own portrait, stale projection/lifecycle, real command intent, keyboard/pointer/held keys | PASS headless interaction evidence |
| Conservative font fit | 24 role-body examples + 6 target/nav examples at 320–1440 layouts and bundled font metrics | PASS: 30 examples; not pixels |
| Asset generation / integrity | 14 independent PNGs, 2 densities, common opaque back; generator `--check`, metadata/hash/size/binding tests | PASS |
| Generic local-host integration | 4 real `LocalMatch` tests: public noninterference, fixed deadlines/Timer stream, reset and completed deterministic timeout matches | PASS |
| Browser host components | 90 Node tests including cancellation/cache/integrity/teardown/runtime mismatch and opaque shield + stale conceal-ack lifecycle | PASS mocked browser components |
| Resource staging | 23 xtask unit tests (16 WASM, 6 mobile, 1 pack) + 2 CLI tests, including 18-payload simulator isolation and exact SHA256 | PASS |
| Loopback serving semantics | 3 HTTP routing/integrity/cache tests | PASS backend HTTP only |
| Portable workspace core | `cargo xtask check` | PASS repeated after rebasing onto develop 729417f; full fmt/clippy/test/deps/game-id/manifest/token/colors/deny order |
| Native / selected WASM / feature matrix | exact final target commands in host/build receipt | PASS native link, selected WASM optimized link, workspace no-default/all-features; compilation is not target execution |

## Artifact ownership / provenance

- Standard source pack: `games/werewolf/assets/`; 14 PNGs plus pinned manifest/source/generator
- Design reference and original attribution: `docs/ui/werewolf-approved/`; source ZIP v1 retained
- Game presenter: `games/werewolf/src/presentation/`; all front typography live Vietnamese
- Opt-in host: `apps/game-client/src/bin/werewolf.rs`; shared existing font/resource boundary
- Rules version 2, no migration of W1/W2 skeleton snapshots. Production `WerewolfModule::bot`
  remains None; simulation policy exists only under test/testkit
- The offline unpublished Mac code/evidence was not imported or claimed as cloud source

The pack contains 1,639,008 encoded bytes across both densities. Selected 1x is 362,117 encoded /
1,794,048 estimated RGBA8 bytes; 2x is 1,276,891 encoded / 7,176,192 estimated RGBA8. All roles load
uniformly by density, independent of assignments. No original all-role atlas is served or
inlined into the deployed WASM. Final static emitted receipt is `loading-receipt.json`.

## Blocked / deferred gates

- Native launch: BLOCKED `XOpenDisplay() failed`; no DISPLAY or X11 socket in this shell
- Dot browser preview: BLOCKED `net::ERR_BLOCKED_BY_CLIENT` for the supported loopback URL;
  a follow-up blocked-page capture was also refused by browser policy. No bypass was attempted
- Real native/browser pixels, 390×844/320×568 mobile pixels, 200% zoom, physical touch,
  real BFCache/cache waterfall, GPU reclamation and assistive technology: NOT_RUN / BLOCKED
- Native/WASM comparable canonical byte execution: NOT_RUN; builds alone are not cross-target proof
- Online multiplayer / authenticated seat privacy / sockets / chat enforcement / moderation /
  voice / persisted resume/replay / CMP WebView embedding / rollout / merge / deploy: out of scope

Approved design-reference images and headless geometry are not real runtime screenshots.
The source is ready for reference and review; `game.toml` rollout stays disabled.
