# Issue #52 — permitted history → result → replay slice

**Status:** deferred by phase prerequisites.

**Outcome:** a supported game's recorded match can be found in real history,
opened as an authoritative result, and inspected through read-only,
viewer-permitted replay.

**Why:** local accepted-input capture and canonical testkit verification do not
provide persistence, account history, projected replay authorization or a scrub
viewer. Sample rows and a changing cursor would hide these missing owners.

**Dependencies:** Phase 4 persistence/protocol and projection/security evidence;
Phase 5 document shell and separate-document handoff; Phase 9 projected playback,
scrub and speed, or an accepted ADR explicitly changing that ordering. ADR-0028
authorizes discovery/setup only. See [#52](https://github.com/loveoverflowcom/tabula/issues/52)
and the [screen index](../../ui/screens/README.md) for the maintained contract.

**Review boundary:** one supported-game slice with real permission/version/resource
failures and hash/authority fixtures, then interrupted/repeated seek, keyboard,
focus/back, four themes, responsive layout and assistive-technology evidence.

**Risks / unknowns:** hidden inputs/seeds cannot reach the user; accepted input
indices may have gaps and cannot become cursor ordinals; permissions may expire
mid-read. Snapshot selection and projected folding must be established by their
real owners before rendering controls is claimed as playback.

**Non-goals:** ratings service, archive migrations, Xiangqi engine/tutor, AI
reconstruction, a second replay controller, or public canonical replay export.
