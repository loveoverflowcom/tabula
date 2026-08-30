//! Errors that are *not* rule rejections.
//!
//! A rule rejection is [`tabula_core::RuleError`] and is normal traffic — it
//! travels to the client and the match continues. Everything in this file is a
//! failure to *set up* or *load*, which the platform handles rather than the
//! player.

use compact_str::CompactString;

/// `GameRules::create` could not build an opening position.
///
/// The lobby has already run `validate_config`, so this should be rare and is
/// worth an alert when it happens.
#[derive(Clone, Debug, thiserror::Error)]
pub enum InitError {
    #[error("no seats in roster")]
    NoSeats,
    #[error("seat count {got} is not supported (allowed: {allowed})")]
    SeatCount { got: u8, allowed: CompactString },
    #[error("invalid config field: {0}")]
    Config(CompactString),
}

/// A lobby-supplied config is not usable for this game and roster.
///
/// Name the field: the room UI highlights it, and a generic "invalid config" is
/// a dead end for the player.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("wrong number of seats for this game")]
    SeatCount,
    #[error("invalid field: {0}")]
    Field(CompactString),
    #[error("config combination is not supported: {0}")]
    Unsupported(CompactString),
}

impl ConfigError {
    #[must_use]
    pub fn field(name: &str) -> Self {
        Self::Field(CompactString::from(name))
    }
}

/// A snapshot from an older `RulesVersion` could not be loaded.
///
/// [`MigrateError::Unsupported`] is the honest answer and the default. Doc 05
/// §10.2: **we never fake a replay** — a plausible-but-wrong reconstruction
/// destroys the audit value of every replay we hold.
#[derive(Clone, Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("this rules version cannot read that one; the replay is unreplayable")]
    Unsupported,
    #[error("snapshot decode failed: {0}")]
    Decode(CompactString),
}
