//! Effects — the game's only way to ask the platform for something.
//! (doc 00 §6.4, doc 03 §7.1)
//!
//! Games cannot call the platform. They **return requests**, which the shell
//! executes *after* the state transition is persisted.
//!
//! ## Every effect must be idempotent
//!
//! Crash recovery re-runs them. The mechanism per effect (doc 03 §7.1):
//!
//! | Effect | Why re-running is safe |
//! |---|---|
//! | `SetTimer` / `CancelTimer` | Timers are re-derived from state; re-arming is a no-op |
//! | `SetChatScopes` / `SetVoiceScopes` | **Absolute, not delta** — re-applying sets the same scopes |
//! | `EndMatch` | Guarded by `matches.ended_at IS NULL` in a single UPDATE |
//! | `RequestBotMove` | Keyed by `(match_id, seat, state_version)`; stale duplicates dropped |
//! | `Notify` | Keyed by `(match_id, audience, notice_id)` with a dedupe window |
//! | `Checkpoint` | Upsert by `(match_id, label)` |
//!
//! That "absolute, not delta" rule for scopes is the reason `SetChatScopes`
//! carries the whole scope map rather than a diff. Deltas are not idempotent.
//!
//! ## The list is additive-only
//!
//! Doc 09 §4: "Effect variants — additive only". Adding one is normal. Removing
//! or repurposing one breaks every stored replay that contains it.

use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use tabula_core::{Audience, MatchOutcome, Millis, SeatId, TimerId};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Effect {
    /// Ask the platform to deliver `Input::Timer { id }` after `delay` of logical
    /// time. Re-arm on every relevant transition; the runtime deduplicates by id.
    SetTimer {
        id: TimerId,
        delay: Millis,
    },

    CancelTimer {
        id: TimerId,
    },

    /// Terminal. Emit **exactly once** per match — the testkit checks this.
    /// The platform records the outcome, applies rating effects, and stops
    /// accepting inputs.
    EndMatch {
        outcome: MatchOutcome,
    },

    /// Who may speak and listen on which channel, right now.
    ///
    /// The chat *transport* is platform; the *scoping* is game-driven (ADR-022).
    /// Werewolf makes this a core rule; chess never sends it.
    SetChatScopes(ChatScopes),

    /// Which voice rooms exist this phase and who is in them.
    /// The game never touches a socket or an SFU. (doc 02 §12.3)
    SetVoiceScopes(VoiceScopes),

    /// Ask the bot runner for a move for this seat.
    ///
    /// The bot's answer comes back as an ordinary `Input::Player` and goes through
    /// the same `apply` — it can be rejected like anyone else. (doc 00 §6.5)
    RequestBotMove {
        seat: SeatId,
        deadline: Millis,
    },

    /// A user-facing notice, localised by key on the client.
    Notify {
        audience: Audience,
        notice: Notice,
    },

    /// Persist a durable marker ("hand 7 complete") for analytics and resume UX.
    Checkpoint {
        label: CheckpointLabel,
    },
}

/// Chat permissions per channel. **Absolute, not a delta.**
///
/// TODO(phase 3): defined and exercised in tests when werewolf's rules land;
/// server-side enforcement is Phase 7. The builder shape sketched in doc 02 §12.3:
///
/// ```rust,ignore
/// ChatScopes::new()
///     .allow("table",  Speak::None,                 Listen::None)
///     .allow("wolves", Speak::Seats(&wolves_alive), Listen::Seats(&wolves_alive))
///     .allow("dead",   Speak::Seats(&dead),         Listen::Seats(&dead))
/// ```
///
/// The chat service then *refuses* a message from a seat without `Speak`
/// permission. Enforcement is at the socket, not in the UI.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChatScopes {
    pub channels: SmallVec<[ChannelScope; 4]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelScope {
    pub key: CompactString,
    pub speak: Participants,
    pub listen: Participants,
}

/// Voice rooms for this phase. The platform moves participants between SFU rooms.
///
/// TODO(phase 8): the wire representation is a Phase 8 contract (doc 07 Phase 8).
/// The *shape* is fixed now so werewolf's rules can be written in Phase 3 against
/// something real.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VoiceScopes {
    pub rooms: SmallVec<[VoiceRoom; 4]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceRoom {
    pub key: CompactString,
    pub members: Participants,
}

/// A set of seats, expressed so that "everyone" and "nobody" are cheap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Participants {
    None,
    Everyone,
    Seats(SmallVec<[SeatId; 8]>),
}

/// A localised notice. Key + args, never raw server text — the client localises.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notice {
    pub key: CompactString,
    pub args: SmallVec<[(CompactString, CompactString); 2]>,
}

/// Label for [`Effect::Checkpoint`]. Game-defined, stable, and public-safe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointLabel(pub CompactString);
