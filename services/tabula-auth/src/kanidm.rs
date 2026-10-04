//! PHASE 4 — provider-specific credential adapter; ADR-0034.
//!
//! TODO(phase 4, #54): implement bounded typed Kanidm operations over verified TLS.
//! Kanidm owns passwords, credential policy, account creation and OIDC signing;
//! no local password database, Tabula signing of provider tokens or password logs.
//! If app forms submit passwords, forward them only here over TLS and retain them
//! only for that request. Do not copy VOT's legacy routes or presentation coupling.
//! TODO(phase 4, #54): give onboarding only operator-provisioned least privilege,
//! handle partial registration/recovery honestly and normalize unknown-account vs
//! wrong-password failures. Disallow automatic redirects; validate any allowed hop.
