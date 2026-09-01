//! `GameCapabilities` — declarative facts the platform needs to run a game safely.
//! (doc 02 §4.2, §5)
//!
//! The types in this module deliberately validate only capability facts. They
//! do not encode game rules or introduce a platform branch on `GameId` (I-9).

use core::num::NonZeroU8;

use serde::{Deserialize, Serialize};
use tabula_core::{BotLevel, Millis};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "GameCapabilitiesSpec")]
pub struct GameCapabilities {
    seats: SeatSpec,
    turn_model: TurnModel,
    hidden_information: bool,
    spectators: SpectatorPolicy,
    chat: ChatPolicy,
    voice: VoiceRequirement,
    ranked: RankedSupport,
    async_turns: AsyncTurnPolicy,
    reconnect: ReconnectPolicy,
    substitution: SubstitutionPolicy,
    pausable: bool,
    durability: Durability,
    client_preview: bool,
    state_size: StateSizeClass,
    apply_budget: Budget,
    max_match_duration: Option<Millis>,
}

/// Named capability authoring input. It may describe a cross-field-invalid
/// combination; [`GameCapabilities::try_from_spec`] is the single validator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameCapabilitiesSpec {
    pub seats: SeatSpec,
    pub turn_model: TurnModel,
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
    pub client_preview: bool,
    pub state_size: StateSizeClass,
    pub apply_budget: Budget,
    pub max_match_duration: Option<Millis>,
}

/// Why a complete capability declaration is self-contradictory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GameCapabilitiesError {
    #[error("ranked games must acknowledge only after persistence")]
    RankedNeedsDurability,
}

impl GameCapabilities {
    /// Establishes the cross-field capability laws consumed by platform policy.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain game.capabilities
    /// @ai.invariant ranked-games-ack-after-persist
    /// @ai.evidence crate::capabilities::tests::ranked_capabilities_require_durable_acknowledgement
    /// @ai.evidence crate::capabilities::tests::capability_deserialization_cannot_bypass_cross_field_validation
    #[allow(clippy::doc_markdown)]
    pub fn try_from_spec(spec: GameCapabilitiesSpec) -> Result<Self, GameCapabilitiesError> {
        if matches!(spec.ranked, RankedSupport::Yes { .. })
            && spec.durability != Durability::AckAfterPersist
        {
            return Err(GameCapabilitiesError::RankedNeedsDurability);
        }
        Ok(Self {
            seats: spec.seats,
            turn_model: spec.turn_model,
            hidden_information: spec.hidden_information,
            spectators: spec.spectators,
            chat: spec.chat,
            voice: spec.voice,
            ranked: spec.ranked,
            async_turns: spec.async_turns,
            reconnect: spec.reconnect,
            substitution: spec.substitution,
            pausable: spec.pausable,
            durability: spec.durability,
            client_preview: spec.client_preview,
            state_size: spec.state_size,
            apply_budget: spec.apply_budget,
            max_match_duration: spec.max_match_duration,
        })
    }

    #[must_use]
    pub const fn seats(&self) -> &SeatSpec {
        &self.seats
    }

    #[must_use]
    pub const fn turn_model(&self) -> TurnModel {
        self.turn_model
    }

    #[must_use]
    pub const fn hidden_information(&self) -> bool {
        self.hidden_information
    }

    #[must_use]
    pub const fn spectators(&self) -> SpectatorPolicy {
        self.spectators
    }

    #[must_use]
    pub const fn chat(&self) -> &ChatPolicy {
        &self.chat
    }

    #[must_use]
    pub const fn voice(&self) -> VoiceRequirement {
        self.voice
    }

    #[must_use]
    pub const fn ranked(&self) -> RankedSupport {
        self.ranked
    }

    #[must_use]
    pub const fn async_turns(&self) -> AsyncTurnPolicy {
        self.async_turns
    }

    #[must_use]
    pub const fn reconnect(&self) -> ReconnectPolicy {
        self.reconnect
    }

    #[must_use]
    pub const fn substitution(&self) -> &SubstitutionPolicy {
        &self.substitution
    }

    #[must_use]
    pub const fn pausable(&self) -> bool {
        self.pausable
    }

    #[must_use]
    pub const fn durability(&self) -> Durability {
        self.durability
    }

    #[must_use]
    pub const fn client_preview(&self) -> bool {
        self.client_preview
    }

    #[must_use]
    pub const fn state_size(&self) -> StateSizeClass {
        self.state_size
    }

