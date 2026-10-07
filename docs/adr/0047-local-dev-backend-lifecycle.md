# ADR-0047: local/dev backend composition and lifecycle sequence

- **Status:** accepted bounded local/dev implementation; acceptance tracked separately
- **Date:** 2026-10-07
- **Owner request:** [issue #110](https://github.com/loveoverflowcom/tabula/issues/110), PR01
- **Amends:** ADR-0034/0036/0038/0040/0041/0042/0044 local runtime gates only
- **Invariants:** none relaxed; I-1, I-5/I-6, I-7/I-8, I-9, I-13/I-14 remain enforced

Issue #110 authorizes executable local/dev service composition and subsequent
room/result lifecycle work. This PR opens the composition first. A skeleton,
fixture-only launcher or readiness response without real authority does not meet
that request. Production provisioning/deployment and Phase 4/5 exits remain separate.

The non-default native `local-dev` feature and explicit `serve --config` command
open the existing service leaves. Both require typed validated TOML/environment
configuration, `mode = "local-dev"`, loopback listeners, and one exact HTTPS
localhost browser origin behind a local TLS edge. Default and WASM bootstraps
keep refusing startup. No service depends on another service.

Kanidm owns credentials, MFA and OIDC. `tabula-auth` discovers the configured real
provider and adapts verified login/enrollment to the existing durable session
authority. The edge routes provider login/callback and session mutations there.
`tabula-server` composes current session context, profile/social authority and the
registry-backed direct-match gateway. Both share PostgreSQL authority, rather
than copying session facts across processes. Public handlers retain their exact
Origin/CSRF, cookie/native-bearer, account/resource and publication fences. No
fixture identities, test fault controls or private audit HTTP routes are enabled.

SQL and pool configuration stay in `tabula-storage`. The local/dev adapter checks
the combined known session/account/social/match/admission migration history and
required schema surfaces. `schema_policy = "check"` writes nothing; `"apply"`
explicitly applies the same combined migration set to an operator-selected dev
database. It never resets a database or discards unknown history. The caller
must choose a development database; this scope does not authorize live migrations.
Enrollment policy changes require a separate explicit operator CLI command.

`/healthz` reports process liveness. `/readyz` checks live database/schema
availability; gameplay additionally requires a real supported registry loaded at
boot and stops readiness when draining. Provider discovery is verified before
the auth listener opens; readiness does not claim ongoing provider availability.
Failures use closed labels and omit URLs, credentials, provider tokens and state.

Shutdown stops accepting HTTP, rejects gateway admission, waits for existing owned
work, then queues the actor's ordered drain and observes owner cleanup within one
configured deadline. Deadline/failure exits report incomplete drain; they do not
report success or infer that an ambiguous operation failed. Accepted inputs,
immutable seats and duplicate watermarks remain in the journal for exact lazy
recovery. This does not add a timer/effect executor or modify game rules/wire types.

Explicit capacities remain inside the previously reviewed hard bounds. In
particular, admission currently has a **lifetime dataset** limit, separate from
the process-local owner/request budgets. Exhaustion has a named error; increasing
constants or deleting history is not a reclaim strategy. PR02 will separate
active resources from history and implement authorized retirement.

PR02's Room/ready/start and PR03's result/history/rematch are authorized by the
issue, but are not implemented or claimed by PR01. Their implementations must
record the concrete authority, concurrency and crash-boundary decisions and
evidence. Native/mobile runtime, clocks/private effects, voice, distributed
placement, hosted provider provisioning and broad phase exits stay gated.

The [runbook](../local-dev-backend.md) provides application service commands and
the [PR01 ledger](../verification/issue-110-pr01/README.md) records source identity,
checks and limits. Browser/real-provider/PostgreSQL execution and crash/restart
continuity remain distinct evidence; a source review or compilation cannot
replace them.
