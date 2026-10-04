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

    /// The authoritative rules-half identity used by canonical replay headers.
    ///
    /// The default is deliberately unusable for an `Exact` replay verdict. A
    /// real game supplies [`GameRules::RULES_HASH`] from a stable build-time
    /// source hash rather than having the testkit guess from debug output or a
    /// binary path. (doc 05 §6.2)
    fn rules_hash() -> [u8; 32] {
        <Self::Rules as GameRules>::RULES_HASH
    }

    /// `&'static` because metadata is generated from `game.toml` at build time
    /// and never varies at runtime. (doc 02 §10.2)
    fn metadata() -> &'static GameMetadata;
    fn capabilities() -> &'static GameCapabilities;

    /// Bot policy levels implemented by this package, in selector order.
    ///
    /// This immutable inventory is available without linking the optional
    /// `bots` feature, so discovery/setup never needs to construct a policy.
    /// With `bots` enabled, each declared level must have a [`Self::bot`]
    /// factory and undeclared levels must not. The inventory alone does not
    /// establish that a gameplay host can run a bot mode. (doc 02 §4, §6)
    fn declared_bot_levels() -> &'static [BotLevel] {
        &[]
    }

    /// Optional bot policies. Server-side; consumes projections only. (doc 02 §6)
    ///
    /// A `Trivial` bot is free for any game that implements `legal_commands`,
    /// and that alone unlocks auto-fill and self-play fuzzing.
    /// A build without `bots` may return `None` for a declared policy level.
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
