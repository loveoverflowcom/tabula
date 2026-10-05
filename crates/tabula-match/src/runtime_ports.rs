//! Offline actor ports. ADR-0039 narrows these to in-memory acceptance.
//!
//! `Authority` covers synchronous apply/submission, not durable commit or
//! eventual buffered delivery. Production adapters need their own review.

use tabula_core::{InputIndex, LogicalTime, MatchId, StateHash, StateVersion};
use tabula_game_api::Effect;
use tabula_protocol::ServerEnvelope;
use tabula_registry::runtime::ClientViewer;

use crate::runtime::Binding;

/// The actual synchronous boundary being authorized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Apply(tabula_core::SeatId),
    Observe(ClientViewer),
    Receipt(ClientViewer),
}

/// Host-owned current session/seat authority, resolved outside game rules.
///
/// The implementation must serialize revocation and `action` in one ordering
/// domain, fail closed, and never invoke `action` twice. It must not await or
/// recursively acquire itself. It cannot claim a database or socket fence.
pub trait Authority: Send + Sync + 'static {
    fn with_current<T>(
        &self,
        binding: &Binding,
        purpose: Purpose,
        action: impl FnOnce() -> T,
    ) -> Result<T, AuthorityLost>;
}

/// Public-safe authority loss. No credentials or diagnostic details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityLost;

/// One canonical journal record, never a client frame (I-5/I-8).
#[derive(Clone, Debug)]
pub struct JournalRecord {
    pub match_id: MatchId,
    pub index: InputIndex,
    pub version: StateVersion,
    pub now: LogicalTime,
    /// Empty at creation, otherwise canonical `Input<Command>`.
    pub input: Vec<u8>,
    pub events: Vec<Vec<u8>>,
    pub hash: StateHash,
}

/// Atomic append of one input/events/version/hash. A known-success receipt only.
///
/// Failure includes indeterminate commit. The actor stops instead of retrying.
/// In-memory implementations establish ordering, not disk durability.
pub trait Journal: Send + Sync + 'static {
    fn append(
        &self,
        record: JournalRecord,
    ) -> impl std::future::Future<Output = Result<(), RuntimePortError>> + Send;
}

/// Synchronous bounded output submission, called inside current authority.
///
/// Implementations must return immediately with Busy for a full queue. They
/// must not retain canonical state/events or send diagnostic side channels.
pub trait Output: Send + Sync + 'static {
    fn submit(&self, binding: &Binding, frame: ServerEnvelope) -> Result<(), RuntimePortError>;
}

/// Execute every game request only after its journal receipt, with a stable key.
///
/// This slice supplies faithful test ports; real timers/chat/voice/bots are gated.
/// Implementations own request-specific idempotence and may not await the actor.
/// A future Notify/chat/voice/bot adapter must separately fence current audience,
/// seat and session authority at its actual effects; Output's guard and a stable
/// committed key do not grant delivery rights. Offline fakes only record requests.
pub trait Effects: Send + Sync + 'static {
    fn execute(
        &self,
        match_id: MatchId,
        index: InputIndex,
        effects: Vec<Effect>,
    ) -> impl std::future::Future<Output = Result<(), RuntimePortError>> + Send;
}

/// No port diagnostics enter the client protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimePortError {
    Unavailable,
    Busy,
    Indeterminate,
}
