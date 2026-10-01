# Werewolf audit rubric

Recheck status. Package: `tabula-game-werewolf`; common features are declared.
The game is **partial**: validated config/roles, canonical state/events,
deterministic initial assignment, metadata/capabilities exist. There is no
complete `GameRules`/`GameModule`, reducer, `View`/projection, SecretModel,
conformance/security fixture, simulation, replay or presenter. Feature and
information-model declarations are not those implementations.

Authorities:

- [Werewolf spec](../../../../docs/games/werewolf.md): W-D decisions,
  ClassicV1, resolution/knowledge/scopes, gaps, ledger and phase boundaries.
- [Doc 08 §5](../../../../docs/architecture/08-first-games-validation-plan.md):
  benchmark, failure signals/acceptance.
- [Implementation plan](../../../../todos/werewolf/),
  [exports/capabilities](../../../../games/werewolf/src/lib.rs),
  [creation kernel](../../../../games/werewolf/src/rules/mod.rs),
  [config](../../../../games/werewolf/src/rules/config.rs),
  [state validation](../../../../games/werewolf/src/rules/state.rs),
  [roles](../../../../games/werewolf/src/rules/role.rs),
  [events](../../../../games/werewolf/src/rules/event.rs).
- Existing [config](../../../../games/werewolf/tests/config.rs),
  [state](../../../../games/werewolf/tests/state.rs),
  [assignment](../../../../games/werewolf/tests/assignment.rs),
  [metadata](../../../../games/werewolf/tests/metadata.rs) tests.

Today's meaningful command:

```bash
cargo nextest run -p tabula-game-werewolf
```

Report exercised claims: exact preset counts, refined bounds/deserialization,
structural invariants, deterministic assignment/roster-order invariance and
metadata. These do not test private actions, timers, resolution, projection,
communication enforcement or completed matches. `xtask selfplay/replay`
supports only Chess/Tiles today.

For a future reducer, route claim families to the maintained decisions rather
than copying their truth tables here:

| Surface | Rubric source / sensitive edges |
|---|---|
| Assignment/config | W-D1/W-D2; 6/12/20 seats, exact multiset/sorted roster, RNG domain, invalid occupants/duplicates/teams |
| Private resolution | W-D3–W-D5/W-D11; Doctor repeats/protection, blind Witch consumption, Hunter precommit, same-batch actor deaths, sorted publication |
| Vote/outcome | W-D6/W-D7/W-D9/W-D10; ties/abstain/replace/unvote, fixed window, zero-alive/village/parity ordering, cap draw/standings |
| Lifecycle/timers | W-D8/W-D16/W-D17; default choices, no early closure, stale/exact/overflow deadlines, absence/reconnect, forbidden substitution |
| Knowledge/events | W-D12/W-D13/W-D15; living-role, dead-seat full vision, outside public-only spectator, private event `None`, death/end reveal, no existence frames |
| Scope values | W-D14; phase-specific speak/listen, absolute data effects; socket/SFU enforcement is a separate later-phase claim |

Headless rule/security/replay tooling is Phase 3; social/online UX Phase 7;
voice Phase 8. A terminal projection viewer is verification tooling, not
`GamePresentation`. Runtime substitution is forbidden; planned test-only
simulation is not a production bot. Report stale illustrative comments
separately from maintained decisions.

State-version/ack side channels and asymmetric voice permissions remain
platform/ADR questions. A future `view_event` pass cannot settle them alone;
read [security](hidden-information.md) and respect phase gates.
