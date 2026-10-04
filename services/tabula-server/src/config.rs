//! PHASE 4 — validated process configuration; doc 01 §1.2 and doc 06 §3.3.
//!
//! TODO(phase 4): load typed TOML/env config through figment; validate once at boot.
//! Include bind address, explicit trusted browser origin, DB pool/statement limits,
//! WS bounds and drain deadline. Reject unknown/invalid configuration.
//! TODO(phase 4, #54): configure the trusted auth boundary under ADR-0034; do not
//! infer trust from Host/forwarded headers or place Kanidm credentials here.
