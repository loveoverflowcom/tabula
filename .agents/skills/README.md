# Agent skills

Skills an agent loads before doing a class of work in this repository. Each is a `SKILL.md` with
YAML frontmatter (`name`, `description`); `references/` holds material read only on demand, and
`agents/openai.yaml` carries the display metadata.

`docs/architecture/00-architecture-principles.md` is normative. A skill that disagrees with doc 00
is a bug in the skill.

## Map

```text
                    rust-verification-testing          ROUTER
                    choose the cheapest adequate oracle;
                    name the evidence level; never say "verified"
                                  │
     ┌───────────────┬────────────┼────────────┬───────────────────┐
     ▼               ▼            ▼            ▼                   ▼
rust-property-   rust-replay-  rust-        rust-kani         rust-fuzzing
   testing       differential- mutation-    bounded model     untrusted bytes:
laws, generators, testing      testing      checking with     panics, hangs,
state machines,  independent   assertion    an honest scope   resource
noninterference  oracles       strength     statement         exhaustion

              PREVENTION                              DOCUMENTATION
   rust-types-as-proofs                          rust-ai-doc-contracts
   invalid states unrepresentable                durable law → evidence links
   rust-functional-core
   architecture that makes verification cheap
```

## Which one, quickly

| You are about to… | Load |
|---|---|
| decide *what evidence* a claim needs | `rust-verification-testing` (always start here) |
| write a law over generated inputs, or a state-machine test | `rust-property-testing` |
| compare against a reference model, published data, a replay, or another target | `rust-replay-differential-testing` |
| find out whether the existing tests actually assert anything | `rust-mutation-testing` |
| prove something over an unenumerable symbolic domain | `rust-kani` |
| harden a decoder against attacker-supplied bytes | `rust-fuzzing` |
| model domain data, add validation, or design a lifecycle | `rust-types-as-proofs` |
| decide where a function belongs, or untangle rules from I/O | `rust-functional-core` |
| link an invariant to the test that evidences it | `rust-ai-doc-contracts` |

## Not present, deliberately

| Tool | Why not | Trigger to add |
|---|---|---|
| Miri | The workspace is `#![forbid(unsafe_code)]` with no FFI; Miri's value for pure safe Rust is close to zero. | An approved `unsafe` ADR, or a C/system dependency entering the graph. |
| Loom | There is no concurrent code yet — `tabula-match` is a doc comment and `tokio` is in no compiled crate. | The first real match actor: model the idempotency cache, the bounded mailbox, and the drain interaction. |
| A separate model-based-testing skill | State-machine modelling lives in `rust-property-testing`; reference-model comparison lives in `rust-replay-differential-testing`. Splitting it would divide one decision across two files. | — |

## Rules for changing these skills

1. **No ceremonial skills.** A skill must change what an agent does. If it only restates general
   Rust advice, delete it.
2. **Keep each one followable in one sitting.** Roughly 150–180 lines. Overflow goes to
   `references/`, which is read on demand.
3. **Examples come from this repository.** A worked example an agent can go and read beats a
   generic snippet.
4. **Cross-reference, do not duplicate.** One home per topic; siblings link to it.
5. **State scope honestly.** Every skill that produces evidence must say what its evidence does
   *not* cover. This is the habit the whole set exists to install.

Background for the current shape, including the measurements that motivated the split:
[`docs/research/develop-architecture-verification-audit.md`](../../docs/research/develop-architecture-verification-audit.md) §21.
