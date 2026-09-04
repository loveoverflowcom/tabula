//! Validated configuration and boundary primitives for Werewolf. (doc 02 §10.2, doc 08 §5.1)
//!
//! # Proof barriers and validated types
//!
//! Domain types in this module enforce their invariants at the construction
//! barrier (see `rust-types-as-proofs`). Serde deserialization routes through
//! raw DTOs and [`TryFrom`], preventing invalid durations, round counts, or
//! seat bounds from bypassing validation.
//!
//! @ai.role domain-types
//! @ai.domain werewolf.rules.config
//! @ai.pure true
//! @ai.invariant seat-count-bounds
//! @ai.invariant phase-duration-bounds
//! @ai.invariant max-rounds-bounds
//! @ai.invariant validated-deserialization-barrier

use serde::{Deserialize, Serialize};
use tabula_core::{Millis, Occupant, SeatRoster};
use tabula_game_api::ConfigError;

use super::role::Preset;

/// The minimum number of seats allowed in a Werewolf match (W-D1, W-D2).
pub const MIN_SEATS: u8 = 6;
/// The maximum number of seats allowed in a Werewolf match (W-D1, W-D2).
pub const MAX_SEATS: u8 = 20;

/// Lower bound on any configurable phase duration: 1 second (W-D16).
pub const MIN_PHASE_DURATION_MS: u64 = 1_000;
/// Upper bound on any configurable phase duration: 10 minutes (W-D16).
pub const MAX_PHASE_DURATION_MS: u64 = 600_000;

/// Lower bound on maximum match rounds: 1 round (W-D17).
pub const MIN_MAX_ROUNDS: u32 = 1;
/// Upper bound on maximum match rounds: 100 rounds (W-D17).
pub const MAX_MAX_ROUNDS: u32 = 100;
/// Default round cap before stalemate draw: 100 rounds (W-D17).
pub const DEFAULT_MAX_ROUNDS: u32 = 100;

/// Default phase durations in milliseconds (W-D16).
pub const DEFAULT_NIGHT_MS: u64 = 30_000;
pub const DEFAULT_DAWN_MS: u64 = 2_000;
pub const DEFAULT_DAY_MS: u64 = 120_000;
pub const DEFAULT_VOTE_MS: u64 = 30_000;
pub const DEFAULT_DUSK_MS: u64 = 2_000;

/// Validated seat count in the range [`MIN_SEATS`..=[`MAX_SEATS`]].
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct SeatCount(u8);

impl SeatCount {
    pub const MIN: u8 = MIN_SEATS;
    pub const MAX: u8 = MAX_SEATS;

    /// Constructs a validated seat count.
    ///
    /// # Errors
    /// Returns [`SeatCountError`] if `count` is outside [`MIN_SEATS`..=[`MAX_SEATS`]].
    pub const fn new(count: u8) -> Result<Self, SeatCountError> {
        if count >= Self::MIN && count <= Self::MAX {
            Ok(Self(count))
        } else {
            Err(SeatCountError::OutOfRange { count })
        }
    }

    /// Returns the underlying seat count.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for SeatCount {
    type Error = SeatCountError;

    fn try_from(count: u8) -> Result<Self, Self::Error> {
        Self::new(count)
    }
}

impl From<SeatCount> for u8 {
    fn from(s: SeatCount) -> Self {
        s.get()
    }
}

/// Why a seat count cannot be used to initialize a Werewolf match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SeatCountError {
    #[error("seat count {count} is out of range [{MIN_SEATS}..={MAX_SEATS}]")]
    OutOfRange { count: u8 },
}

/// A validated phase duration in the range [`MIN_PHASE_DURATION_MS`..=[`MAX_PHASE_DURATION_MS`]].
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct PhaseDuration(Millis);

impl PhaseDuration {
    pub const MIN: Millis = Millis(MIN_PHASE_DURATION_MS);
    pub const MAX: Millis = Millis(MAX_PHASE_DURATION_MS);

    /// Validates and wraps a duration in milliseconds.
    ///
    /// # Errors
    /// Returns [`DurationError`] if `millis` is outside [`MIN_PHASE_DURATION_MS`..=[`MAX_PHASE_DURATION_MS`]].
    pub const fn from_millis(millis: Millis) -> Result<Self, DurationError> {
        if millis.0 >= Self::MIN.0 && millis.0 <= Self::MAX.0 {
            Ok(Self(millis))
        } else {
            Err(DurationError::OutOfRange { millis: millis.0 })
        }
    }

