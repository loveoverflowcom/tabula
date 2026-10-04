//! PHASE 4 — Kanidm OIDC flow; ADR-0034.
//!
//! TODO(phase 4, #54): discover the configured issuer, validate bounded metadata
//! and trusted endpoints, then use authorization code + state/nonce/PKCE.
//! Keep pending flows bounded, short-lived and single-use with exact callbacks.
//! Verify issuer, audience, signature/algorithm, nonce and expiry against trusted
//! keys; handle key rotation and provider failures without accepting stale authority.
//! TODO(phase 4, #54): resolve identity by (issuer, subject), not email or display
//! name. Provider credentials stay server-side, never in game URLs, Hello, browser
//! storage or the mobile GameHost bridge. No mock discovery/issued access tokens.
