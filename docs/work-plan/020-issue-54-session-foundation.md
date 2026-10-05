# Issue #54 PR1 — durable session authority

**Status:** implementing the owner-authorized isolated exception in ADR-0035.

**Outcome:** checked internal session policy and an opt-in PostgreSQL adapter
provide durable expiry, verifier rotation, current-device revocation and account
invalidation, with real transaction/interleaving receipts.

**Why:** the accepted policy and service frames do not enforce current authority.
A UI flag or another skeleton cannot close that gap. A storage-first isolated
slice permits testing the dangerous ordering without activating credentials or
online games before their gates.

**Dependencies:** fresh develop `729417ffb6de4b376d2736bdfbf78c46f455630e`, accepted
ADR-0031/0034/0035. No dependency on unmerged Werewolf/assets PRs. Two subsequent
PRs wait for this PR's verified completion; unmerged predecessors require an
explicit stacked base.

**Review boundary:** exact identity keys, opaque redacted credentials, checked
session policy/ports, additive PostgreSQL schema and offline queries, real
non-empty migrations/lifecycle/race/fault tests. Existing aggregate/features/
targets and exact-head CI remain required.

**Risks / unknowns:** commit-order fencing does not fence private output; fixture
issuer+subject does not prove provider authentication; local DB is absent and
real tests run in ephemeral CI. Deployment-clock correctness and real provider
invalidation synchronization remain production gates.

**Non-goals:** production listener/migration/deployment, Kanidm provisioning,
registration/handle/agreement policy, profiles/friends, game authority, WS output,
secure stores, UI, voice, merging and phase exits.