    /// Validates and converts seconds into a phase duration.
    ///
    /// # Errors
    /// Returns [`DurationError`] if the resulting duration is outside the permitted bounds.
    pub const fn from_secs(secs: u64) -> Result<Self, DurationError> {
        Self::from_millis(Millis::from_secs(secs))
    }

    /// Returns the wrapped logical time duration.
    #[must_use]
    pub const fn get(self) -> Millis {
        self.0
    }

    /// Returns the duration in milliseconds.
    #[must_use]
    pub const fn millis(self) -> u64 {
        self.0 .0
    }
}

impl TryFrom<u64> for PhaseDuration {
    type Error = DurationError;

    fn try_from(millis: u64) -> Result<Self, Self::Error> {
        Self::from_millis(Millis(millis))
    }
}

impl From<PhaseDuration> for u64 {
    fn from(d: PhaseDuration) -> Self {
        d.millis()
    }
}

impl TryFrom<Millis> for PhaseDuration {
    type Error = DurationError;

    fn try_from(millis: Millis) -> Result<Self, Self::Error> {
        Self::from_millis(millis)
    }
}

impl From<PhaseDuration> for Millis {
    fn from(d: PhaseDuration) -> Self {
        d.get()
    }
}

/// Why a configured phase duration is out of bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DurationError {
    #[error(
        "duration {millis} ms is out of range [{MIN_PHASE_DURATION_MS}..={MAX_PHASE_DURATION_MS}]"
    )]
    OutOfRange { millis: u64 },
}

/// Bounded maximum match rounds (W-D17).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct MaxRounds(u32);

impl MaxRounds {
    pub const MIN: u32 = MIN_MAX_ROUNDS;
    pub const MAX: u32 = MAX_MAX_ROUNDS;
    pub const DEFAULT: u32 = DEFAULT_MAX_ROUNDS;

    /// Validates and constructs a [`MaxRounds`] wrapper.
    ///
    /// # Errors
    /// Returns [`MaxRoundsError`] if `rounds` is outside [`MIN_MAX_ROUNDS`..=[`MAX_MAX_ROUNDS`]].
    pub const fn new(rounds: u32) -> Result<Self, MaxRoundsError> {
        if rounds >= Self::MIN && rounds <= Self::MAX {
            Ok(Self(rounds))
        } else {
            Err(MaxRoundsError::OutOfRange { rounds })
        }
    }

    /// Returns the round cap value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for MaxRounds {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl TryFrom<u32> for MaxRounds {
    type Error = MaxRoundsError;

    fn try_from(rounds: u32) -> Result<Self, Self::Error> {
        Self::new(rounds)
    }
}

impl From<MaxRounds> for u32 {
    fn from(r: MaxRounds) -> Self {
        r.get()
    }
}

/// Why a round cap is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MaxRoundsError {
    #[error("max_rounds {rounds} is out of range [{MIN_MAX_ROUNDS}..={MAX_MAX_ROUNDS}]")]
    OutOfRange { rounds: u32 },
}

/// Day voting resolution threshold mode (W-D6).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum VoteMode {
    /// Unique plurality of votes eliminates; ties yield no elimination.
    #[default]
    Plurality,
    /// Must exceed half of all living seats (including abstainers); otherwise no elimination.
    AbsoluteMajority,
}

/// Validated fixed durations for all five Werewolf phases (W-D16).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(try_from = "RawPhaseDurations", into = "RawPhaseDurations")]
pub struct PhaseDurations {
    pub night: PhaseDuration,
    pub dawn: PhaseDuration,
    pub day: PhaseDuration,
    pub vote: PhaseDuration,
    pub dusk: PhaseDuration,
}

