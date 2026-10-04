//! PHASE 4 — authoritative runtime composition; doc 03 §4–§9 (I-5/I-6/I-9/I-14).
//!
//! TODO(phase 4): wire tabula-registry erased dispatch, tabula-match actors and
//! tabula-storage implementations of ports. SQL and migrations stay in storage.
//! One match has one writer; persist ordered inputs before disclosed effects;
//! project/view_event are the only paths to client output. Build release-server
//! with panic=unwind and contain rule panics at the match boundary.
//! TODO(phase 5): wire lobby/presence and rule-driven chat scopes in this process.
//! Voice remains Phase 8; authentication does not grant a seat or private view.
