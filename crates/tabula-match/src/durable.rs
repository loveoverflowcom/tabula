//! SQL-free server-only journal boundary (doc 03 §9, ADR-0040).
//!
//! The contract is isolated in `tabula-match-journal` so enabling an actor's
//! registry/game features cannot unify into storage or authentication graphs.
pub use tabula_match_journal::*;
