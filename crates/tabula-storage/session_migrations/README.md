# Isolated session migrations

ADR-0035 permits these additive migrations for disposable PostgreSQL acceptance
validation only. `PgSessionStore::migrate` explicitly selects this directory;
neither production service loads it. The future general storage migrations stay
separate and phase-gated.

Provider identity is the exact issuer + opaque subject pair. Fixture linkage
does not verify Kanidm or OIDC. Each UTF-8 component is bounded to 1024 bytes so
the byte-exact composite unique key fits PostgreSQL's B-tree tuple limit without
truncation or hash-collision semantics. Credentials are persisted only as 32-byte digests;
the stable session context ID is non-authorizing metadata, never a CSRF token.
Actual CSRF issuance and checking remain gated. SQL constraints are a second boundary;
every loaded row also passes the session owner's checked domain conversion.

Schema/query changes require authentic SQLx metadata regenerated against real
PostgreSQL and committed, as doc 01 §1.2 requires. A pending preparation job or
handwritten metadata is not evidence of a compile-time checked query.

Native `session-postgres` uses SQLx 0.9 and requires Rust 1.94; the pinned
repository toolchain is 1.96. The default storage and deterministic SDK retain
the workspace 1.85 declaration. ADR-0035 explains the feature-specific policy
and the rejected SQLx 0.8 kernel-entropy unification.
