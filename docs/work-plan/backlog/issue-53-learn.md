# Issue #53 — evidence-grounded Learn

**Status:** deferred until real current-position evidence consumers exist.

**Outcome:** permitted practice/analysis/postgame supports tiered structured
hints and evidence references, with explicit abstention and optional explanation.

**Why:** a chat answer or book citation cannot supply missing rules/engine
evidence. Missing LLM must preserve the actual engine/template path; missing
engine must not become a successful chat-generated evaluation.

**Dependencies:** [board/modes](issue-53-board-modes.md),
[engine/evidence](issue-53-engine-evidence.md) for tactical hints, licensed
source metadata for book knowledge, and [Learn contract](../../ui/screens/12-learn.md).
LLM/Savy connection is optional and explicitly chosen, not a prerequisite
for the game or engine/template slice.

**Review boundary:** structured hints first; optional explanation in a separate
provider change. Quality oracle covers supported/unsupported tactical claims,
wrong-position/stale citations, no-evidence abstention and tier reveal.
Permission loss clears cached hints; cancelled/late explanations cannot paint
new positions or send commands. Test no-LLM/offline/error/retry separately.

**Risks / unknowns:** evidence-reference formatting does not prove prose truth.
Book facts, finite engine evaluations and proven rule facts require different
labels and independent quality cases.

**Non-goals:** a generic AI/chat/multi-agent framework, automatic cloud fallback,
Savy ingestion port, new game IDs or book citations used as motif proof.
