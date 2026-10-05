//! Resolving a validated configuration into a real handoff.
//!
//! ADR-011: gameplay is a **separate document**, never a canvas mounted inside
//! the shell's DOM runtime. The shell therefore produces a URL and navigates;
//! it does not construct a match.
//!
//! A server match id is minted by the authority, which does not exist at this
//! phase (doc 07 Phase 4). This module never invents one: a local session
//! resolves to the literal `local` session document, and a network session is
//! not resolvable until the adapter declares a deployed direct document.
//! ADR-0041 setup helpers alone do not establish such a runtime.

use crate::{availability::UnavailableReason, config::NormalizedConfig, i18n::Locale};

/// Where this build's gameplay document lives, if it is deployed at all.
///
/// `None` is the honest default: a shell build that was not given a gameplay
/// bundle cannot start a session, and says so rather than navigating nowhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeBinding {
    play_base: Option<&'static str>,
    direct_online: bool,
}

impl RuntimeBinding {
    /// No gameplay document is deployed with this shell.
    #[must_use]
    pub const fn unbound() -> Self {
        Self {
            play_base: None,
            direct_online: false,
        }
    }

    /// Bind only the same-origin `/play` document base.
    /// Other values remain unbound, including origins, query strings and dot
    /// segments. This opt-in is deployment configuration, not a redirect input.
    #[must_use]
    pub const fn bound(play_base: &'static str) -> Self {
        Self {
            play_base: Some(play_base),
            direct_online: false,
        }
    }

    /// Explicit deployment opt-in for the direct HTTP document (ADR-0041).
    /// Invalid or external bases remain unavailable. Ordinary `bound()` never opts a package into direct play.
    #[must_use]
    pub const fn direct_online(play_base: &'static str) -> Self {
        Self {
            play_base: Some(play_base),
            direct_online: true,
        }
    }

    /// The one generic consumer of deployment opt-in and package eligibility.
    /// This is a navigation fact, never authentication, permission or live-service proof.
    #[must_use]
    pub fn supports_direct(&self, game: &dyn crate::ErasedGame) -> bool {
        self.is_bound()
            && (game.direct_document() || (self.direct_online && game.direct_host_supported()))
    }

    #[must_use]
    pub fn is_bound(&self) -> bool {
        matches!(self.play_base, Some("/play" | "/play/"))
    }
}

/// A confirmed handoff target: a real document navigation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchHandoff {
    pub url: String,
}

/// Resolve the handoff for a normalized configuration.
///
/// # Errors
/// [`UnavailableReason::NoGameplayRuntime`] when this build has no gameplay
/// document to hand off to. The caller keeps the reason visible instead of
/// reporting a started match.
pub fn resolve(
    binding: RuntimeBinding,
    config: &NormalizedConfig,
) -> Result<LaunchHandoff, UnavailableReason> {
    resolve_with_locale(binding, config, Locale::En)
}

/// Resolve an immutable, validated local configuration and the shell's locale.
/// The registry selects the return target; no address-bar redirect is accepted.
pub fn resolve_with_locale(
    binding: RuntimeBinding,
    config: &NormalizedConfig,
    locale: Locale,
) -> Result<LaunchHandoff, UnavailableReason> {
    if !binding.is_bound() {
        return Err(UnavailableReason::NoGameplayRuntime);
    }
    let return_to = config
        .local_return_to
        .as_ref()
        .ok_or(UnavailableReason::NoModeRuntime)?;
    let mut args = config.launch_args.clone();
    args.extend([
        ("source".to_owned(), "tabula".to_owned()),
        ("return_to".to_owned(), return_to.clone()),
        ("locale".to_owned(), locale.tag().to_owned()),
    ]);
    let query = args
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    Ok(LaunchHandoff {
        url: format!("/play/local/?{query}"),
    })
}

/// Percent-encode everything outside the unreserved set.
///
/// Launch arguments are adapter-produced configuration, never a credential or
/// a player identity (docs/ui/screens/discovery.md).
fn encode(value: &str) -> String {
    use core::fmt::Write as _;

    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            other => {
                let _ = write!(out, "%{other:02X}");
            }
        }
    }
    out
}

/// Resolve direct play using a non-secret match routing hint only.
/// Grants, attachments, sequences and account identities never enter this URL.
pub fn resolve_direct(
    binding: RuntimeBinding,
    game: &dyn crate::ErasedGame,
    match_id: &str,
    locale: Locale,
) -> Result<LaunchHandoff, UnavailableReason> {
    if !binding.is_bound() {
        return Err(UnavailableReason::NoGameplayRuntime);
    }
    if !binding.supports_direct(game)
        || match_id.len() != 32
        || !match_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !u128::from_str_radix(match_id, 16).is_ok_and(|id| id != 0)
    {
        return Err(UnavailableReason::NoModeRuntime);
    }
    let id = game.metadata().id().as_str();
    Ok(LaunchHandoff { url: format!("/play/local/?game={}&mode=network&seats={}&source=tabula&return_to={}&locale={}&match_id={}", encode(id), game.capabilities().seats().allowed().min(), encode(&format!("/games/{id}?setup=1")), locale.tag(), match_id) })
}

