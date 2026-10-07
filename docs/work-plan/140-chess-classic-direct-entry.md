# Classic pieces and direct play entry

## Outcome

Implement the approved classic Chessnut pieces and simpler existing direct
create/join entry as a new draft PR from freshly fetched develop `face8a3e`.
The user reviews the draft; merge, deploy and issue closure remain outside it.

## Why

Familiar black/white silhouettes should remain readable at small board and
promotion sizes. Direct entry should make creating a match and joining a shared
code distinct, while preserving current session and server-owned admission facts.

## Review boundary

Game-owned source pieces, reproducible bounded atlases, versioned hashed pack,
retained rights and necessary generic staging consumers; existing opted-in web
direct-entry composition, native form semantics, safe status copy and lifecycle.
Shared Tabula branding, canonical rules/protocol/seat authority, existing local
play and closed production/queue/native gates stay with their current owners.

## Dependencies and unknowns

[Scoped verification](../verification/chess-classic-entry/README.md) separates
local semantic/build checks from actual browser/native evidence. PR112 is an
independent draft: reconcile its board-grain/material/motion pack only if both
are later selected; this change does not stack or merge it. Reserve pack 0.3.0
instead of assigning different art to PR112's 0.2.0 identity.

## Completion and next work

Finish local checks and independent review, publish coherent checkpoints and
verify the remote head/tree and draft target. Capture actual exact-source pixels
through the separately authorized dedicated graphics route where supported.
Real matchmaking remains in #55's gated sequence; artwork is not a queue API.
