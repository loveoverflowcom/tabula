//! `GameCapabilities` — declarative facts the platform needs to run a game safely.
//! (doc 02 §4.2, §5)
//!
//! # The anti-bloat contract
//!
//! > This is the most over-designable type in the platform.
//!
//! **Every field must be consumed by a named platform subsystem, today or in a
//! named phase. A field with no consumer is deleted at review.** The table below
//! is that contract; keep it current or the discipline evaporates.
//!
//! | Field | Consumed by | Used for |
//! |---|---|---|
//! | `seats.min/max/allowed` | lobby, matchmaking, room UI | Room validation, queue bucketing |
//! | `seats.teams` | lobby, ratings | Team formation, `TeamElo` |
//! | `seats.symmetric` | matchmaking | Side alternation / fairness |
//! | `seats.fill_with_bots` | lobby, match runtime | Auto-fill on queue timeout |
//! | `turn_model` | client shell, presence, idle detection | "Your turn" badges, notification policy, AFK thresholds |
//! | `hidden_information` | match runtime, ops tooling | Enables strict projection auditing; forbids naive send-state-to-all debug paths |
//! | `spectators` | gateway, match runtime | Attach authorisation + delay buffering |
//! | `chat` | chat service | Which channels to create; whether to await `SetChatScopes` |
//! | `voice` | voice service, client UI | Whether to provision a room; whether to prompt for mic |
//! | `ranked` | rating service, matchmaking | Whether outcomes affect the ladder; which algorithm |
//! | `async_turns` | match runtime, notifications | Whether to hibernate the actor and push instead |
//! | `reconnect` | gateway, match runtime | Grace timers; whether to inject `Input::Seat` |
//! | `substitution` | lobby, bot runner | Whether bot takeover is offered |
//! | `pausable` | match runtime, admin | Whether `Admin(Pause)` is accepted |
//! | `durability` | match runtime | Ack point relative to log commit |
//! | `client_preview` | client | Whether to render optimistic previews |
//! | `state_size` | snapshot policy | Snapshot cadence and storage target |
//! | `apply_budget` | match runtime observability | Warn when a game's apply is slow |
//! | `max_match_duration` | match runtime | Hard stop for runaway matches |
//!
//! # Why this type exists at all
//!
//! It is how the platform absorbs the difference between 2-seat chess and 20-seat
//! werewolf **without branching on `game_id`** (I-9). Every variation in doc 02
//! §12.5 is either a field here or a behaviour behind the same five functions.

