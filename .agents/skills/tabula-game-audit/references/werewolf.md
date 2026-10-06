# Werewolf audit rubric

Recheck the checkout and requested change. Package: `tabula-game-werewolf`.
ClassicV1 pure rules, projection/event non-existence, conformance/SecretModel,
deterministic simulation and canonical replays are implemented. A game-owned
presenter and isolated-seat native/WASM local host are authorized by
[ADR-0035](../../../../docs/adr/0035-werewolf-local-simulator.md). This bounded
exception does not open social/online, chat enforcement, voice, authenticated
seats, CMP WebView embedding or rollout. Declaration and compilation do not
establish real platform execution.

Authorities:

- [Werewolf spec](../../../../docs/games/werewolf.md): W-D decisions,
  ClassicV1, resolution/knowledge/scopes, gaps, ledger and phase boundaries.
- [Doc 08 §5](../../../../docs/architecture/08-first-games-validation-plan.md):
  benchmark and phase acceptance.
- [Implementation plan](../../../../todos/werewolf/),
  [exports/capabilities](../../../../games/werewolf/src/lib.rs),
  [creation kernel](../../../../games/werewolf/src/rules/mod.rs),
  [config](../../../../games/werewolf/src/rules/config.rs),
  [state validation](../../../../games/werewolf/src/rules/state.rs),
  [roles](../../../../games/werewolf/src/rules/role.rs),
  [events](../../../../games/werewolf/src/rules/event.rs).
- [Presenter](../../../../games/werewolf/src/presentation/mod.rs),
  [rendering](../../../../games/werewolf/src/presentation/render.rs),
  [assets](../../../../games/werewolf/src/presentation/assets.rs),
  [local host](../../../../apps/game-client/src/bin/werewolf.rs),
  [screen contract](../../../../docs/ui/screens/07-werewolf.md).
- [Standalone evidence](../../../../docs/verification/werewolf-standalone/README.md)
  and [#84 redesign evidence](../../../../docs/verification/werewolf-redesign-84/README.md).
  Historical receipts apply only to their named source and scope.

Select checks by the changed behavior. Existing targets include config, state,
assignment, metadata, rules, conformance, security and replay integration tests,
plus feature-gated presentation tests and the client's local-simulator integration.

```bash
cargo nextest run -p tabula-game-werewolf --features presentation
cargo nextest run -p tabula-game-werewolf --features presentation --lib -E 'test(presentation::tests)'
cargo test -p tabula-game-client --no-default-features --features werewolf --test werewolf_simulator
```

The [testkit-only simulation adapter](../../../../games/werewolf/src/simulation.rs)
has a [bounded example](../../../../games/werewolf/examples/verify_simulation.rs).
Production `WerewolfModule::bot` remains `None`. `xtask replay` supports Werewolf's
three committed [canonical goldens](../../../../games/werewolf/tests/replays/README.md);
`xtask selfplay` still dispatches only Chess/Tiles. Preserve replay identity and
expectations; do not regenerate them for a presentation edit.

Route claim families to the maintained decisions rather
than copying their truth tables here:

| Surface | Rubric source / sensitive edges |
|---|---|
| Assignment/config | W-D1/W-D2; 6/12/20 seats, exact multiset/sorted roster, RNG domain, invalid occupants/duplicates/teams |
| Private resolution | W-D3–W-D5/W-D11; Doctor repeats/protection, blind Witch consumption, Hunter precommit, same-batch actor deaths, sorted publication |
| Vote/outcome | W-D6/W-D7/W-D9/W-D10; ties/abstain/replace/unvote, fixed window, zero-alive/village/parity ordering, cap draw/standings |
| Lifecycle/timers | W-D8/W-D16/W-D17; default choices, no early closure, stale/exact/overflow deadlines, absence/reconnect, forbidden substitution |
| Knowledge/events | W-D12/W-D13/W-D15; living-role, dead-seat full vision, outside public-only spectator, private event `None`, death/end reveal, no existence frames |
| Scope values | W-D14; phase-specific speak/listen, absolute data effects; socket/SFU enforcement is a separate later-phase claim |
| Local presentation | I-5/I-6/I-10; reveal bound to viewer/phase/round/lifecycle, immediate conceal, held-key guard, selection then explicit intent, public-only avatar/ballot feedback |
| Responsive/renderer | Shared clip/text/input mechanism; portrait hit geometry, drawer focus, host/dock ownership, four themes and reduced motion; RenderList assertions do not prove pixels |
| Public identity | Host-owned permitted display facts; neutral fallback while account avatar provider is unavailable; role/phase-independent image, occupant replacement and late-load clearing |

For the #84 presenter, check twelve-seat desktop ellipse, portrait 4×3 grid,
compact/landscape bounds, larger-roster paging, explicit private drawer and
secondary simulator controls. Motion uses projected events and bounded local
state, never gates submission/deadlines, and stops private disclosure immediately
on interruption. Private night actions cannot trigger outsider/teammate cues.

Headless rule/security/replay tooling is Phase 3. ADR-0035's local presenter is
a recorded exception; social/online UX and voice remain Phase 7/8. Runtime
substitution is forbidden, and the test-only policy is not a production bot.

State-version/ack side channels and asymmetric voice permissions remain
platform/ADR questions. A local `view_event` pass cannot settle them alone;
read [security](hidden-information.md) and respect phase gates.
