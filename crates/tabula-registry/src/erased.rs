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

use tabula_core::{BotLevel, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId};
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

    /// Lightweight decorative SVG for the Home/Library catalog-card consumer.
    ///
    /// Game-owned, compile-time artwork only: no scripts, external resources,
    /// runtime loading, rules/config facts or interactive state (I-9; doc 04 §3.2).
    /// Colors use semantic design roles; absence uses the shell's neutral art.
    fn catalog_cover_svg() -> Option<&'static str> {
        None
    }

    /// The setup form this game offers.
    fn form() -> &'static ConfigForm;

    /// Modes this build can actually construct for this game.
    fn modes() -> &'static [ModeSupport];

    /// Whether this adapter has a deployable local hot-seat document.
    /// The handoff resolver consumes this declaration; bot descriptors and rules
    /// capabilities alone never establish a gameplay runtime (ADR-011).
    fn local_document() -> bool {
        false
    }

    /// Deployable direct browser host declaration for the handoff resolver.
    /// Remains false until an actual online document and gateway are available.
    fn direct_document() -> bool {
        false
    }
    /// This package has a first-party direct-play presenter/transport contract.
    /// Consumed by explicit `RuntimeBinding` handoff and isolated gateway admission;
    /// eligibility alone never advertises a deployed document or grants a seat.
    fn direct_host_supported() -> bool {
        false
    }
    /// Configs the direct slice can execute, including its effect adapters.
    fn direct_config_supported(
        _config: &<<Self::Module as GameModule>::Rules as GameRules>::Config,
    ) -> bool {
        false
    }

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
    /// Game-owned lightweight decorative art for Home/Library cards only.
    /// This is trusted compile-time SVG, never user input or a runtime asset pack.
    fn catalog_cover_svg(&self) -> Option<&'static str>;
    fn form(&self) -> &'static ConfigForm;
    fn modes(&self) -> &'static [ModeSupport];
    /// Explicit direct browser host declaration, never capability inference.
    fn direct_document(&self) -> bool;
    /// Package eligibility for the explicitly opted-in direct host consumer.
    fn direct_host_supported(&self) -> bool;
    /// Parse a candidate direct draft without asserting runtime availability.
    /// The server must re-normalize the request and validate its actual roster.
    fn normalize_direct(
        &self,
        seats: u8,
        draft: &crate::ConfigDraft,
    ) -> Result<NormalizedConfig, ConfigRejection>;
    /// This game's own visible copy for one locale.
    fn messages(&self, locale: Locale) -> Messages;
    /// The package's declared bot policy levels, independent of linked factories.
    /// This inventory cannot establish a gameplay host's bot-mode support.
    fn bot_levels(&self) -> Vec<BotLevel>;
    /// Parse, plan the seats, and run the game's own validation.
    ///
    /// # Errors
    /// [`ConfigRejection`] from parsing, the seat plan, or the module.
    fn normalize(&self, request: &SetupRequest) -> Result<NormalizedConfig, ConfigRejection>;
    /// Construct an isolated canonical match authority through the typed module.
    /// This factory adds no network, migration, or service activation
    /// (ADR-0039; doc 02 §8).
    fn create_match(
        &self,
        config: &[u8],
        roster: &SeatRoster,
        seed: MatchSeed,
    ) -> Result<crate::runtime::CreatedMatch, crate::runtime::RuntimeError>;
    /// Decode a bounded server-only snapshot under its exact recorded package
    /// and rules identity (I-5/I-16; doc 05 §§7–8; ADR-0040). The actor verifies the
    /// snapshot's hash and replay history before publishing projected output.
    fn restore_match(
        &self,
        identity: &crate::runtime::RuntimeIdentity,
        config: &[u8],
        roster: &SeatRoster,
        snapshot: &[u8],
    ) -> Result<Box<dyn crate::runtime::ErasedMatch>, crate::runtime::RuntimeError>;
}

/// The single blanket bridge. One implementation, generic over the adapter.
#[derive(Debug, Default)]
pub struct Adapter<S: GameSetup>(core::marker::PhantomData<S>);

impl<S: GameSetup> Adapter<S> {
    /// Shared typed validation and serialization for both setup paths.
    /// The roster here is a hypothetical plan, never authenticated membership.
    fn normalize_parsed(
        request: &SetupRequest,
        parsed: ParseResult<S>,
    ) -> Result<NormalizedConfig, ConfigRejection> {
        let allowed = S::Module::capabilities().seats().allowed();
        if !allowed.contains(request.seats) {
            return Err(ConfigRejection::whole(RejectionReason::SeatCount));
        }

        let bot_level = match (request.mode.fills_with_bots(), request.bot_level) {
            (true, Some(level)) if S::Module::declared_bot_levels().contains(&level) => Some(level),
            (true, _) => return Err(ConfigRejection::whole(RejectionReason::Unsupported)),
            (false, _) => None,
        };

        let (config, mut summary, mut launch_args) = parsed?;
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

        let canonical_config = tabula_core::canonical_encode(&config)
            .map_err(|_| ConfigRejection::whole(RejectionReason::Unsupported))?;
        if canonical_config.len() > crate::runtime::MAX_RUNTIME_PAYLOAD_BYTES {
            return Err(ConfigRejection::whole(RejectionReason::Unsupported));
        }
        Ok(NormalizedConfig {
            summary,
            canonical_config,
            launch_args: args,
            local_return_to: (S::local_document() && request.mode == LaunchMode::LocalHotSeat)
                .then(|| format!("/games/{}?setup=1", metadata.id().as_str())),
        })
    }