impl PhaseDurations {
    pub const DEFAULT_NIGHT: PhaseDuration = PhaseDuration(Millis(DEFAULT_NIGHT_MS));
    pub const DEFAULT_DAWN: PhaseDuration = PhaseDuration(Millis(DEFAULT_DAWN_MS));
    pub const DEFAULT_DAY: PhaseDuration = PhaseDuration(Millis(DEFAULT_DAY_MS));
    pub const DEFAULT_VOTE: PhaseDuration = PhaseDuration(Millis(DEFAULT_VOTE_MS));
    pub const DEFAULT_DUSK: PhaseDuration = PhaseDuration(Millis(DEFAULT_DUSK_MS));
}

impl Default for PhaseDurations {
    fn default() -> Self {
        Self {
            night: Self::DEFAULT_NIGHT,
            dawn: Self::DEFAULT_DAWN,
            day: Self::DEFAULT_DAY,
            vote: Self::DEFAULT_VOTE,
            dusk: Self::DEFAULT_DUSK,
        }
    }
}

/// Unvalidated DTO for [`PhaseDurations`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPhaseDurations {
    #[serde(default = "default_raw_night")]
    pub night_ms: u64,
    #[serde(default = "default_raw_dawn")]
    pub dawn_ms: u64,
    #[serde(default = "default_raw_day")]
    pub day_ms: u64,
    #[serde(default = "default_raw_vote")]
    pub vote_ms: u64,
    #[serde(default = "default_raw_dusk")]
    pub dusk_ms: u64,
}

const fn default_raw_night() -> u64 {
    DEFAULT_NIGHT_MS
}
const fn default_raw_dawn() -> u64 {
    DEFAULT_DAWN_MS
}
const fn default_raw_day() -> u64 {
    DEFAULT_DAY_MS
}
const fn default_raw_vote() -> u64 {
    DEFAULT_VOTE_MS
}
const fn default_raw_dusk() -> u64 {
    DEFAULT_DUSK_MS
}

impl Default for RawPhaseDurations {
    fn default() -> Self {
        Self {
            night_ms: DEFAULT_NIGHT_MS,
            dawn_ms: DEFAULT_DAWN_MS,
            day_ms: DEFAULT_DAY_MS,
            vote_ms: DEFAULT_VOTE_MS,
            dusk_ms: DEFAULT_DUSK_MS,
        }
    }
}

impl TryFrom<RawPhaseDurations> for PhaseDurations {
    type Error = DurationError;

    fn try_from(raw: RawPhaseDurations) -> Result<Self, Self::Error> {
        Ok(Self {
            night: PhaseDuration::from_millis(Millis(raw.night_ms))?,
            dawn: PhaseDuration::from_millis(Millis(raw.dawn_ms))?,
            day: PhaseDuration::from_millis(Millis(raw.day_ms))?,
            vote: PhaseDuration::from_millis(Millis(raw.vote_ms))?,
            dusk: PhaseDuration::from_millis(Millis(raw.dusk_ms))?,
        })
    }
}

impl From<PhaseDurations> for RawPhaseDurations {
    fn from(pd: PhaseDurations) -> Self {
        Self {
            night_ms: pd.night.millis(),
            dawn_ms: pd.dawn.millis(),
            day_ms: pd.day.millis(),
            vote_ms: pd.vote.millis(),
            dusk_ms: pd.dusk.millis(),
        }
    }
}

/// Authoritative match configuration for Werewolf.
///
/// Deserialization is protected by [`RawConfig`] and [`TryFrom`] to guarantee
/// that all loaded domain configurations satisfy bounds and invariants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawConfig", into = "RawConfig")]
pub struct Config {
    pub preset: Preset,
    pub vote_mode: VoteMode,
    pub phase_durations: PhaseDurations,
    pub max_rounds: MaxRounds,
}

impl Config {
    /// Constructs a validated configuration from strongly-typed primitives.
    #[must_use]
    pub const fn new(
        preset: Preset,
        vote_mode: VoteMode,
        phase_durations: PhaseDurations,
        max_rounds: MaxRounds,
    ) -> Self {
        Self {
            preset,
            vote_mode,
            phase_durations,
            max_rounds,
        }
    }

