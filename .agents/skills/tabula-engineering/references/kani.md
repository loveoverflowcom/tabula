# Rust Kani

Supporting technique for [Tabula engineering](../SKILL.md). The foundation owns the overall
workflow, check status, and evidence report; use this reference only for the named technique.

Kani is a bounded model checker (CBMC backend) for Rust. It answers **one** kind of question well:
*does this proposition hold for every value in this symbolic domain?* It answers nothing else, and
a passing harness is worth exactly the proposition it states.

Read the nearest `AGENTS.md` and the repository's normative architecture before adding a harness.
Kani is a development-only technique. Inspect repository support through the foundation before
assuming any harness runs locally, in CI, or as a merge gate.

## Gate: is Kani the right tool?

Answer all four **yes** before writing a harness.

1. **Is the domain too large to enumerate?** `u64` arithmetic: yes. A 9-cell board and two `u8`s
   (65 536 cases): **no — write the loop.** A 3⁹ board space: borderline, and the loop is usually
   still faster to write and to run.
2. **Is there an independent oracle inside the harness?** A proof whose "expected" value is the
   function under test proves nothing. Use `checked_*` against `saturating_*`, a closed-form
   against an iterative form, or an explicit postcondition.
3. **Is the target reachable without stubbing out the thing you care about?** If the proposition
   only holds because you stubbed the code that would break it, the harness is theatre.
4. **Is the cost proportional?** Record solver time and compare it to the repository's actual
   ordinary gate budget. Put an expensive harness in an explicitly scoped deeper run.

If any answer is no, reconsider [verification selection](verification-testing.md). Common
better answers: an exhaustive loop test, published reference data
([replay-differential-testing](replay-differential-testing.md)), or a property test ([property-testing](property-testing.md)).

## Formulate the proposition before writing code

Write this comment above the harness, then implement it:

```text
Proposition P:   for all x in D, f(x) satisfies Q(x)
Domain D:        which values are symbolic, and their exact types
Assumptions A:   every kani::assume, and why each is not hiding a real case
Bound B:         unwinding bounds, container sizes, loop limits
Reached:         which production functions execute
Stubbed:         what is replaced, and why the replacement cannot mask a failure of P
```

If you cannot write the domain line precisely, the harness is not ready.

## Avoid vacuity — the failure mode that looks like success

Kani reports `SUCCESS` for a property that "may hold *vacuously* ... because the property is
unreachable, or because the harness is *over-constrained*". Three defences, in order of value:

1. **Prefer symbolic values with no `assume` at all.** `kani::any::<u64>()` over the full type is
   unconstrainable and therefore unvacuous.
2. **Add `kani::cover!` for every branch the proposition distinguishes.** A cover check reports
   `SATISFIED` when Kani found an execution reaching it; `UNREACHABLE` or `UNSATISFIABLE`
   "may indicate an incomplete proof". If your proposition is guarded by `if result.is_err()`, then
   `kani::cover!(result.is_err())` **and** `kani::cover!(result.is_ok())` are mandatory — otherwise
   a refactor that makes the function infallible turns the proof into a no-op and nothing tells you.
3. **Justify every `kani::assume` in one line.** An assumption that removes the difficult case is a
   bug in the harness, not a simplification. Prefer encoding the constraint in a `kani::Arbitrary`
   impl for a validated newtype, so the constraint is the *type's* invariant rather than the
   harness's.

## Bounds, honestly

- Kani inserts unwinding assertions; if they pass, the unwinding is sufficient for that harness.
  Do not silence them with `--default-unwind` to make a harness finish — that converts a real
  incompleteness into a silent one.
- `kani::any()` cannot bound a heap collection. Construct bounded collections explicitly and keep
  the bound very small; state the bound in the harness name or its doc comment.
- **The bound belongs in the name.** `preserves_state_for_boards_up_to_4_pieces` is honest;
  `preserves_state` is not.

## Stubbing safely

`#[kani::stub(path::to::real, path::to::model)]` (with `-Z stubbing`) replaces a function to keep
CBMC tractable. It is safe only when:

- the stub is on a path the proposition **does not constrain** (e.g. stubbing the commit half of a
  transition when the proposition is about the *rejected* path), **or**
- the stub is a total, obviously-correct model whose failure modes are separately covered.

