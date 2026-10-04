//! PHASE 4 — Tabula session lifecycle; ADR-0031/0034.
//!
//! TODO(phase 4, #54): after verified identity, mint channel-bound opaque sessions
//! with OS entropy; persist only credential digests behind tabula-storage ports.
//! Browser: __Host-tabula_session Secure/HttpOnly/SameSite=Lax, Path=/, no Domain.
//! Native: explicit bearer held in the OS secure store; no plaintext fallback.
//! Preserve ADR-0031's 30-minute idle and 24-hour absolute deadlines, atomic
//! credential rotation, account epochs and durable current-session logout.
//! TODO(phase 4, #54): define and test cross-service revocation/expiry fences with
//! tabula-server before enabling auth; unavailable storage/provider fails closed.
//! Profile/friends/presence, seat authorization and match grants are not owned here.
