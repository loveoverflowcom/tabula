# Issue 60 — remaining renderer and embedding evidence

Status: deferred evidence work; scoped RFC/prototype delivery is recorded in
[the execution ledger](../../verification/issue-60/README.md). GitHub
[#60](https://github.com/loveoverflowcom/tabula/issues/60) owns acceptance.

## Outcome and priority

Resolve the production renderer and runtime containment choices only after
comparable target evidence exists. ADR-0029 keeps Macroquad and ADR-011 while
allowing the isolated experiment. This follows the queue's #59 baseline slice;
it does not introduce a new production feature ahead of Phase 3 completion.

## Next small review boundaries

1. Execute Safari/WebKit lifecycle and the exact current asset/workload. Safari
   remote automation is disabled and no alternate WebKit binary is installed.
   Changing that user setting needs separate authorization. Record a target
   receipt rather than inferring browser support from a successful compile.
2. Replace the native HTTP measurement shim with an isolated Rust/WASM boundary,
   retaining Rust-owned presentation/rules and generated contract validation.
   Measure copy/serialization/input feedback before proposing production wiring.
3. Add a matching scripted Macroquad build and actual timed motion/input samples,
   first classifying host events. Match the Pixi workload/theme/DPI; measure
   wrapper cost separately and use comparable draw/completion boundaries.
4. Exercise audio unlock/preferences, navigation/back/deep link, resize/DPI and
   context recovery on each selected target. The present fixture has no audio
   and the existing iframe uses immutable compiled theme/motion settings.
5. Revisit the ADR with runtime quality, accessibility and operational evidence;
   complete visual parity, online command/session security and rollback design
   before any Phase 4/5 production integration proposal.

## Dependencies, risk and verification

Reuse the #59 verified atlas pack, permitted RenderList and checkpoint protocol.
ADR-011/0028, deps.toml, pure rules and projection privacy remain binding.
The prototype checkpoint guard prevents queued DOM activations crossing a
canonical transition; production intent identity/version semantics require
separate review. Board Reader/native accessibility and exact GPU allocation are
not established by prototype canvas labels, DOM handle counts or process RSS.
Each review unit runs meaningful focused negative/lifecycle checks, the core
gate, actual target checks, and saves raw receipts with source/build provenance.

Non-goals: production engine/route rewrite, second match socket, JS rules or
canonical state, plugin sandbox, new engines, or changing Safari preferences.
