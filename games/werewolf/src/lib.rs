//! # `tabula-game-werewolf` — the social and scale benchmark
//!
//! > ## PHASE 3 (rules, headless) → PHASE 7 (presentation + online)
//! >
//! > The rules skeleton lands in Phase 3 so the contract is stressed by a
//! > phased, many-seat, heavily-redacted game **before** the protocol is frozen.
//! > Everything social — scoped chat enforcement, moderation, the party model —
//! > waits for Phase 7, after the web shell exists.
//!
//! Werewolf is the game that forces three platform capabilities nothing else
//! needs: **event non-existence**, **game-driven communication scoping**, and
//! **6–20 seats with phases instead of turns**. (doc 08 §5)
//!
//! ## Scope (doc 08 §5)
//!
//! ```text
//! IN:  6–20 seats; roles Villager, Werewolf, Seer, Doctor, Hunter, Witch
//!      (configurable preset sets per seat count); phases Night → Dawn →
//!      Day discussion → Vote → Dusk; night actions; majority/plurality voting
//!      with configurable ties; death reveals role; win conditions (wolves
//!      eliminated / wolves reach parity); dead players become spectators-with-
//!      full-vision; chat scopes per phase; voice scopes per phase; per-phase
//!      timers; NO bot substitution
//! OUT: moderator-run mode, advanced roles (Cupid/lovers, Jester, Alpha),
//!      custom rulesets beyond presets, cross-match reputation — Phase 9+
//! ```
//!
//! ## Contract sketch (doc 02 §12.3)
//!
//! ```rust,ignore
//! struct State {
//!     phase: Phase,                                 // Lobby, Night{n}, Dawn{n}, Day{n}, Vote{n}, Dusk{n}, Ended
//!     phase_ends_at: LogicalTime,
//!     roles: BTreeMap<SeatId, Role>,                // SECRET
//!     alive: BTreeSet<SeatId>,
//!     night_actions: BTreeMap<SeatId, NightAction>, // SECRET until resolution
//!     votes: BTreeMap<SeatId, SeatId>,              // public in most variants
//!     revealed: BTreeMap<SeatId, Role>,             // becomes public on death
//!     speech: Option<SpeechToken>,
//!     config: Ruleset,
//! }
//! ```
//!
//! Note `BTreeMap`/`BTreeSet` throughout, never `HashMap` (I-2). With 20 seats
//! and iteration that drives vote resolution, nondeterministic ordering would
//! change match outcomes.
//!
//! ## The five contract lessons
//!
//! 1. **`view_event` returns `None` for real.** `NightActionSubmitted` must not
//!    even be *known to exist* by other players — otherwise timing analysis
//!    reveals who acted. `Saved` (doctor) is invisible to everyone but the
//!    doctor until dawn. This is the only game that forces the platform to
//!    support hiding an event's existence.
//! 2. **`RolesAssigned` uses `Audience::ServerOnly`** — in the canonical log for
//!    replay and audit, reaching no client until deaths reveal roles.
//! 3. **Phases are timers.** Each transition emits `Effect::SetTimer` plus
//!    `Effect::SetChatScopes` and `Effect::SetVoiceScopes`:
//!    ```rust,ignore
//!    effects.push(Effect::SetChatScopes(ChatScopes::new()
//!        .allow("table",  Speak::None,                 Listen::None)
//!        .allow("wolves", Speak::Seats(&wolves_alive), Listen::Seats(&wolves_alive))
//!        .allow("dead",   Speak::Seats(&dead),         Listen::Seats(&dead))));
//!    ```
//!    The platform enforces it **at the socket**: the chat service refuses a
//!    message from a seat without `Speak`. The game never touches a socket or an
//!    SFU, and the platform learns nothing about werewolf.
//! 4. **`substitution = Forbidden`.** A werewolf seat carries secret knowledge;
//!    handing it to a bot or another human would leak it or destroy the social
//!    game. A disconnected player is handled by rules — auto-abstain, or death at
//!    dawn per the ruleset.
//! 5. **`spectators = GameControlled`.** Dead players are `Viewer::Seat(_)` and
//!    see everything; outsiders are `Viewer::Spectator(_)` and see only public
//!    information. **This is precisely why `Viewer` is an enum with a seat
//!    variant rather than an `Option<SeatId>`.**
//!
//! `durability = AckAfterApply`: no ranked stakes, and snappy voting matters more
//! than a 50 ms loss window that the phase timer recovers from anyway.
//!
//! ## Failure signals (doc 08 §5)
//!
//! - Any villager socket frame derived from a night action — **critical**.
//! - Chat scope enforcement needing game knowledge.
//! - The platform needing to know phase names.
//!
//! ## Acceptance (doc 08 §5)
//!
//! ```text
//! [ ] 20-seat golden integration match with per-seat projection assertions at
//!     every phase
//! [ ] socket-level chat scope tests (a wolf message never appears on a villager
//!     socket)
//! [ ] projection scan green at 6, 12, and 20 seats, for every role set
//! [ ] disconnect during night, during vote, and during dawn — all per ruleset
//! [ ] vote-burst load scenario (L2) within latency SLO
//! [ ] twelve-human playtest with a leak review by a SECOND engineer
//! [ ] dead-player vision correct; outside spectator vision correct; different
//! [ ] voice scope enforcement verified at the SFU (Phase 8)
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub mod rules;

pub use rules::{
    Alignment, Config, ConfigValidationError, DurationError, MaxRounds, MaxRoundsError,
    PhaseDuration, PhaseDurations, Preset, RawConfig, RawPhaseDurations, Role, RoleCounts,
    SeatCount, SeatCountError, VoteMode, DEFAULT_DAWN_MS, DEFAULT_DAY_MS, DEFAULT_DUSK_MS,
    DEFAULT_MAX_ROUNDS, DEFAULT_NIGHT_MS, DEFAULT_VOTE_MS, MAX_MAX_ROUNDS, MAX_PHASE_DURATION_MS,
    MAX_SEATS, MIN_MAX_ROUNDS, MIN_PHASE_DURATION_MS, MIN_SEATS,
};
