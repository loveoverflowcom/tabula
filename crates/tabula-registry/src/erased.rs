//! Type erasure: the one bridge between typed games and the game-agnostic
//! platform (doc 02 §8).
//!
//! ```text
//! typed world                 the bridge                game-agnostic shell
//! ChessModule: GameModule → Adapter<ChessSetup> → &dyn ErasedGame
//! ```
//!
//! There is exactly one implementation of [`ErasedGame`], generic over the
//! adapter. A game is named only by its [`GameSetup`] implementation inside
//! this crate's `games` module, which is why no shell crate can branch on a
//! game id (I-9).

use tabula_core::{BotLevel, Occupant, SeatEntry, SeatId, SeatRoster, UserId};
use tabula_game_api::{ConfigError, GameCapabilities, GameMetadata, GameModule, GameRules};

use crate::{
    availability::{LaunchMode, ModeSupport},
    config::{ConfigForm, ConfigRejection, NormalizedConfig, RejectionReason, SummaryLine},
    i18n::{Locale, Messages},
};

/// Everything the shell needs to turn a draft into a validated configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupRequest {
    pub mode: LaunchMode,
    pub seats: u8,
    /// Chosen only when the mode fills seats with bots.
    pub bot_level: Option<BotLevel>,
    pub draft: crate::config::ConfigDraft,
}

/// The game-side half of the bridge: one impl per game, inside this crate.
///
/// Implementations own field meaning and summary wording. They never own
/// validation: the typed value they build is handed to the game's own
/// [`GameModule::validate_config`] by [`Adapter`].
pub trait GameSetup: Send + Sync + 'static {
    type Module: GameModule;

    /// The setup form this game offers.
    fn form() -> &'static ConfigForm;

    /// Modes this build can actually construct for this game.
    fn modes() -> &'static [ModeSupport];

    /// This game's own visible copy, including its metadata message keys.
    fn messages(locale: Locale) -> Messages;

    /// Translate a field name used by the game's own `ConfigError` into the
    /// form key the shell can focus. The default assumes they are the same.
    fn field_key(_module_field: &str) -> Option<&'static str> {
        None
    }

    /// Parse the draft into the game's typed config plus its summary lines.
    ///
    /// # Errors
    /// [`ConfigRejection`] naming the offending form field.
    fn parse(draft: &crate::config::ConfigDraft) -> ParseResult<Self>;
}

/// What an adapter produces from a draft: the game's typed config, the lines
/// that summarize it, and the launch arguments the gameplay document receives.
pub type ParseResult<S> = Result<
    (
        <<<S as GameSetup>::Module as GameModule>::Rules as GameRules>::Config,
        Vec<SummaryLine>,
        Vec<(String, String)>,
    ),
    ConfigRejection,
>;

/// The game-agnostic façade the catalog and the shell consume.
pub trait ErasedGame: Send + Sync {
    fn metadata(&self) -> &'static GameMetadata;
    fn capabilities(&self) -> &'static GameCapabilities;
    fn form(&self) -> &'static ConfigForm;
    fn modes(&self) -> &'static [ModeSupport];
    /// This game's own visible copy for one locale.
    fn messages(&self, locale: Locale) -> Messages;
    /// Bot levels whose factory this build actually links.
    fn bot_levels(&self) -> Vec<BotLevel>;
    /// Parse, plan the seats, and run the game's own validation.
    ///
    /// # Errors
    /// [`ConfigRejection`] from parsing, the seat plan, or the module.
    fn normalize(&self, request: &SetupRequest) -> Result<NormalizedConfig, ConfigRejection>;
}

/// The single blanket bridge. One implementation, generic over the adapter.
#[derive(Debug, Default)]
pub struct Adapter<S: GameSetup>(core::marker::PhantomData<S>);

impl<S: GameSetup> Adapter<S> {
    #[must_use]
    pub const fn new() -> Self {
        Self(core::marker::PhantomData)
    }
}

