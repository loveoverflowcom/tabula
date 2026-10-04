//! PHASE 4 — auth configuration; ADR-0034 and ADR-0031.
//!
//! TODO(phase 4, #54): validate typed TOML/env configuration once at startup:
//! bind address, trusted browser origin, Kanidm origin/issuer, client id,
//! callback allow-list, upstream timeouts/body limits and session-storage config.
//! Suggested env prefix: `TABULA_AUTH_`; deployment supplies URLs and branding.
//! Require verified TLS and explicit trusted issuers/callbacks; never derive them
//! from user input. Operator-managed secrets stay out of source, Debug and logs.