use serde::{Deserialize, Serialize};
use tabula_core::{BotLevel, Millis};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameCapabilities {
    pub seats: SeatSpec,
    pub turn_model: TurnModel,

    /// Turns on strict projection auditing and forbids debug paths that would
    /// send canonical state anywhere. Games with this set **must** provide a
    /// `SecretModel` (doc 02 §7.3).
    pub hidden_information: bool,

    pub spectators: SpectatorPolicy,
    pub chat: ChatPolicy,
    pub voice: VoiceRequirement,
    pub ranked: RankedSupport,
    pub async_turns: AsyncTurnPolicy,
    pub reconnect: ReconnectPolicy,
    pub substitution: SubstitutionPolicy,
    pub pausable: bool,
    pub durability: Durability,

    /// False means the client shows a spinner instead of an optimistic preview.
    /// Set it false when hidden information affects legality, so the client
    /// cannot honestly predict the answer. (doc 00 §4.1)
    pub client_preview: bool,

    pub state_size: StateSizeClass,
    pub apply_budget: Budget,
    pub max_match_duration: Option<Millis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeatSpec {
    pub min: u8,
    pub max: u8,
    /// Some counts may be illegal in between — werewolf role sets are only
    /// balanced at particular player counts.
    pub allowed: SeatCounts,
    pub teams: Option<TeamSpec>,
    pub fill_with_bots: bool,
    /// `false` for chess (white has an advantage), so matchmaking alternates
    /// colours across a series.
    pub symmetric: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SeatCounts {
    Range { min: u8, max: u8 },
    Exact(Vec<u8>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeamSpec {
    pub teams: u8,
    pub seats_per_team: u8,
}

/// How the platform should present and police turn-taking.
///
/// The platform reads this **only** for UI affordances, idle thresholds, and
/// notification policy. Actual turn order lives in the game's own state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnModel {
    /// Exactly one seat may act at a time — a clear "your turn" affordance is
    /// possible. Chess, cards, tiles.
    StrictSequential,
    /// Many seats act in the same window: werewolf night, simultaneous bidding.
    Simultaneous,
    /// Who may act depends on the phase. Werewolf overall.
    Phased,
    /// Anyone, any time. Party games.
    FreeForm,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum SpectatorPolicy {
    Forbidden,
    Live,
    /// Platform buffers by this much so a spectator cannot relay information to
    /// a player in real time. Ranked cards uses 30 s. (doc 02 §12.2)
    Delayed {
        by: Millis,
    },
    /// The game's `project(Spectator)` decides; the platform allows attach.
    /// Werewolf uses this: dead players see everything, outsiders see nothing.
    GameControlled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatPolicy {
    /// Channels the game knows about. The platform creates transport for these.
    pub channels: Vec<ChatChannelSpec>,
    /// If true, the game will send `Effect::SetChatScopes` and the platform must
    /// wait for it and enforce it. (ADR-022)
    pub game_scoped: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatChannelSpec {
    pub key: String,
    pub kind: ChatKind,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ChatKind {
    Table,
    Team,
    Dead,
    Whisper,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum VoiceRequirement {
    No,
    Optional,
    /// Werewolf. The social game *is* the voice channel.
    Recommended,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum RankedSupport {
    No,
    Yes { rating: RatingKind },
}

/// Platform-implemented, game-selected. Games never compute ratings (ADR-024).
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum RatingKind {
    Elo,
    Glicko2,
    TeamElo,
    /// Multi-seat finishing order — cards, tiles.
    Placement,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct AsyncTurnPolicy {
    pub supported: bool,
    /// How long a seat may sit on a turn before the platform emits `WentIdle`.
    pub turn_deadline: Option<Millis>,
    /// Total match TTL for async matches.
    pub match_ttl: Option<Millis>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct ReconnectPolicy {
    /// How long the platform holds the seat before emitting `Abandoned`.
    pub grace: Millis,
    /// Whether the game wants `Disconnected`/`Reconnected` inputs at all.
    /// Chess says yes (and then does nothing, so the clock keeps burning).
    pub notify_rules: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SubstitutionPolicy {
    /// Werewolf. The seat carries secret knowledge; handing it over would leak
    /// or destroy the social game. (doc 02 §12.3)
    Forbidden,
    BotOnly {
        levels: Vec<BotLevel>,
    },
    HumanOrBot,
}

/// When does the server ack the player's command? (doc 03 §8.3)
///
/// **A capability, not a global setting.** Ranked chess wants durability; casual
/// werewolf wants snappy voting.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Durability {
    /// `apply → Ack + broadcast → append (batched)`. p95 ~5 ms.
    /// Loss window: up to 50 ms of events on a hard kill; recovery replays from
    /// the last committed input and attached clients get a `Resync`.
    AckAfterApply,
    /// `apply → append → commit → Ack → broadcast`. p95 ~25 ms same-region.
    /// No loss window. **Required for ranked and anything with stakes.**
    AckAfterPersist,
}

/// Drives snapshot cadence and storage target. (doc 03 §9.2)
///
/// | Class | Interval | Storage |
/// |---|---|---|
/// | `Tiny` (< 1 KiB) | every 200 inputs + on end | Postgres `BYTEA` |
/// | `Small` (< 16 KiB) | every 100 inputs + on end | Postgres `BYTEA` |
/// | `Medium` (< 256 KiB) | every 50 inputs + on end + on hibernate | `BYTEA`, zstd |
/// | `Large` (≥ 256 KiB) | every 25 inputs + on hibernate | Object storage + pointer row |
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateSizeClass {
    Tiny,
    Small,
    Medium,
    Large,
}

/// A soft observability budget, never an enforcement limit. (doc 02 §9.3)
#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Budget {
    pub max_apply_micros: u32,
    pub max_events_per_input: u16,
}