    #[must_use]
    pub const fn apply_budget(&self) -> Budget {
        self.apply_budget
    }

    #[must_use]
    pub const fn max_match_duration(&self) -> Option<Millis> {
        self.max_match_duration
    }
}

impl TryFrom<GameCapabilitiesSpec> for GameCapabilities {
    type Error = GameCapabilitiesError;

    fn try_from(spec: GameCapabilitiesSpec) -> Result<Self, Self::Error> {
        Self::try_from_spec(spec)
    }
}

/// The supported player counts for one game. A range and an exact set are
/// mutually exclusive representations, so no second `min`/`max` authority is
/// exposed by [`SeatSpec`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "RawSeatCounts", into = "RawSeatCounts")]
pub struct SeatCounts(SeatCountsRepr);

#[derive(Clone, Debug, PartialEq, Eq)]
enum SeatCountsRepr {
    Range { min: u8, max: u8 },
    Exact(Vec<u8>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum RawSeatCounts {
    Range { min: u8, max: u8 },
    Exact(Vec<u8>),
}

/// Why a supported-seat-count declaration cannot be canonical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SeatCountsError {
    #[error("seat counts must start at one")]
    Zero,
    #[error("seat count minimum must not exceed maximum")]
    InvertedRange,
    #[error("exact seat counts must not be empty")]
    EmptyExact,
    #[error("exact seat count {count} occurs more than once")]
    DuplicateExact { count: u8 },
}

impl SeatCounts {
    pub fn range(min: u8, max: u8) -> Result<Self, SeatCountsError> {
        if min == 0 || max == 0 {
            return Err(SeatCountsError::Zero);
        }
        if min > max {
            return Err(SeatCountsError::InvertedRange);
        }
        Ok(Self(SeatCountsRepr::Range { min, max }))
    }

    pub fn exact(mut counts: Vec<u8>) -> Result<Self, SeatCountsError> {
        if counts.is_empty() {
            return Err(SeatCountsError::EmptyExact);
        }
        if counts.contains(&0) {
            return Err(SeatCountsError::Zero);
        }
        counts.sort_unstable();
        if let Some(duplicate) = counts
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(pair[0]))
        {
            return Err(SeatCountsError::DuplicateExact { count: duplicate });
        }
        Ok(Self(SeatCountsRepr::Exact(counts)))
    }

    #[must_use]
    pub fn contains(&self, count: u8) -> bool {
        match &self.0 {
            SeatCountsRepr::Range { min, max } => (*min..=*max).contains(&count),
            SeatCountsRepr::Exact(counts) => counts.binary_search(&count).is_ok(),
        }
    }

    #[must_use]
    pub fn min(&self) -> u8 {
        match &self.0 {
            SeatCountsRepr::Range { min, .. } => *min,
            SeatCountsRepr::Exact(counts) => counts[0],
        }
    }

    #[must_use]
    pub fn max(&self) -> u8 {
        match &self.0 {
            SeatCountsRepr::Range { max, .. } => *max,
            SeatCountsRepr::Exact(counts) => {
                *counts.last().expect("validated exact counts are non-empty")
            }
        }
    }
}

impl TryFrom<RawSeatCounts> for SeatCounts {
    type Error = SeatCountsError;

    fn try_from(raw: RawSeatCounts) -> Result<Self, Self::Error> {
        match raw {
            RawSeatCounts::Range { min, max } => Self::range(min, max),
            RawSeatCounts::Exact(counts) => Self::exact(counts),
        }
    }
}

