//! Identifiers. (doc 02 §2)
//!
//! Design rule: **newtypes over small integers, not `Uuid` everywhere.** Compact
//! state, fast comparison, deterministic encoding. `Uuid` appears only at platform
//! boundaries (the database, the HTTP API) and is converted at that boundary by
//! `tabula-storage` / `tabula-protocol` — never inside rules. (doc 01 §1.1)
//!
//! Every id here is `Copy + Ord`, so they can be `BTreeMap` keys without a clone
//! and iterate deterministically (I-2).

use alloc::string::String;
use core::fmt;

use serde::{Deserialize, Serialize};

/// An addressable participant slot in a match.
///
/// Seats are **stable**; occupants are not. A seat outlives the human sitting in
/// it — that is what makes reconnect, substitution, and bot takeover expressible
/// without a second identity system. (doc 00 §13)
///
/// `u8` because werewolf needs ~20 and nothing plausible needs 256.
/// Never invent a game-side "player index" alongside this. (doc 02 §13)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SeatId(pub u8);

/// A registered account. Carries `UUIDv7` bytes; conversion lives at the boundary.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct UserId(pub u128);

/// One instance of one game being played. The unit of ownership, ordering, and
/// persistence. (doc 00 §13)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct MatchId(pub u128);

/// A timer the *game* asked for. Game-scoped: `TimerId(1)` means whatever the
/// game says it means, and two games' timer ids never collide because they never
/// share a match. (doc 00 §6.3)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct TimerId(pub u16);

/// Monotonic per-match counter: **+1 per successfully applied input, and never
/// otherwise** (I-7). Drives reconnect, idempotency, and ordering.
///
/// A rejected input must leave this unchanged — that is half of contract R2.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct StateVersion(pub u64);

/// Position of an input in the match's log.
///
/// Two jobs: it is the event-log row ordinal, and it is the **RNG domain root**
/// (`DetRng::for_input`). That second job is why it must be assigned by the log,
/// not by a counter that could drift. (doc 02 §3.1)
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct InputIndex(pub u64);

/// Process-local connection id. Cheap on purpose — it never leaves the process
/// and never appears in the event log. (doc 03 §4)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SessionId(pub u64);

/// Reverse-DNS game identity, e.g. `com.tabula.chess`. (doc 02 §4.1)
///
/// **No platform crate may compare this against a literal** (I-9). It exists to
/// be looked up in `tabula-registry`, never to be branched on.
/// `xtask check-no-game-ids` greps for exactly that mistake.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GameId(String);

/// Why a [`GameId`] could not cross the identity trust boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GameIdError {
    #[error("game id must not be empty")]
    Empty,
    #[error("game id must contain at least two dot-separated segments")]
    TooFewSegments,
    #[error("game id segment {segment} must not be empty")]
    EmptySegment { segment: usize },
    #[error("game id segment {segment} must start with a lowercase ASCII letter")]
    InvalidSegmentStart { segment: usize },
    #[error("game id segment {segment} contains a non-canonical character")]
    InvalidSegmentCharacter { segment: usize },
}

