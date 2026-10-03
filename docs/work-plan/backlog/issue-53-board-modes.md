# Issue #53 — Xiangqi board and modes

**Status:** deferred by #48 reconciliation and Xiangqi rules gates.

**Outcome:** one real Xiangqi rules/projection authority supports local Play
and a separate permitted Analyze branch with no-engine/no-LLM states.

**Why:** no `games/xiangqi` exists at `b16dd2e`; mock positions and existing
Chess/Tiles launchers cannot supply it. Doc 07 still defers Xiangqi to Phase 9,
doc 02 retains No ML in MVP, and ADR-0028 opens discovery/setup only.

**Dependencies:** [#48 vision/ADR](https://github.com/loveoverflowcom/tabula/issues/48)
and ruleset/version/ownership decision; real SDK rules, projection, notation,
replay/conformance evidence; supported branch reconstruction and generic
projected replay gate where used. See [#53 shared contract](../../ui/screens/xiangqi.md).

**Review boundary:** one supported module/presenter slice. Every executed
human/bot/branch move uses rules validation; original match bytes/history/
outcome remain immutable through candidates and mode switches. Exercise
permission loss, illegal/rejected command, dirty branch/back, unsupported
platform and keyboard/interruption/responsive/a11y in real consumers.

**Risks / unknowns:** Xiangqi repetition/perpetual-check rules and safe public
position reconstruction are undecided. `View` cannot be assumed to contain
all canonical data. 390 dp artwork does not prove 44 dp board targets.

**Non-goals:** engine I/O, AI scores/PV, pack install, LLM/chat framework,
dynamic plugin loader or silent phase exception.
