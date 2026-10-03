# Rust property testing

Supporting technique for [Tabula engineering](../SKILL.md). The foundation owns the overall
workflow, check status, and evidence report; use this reference only for the named technique.

A property test is a **law plus a generator plus a shrinker**. If any of the three is weak, the
test is decoration. Read the nearest `AGENTS.md` and the module's invariant docs before writing
one.

`proptest` is preferred over `quickcheck` for shrinking quality: when a 400-step sequence fails,
shrinking to the three steps that matter is the difference between a fixable bug and a shrug.

## Gate

Reach for a property test when **all** hold:

- the input space is too large to enumerate but small enough to sample usefully;
- you can state the law **without calling the code under test as its own oracle**;
- a failure would be actionable after shrinking.

Do **not** reach for it when:

- the whole space is manageable to enumerate — write the loop (see
  [replay / differential testing](replay-differential-testing.md));
- published reference data exists — use it (perft, published vectors);
- a large arithmetic domain needs exhaustiveness — consider [Kani](kani.md);
- the input is untrusted **bytes** and the claim is "does not crash" — use [fuzzing](fuzzing.md).

## Write the law first

| Law | Shape | Typical target |
|---|---|---|
| Round trip | `decode(encode(x)) == x` | canonical encoding, wire types |
| Canonical form | equivalent values encode to identical bytes | anything hashed |
| Idempotence | `f(f(x)) == f(x)` | normalisation, migrations, re-applied effects |
| Invariant preservation | `valid(s) && accepted(step) => valid(s')` | reducers, rules |
| **Transactional rejection** | `rejected(step) => bytes(s') == bytes(s)` | any `&mut`-taking reducer |
| Determinism | same inputs ⇒ identical output **bytes** | replayable systems |
| Replay equivalence | `live(seq) == replay(record(seq))` | event-sourced systems |
| **Noninterference** | changing only hidden data does not change an unauthorised output | projections, redaction, security boundaries |
| Metamorphic | a known input transformation implies a known output relation | when no oracle exists |
| Model agreement | optimised impl == small reference model | resolvers, schedulers |
| Symmetry | relabelling equivalent actors relabels the result | seat/player identity |

Say the law in one sentence in the test name: `rejected_apply_leaves_canonical_bytes_unchanged`,
not `test_apply_2`.

## Reachable state vs arbitrary state

**They are different generators for different claims. Choosing wrong wastes the test.**

| Generator | Use for | Wrong for |
|---|---|---|
| **Arbitrary / representable** — anything the type can hold, including nonsense | robustness: no panic, decoder rejects garbage, no out-of-bounds indexing, hostile-wire-value handling | semantic laws — asserting a rule over a board with three kings proves nothing and produces false failures |
| **Reachable** — produced only by the system's own legal transitions | semantic laws: invariant preservation, transactional rejection, noninterference, `legal_commands` soundness | robustness — it never produces the malformed input you need |

**Generate reachable states cheaply by replaying a legal prefix.** If the domain exposes an
enumeration of legal actions, that is your generator:

```rust
fn reachable_state(len: usize) -> impl Strategy<Value = State> {
    (0..len).prop_map(|n| {
        let mut s = create_initial();
        for i in 0..n {
            let legal = legal_commands(&s, whose_turn(&s));
            if legal.is_empty() { break; }
            apply(&mut s, pick_deterministically(&legal, i), &mut ctx(i))
                .expect("an advertised legal command must be accepted");
        }
        s
    })
}
```

Note the important detail: **a type whose deserialization is unvalidated makes arbitrary states
reachable from the wire.** If `State` derives `Deserialize` with public fields, robustness
properties over arbitrary states stop being hypothetical — write them.

## Generators: bias, validity, and shrinking

- **Build valid values through public constructors**, never by filling fields. Otherwise the
  generator can produce states the type promises are impossible, and the failure is meaningless.
- **Generate invalid input separately and deliberately.** A generator that only emits legal actions
  cannot prove rejection behaviour. Mix: mostly-legal sequences with a stated hostile fraction.
- **Watch the bias.** `0..len` with a uniform length concentrates on short sequences; late-game
  states are where phase-transition bugs live. Weight toward longer prefixes explicitly.
- **The shrinker must preserve validity.** If shrinking a reachable state produces an unreachable
  one, the minimised counterexample is noise. Shrink the *action sequence*, not the state: rebuild
  the state from the shrunken sequence.
