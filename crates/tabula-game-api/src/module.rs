//! `GameModule` — the package around the rules. (doc 02 §4)
//!
//! `GameRules` is the maths. `GameModule` is the package: identity,
//! capabilities, bots, and config validation.
//!
//! Client-side presentation is deliberately a **separate trait in a separate
//! crate** (`tabula_presentation::GamePresentation`) so the server never links
//! it. That split is I-1 in practice, not cosmetics: the server compiles a game
//! without a renderer, the client compiles it without a database.

use tabula_core::{BotLevel, SeatRoster};

use crate::{
    bot::GameBot, capabilities::GameCapabilities, error::ConfigError, metadata::GameMetadata,
    rules::GameRules,
};

pub trait GameModule: Send + Sync + 'static {
    type Rules: GameRules;

    /// `&'static` because metadata is generated from `game.toml` at build time
    /// and never varies at runtime. (doc 02 §10.2)
    fn metadata() -> &'static GameMetadata;
    fn capabilities() -> &'static GameCapabilities;

    /// Optional bot policies. Server-side; consumes projections only. (doc 02 §6)
    ///
    /// A `Trivial` bot is free for any game that implements `legal_commands`,
    /// and that alone unlocks auto-fill and self-play fuzzing.
    fn bot(_level: BotLevel) -> Option<Box<dyn GameBot<Self::Rules>>> {
        None
    }

    /// Validate and normalise a lobby-supplied config **before** match creation.
    ///
    /// The platform calls this so a bad config fails at creation, not mid-match.
    /// It is the game's chance to reject a seat count its role set cannot balance,
    /// or a time control that makes no sense.
    ///
    /// # Errors
    /// [`ConfigError`] naming the offending field, so the lobby can highlight it.
    fn validate_config(
        cfg: &<Self::Rules as GameRules>::Config,
        roster: &SeatRoster,
    ) -> Result<(), ConfigError>;
}