#[cfg(all(test, feature = "game-chess", feature = "game-tiles"))]
mod direct_tests {
    use super::*;
    use crate::{games, Adapter, ConfigDraft, ErasedGame};
    // Test-only deployed host so URL rejection reaches the hint validator.
    struct DeployedDirectSetup;
    impl crate::GameSetup for DeployedDirectSetup {
        type Module = <games::chess::ChessSetup as crate::GameSetup>::Module;
        fn form() -> &'static crate::ConfigForm {
            games::chess::ChessSetup::form()
        }
        fn modes() -> &'static [crate::ModeSupport] {
            games::chess::ChessSetup::modes()
        }
        fn direct_document() -> bool {
            true
        }
        fn messages(locale: Locale) -> crate::i18n::Messages {
            games::chess::ChessSetup::messages(locale)
        }
        fn parse(draft: &ConfigDraft) -> crate::erased::ParseResult<Self> {
            games::chess::ChessSetup::parse(draft)
        }
    }
    #[test]
    fn selected_game_owns_direct_support_and_untimed_config() {
        let game = Adapter::<games::chess::ChessSetup>::new();
        let mut draft = ConfigDraft::with_defaults(game.form());
        let direct = game.normalize_direct(2, &draft).unwrap();
        assert_eq!(
            direct.canonical_config(),
            tabula_core::canonical_encode(&tabula_game_chess::Config { clock: None }).unwrap()
        );
        for seats in [0, 1, 3, u8::MAX] {
            assert_eq!(
                game.normalize_direct(seats, &draft),
                Err(crate::ConfigRejection::whole(
                    crate::RejectionReason::SeatCount
                ))
            );
        }
        assert_eq!(
            resolve_direct(
                RuntimeBinding::bound("/play"),
                &game,
                "00000000000000000000000000000007",
                Locale::En
            ),
            Err(UnavailableReason::NoModeRuntime)
        );
        draft.set("clock", "fischer");
        assert!(game.normalize_direct(2, &draft).is_err());
        let other = Adapter::<games::tiles::TilesSetup>::new();
        assert!(!other.direct_document());
        assert!(other
            .normalize_direct(2, &ConfigDraft::with_defaults(other.form()))
            .is_err());
    }
    #[test]
    fn public_match_hint_cannot_supply_redirect_or_grant() {
        let game = Adapter::<DeployedDirectSetup>::new();
        let valid = "00000000000000000000000000000007";
        let handoff =
            resolve_direct(RuntimeBinding::bound("/play"), &game, valid, Locale::Vi).unwrap();
        assert_eq!(handoff.url, format!("/play/local/?game={}&mode=network&seats=2&source=tabula&return_to={}&locale=vi&match_id={valid}", encode(game.metadata().id().as_str()), encode(&format!("/games/{}?setup=1", game.metadata().id().as_str()))));
        for id in [
            "../account",
            "0000000000000000000000000000000A",
            "7",
            "00000000000000000000000000000000",
            "00000000000000000000000000000007&binding_id=x",
        ] {
            assert!(resolve_direct(RuntimeBinding::bound("/play"), &game, id, Locale::En).is_err());
        }
        assert!(resolve_direct(
            RuntimeBinding::unbound(),
            &game,
            "00000000000000000000000000000007",
            Locale::En
        )
        .is_err());
    }
    #[test]
    fn explicit_direct_binding_keeps_default_gates_and_checks_package_and_config() {
        let game = Adapter::<games::chess::ChessSetup>::new();
        let valid = "00000000000000000000000000000007";
        assert!(!game.direct_document());
        assert!(game.direct_host_supported());
        assert!(!RuntimeBinding::bound("/play").supports_direct(&game));
        for base in ["/play", "/play/"] {
            let binding = RuntimeBinding::direct_online(base);
            assert!(binding.supports_direct(&game));
            assert!(resolve_direct(binding, &game, valid, Locale::En)
                .unwrap()
                .url
                .contains("mode=network"));
        }
        for base in [
            "https://foreign.example/play",
            "/play/../",
            "/other",
            "/play?x=1",
        ] {
            let binding = RuntimeBinding::direct_online(base);
            assert!(!binding.supports_direct(&game));
            assert_eq!(
                resolve_direct(binding, &game, valid, Locale::En),
                Err(UnavailableReason::NoGameplayRuntime)
            );
        }
        let other = Adapter::<games::tiles::TilesSetup>::new();
        assert!(!RuntimeBinding::direct_online("/play").supports_direct(&other));
        assert_eq!(
            resolve_direct(
                RuntimeBinding::direct_online("/play"),
                &other,
                valid,
                Locale::En
            ),
            Err(UnavailableReason::NoModeRuntime)
        );
        let mut timed = ConfigDraft::with_defaults(game.form());
        timed.set("clock", "fischer");
        assert!(game.normalize_direct(2, &timed).is_err());
    }

    #[test]
    fn direct_draft_rejects_unknown_form_keys() {
        let game = Adapter::<games::chess::ChessSetup>::new();
        let mut draft = ConfigDraft::with_defaults(game.form());
        draft.set("unrecognized", "ignored is unsafe");
        assert!(game.normalize_direct(2, &draft).is_err());
    }
    #[test]
    fn direct_setup_describes_network_without_enabling_missing_document() {
        let game = Adapter::<games::chess::ChessSetup>::new();
        let direct = game
            .normalize_direct(2, &ConfigDraft::with_defaults(game.form()))
            .unwrap();
        assert_eq!(
            direct
                .launch_args()
                .iter()
                .find(|(key, _)| key == "mode")
                .unwrap()
                .1,
            "network"
        );
        assert!(direct
            .summary()
            .iter()
            .any(|line| line.label_key == "setup.summary.mode"
                && line.value == crate::SummaryValue::Key(crate::LaunchMode::Network.label_key())));
        assert!(resolve(RuntimeBinding::bound("/play"), &direct).is_err());
        assert!(!game.direct_document());
        assert_eq!(
            resolve_direct(
                RuntimeBinding::bound("/play"),
                &game,
                "00000000000000000000000000000007",
                Locale::En
            ),
            Err(UnavailableReason::NoModeRuntime)
        );
    }
}
