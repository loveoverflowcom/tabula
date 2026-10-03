# Tabula agent entrypoint

Read [AGENTS.md](AGENTS.md), then the architecture contract it names, before editing.
Use [tabula-engineering](.agents/skills/tabula-engineering/SKILL.md) for engineering work and
compose [tabula-game-audit](.agents/skills/tabula-game-audit/SKILL.md) for games and shared
contracts affecting them. Load references on demand.

`.claude/skills` links to `.agents/skills`, the only maintained skill tree. If your runtime
does not discover symlinked skills, read the canonical files above directly. Do not recreate
independent copies. Discovery and migration checks are documented in the
[skill map](.agents/skills/README.md).