impl<S: GameSetup> ErasedGame for Adapter<S> {
    fn metadata(&self) -> &'static GameMetadata {
        S::Module::metadata()
    }

    fn capabilities(&self) -> &'static GameCapabilities {
        S::Module::capabilities()
    }

    fn form(&self) -> &'static ConfigForm {
        S::form()
    }

    fn modes(&self) -> &'static [ModeSupport] {
        S::modes()
    }

    fn messages(&self, locale: Locale) -> Messages {
        S::messages(locale)
    }

    fn bot_levels(&self) -> Vec<BotLevel> {
        [
            BotLevel::Trivial,
            BotLevel::Easy,
            BotLevel::Medium,
            BotLevel::Hard,
        ]
        .into_iter()
        .filter(|level| S::Module::bot(*level).is_some())
        .collect()
    }

    fn normalize(&self, request: &SetupRequest) -> Result<NormalizedConfig, ConfigRejection> {
        let modes = S::modes();
        if !modes
            .iter()
            .any(|support| support.mode == request.mode && support.is_available())
        {
            return Err(ConfigRejection::whole(RejectionReason::Unsupported));
        }

        let allowed = S::Module::capabilities().seats().allowed();
        if !allowed.contains(request.seats) {
            return Err(ConfigRejection::whole(RejectionReason::SeatCount));
        }

        let bot_level = match (request.mode.fills_with_bots(), request.bot_level) {
            (true, Some(level)) if S::Module::bot(level).is_some() => Some(level),
            (true, _) => return Err(ConfigRejection::whole(RejectionReason::Unsupported)),
            (false, _) => None,
        };

        let (config, mut summary, mut launch_args) = S::parse(&request.draft)?;
        let roster = roster_for(request.seats, bot_level);

        S::Module::validate_config(&config, &roster).map_err(map_config_error::<S>)?;

        summary.insert(
            0,
            SummaryLine {
                label_key: "setup.summary.seats",
                value: crate::config::SummaryValue::Seats {
                    count: u64::from(request.seats),
                },
            },
        );
        summary.insert(
            0,
            SummaryLine {
                label_key: "setup.summary.mode",
                value: crate::config::SummaryValue::Key(request.mode.label_key()),
            },
        );

        let metadata = S::Module::metadata();
        let mut args = vec![
            ("game".to_owned(), metadata.id().as_str().to_owned()),
            ("mode".to_owned(), request.mode.as_str().to_owned()),
            ("seats".to_owned(), request.seats.to_string()),
        ];
        if let Some(level) = bot_level {
            args.push(("bot".to_owned(), bot_level_arg(level).to_owned()));
        }
        args.append(&mut launch_args);

        Ok(NormalizedConfig {
            summary,
            launch_args: args,
        })
    }
}

/// Build the roster a local seat plan resolves to.
///
/// Seat 0 is the player at this device; bot modes fill every other seat with
/// the game's own bot. Network seat plans are not occupied rosters and are not
/// built here (docs/ui/screens/03-new-match.md).
fn roster_for(seats: u8, bot_level: Option<BotLevel>) -> SeatRoster {
    SeatRoster::new(
        (0..seats)
            .map(|index| SeatEntry {
                seat: SeatId(index),
                occupant: match bot_level {
                    Some(level) if index > 0 => Occupant::Bot { level },
                    _ => Occupant::Human(UserId(u128::from(index) + 1)),
                },
                team: None,
            })
            .collect(),
    )
    .expect("seat indices 0..seats are distinct")
}

/// Map a game's own rejection onto a form-field rejection.
///
/// `ConfigError::Field` already names the offending field; game adapters use
/// their form keys as those names so the shell can focus the right input.
fn map_config_error<S: GameSetup>(error: ConfigError) -> ConfigRejection {
    match error {
        ConfigError::SeatCount => ConfigRejection::whole(RejectionReason::SeatCount),
        ConfigError::Field(name) => ConfigRejection::field(
            S::field_key(name.as_str()).unwrap_or(name.as_str()),
            RejectionReason::ModuleField,
        ),
        ConfigError::Unsupported(_) => ConfigRejection::whole(RejectionReason::Unsupported),
    }
}

/// Stable textual bot level for launch arguments.
const fn bot_level_arg(level: BotLevel) -> &'static str {
    match level {
        BotLevel::Trivial => "trivial",
        BotLevel::Easy => "easy",
        BotLevel::Medium => "medium",
        BotLevel::Hard => "hard",
    }
}

/// Localized bot level label key, for the shell's selector and summary.
#[must_use]
pub const fn bot_level_label_key(level: BotLevel) -> &'static str {
    match level {
        BotLevel::Trivial => "bot.trivial",
        BotLevel::Easy => "bot.easy",
        BotLevel::Medium => "bot.medium",
        BotLevel::Hard => "bot.hard",
    }
}