- Pin a measured case count for ordinary runs (often 64–256). Larger `PROPTEST_CASES` runs belong
  in an authorized bounded campaign or existing schedule; do not create a schedule implicitly.

## State machines

For reducers and game rules, generate **sequences**, not isolated states, and include every input
variant the state machine accepts — not just the obvious one. Timer, lifecycle, and admin inputs
are where the untested arms live.

At every step assert:

- accepted ⇒ the domain invariant still holds, and the version counter advanced by exactly one;
- rejected ⇒ canonical bytes unchanged, no events emitted, no effects emitted, counter unchanged;
- terminal reached ⇒ subsequent inputs behave as the contract says (usually rejected).

`proptest-state-machine` provides `ReferenceStateMachine` (the model: `State`, `Transition`,
`init_state`, `transitions`, `apply`, optional `preconditions`) and `StateMachineTest` (the system
under test: `init_test`, `apply`, `check_invariants`), driven by `prop_state_machine!`. Its
shrinker removes transitions from the end, simplifies them from the front, and minimises the
initial state.

**Adopt it only when hand-rolled shrinking has proved inadequate.** A plain
`proptest!` over `Vec<Input<Command>>` fed to an existing deterministic run-harness reuses code
that already exists and adds no dependency. Record the decision either way — "we added a
framework" is a recognised failure mode.

## Noninterference: the property that catches derived leaks

A containment scan asks *"do the secret's bytes appear in this view?"* — coarse, and it cannot see
a leak carried by a length, an ordering, or a count. Noninterference asks the stronger question:

```text
for a reachable state s, and s' = scramble_only_secret_parts(s, rng):
    canonical(project(s,  unauthorized_viewer))
 == canonical(project(s', unauthorized_viewer))
```

Preserve the viewer's authorized knowledge and public facts while scrambling secrets; some secret
changes legitimately alter a public count or revealed outcome. If projections differ under that
public equivalence, hidden data influenced the output. Add a secret-scrambling hook only where the
domain and phase support it. Pair this check with the containment scan; they catch different things.

## Properties to avoid

- **Re-implementing the transition in the test.** If the expected value comes from the same
  algorithm, the test asserts `f(x) == f(x)`.
- **Properties of your dependencies.** "Two different states hash differently" is a BLAKE3
  property.
- **Properties with no named detectable defect.** Ask what violation the assertion catches.
  A scoped [mutation campaign](mutation-testing.md) can evaluate assertion strength when that
  question justifies its cost; it is not mandatory for every new property.

## On failure

1. Record the seed and the shrunken counterexample.
2. **Commit the minimised case as an ordinary deterministic `#[test]`** before fixing anything. The
   property is the discovery mechanism; the regression test is the durable artefact and it runs on
   every PR.
3. Only then refactor.

## Worked example set for a deterministic game/rules runtime

Ranked by value, using the reachable-prefix generator above:

1. **Rejected apply leaves canonical bytes unchanged**, over reachable states and arbitrary hostile
   inputs. (Strictly stronger than a fixture-driven rejection check and than a bounded model check
   over two concrete states.)
2. **Ordered input streams yield identical state hashes** — run a generated sequence twice from
   scratch and compare bytes. Two independently constructed hash maps in one process get different
   seeds, so this is what actually catches nondeterministic iteration order.
3. **Every enumerated legal command is accepted by apply**, over reachable states — not just at the
   initial and final positions.
4. **Replay is deterministic**: `live(seq)` and `replay(record(seq))` produce the same final
   canonical state hash.
5. **Resource resolution is deterministic** and agrees with a naive reference resolver over
   generated manifests (see [replay-differential-testing](replay-differential-testing.md) for writing the reference).
6. **Projection noninterference**, once the domain exposes a secret model.

## Technique evidence details

Add these technique-specific fields to the foundation ledger/report when this method was used.

```text
Law:               <one sentence>
Generator:         reachable | arbitrary | mixed (hostile fraction f)
Cases:             actual executed N; larger scheduled/campaign count only if applicable
Shrinking:         shrinks the action sequence / the value; validity preserved: yes|no
Oracle:            <independent of the implementation how?>
Counterexamples:   committed as <test names>
Not covered:       <what this law does not say>
```
