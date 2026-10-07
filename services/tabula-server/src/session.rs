//! PHASE 4 — session enforcement and match authority; ADR-0031/0034, doc 03 §21.
//!
//! TODO(phase 4, #54): resolve channel-bound opaque Tabula sessions using durable
//! authority owned by tabula-auth and ports implemented in tabula-storage.
//! Recheck expiry, revocation/account epoch and resource permission before reads,
//! writes, Attach, command commit and private outbound delivery; fail closed.
//! Define ordering against durable revocation across both services before enablement.
//! TODO(phase 4): issue memory-only match grants scoped to current subject,
//! session, epoch, match and allowed viewer; a Kanidm token is never a match grant.
//! Recheck seat/spectator permission; never admit client-requested Audit authority.