impl GameId {
    /// Validates a canonical reverse-DNS identity such as `com.tabula.chess`.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain game.identity
    /// @ai.invariant canonical-reverse-dns-game-id
    /// @ai.evidence crate::ids::tests::game_id_constructor_partitions
    /// @ai.evidence crate::ids::tests::game_id_deserialization_cannot_bypass_validation
    #[allow(clippy::doc_markdown)]
    pub fn new(value: impl Into<String>) -> Result<Self, GameIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(GameIdError::Empty);
        }

        let mut segments = value.split('.').enumerate();
        let mut count = 0;
        for (segment, part) in &mut segments {
            count += 1;
            if part.is_empty() {
                return Err(GameIdError::EmptySegment { segment });
            }
            let mut chars = part.bytes();
            if !chars.next().is_some_and(|byte| byte.is_ascii_lowercase()) {
                return Err(GameIdError::InvalidSegmentStart { segment });
            }
            if !chars.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()) {
                return Err(GameIdError::InvalidSegmentCharacter { segment });
            }
        }
        if count < 2 {
            return Err(GameIdError::TooFewSegments);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GameId {
    type Error = GameIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for GameId {
    type Error = GameIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<GameId> for String {
    fn from(value: GameId) -> Self {
        value.0
    }
}

impl fmt::Display for GameId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Semver of the game *package*: presentation, bots, assets, docs, fixes.
///
/// Distinct from [`RulesVersion`] on purpose — see doc 02 §9.2. A presentation
/// bug fix bumps this and nothing else, and live matches are unaffected.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GameVersion(String);

/// Why a [`GameVersion`] is not valid Semantic Versioning 2.0.0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("game version must be a Semantic Versioning 2.0.0 value")]
pub struct GameVersionError;

impl GameVersion {
    /// Validates the `SemVer` package version recorded in game metadata.
    pub fn new(value: impl Into<String>) -> Result<Self, GameVersionError> {
        let value = value.into();
        is_semver(&value)
            .then_some(Self(value))
            .ok_or(GameVersionError)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GameVersion {
    type Error = GameVersionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for GameVersion {
    type Error = GameVersionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<GameVersion> for String {
    fn from(value: GameVersion) -> Self {
        value.0
    }
}

impl fmt::Display for GameVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn is_semver(value: &str) -> bool {
    let (without_build, build) = match value.split_once('+') {
        Some((version, build)) if !build.contains('+') => (version, Some(build)),
        Some(_) => return false,
        None => (value, None),
    };
    let (core, prerelease) = match without_build.split_once('-') {
        Some((version, prerelease)) => (version, Some(prerelease)),
        None => (without_build, None),
    };

    let mut core_parts = core.split('.');
    let Some(major) = core_parts.next() else {
        return false;
    };
    let Some(minor) = core_parts.next() else {
        return false;
    };
    let Some(patch) = core_parts.next() else {
        return false;
    };
    if core_parts.next().is_some()
        || !is_numeric_identifier(major, true)
        || !is_numeric_identifier(minor, true)
        || !is_numeric_identifier(patch, true)
    {
        return false;
    }

    prerelease.is_none_or(|identifiers| valid_identifiers(identifiers, true))
        && build.is_none_or(|identifiers| valid_identifiers(identifiers, false))
}

fn valid_identifiers(value: &str, reject_numeric_leading_zeroes: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && (!reject_numeric_leading_zeroes
                    || !identifier.bytes().all(|byte| byte.is_ascii_digit())
                    || is_numeric_identifier(identifier, true))
        })
}

fn is_numeric_identifier(value: &str, reject_leading_zeroes: bool) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (!reject_leading_zeroes || value.len() == 1 || !value.starts_with('0'))
}

/// Monotonic integer, bumped on **any** change to `State`/`Command`/`Event`
/// encoding or to `apply`/`project` behaviour. (doc 02 §9.2)
///
/// A match runs exactly one `RulesVersion` for its whole life. Upgrading a
/// running match's rules is not supported and never will be.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RulesVersion(pub u32);

impl RulesVersion {
    /// The version as it enters a hash preimage, little-endian.
    ///
    /// [`crate::hash::state_hash`] takes a `RulesVersion` directly rather than a
    /// free-form tag, so domain separation between two rules versions of one game
    /// is structural: there is no way for a caller to leave the version out.
    /// (ADR-026 §2 — the earlier `tag() -> &'static str` idea could not be
    /// written for a runtime value, and the `&str` shape it was papering over is
    /// what allowed the version-blind default this replaced.)
    #[must_use]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{canonical_decode, canonical_encode};

    #[test]
    fn game_id_constructor_partitions() {
        for valid in ["a.b", "com.example.game", "x1.y2"] {
            assert!(GameId::new(valid).is_ok(), "{valid} should be valid");
        }
        assert_eq!(GameId::new(""), Err(GameIdError::Empty));
        assert_eq!(GameId::new("one"), Err(GameIdError::TooFewSegments));
        assert_eq!(
            GameId::new("a..b"),
            Err(GameIdError::EmptySegment { segment: 1 })
        );
        assert_eq!(
            GameId::new("1.a"),
            Err(GameIdError::InvalidSegmentStart { segment: 0 })
        );
        assert_eq!(
            GameId::new("a.B"),
            Err(GameIdError::InvalidSegmentStart { segment: 1 })
        );
        assert_eq!(
            GameId::new("a.b-c"),
            Err(GameIdError::InvalidSegmentCharacter { segment: 1 })
        );
    }

    #[test]
    fn game_id_deserialization_cannot_bypass_validation() {
        let valid = GameId::new("com.example.game").unwrap();
        let encoded = canonical_encode(&valid).unwrap();
        assert_eq!(canonical_decode::<GameId>(&encoded).unwrap(), valid);

        let invalid = canonical_encode("Com.Example.Game").unwrap();
        assert!(canonical_decode::<GameId>(&invalid).is_err());
    }

    #[test]
    fn game_version_accepts_semver_and_rejects_near_misses() {
        for valid in ["0.0.0", "1.2.3", "1.2.3-alpha.1", "1.2.3+build.01"] {
            assert!(
                GameVersion::new(valid).is_ok(),
                "{valid} should be valid SemVer"
            );
        }
        for invalid in [
            "1", "1.2", "01.2.3", "1.2.3-01", "1.2.3+", "1.2.3-", "v1.2.3",
        ] {
            assert_eq!(
                GameVersion::new(invalid),
                Err(GameVersionError),
                "{invalid}"
            );
        }
    }

    #[test]
    fn game_version_deserialization_cannot_bypass_validation() {
        let invalid = canonical_encode("1.2").unwrap();
        assert!(canonical_decode::<GameVersion>(&invalid).is_err());
    }
}
