//! PHASE 4 — proposed account API; doc 03 §2, ADR-0031/0034.
//!
//! TODO(phase 4, #54): compose /api/v1/auth/{context,login,register,logout,refresh}
//! and the Kanidm OIDC start/callback routes under the app's trusted HTTPS origin.
//! No rendered UI, CSS, generic provider registry or localStorage authentication.
//! Enforce exact browser Origin + JSON + context-bound CSRF before credential
//! effects; native bearer requests must not acquire ambient cookie authority.
//! TODO(phase 4, #54): apply no-store, generic anti-enumeration results, rate/body
//! limits and bounded safe return routes; never leak secrets or raw provider errors.
//! Register fields/agreement and duplicate-submit behavior need their own contract.