    #[must_use]
    pub const fn new() -> Self {
        Self(core::marker::PhantomData)
    }
}

impl<S: GameSetup> ErasedGame for Adapter<S> {
    fn create_match(
        &self,
        config: &[u8],
        roster: &SeatRoster,
        seed: MatchSeed,
    ) -> Result<crate::runtime::CreatedMatch, crate::runtime::RuntimeError> {
        crate::runtime::TypedMatch::<S::Module>::create(config, roster, seed)
    }

    fn restore_match(
        &self,
        identity: &crate::runtime::RuntimeIdentity,
        config: &[u8],
        roster: &SeatRoster,
        snapshot: &[u8],
    ) -> Result<Box<dyn crate::runtime::ErasedMatch>, crate::runtime::RuntimeError> {
        crate::runtime::TypedMatch::<S::Module>::restore(identity, config, roster, snapshot)
    }

    fn metadata(&self) -> &'static GameMetadata {
        S::Module::metadata()
    }

    fn capabilities(&self) -> &'static GameCapabilities {
        S::Module::capabilities()
    }

    fn catalog_cover_svg(&self) -> Option<&'static str> {
        S::catalog_cover_svg()
    }

    fn form(&self) -> &'static ConfigForm {
        S::form()
    }

    fn modes(&self) -> &'static [ModeSupport] {
        S::modes()
    }

    fn direct_document(&self) -> bool {
        S::direct_document()
    }
    fn direct_host_supported(&self) -> bool {
        S::direct_host_supported()
    }
    fn normalize_direct(
        &self,
        seats: u8,
        draft: &crate::ConfigDraft,
    ) -> Result<NormalizedConfig, ConfigRejection> {
        if draft.keys().any(|key| S::form().field(key).is_none()) {
            return Err(ConfigRejection::whole(RejectionReason::Unsupported));
        }
        let parsed = S::parse(draft)?;
        if !S::direct_config_supported(&parsed.0) {
            return Err(ConfigRejection::whole(RejectionReason::Unsupported));
        }
        Self::normalize_parsed(
            &SetupRequest {
                mode: LaunchMode::Network,
                seats,
                bot_level: None,
                draft: draft.clone(),
            },
            Ok(parsed),
        )
    }
    fn messages(&self, locale: Locale) -> Messages {
        S::messages(locale)
    }

    fn bot_levels(&self) -> Vec<BotLevel> {
        S::Module::declared_bot_levels().to_vec()
    }

    fn normalize(&self, request: &SetupRequest) -> Result<NormalizedConfig, ConfigRejection> {
        let modes = S::modes();
        if !modes
            .iter()
            .any(|support| support.mode == request.mode && support.is_available())
        {
            return Err(ConfigRejection::whole(RejectionReason::Unsupported));
        }

        Self::normalize_parsed(request, S::parse(&request.draft))
    }
}

/// Build a hypothetical roster to validate a setup seat plan.
///
/// Seat 0 is the player at this device; bot modes fill every other seat with
/// the game's own bot. Direct setup uses placeholder humans only; the server
/// must validate its real authenticated roster again (doc 02 §4).
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

#[cfg(all(test, any(feature = "game-chess", feature = "game-tiles")))]
mod catalog_cover_tests {
    /// Current linked discovery modules ship small self-contained vector art.
    /// This checks the declarations, not arbitrary untrusted SVG safety.
    #[test]
    fn linked_catalog_covers_are_lightweight_static_semantic_art() {
        let games = crate::registered_games();
        assert!(
            !games.is_empty(),
            "feature-selected cover test must exercise a module"
        );
        for game in games {
            let cover = game
                .catalog_cover_svg()
                .expect("linked module has catalog art");
            assert!(cover.starts_with("<svg "));
            assert!(cover.len() < 8192, "catalog art must stay lightweight");
            assert!(cover.contains("var(--sys-color-shell-"));
            assert!(
                !cover.contains('#'),
                "source artwork has no raw color/fragment references"
            );
            for external_or_active in ["<script", "<image", "<foreignObject", "href=", "<style"] {
                assert!(
                    !cover.contains(external_or_active),
                    "catalog art is self-contained and inert"
                );
            }
        }
    }
}
