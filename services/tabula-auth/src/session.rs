//! Session ownership after ADR-0038's isolated browser implementation.
//!
//! Verified exact provider identity and the epoch captured before redirect cross
//! the trusted BrowserLoginProvider port. tabula-session-http issues through the
//! injected durable SessionAuthority; tabula-storage owns atomic SQL and stores
//! only credential digests. Provider tokens never become Tabula credentials.
//! Browser HttpOnly cookie, current /me, rotation/logout and terminal expiry use
//! the existing ADR-0031/0036 adapter; this service owns no parallel session store.
//!
//! TODO(phase 4, #54): shipping native secure-store login and production security-
//! event synchronization/output fencing remain gated and have no fallback.