    /// Validates a seat roster against Werewolf match creation rules (W-D1, W-D2).
    ///
    /// Requires:
    /// - Unique occupied seats between [`MIN_SEATS`] and [`MAX_SEATS`].
    /// - No empty seats.
    /// - No pre-assigned teams (factions are game-owned and asymmetric).
    ///
    /// # Errors
    /// Returns [`ConfigError`] naming the invalid condition or field.
    pub fn validate_roster(&self, roster: &SeatRoster) -> Result<SeatCount, ConfigError> {
        let count_u8 = u8::try_from(roster.len()).map_err(|_| ConfigError::SeatCount)?;
        let seat_count = SeatCount::new(count_u8).map_err(|_| ConfigError::SeatCount)?;

        for entry in roster {
            if entry.occupant == Occupant::Empty {
                return Err(ConfigError::field("occupant"));
            }
            if entry.team.is_some() {
                return Err(ConfigError::field("team"));
            }
        }

        Ok(seat_count)
    }

    /// Validates an arbitrary seat count for compatibility with this configuration.
    ///
    /// # Errors
    /// Returns [`ConfigError::SeatCount`] if `count` is outside [`MIN_SEATS`..=[`MAX_SEATS`]].
    pub fn validate_seat_count(count: usize) -> Result<SeatCount, ConfigError> {
        let count_u8 = u8::try_from(count).map_err(|_| ConfigError::SeatCount)?;
        SeatCount::new(count_u8).map_err(|_| ConfigError::SeatCount)
    }
}

/// Unvalidated DTO for [`Config`] serialization barrier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawConfig {
    #[serde(default)]
    pub preset: Preset,
    #[serde(default)]
    pub vote_mode: VoteMode,
    #[serde(default)]
    pub phase_durations: Option<RawPhaseDurations>,
    pub night_duration_ms: Option<u64>,
    pub dawn_duration_ms: Option<u64>,
    pub day_duration_ms: Option<u64>,
    pub vote_duration_ms: Option<u64>,
    pub dusk_duration_ms: Option<u64>,
    #[serde(default = "default_raw_max_rounds")]
    pub max_rounds: u32,
}

const fn default_raw_max_rounds() -> u32 {
    DEFAULT_MAX_ROUNDS
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            preset: Preset::default(),
            vote_mode: VoteMode::default(),
            phase_durations: Some(RawPhaseDurations::default()),
            night_duration_ms: None,
            dawn_duration_ms: None,
            day_duration_ms: None,
            vote_duration_ms: None,
            dusk_duration_ms: None,
            max_rounds: DEFAULT_MAX_ROUNDS,
        }
    }
}

/// Error returned when deserializing an invalid raw configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigValidationError {
    #[error(transparent)]
    Duration(#[from] DurationError),
    #[error(transparent)]
    MaxRounds(#[from] MaxRoundsError),
}

impl TryFrom<RawConfig> for Config {
    type Error = ConfigValidationError;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        let raw_durations = raw.phase_durations.unwrap_or_default();

        let night_ms = raw.night_duration_ms.unwrap_or(raw_durations.night_ms);
        let dawn_ms = raw.dawn_duration_ms.unwrap_or(raw_durations.dawn_ms);
        let day_ms = raw.day_duration_ms.unwrap_or(raw_durations.day_ms);
        let vote_ms = raw.vote_duration_ms.unwrap_or(raw_durations.vote_ms);
        let dusk_ms = raw.dusk_duration_ms.unwrap_or(raw_durations.dusk_ms);

        let phase_durations = PhaseDurations {
            night: PhaseDuration::from_millis(Millis(night_ms))?,
            dawn: PhaseDuration::from_millis(Millis(dawn_ms))?,
            day: PhaseDuration::from_millis(Millis(day_ms))?,
            vote: PhaseDuration::from_millis(Millis(vote_ms))?,
            dusk: PhaseDuration::from_millis(Millis(dusk_ms))?,
        };

        let max_rounds = MaxRounds::new(raw.max_rounds)?;

        Ok(Self {
            preset: raw.preset,
            vote_mode: raw.vote_mode,
            phase_durations,
            max_rounds,
        })
    }
}

impl From<Config> for RawConfig {
    fn from(cfg: Config) -> Self {
        Self {
            preset: cfg.preset,
            vote_mode: cfg.vote_mode,
            phase_durations: Some(RawPhaseDurations::from(cfg.phase_durations)),
            night_duration_ms: None,
            dawn_duration_ms: None,
            day_duration_ms: None,
            vote_duration_ms: None,
            dusk_duration_ms: None,
            max_rounds: cfg.max_rounds.get(),
        }
    }
}