Always write, next to the stub, one sentence naming which branch the stub is on and which harness
or test covers the stubbed behaviour. Mark the real function `#[cfg_attr(kani, inline(never))]` so
the interposition works.

## Keep the proof from drifting away from production code

The strongest anti-drift device is **exhaustive destructuring**: if the harness compares state
field by field, destructure the struct so that adding a field breaks compilation until someone
reviews the comparison.

```rust
fn canonical_fields_equal(before: &State, after: &State) -> bool {
    let State { board: b1, seats: s1, turn: t1, status: st1, timeout: to1 } = before;
    let State { board: b2, seats: s2, turn: t2, status: st2, timeout: to2 } = after;
    b1 == b2 && s1 == s2 && t1 == t2 && st1 == st2 && to1 == to2
}
```

Prefer comparing the **canonical encoding** where the real contract is byte-level; a field list is
a weaker statement than the contract if the contract says "byte-identical".

## Combine with the other evidence levels

Kani is one column in the evidence matrix, not a replacement for any other.

| Also needed | Why |
|---|---|
| ordinary semantic tests | keep a cheap regression in the usual suite; inspect actual CI wiring before assuming the Kani harness is run |
| reachable-state evidence when the claim needs it | a symbolic representable-state proof may not establish lifecycle reachability |
| scoped mutation testing when assertion strength is in question | exclude uncompiled `#[cfg(kani)]` modules from ordinary-test mutation runs (see [mutation testing](mutation-testing.md)) |
| differential/reference data | for domains where the specification lives outside the code |

## Reporting: the sentence you are allowed to write

**Never:** "crate X is formally verified", "R2 is proven", "the RNG is verified".

**Always:**

```text
Harness `H` proves proposition P for symbolic domain D, under assumptions A, with bound B.
Production functions reached: ...
Stubbed: ... (on the <accepted|rejected|...> path only)
Not covered: ...
Solver time: Ns
```

Put that block in the harness's doc comment **and** in the PR description. The summary that
survives into a roadmap is the one you wrote, not the one in the code.

## Historical examples and candidate propositions

These describe earlier observations, not the current capability inventory. Inspect the live
implementation and harness before treating any example as existing evidence or a missing task.

**Good — keep as-is.** `tabula-core::time::verification::millis_from_secs_is_exact_or_saturates`:
`seconds: u64` fully symbolic (2⁶⁴), no assumptions, no stubs, oracle is `checked_mul`, 18 s.
Also `logical_time_plus_is_exact_or_saturates` and `logical_time_since_never_wraps` over 2¹²⁸
pairs, in under half a second each. No test can do this.

**Narrow — state the scope.** The tic-tac-toe `rejected_*_place_preserves_state` harnesses prove
R2 for **two concrete states** over `(SeatId(u8), cell: u8)` = 65 536 cases, comparing five listed
fields, with `commit_place` stubbed (safely — it is unreachable on the rejected path). An
exhaustive loop test covers the same domain in microseconds *and* compares canonical bytes. Keep
the harness only if you upgrade the state itself to symbolic; add `kani::cover!(result.is_err())`
either way.

**Ceremonial — retire or relabel.** `concrete_opening_place_is_accepted` has an empty symbolic
domain: it is a unit test running under CBMC for 16 seconds. If you keep it, call it a
tractability canary, not a proof.

**Candidate arithmetic proposition.** `DetRng::below` computes `zone = 2^32 - (2^32 % n)` and loops until
a draw lands below it. Mutating `-` to `+` survives the whole test suite and silently restores
modulo bias; three other mutants make the loop **hang**. A harness over all `n: u32, n >= 2`
proving `zone % n == 0 && zone > 0 && 2^32 - zone < n` can cover a large finite domain with a
stub-free independent postcondition. This proves the rejection-zone arithmetic, not eventual
termination of an arbitrary RNG stream; state that excluded behavior explicitly.

## Running

```bash
cargo kani -p <crate>                 # all harnesses in a crate
cargo kani -Z stubbing -p <crate>     # when any harness uses #[kani::stub]
cargo kani -p <crate> --harness <name>
```

When a change affects an existing proved function, run its applicable harness if tool support and
task scope allow it, or record the gap. Place expensive runs in an authorized bounded campaign or
existing schedule. Report solver time and reachability alongside the scoped proof result.
