//! Canonical event definitions for Werewolf. (doc 02 §12.3, doc 08 §5.2)
//!
//! Canonical events record authoritative match transitions in the append-only log.
//! Client redaction and visibility (`view_event`) are handled in W7; at this layer,
//! events represent unredacted audit facts.
//!
//! @ai.role domain-types
//! @ai.domain werewolf.rules.event
//! @ai.pure true

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tabula_core::{LogicalTime, SeatId, TimerId};

use super::role::Role;
use super::state::Phase;

/// Authoritative canonical events emitted by the Werewolf rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// Initial secret role assignment across all roster seats.
    ///
    /// This event is server/audit-only in the canonical log (I-5, I-6);
    /// client non-observability is enforced by `view_event -> None` in W7.
    RolesAssigned { roles: BTreeMap<SeatId, Role> },
    /// A match phase has begun.
    PhaseChanged {
        phase: Phase,
        round: u32,
        timer_id: TimerId,
        ends_at: LogicalTime,
    },
}
