# Tabula agent entrypoint

Read [AGENTS.md](AGENTS.md), then the architecture contract it names, before editing.
Use [tabula-code-review](.agents/skills/tabula-code-review/SKILL.md) for read-only review of
an existing PR, range, patch or local change; it composes the criteria below without fixing code.
Use [tabula-engineering](.agents/skills/tabula-engineering/SKILL.md) for engineering work and
compose [tabula-game-audit](.agents/skills/tabula-game-audit/SKILL.md) for games and shared
contracts affecting them. Load references on demand.

Compose [tabula-cmp-engineering](.agents/skills/tabula-cmp-engineering/SKILL.md) for
`apps/mobile` Compose UI, navigation, state/lifecycle and the existing MCP Desktop Preview
inspection loop. Its CMP criteria also compose with the read-only review workflow.

`.claude/skills` links to `.agents/skills`, the only maintained skill tree. If your runtime
does not discover symlinked skills, read the canonical files above directly. Do not recreate
independent copies. Discovery and migration checks are documented in the
[skill map](.agents/skills/README.md).
