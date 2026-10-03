# Projection and event security layer

Authorities: doc 00 I-5/I-6,
[doc 02 §7](../../../../docs/architecture/02-game-module-and-sdk-design.md),
the maintained game information model, and
[projection helpers](../../../../crates/tabula-testkit/src/projection.rs).
Record what is secret and which viewers may know it through each relevant
phase/lifecycle transition. `Viewer::Audit` is not a client spectator.

For `hidden_information = true`, locate an actual `SecretModel`,
`HiddenInformationFixture` and `projection_security!` invocation. Read
[security.rs](../../../../crates/tabula-testkit/src/conformance/security.rs):
the viewer universe includes every roster seat plus declared spectators;
`GameControlled` fixtures must name spectator tiers. A model listing only
authorized viewers cannot define the unauthorized population. Non-vacuity
counters must exercise real secrets, unauthorized projections and events.

| Claim | Check |
|---|---|
| Values are absent from unauthorized outputs | Containment tokens in serialized `View`/`ViewEvent`, with meaningful tokens and explicit coverage limits |
| Derived outputs do not reveal a secret | Scramble that secret while preserving valid public facts; compare projections/events for every unauthorized viewer |
| The property exercises a real difference | Assert the secret changed, a public change stays observable, and authorized knowledge remains available |
| Private event existence stays private | Unauthorized `view_event == None`, not `Some(Redacted)`; check count/order/timing where implemented shells expose it |
| Errors/affordances do not become oracles | Public-safe errors, hints, descriptions, cues and render inputs; hidden values must not leak through consequences |

Containment is not complete secrecy evidence. Tiny common encodings cause
false positives; hashes, counts, timing, order and booleans can reveal derived
information without containing secret bytes. State secret/viewer/phase
coverage for each oracle.

Tiles is an implemented secondary hidden-information case. Its
[SecretModel and tests](../../../../games/tiles/src/rules/secret.rs) live under
test/testkit configuration in the rules subtree, not `tests/projection.rs`.
The bag multiset/count is public; order is secret. Containment tokens exist
only while at least four tiles remain. Bag-permutation noninterference and
observability controls complement that bounded scan:

```bash
cargo nextest run -p tabula-game-tiles --lib -E 'test(rules::secret::tests)'
cargo nextest run -p tabula-testkit --test projection_noninterference
```

Inspect fixture validity and non-vacuity before claiming the short-bag or
event cases cover all orders/lengths. The current short-bag test truncates a
state without asserting invariants or a changed secret; the event permutation
test lacks its own secret-change assertion. These are evidence gaps to review,
not proof of an actual client leak.

Chess has no hidden information. Its
[projection controls](../../../../games/chess/tests/projection_control.rs)
test deterministic projection and public moves; do not label them secrecy
noninterference.

Werewolf's [knowledge matrix](../../../../docs/games/werewolf.md) requires
different living/dead-seat, outside-spectator and Audit knowledge plus private
event non-existence. No reducer/projection/SecretModel implements it yet;
report pending claims. The spec also records state-version/ack metadata and
asymmetric voice gaps. Scope values alone cannot prove socket/SFU enforcement.
Respect Phase 4/7/8 gates instead of filling them during an audit.