impl From<SeatCounts> for RawSeatCounts {
    fn from(value: SeatCounts) -> Self {
        match value.0 {
            SeatCountsRepr::Range { min, max } => Self::Range { min, max },
            SeatCountsRepr::Exact(counts) => Self::Exact(counts),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeatSpec {
    allowed: SeatCounts,
    teams: Option<TeamSpec>,
    fill_with_bots: bool,
    symmetric: bool,
}

impl SeatSpec {
    /// Builds the one authoritative seat-policy representation.
    ///
    /// @ai.role proof-boundary
    /// @ai.domain game.capabilities
    /// @ai.invariant canonical-allowed-seat-counts
    /// @ai.evidence crate::capabilities::tests::seat_count_constructor_partitions
    #[allow(clippy::doc_markdown)]
    #[must_use]
    pub fn new(
        allowed: SeatCounts,
        teams: Option<TeamSpec>,
        fill_with_bots: bool,
        symmetric: bool,
    ) -> Self {
        Self {
            allowed,
            teams,
            fill_with_bots,
            symmetric,
        }
    }

    #[must_use]
    pub const fn allowed(&self) -> &SeatCounts {
        &self.allowed
    }

    #[must_use]
    pub const fn teams(&self) -> Option<&TeamSpec> {
        self.teams.as_ref()
    }

    #[must_use]
    pub const fn fill_with_bots(&self) -> bool {
        self.fill_with_bots
    }

    #[must_use]
    pub const fn symmetric(&self) -> bool {
        self.symmetric
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeamSpec {
    teams: NonZeroU8,
    seats_per_team: NonZeroU8,
}

/// Why team cardinality cannot describe a real team game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TeamSpecError {
    #[error("team count must be non-zero")]
    ZeroTeams,
    #[error("seats per team must be non-zero")]
    ZeroSeatsPerTeam,
}

impl TeamSpec {
    pub fn new(teams: u8, seats_per_team: u8) -> Result<Self, TeamSpecError> {
        let teams = NonZeroU8::new(teams).ok_or(TeamSpecError::ZeroTeams)?;
        let seats_per_team =
            NonZeroU8::new(seats_per_team).ok_or(TeamSpecError::ZeroSeatsPerTeam)?;
        Ok(Self {
            teams,
            seats_per_team,
        })
    }

    #[must_use]
    pub const fn teams(&self) -> u8 {
        self.teams.get()
    }

    #[must_use]
    pub const fn seats_per_team(&self) -> u8 {
        self.seats_per_team.get()
    }
}

/// How the platform should present and police turn-taking. Actual turn order
/// remains game-owned state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnModel {
    StrictSequential,
    Simultaneous,
    Phased,
    FreeForm,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum SpectatorPolicy {
    Forbidden,
    Live,
    Delayed { by: Millis },
    GameControlled,
}

/// Chat channels declared by a game. Channel identity is unique and authored
/// order is retained because it is catalog-visible presentation data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawChatPolicy")]
pub struct ChatPolicy {
    channels: Vec<ChatChannelSpec>,
    game_scoped: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawChatPolicy {
    channels: Vec<RawChatChannelSpec>,
    game_scoped: bool,
}

/// Stable identity of a platform chat channel within one game.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChatChannelKey(String);

/// Why a chat channel key cannot identify a channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChatChannelKeyError {
    #[error("chat channel key must not be empty")]
    Empty,
    #[error("chat channel key must not be whitespace-only")]
    WhitespaceOnly,
    #[error("chat channel key must not have surrounding whitespace")]
    SurroundingWhitespace,
}

impl ChatChannelKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ChatChannelKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ChatChannelKeyError::Empty);
        }
        if value.trim().is_empty() {
            return Err(ChatChannelKeyError::WhitespaceOnly);
        }
        if value.trim() != value {
            return Err(ChatChannelKeyError::SurroundingWhitespace);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ChatChannelKey {
    type Error = ChatChannelKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ChatChannelKey {
    type Error = ChatChannelKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ChatChannelKey> for String {
    fn from(value: ChatChannelKey) -> Self {
        value.0
    }
}

/// One channel's validated identity and transport kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawChatChannelSpec", into = "RawChatChannelSpec")]
pub struct ChatChannelSpec {
    key: ChatChannelKey,
    kind: ChatKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawChatChannelSpec {
    key: String,
    kind: ChatKind,
}

/// Why a chat policy cannot be represented without ambiguity.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChatPolicyError {
    #[error("chat channel key is invalid: {0}")]
    InvalidChannelKey(#[from] ChatChannelKeyError),
    #[error("chat channel key `{key}` occurs more than once")]
    DuplicateChannelKey { key: String },
}

impl ChatChannelSpec {
    pub fn new(key: impl Into<String>, kind: ChatKind) -> Result<Self, ChatChannelKeyError> {
        Ok(Self {
            key: ChatChannelKey::new(key)?,
            kind,
        })
    }

    #[must_use]
    pub const fn key(&self) -> &ChatChannelKey {
        &self.key
    }

    #[must_use]
    pub const fn kind(&self) -> ChatKind {
        self.kind
    }
}

impl TryFrom<RawChatChannelSpec> for ChatChannelSpec {
    type Error = ChatChannelKeyError;

    fn try_from(raw: RawChatChannelSpec) -> Result<Self, Self::Error> {
        Self::new(raw.key, raw.kind)
    }
}

impl From<ChatChannelSpec> for RawChatChannelSpec {
    fn from(value: ChatChannelSpec) -> Self {
        Self {
            key: value.key.into(),
            kind: value.kind,
        }
    }
}

impl ChatPolicy {
    /// Rejects duplicate channel identities without changing authored order.
    ///
    /// @ai.role proof-boundary
    /// @ai.domain game.capabilities.chat
    /// @ai.invariant unique-chat-channel-identities
    /// @ai.evidence crate::capabilities::tests::chat_policy_rejects_duplicate_channel_identity
    #[allow(clippy::doc_markdown)]
    pub fn new(channels: Vec<ChatChannelSpec>, game_scoped: bool) -> Result<Self, ChatPolicyError> {
        let mut seen = Vec::with_capacity(channels.len());
        for channel in &channels {
            if seen.iter().any(|key: &ChatChannelKey| key == channel.key()) {
                return Err(ChatPolicyError::DuplicateChannelKey {
                    key: channel.key().as_str().to_owned(),
                });
            }
            seen.push(channel.key().clone());
        }
        Ok(Self {
            channels,
            game_scoped,
        })
    }

    #[must_use]
    pub fn channels(&self) -> &[ChatChannelSpec] {
        &self.channels
    }

    #[must_use]
    pub const fn game_scoped(&self) -> bool {
        self.game_scoped
    }
}

impl TryFrom<RawChatPolicy> for ChatPolicy {
    type Error = ChatPolicyError;

    fn try_from(raw: RawChatPolicy) -> Result<Self, Self::Error> {
        let channels = raw
            .channels
            .into_iter()
            .map(ChatChannelSpec::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(channels, raw.game_scoped)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    Recommended,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum RankedSupport {
    No,
    Yes { rating: RatingKind },
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum RatingKind {
    Elo,
    Glicko2,
    TeamElo,
    Placement,
}

/// Closed async configuration: disabled games carry no latent deadlines.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum AsyncTurnPolicy {
    Disabled,
    Enabled {
        turn_deadline: Option<Millis>,
        match_ttl: Option<Millis>,
    },
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct ReconnectPolicy {
    pub grace: Millis,
    pub notify_rules: bool,
}

/// A deterministic non-empty bot-level list.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "Vec<BotLevel>", into = "Vec<BotLevel>")]
pub struct BotLevels(Vec<BotLevel>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BotLevelsError {
    #[error("bot substitution requires at least one level")]
    Empty,
    #[error("bot level {level:?} occurs more than once")]
    Duplicate { level: BotLevel },
}

impl BotLevels {
    pub fn new(mut levels: Vec<BotLevel>) -> Result<Self, BotLevelsError> {
        if levels.is_empty() {
            return Err(BotLevelsError::Empty);
        }
        levels.sort_unstable();
        if let Some(level) = levels
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(pair[0]))
        {
            return Err(BotLevelsError::Duplicate { level });
        }
        Ok(Self(levels))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[BotLevel] {
        &self.0
    }
}

impl TryFrom<Vec<BotLevel>> for BotLevels {
    type Error = BotLevelsError;

    fn try_from(levels: Vec<BotLevel>) -> Result<Self, Self::Error> {
        Self::new(levels)
    }
}

impl From<BotLevels> for Vec<BotLevel> {
    fn from(levels: BotLevels) -> Self {
        levels.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SubstitutionPolicy {
    Forbidden,
    BotOnly { levels: BotLevels },
    HumanOrBot,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Durability {
    AckAfterApply,
    AckAfterPersist,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateSizeClass {
    Tiny,
    Small,
    Medium,
    Large,
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Budget {
    pub max_apply_micros: u32,
    pub max_events_per_input: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::canonical_encode;

    #[test]
    fn seat_count_constructor_partitions() {
        assert!(SeatCounts::range(1, 1).is_ok());
        assert!(SeatCounts::range(2, 8).is_ok());
        assert!(matches!(
            SeatCounts::range(0, 1),
            Err(SeatCountsError::Zero)
        ));
        assert!(matches!(
            SeatCounts::range(3, 2),
            Err(SeatCountsError::InvertedRange)
        ));
        assert!(matches!(
            SeatCounts::exact(Vec::new()),
            Err(SeatCountsError::EmptyExact)
        ));
        assert!(matches!(
            SeatCounts::exact(vec![1, 0]),
            Err(SeatCountsError::Zero)
        ));
        assert!(matches!(
            SeatCounts::exact(vec![2, 2]),
            Err(SeatCountsError::DuplicateExact { count: 2 })
        ));
        assert_eq!(SeatCounts::exact(vec![4, 2]).unwrap().min(), 2);
    }

    #[test]
    fn bot_levels_and_team_sizes_reject_degenerate_values() {
        assert!(matches!(
            BotLevels::new(Vec::new()),
            Err(BotLevelsError::Empty)
        ));
        assert!(matches!(
            BotLevels::new(vec![BotLevel::Easy, BotLevel::Easy]),
            Err(BotLevelsError::Duplicate {
                level: BotLevel::Easy
            })
        ));
        assert!(matches!(TeamSpec::new(0, 1), Err(TeamSpecError::ZeroTeams)));
        assert!(matches!(
            TeamSpec::new(1, 0),
            Err(TeamSpecError::ZeroSeatsPerTeam)
        ));
        assert_eq!(TeamSpec::new(2, 3).unwrap().seats_per_team(), 3);
    }

    #[test]
    fn ranked_capabilities_require_durable_acknowledgement() {
        let seats = SeatSpec::new(SeatCounts::range(2, 2).unwrap(), None, false, true);
        let result = GameCapabilities::try_from(GameCapabilitiesSpec {
            seats,
            turn_model: TurnModel::StrictSequential,
            hidden_information: false,
            spectators: SpectatorPolicy::Live,
            chat: ChatPolicy::new(Vec::new(), false).unwrap(),
            voice: VoiceRequirement::No,
            ranked: RankedSupport::Yes {
                rating: RatingKind::Elo,
            },
            async_turns: AsyncTurnPolicy::Disabled,
            reconnect: ReconnectPolicy {
                grace: Millis(1),
                notify_rules: false,
            },
            substitution: SubstitutionPolicy::Forbidden,
            pausable: false,
            durability: Durability::AckAfterApply,
            client_preview: true,
            state_size: StateSizeClass::Tiny,
            apply_budget: Budget::default(),
            max_match_duration: None,
        });
        assert!(matches!(
            result,
            Err(GameCapabilitiesError::RankedNeedsDurability)
        ));
    }

    #[test]
    fn capability_deserialization_cannot_bypass_cross_field_validation() {
        let raw = GameCapabilitiesSpec {
            seats: SeatSpec::new(SeatCounts::range(2, 2).unwrap(), None, false, true),
            turn_model: TurnModel::StrictSequential,
            hidden_information: false,
            spectators: SpectatorPolicy::Live,
            chat: ChatPolicy::new(Vec::new(), false).unwrap(),
            voice: VoiceRequirement::No,
            ranked: RankedSupport::Yes {
                rating: RatingKind::Elo,
            },
            async_turns: AsyncTurnPolicy::Disabled,
            reconnect: ReconnectPolicy {
                grace: Millis(1),
                notify_rules: false,
            },
            substitution: SubstitutionPolicy::Forbidden,
            pausable: false,
            durability: Durability::AckAfterApply,
            client_preview: true,
            state_size: StateSizeClass::Tiny,
            apply_budget: Budget::default(),
            max_match_duration: None,
        };
        let bytes = canonical_encode(&raw).unwrap();
        assert!(tabula_core::canonical_decode::<GameCapabilities>(&bytes).is_err());
    }

    #[test]
    fn capability_readers_expose_declarative_facts_without_mutation() {
        let seats = SeatSpec::new(
            SeatCounts::exact(vec![2]).unwrap(),
            Some(TeamSpec::new(2, 1).unwrap()),
            true,
            false,
        );
        let spec = GameCapabilitiesSpec {
            seats,
            turn_model: TurnModel::Phased,
            hidden_information: true,
            spectators: SpectatorPolicy::Delayed { by: Millis(30) },
            chat: ChatPolicy::new(
                vec![ChatChannelSpec::new("table", ChatKind::Table).unwrap()],
                true,
            )
            .unwrap(),
            voice: VoiceRequirement::Recommended,
            ranked: RankedSupport::Yes {
                rating: RatingKind::Glicko2,
            },
            async_turns: AsyncTurnPolicy::Enabled {
                turn_deadline: Some(Millis(60)),
                match_ttl: None,
            },
            reconnect: ReconnectPolicy {
                grace: Millis(10),
                notify_rules: true,
            },
            substitution: SubstitutionPolicy::BotOnly {
                levels: BotLevels::new(vec![BotLevel::Easy]).unwrap(),
            },
            pausable: true,
            durability: Durability::AckAfterPersist,
            client_preview: true,
            state_size: StateSizeClass::Medium,
            apply_budget: Budget {
                max_apply_micros: 1_000,
                max_events_per_input: 2,
            },
            max_match_duration: Some(Millis(600)),
        };
        let capabilities = GameCapabilities::try_from(spec.clone()).unwrap();
        assert_eq!(
            canonical_encode(&spec).unwrap(),
            canonical_encode(&capabilities).unwrap()
        );

        assert!(capabilities.seats().allowed().contains(2));
        assert_eq!(capabilities.seats().teams().unwrap().teams(), 2);
        assert!(capabilities.seats().fill_with_bots());
        assert!(!capabilities.seats().symmetric());
        assert!(matches!(capabilities.turn_model(), TurnModel::Phased));
        assert!(capabilities.hidden_information());
        assert!(matches!(
            capabilities.spectators(),
            SpectatorPolicy::Delayed { .. }
        ));
        assert_eq!(capabilities.chat().channels().len(), 1);
        assert!(matches!(
            capabilities.voice(),
            VoiceRequirement::Recommended
        ));
        assert!(matches!(capabilities.ranked(), RankedSupport::Yes { .. }));
        assert!(matches!(
            capabilities.async_turns(),
            AsyncTurnPolicy::Enabled { .. }
        ));
        assert!(capabilities.reconnect().notify_rules);
        assert!(matches!(
            capabilities.substitution(),
            SubstitutionPolicy::BotOnly { .. }
        ));
        assert!(capabilities.pausable());
        assert_eq!(capabilities.durability(), Durability::AckAfterPersist);
        assert!(capabilities.client_preview());
        assert_eq!(capabilities.state_size(), StateSizeClass::Medium);
        assert_eq!(capabilities.apply_budget().max_events_per_input, 2);
        assert_eq!(capabilities.max_match_duration(), Some(Millis(600)));
    }

    #[test]
    fn refined_capability_deserialization_rejects_invalid_parts() {
        let zero_range =
            tabula_core::canonical_encode(&RawSeatCounts::Range { min: 0, max: 2 }).unwrap();
        assert!(tabula_core::canonical_decode::<SeatCounts>(&zero_range).is_err());

        let empty_bot_levels = tabula_core::canonical_encode(&Vec::<BotLevel>::new()).unwrap();
        assert!(tabula_core::canonical_decode::<BotLevels>(&empty_bot_levels).is_err());

        let zero_team = tabula_core::canonical_encode(&(0u8, 1u8)).unwrap();
        assert!(tabula_core::canonical_decode::<TeamSpec>(&zero_team).is_err());
    }

    #[test]
    fn chat_policy_rejects_duplicate_channel_identity() {
        let channels = vec![
            ChatChannelSpec::new("table", ChatKind::Table).unwrap(),
            ChatChannelSpec::new("table", ChatKind::Whisper).unwrap(),
        ];
        assert_eq!(
            ChatPolicy::new(channels, false),
            Err(ChatPolicyError::DuplicateChannelKey {
                key: "table".to_owned()
            })
        );
        assert_eq!(
            ChatChannelSpec::new("", ChatKind::Table),
            Err(ChatChannelKeyError::Empty)
        );
        assert_eq!(
            ChatChannelSpec::new(" \t", ChatKind::Table),
            Err(ChatChannelKeyError::WhitespaceOnly)
        );
        assert_eq!(
            ChatChannelSpec::new(" table", ChatKind::Table),
            Err(ChatChannelKeyError::SurroundingWhitespace)
        );
        let empty_key = canonical_encode(&String::new()).unwrap();
        assert!(tabula_core::canonical_decode::<ChatChannelKey>(&empty_key).is_err());
        let whitespace_key = canonical_encode(&String::from(" \t")).unwrap();
        assert!(tabula_core::canonical_decode::<ChatChannelKey>(&whitespace_key).is_err());

        let raw = RawChatPolicy {
            channels: vec![
                RawChatChannelSpec {
                    key: "table".to_owned(),
                    kind: ChatKind::Table,
                },
                RawChatChannelSpec {
                    key: "table".to_owned(),
                    kind: ChatKind::Whisper,
                },
            ],
            game_scoped: false,
        };
        let bytes = canonical_encode(&raw).unwrap();
        assert!(tabula_core::canonical_decode::<ChatPolicy>(&bytes).is_err());
    }
}
