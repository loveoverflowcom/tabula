//! PHASE 4 — proposed HTTP composition; doc 03 §2 and ADR-0034.
//!
//! TODO(phase 4): compose Axum routes for registry catalog, matches and ops.
//! Use bounded inputs, RFC 9457 errors and resource-POST idempotency.
//! /readyz must prove storage reachability, migrations and registry readiness.
//! TODO(phase 4, #54): protect /api/v1/me with current Tabula session authority;
//! /api/v1/auth/* is owned by tabula-auth via same-origin reverse proxy routing.
//! TODO(phase 5, #54): add profile mutations, friends/invites and timestamped
//! presence only after typed ownership, privacy and permission contracts exist.
//! TODO(phase 5): compose lobby/queue/admin routes without branching on game ids.
