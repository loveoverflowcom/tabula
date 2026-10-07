# Isolated online admission migrations

The online adapter explicitly composes session_migrations, match_migrations and
this additive set with Migrator::with_migrations, preserving every original
version, SQL and checksum. Unknown/missing versions and checksum drift fail.
No ignore_missing, live rollout or automatic production migration is used.
The separate narrower adapters must not migrate this composed database afterward.

All three schemas resolve within the same PostgreSQL search path. Authority uses
the existing relation-OID account advisory key, account row, session row, room,
then journal head locks. A deferred constraint checks actual COMMIT-time expiry
in addition to locked credential/context/epoch and durable seat checks. Accepted
activity shares the journal transaction. Plain seeds remain isolated test data.

Apply uses a separately committed, bounded account exclusion plus a live
account/session/resource transaction. The exclusion owns a dedicated physical
close-on-drop backend and cannot leak pooled session advisory locks. The
composed pool needs at least two connections per simultaneous apply guard.
Capacity-changing creation/join/initialization use directory-before-room order;
started rooms cannot return to waiting through this adapter. Rejected join work
uses a savepoint: durable attempts/authority observations remain, while failed
seat/admission changes roll back. Successful joins and initialization also clamp
COMMIT to the original code deadline.

A failed authenticated write rolls back and re-observes the actual committed
current credential without activity before returning the original failure,
which stops the owner. Under the trusted deployment-clock assumption, this persists
terminal expiry that the rejected transaction could not preserve. A clock
regression between the rolled-back deferred sample and that new observation,
or a failed independent observation, does not establish durable late-expiry
persistence. No resilient-clock/clock-correction proof is claimed; ADR-0036's
trusted-clock limitation and all production activation gates remain. Accepted
activity and every canonical/ledger write remain one atomic transaction; the
failure observation never creates accepted activity or speculative output.
