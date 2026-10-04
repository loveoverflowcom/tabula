//! Catalog, erased setup, and handoff behavior.
//!
//! The oracles here are the games' own contracts: the seat sets and duration
//! estimates the modules declare, and the clock/deadline validators the rules
//! own. A test that only re-asserted this crate's own constants could not
//! detect the defect these guard against — a shell option the game would
//! reject, or a value silently corrected on its way to the rules.

use std::collections::BTreeMap;

use tabula_core::BotLevel;
use tabula_game_api::{
    metadata::{Complexity, I18nKey},
    GameId,
};

use crate::{
    availability::{LaunchMode, UnavailableReason},
    catalog::{Catalog, CatalogQuery, Localizer},
    config::{ConfigDraft, RejectionReason, SummaryValue, TimeControlKind},
    erased::SetupRequest,
    launch::{resolve, RuntimeBinding},
};

/// A message table standing in for the shell's locale data.
struct Stub(BTreeMap<String, String>);

impl Stub {
    fn vi() -> Self {
        let mut table = BTreeMap::new();
        table.insert("game.chess.name".to_owned(), "Cờ vua".to_owned());
        table.insert(
            "game.chess.tagline".to_owned(),
            "Ván cờ kinh điển".to_owned(),
        );
        table.insert("game.tiles.name".to_owned(), "Xếp mảnh".to_owned());
        table.insert(
            "game.tiles.tagline".to_owned(),
            "Đặt mảnh ghép bản đồ".to_owned(),
        );
        Self(table)
    }
}

impl Localizer for Stub {
    fn text(&self, key: &I18nKey) -> Option<&str> {
        self.0.get(key.as_str()).map(String::as_str)
    }
}

fn chess_id() -> GameId {
    GameId::new("com.tabula.chess").expect("literal id")
}

fn tiles_id() -> GameId {
    GameId::new("com.tabula.tiles").expect("literal id")
}

fn draft(pairs: &[(&str, &str)]) -> ConfigDraft {
    let mut draft = ConfigDraft::new();
    for (key, value) in pairs {
        draft.set(key, *value);
    }
    draft
}

fn local(seats: u8, pairs: &[(&str, &str)]) -> SetupRequest {
    SetupRequest {
        mode: LaunchMode::LocalHotSeat,
        seats,
        bot_level: None,
        draft: draft(pairs),
    }
}

#[test]
fn catalog_lists_every_linked_game_once() {
    let catalog = crate::catalog(&Stub::vi());
    let ids: Vec<&str> = catalog
        .entries()
        .iter()
        .map(|entry| entry.id().as_str())
        .collect();
    assert_eq!(ids, vec!["com.tabula.chess", "com.tabula.tiles"]);
}

#[test]
fn catalog_orders_by_folded_localized_title() {
    // "Cờ vua" folds to "co vua" and "Xếp mảnh" to "xep manh": c before x,
    // which is the opposite of the raw code-point order of 'C' and 'X' only by
    // accident — the point is that ordering reads the localized title, not the
    // id. Swapping the titles swaps the order.
    let mut table = Stub::vi();
    table
        .0
        .insert("game.chess.name".to_owned(), "Zèbre".to_owned());
    let catalog = Catalog::new(crate::registered_games(), &table);
    let ids: Vec<&str> = catalog
        .entries()
        .iter()
        .map(|entry| entry.id().as_str())
        .collect();
    assert_eq!(ids, vec!["com.tabula.tiles", "com.tabula.chess"]);
}

#[test]
fn seat_filter_uses_the_modules_declared_counts() {
    let localizer = Stub::vi();
    let catalog = crate::catalog(&localizer);

    let two = catalog.query(
        &CatalogQuery {
            players: Some(2),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert_eq!(two.len(), 2, "both games seat two players");

    let three = catalog.query(
        &CatalogQuery {
            players: Some(3),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert_eq!(
        three.iter().map(|e| e.id().as_str()).collect::<Vec<_>>(),
        vec!["com.tabula.tiles"],
        "chess declares exactly two seats"
    );

    let six = catalog.query(
        &CatalogQuery {
            players: Some(6),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert!(six.is_empty(), "no linked game seats six players");
}

#[test]
fn duration_filter_is_a_budget_on_the_estimated_maximum() {
    let localizer = Stub::vi();
    let catalog = crate::catalog(&localizer);
    let chess_max = catalog
        .get(&chess_id())
        .expect("chess is linked")
        .game()
        .metadata()
        .estimated_minutes()
        .max();

    let inside = catalog.query(
        &CatalogQuery {
            max_minutes: Some(chess_max),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert!(inside.iter().any(|entry| entry.id() == &chess_id()));

    let outside = catalog.query(
        &CatalogQuery {
            max_minutes: Some(chess_max - 1),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert!(!outside.iter().any(|entry| entry.id() == &chess_id()));
}

#[test]
fn category_and_complexity_filters_read_metadata() {
    let localizer = Stub::vi();
    let catalog = crate::catalog(&localizer);
    let chess = catalog.get(&chess_id()).expect("chess is linked");
    let category = chess.game().metadata().categories()[0];
    let complexity = chess.game().metadata().complexity();

    let by_category = catalog.query(
        &CatalogQuery {
            category: Some(category),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert!(by_category.iter().any(|entry| entry.id() == &chess_id()));

    let impossible = catalog.query(
        &CatalogQuery {
            category: Some(category),
            complexity: Some(opposite(complexity)),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert!(
        !impossible.iter().any(|entry| entry.id() == &chess_id()),
        "independent axes combine with AND"
    );
}

fn opposite(complexity: Complexity) -> Complexity {
    match complexity {
        Complexity::Light => Complexity::Heavy,
        _ => Complexity::Light,
    }
}

#[test]
fn search_folds_case_and_vietnamese_tone_marks() {
    let localizer = Stub::vi();
    let catalog = crate::catalog(&localizer);
    for needle in ["Cờ vua", "co vua", "CO VUA", "  cờ  "] {
        let found = catalog.query(
            &CatalogQuery {
                text: Some(needle.to_owned()),
                ..CatalogQuery::default()
            },
            &localizer,
        );
        assert_eq!(
            found.iter().map(|e| e.id().as_str()).collect::<Vec<_>>(),
            vec!["com.tabula.chess"],
            "searching {needle:?}"
        );
    }
}

#[test]
fn search_matches_declared_tags() {
    let localizer = Stub::vi();
    let catalog = crate::catalog(&localizer);
    let tag = catalog
        .get(&chess_id())
        .expect("chess is linked")
        .game()
        .metadata()
        .tags()[0]
        .clone();
    let found = catalog.query(
        &CatalogQuery {
            text: Some(tag),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert!(found.iter().any(|entry| entry.id() == &chess_id()));
}

#[test]
fn an_unknown_well_formed_id_is_not_found_rather_than_a_fallback() {
    let catalog = crate::catalog(&Stub::vi());
    let unknown = GameId::new("com.tabula.xiangqi").expect("literal id");
    assert!(catalog.get(&unknown).is_none());
}

#[test]
fn network_mode_is_unavailable_everywhere_at_this_phase() {
    let localizer = Stub::vi();
    let catalog = crate::catalog(&localizer);
    for entry in catalog.entries() {
        let network = entry
            .modes()
            .iter()
            .find(|support| support.mode == LaunchMode::Network)
            .expect("every game declares the network mode");
        assert_eq!(
            network.unavailable_reason(),
            Some(UnavailableReason::NoNetworkService)
        );
    }
    let filtered = catalog.query(
        &CatalogQuery {
            mode: Some(LaunchMode::Network),
            ..CatalogQuery::default()
        },
        &localizer,
    );
    assert!(filtered.is_empty(), "an unavailable mode matches no entry");
}

#[test]
fn chess_links_exactly_the_bot_levels_it_builds() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked");
    assert_eq!(
        chess.game().bot_levels(),
        vec![BotLevel::Trivial, BotLevel::Easy]
    );
}

#[test]
fn chess_untimed_and_timed_configurations_normalize_distinctly() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();

    let untimed = chess
        .normalize(&local(2, &[("clock", "untimed")]))
        .expect("untimed chess is valid");
    assert!(untimed.summary.iter().any(|line| line.value
        == SummaryValue::TimeControl {
            initial_ms: 0,
            control: TimeControlKind::Untimed
        }));

    let fischer = chess
        .normalize(&local(
            2,
            &[
                ("clock", "fischer"),
                ("initial_minutes", "5"),
                ("increment_seconds", "2"),
            ],
        ))
        .expect("5+2 Fischer is valid");
    let bronstein = chess
        .normalize(&local(
            2,
            &[
                ("clock", "bronstein"),
                ("initial_minutes", "5"),
                ("delay_seconds", "2"),
            ],
        ))
        .expect("5+2 Bronstein is valid");
    assert_ne!(
        fischer.summary, bronstein.summary,
        "an increment and a delay must not summarize identically"
    );
    assert!(fischer.summary.iter().any(|line| line.value
        == SummaryValue::TimeControl {
            initial_ms: 300_000,
            control: TimeControlKind::Increment { millis: 2_000 }
        }));
    assert!(fischer
        .launch_args
        .contains(&("initial_ms".to_owned(), "300000".to_owned())));
}

#[test]
fn chess_rejects_a_zero_initial_clock_through_its_own_validator() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();
    let rejection = chess
        .normalize(&local(
            2,
            &[
                ("clock", "fischer"),
                ("initial_minutes", "0"),
                ("increment_seconds", "2"),
            ],
        ))
        .expect_err("the rules reject a zero initial budget");
    assert_eq!(rejection.reason, RejectionReason::ModuleField);
    assert_eq!(rejection.field.as_deref(), Some("clock"));
}

#[test]
fn chess_rejects_unparseable_and_out_of_range_input_without_clamping() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();

    for bad in ["5.5", "-1", "five", "5 min", ""] {
        let rejection = chess
            .normalize(&local(
                2,
                &[
                    ("clock", "fischer"),
                    ("initial_minutes", bad),
                    ("increment_seconds", "2"),
                ],
            ))
            .expect_err("{bad} is not a whole number of minutes");
        assert_eq!(rejection.field.as_deref(), Some("initial_minutes"));
        assert!(matches!(
            rejection.reason,
            RejectionReason::NotANumber | RejectionReason::Missing
        ));
    }

    let rejection = chess
        .normalize(&local(
            2,
            &[
                ("clock", "fischer"),
                ("initial_minutes", "181"),
                ("increment_seconds", "2"),
            ],
        ))
        .expect_err("181 minutes is outside the form range");
    assert_eq!(
        rejection.reason,
        RejectionReason::OutOfRange { min: 0, max: 180 }
    );
}

#[test]
fn the_chess_form_range_cannot_produce_a_config_the_rules_call_unrepresentable() {
    // The rules reject an initial budget whose sum with the increment or delay
    // overflows. The form's own bounds must stay inside that law, so the
    // extreme corners of the range still normalize.
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();
    for control in ["fischer", "bronstein"] {
        let field = if control == "fischer" {
            "increment_seconds"
        } else {
            "delay_seconds"
        };
        chess
            .normalize(&local(
                2,
                &[
                    ("clock", control),
                    ("initial_minutes", "180"),
                    (field, "60"),
                ],
            ))
            .expect("the maximum the form offers is representable");
    }
}

#[test]
fn chess_rejects_a_seat_count_it_does_not_declare() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();
    for seats in [1, 3] {
        let rejection = chess
            .normalize(&local(seats, &[("clock", "untimed")]))
            .expect_err("chess seats exactly two");
        assert_eq!(rejection.reason, RejectionReason::SeatCount);
    }
}

#[test]
fn an_unavailable_mode_cannot_be_normalized() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();
    let rejection = chess
        .normalize(&SetupRequest {
            mode: LaunchMode::Network,
            seats: 2,
            bot_level: None,
            draft: draft(&[("clock", "untimed")]),
        })
        .expect_err("no network service exists at this phase");
    assert_eq!(rejection.reason, RejectionReason::Unsupported);
}

#[test]
fn a_bot_mode_requires_a_level_this_build_actually_links() {
    let catalog = crate::catalog(&Stub::vi());
    let game = catalog.get(&tiles_id()).expect("tiles is linked").game();

    let missing = game
        .normalize(&SetupRequest {
            mode: LaunchMode::LocalBots,
            seats: 2,
            bot_level: None,
            draft: draft(&[("deadline", "none")]),
        })
        .expect_err("a bot mode needs a level");
    assert_eq!(missing.reason, RejectionReason::Unsupported);

    let unlinked = game
        .normalize(&SetupRequest {
            mode: LaunchMode::LocalBots,
            seats: 2,
            bot_level: Some(BotLevel::Hard),
            draft: draft(&[("deadline", "none")]),
        })
        .expect_err("tiles builds no Hard bot");
    assert_eq!(unlinked.reason, RejectionReason::Unsupported);

    let linked = game
        .normalize(&SetupRequest {
            mode: LaunchMode::LocalBots,
            seats: 2,
            bot_level: Some(BotLevel::Easy),
            draft: draft(&[("deadline", "none")]),
        })
        .expect("tiles builds an Easy bot");
    assert!(linked
        .launch_args
        .contains(&("bot".to_owned(), "easy".to_owned())));
}

#[test]
fn tiles_deadline_boundaries_follow_the_rules_floor() {
    let catalog = crate::catalog(&Stub::vi());
    let tiles = catalog.get(&tiles_id()).expect("tiles is linked").game();

    tiles
        .normalize(&local(3, &[("deadline", "none")]))
        .expect("no deadline is valid");
    tiles
        .normalize(&local(
            3,
            &[("deadline", "timed"), ("deadline_seconds", "5")],
        ))
        .expect("5,000 ms is the lowest accepted deadline");

    let below = tiles
        .normalize(&local(
            3,
            &[("deadline", "timed"), ("deadline_seconds", "4")],
        ))
        .expect_err("4,000 ms is below the rules floor");
    assert_eq!(below.reason, RejectionReason::ModuleField);
    assert_eq!(
        below.field.as_deref(),
        Some("deadline_seconds"),
        "the rules name turn_deadline_ms; the player sees the seconds field"
    );

    let zero = tiles
        .normalize(&local(
            3,
            &[("deadline", "timed"), ("deadline_seconds", "0")],
        ))
        .expect_err("zero seconds is not a timed deadline");
    assert_eq!(
        zero.reason,
        RejectionReason::OutOfRange {
            min: 1,
            max: 86_400
        }
    );
}

#[test]
fn tiles_accepts_every_seat_count_it_declares_and_no_other() {
    let catalog = crate::catalog(&Stub::vi());
    let tiles = catalog.get(&tiles_id()).expect("tiles is linked").game();
    for seats in 2..=5 {
        tiles
            .normalize(&local(seats, &[("deadline", "none")]))
            .expect("tiles seats two to five");
    }
    for seats in [1, 6] {
        let rejection = tiles
            .normalize(&local(seats, &[("deadline", "none")]))
            .expect_err("outside the declared seat set");
        assert_eq!(rejection.reason, RejectionReason::SeatCount);
    }
}

#[test]
fn a_form_hides_the_fields_the_selected_choice_does_not_reveal() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();
    let form = chess.form();

    let visible = |value: &str| {
        form.visible_fields(&draft(&[("clock", value)]))
            .into_iter()
            .map(|field| field.key)
            .collect::<Vec<_>>()
    };
    assert_eq!(visible("untimed"), vec!["clock"]);
    assert_eq!(
        visible("fischer"),
        vec!["clock", "initial_minutes", "increment_seconds"]
    );
    assert_eq!(
        visible("bronstein"),
        vec!["clock", "initial_minutes", "delay_seconds"]
    );
}

#[test]
fn defaults_come_from_the_form_rather_than_the_shell() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();
    let defaults = ConfigDraft::with_defaults(chess.form());
    assert_eq!(defaults.get("clock"), Some("untimed"));
    chess
        .normalize(&SetupRequest {
            mode: LaunchMode::LocalHotSeat,
            seats: 2,
            bot_level: None,
            draft: defaults,
        })
        .expect("a game's own defaults validate");
}

#[test]
fn an_unbound_build_reports_no_gameplay_runtime_instead_of_navigating() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();
    let config = chess
        .normalize(&local(2, &[("clock", "untimed")]))
        .expect("valid config");
    assert_eq!(
        resolve(RuntimeBinding::unbound(), &config),
        Err(UnavailableReason::NoGameplayRuntime)
    );
}

#[test]
fn a_bound_build_hands_off_to_a_separate_document_with_encoded_arguments() {
    let catalog = crate::catalog(&Stub::vi());
    let chess = catalog.get(&chess_id()).expect("chess is linked").game();
    let config = chess
        .normalize(&local(2, &[("clock", "untimed")]))
        .expect("valid config");
    let handoff = resolve(RuntimeBinding::bound("/play"), &config).expect("a bound build resolves");
    assert!(handoff.url.starts_with("/play/local/?"));
    assert!(handoff.url.contains("game=com.tabula.chess"));
    assert!(handoff.url.contains("mode=local"));
    assert!(!handoff.url.contains(' '));
}

#[test]
fn local_runtime_binding_cannot_redirect_to_another_origin_or_route() {
    let catalog = crate::catalog(&Stub::vi());
    let config = catalog
        .get(&chess_id())
        .unwrap()
        .game()
        .normalize(&local(2, &[("clock", "untimed")]))
        .unwrap();
    for base in [
        "https://other.example/play",
        "//other.example/play",
        "/",
        "/other",
        "/play?next=1",
        "/play/../other",
    ] {
        assert_eq!(
            resolve(RuntimeBinding::bound(base), &config),
            Err(UnavailableReason::NoGameplayRuntime),
            "{base}"
        );
    }
}

#[test]
fn chess_bot_factories_do_not_advertise_an_unimplemented_local_ai_runtime() {
    let catalog = crate::catalog(&Stub::vi());
    let game = catalog.get(&chess_id()).unwrap().game();
    assert!(!game
        .modes()
        .iter()
        .find(|support| support.mode == LaunchMode::LocalBots)
        .unwrap()
        .is_available());
    assert!(game
        .normalize(&SetupRequest {
            mode: LaunchMode::LocalBots,
            seats: 2,
            bot_level: Some(BotLevel::Easy),
            draft: draft(&[("clock", "untimed")]),
        })
        .is_err());
}

#[test]
fn chess_setup_rejects_values_the_standalone_runtime_cannot_construct() {
    let catalog = crate::catalog(&Stub::vi());
    let game = catalog.get(&chess_id()).unwrap().game();
    for pairs in [
        [
            ("clock", "fischer"),
            ("initial_minutes", "181"),
            ("increment_seconds", "2"),
        ],
        [
            ("clock", "fischer"),
            ("initial_minutes", "5"),
            ("increment_seconds", "61"),
        ],
        [
            ("clock", "bronstein"),
            ("initial_minutes", "5"),
            ("delay_seconds", "61"),
        ],
    ] {
        assert!(game.normalize(&local(2, &pairs)).is_err(), "{pairs:?}");
    }
}

#[test]
fn integrated_launch_carries_a_registry_selected_setup_return_route() {
    let catalog = crate::catalog(&Stub::vi());
    let config = catalog
        .get(&chess_id())
        .unwrap()
        .game()
        .normalize(&local(2, &[("clock", "untimed")]))
        .unwrap();
    let handoff = resolve(RuntimeBinding::bound("/play"), &config).unwrap();
    assert!(handoff.url.starts_with("/play/local/?"));
    assert!(handoff.url.contains("source=tabula"));
    assert!(handoff
        .url
        .contains("return_to=%2Fgames%2Fcom.tabula.chess%3Fsetup%3D1"));
}

#[test]
fn every_local_clock_handoff_is_complete_and_excludes_irrelevant_fields() {
    let catalog = crate::catalog(&Stub::vi());
    let game = catalog.get(&chess_id()).unwrap().game();
    for (pairs, expected) in [
        (vec![("clock", "untimed")], vec![("clock", "untimed")]),
        (
            vec![
                ("clock", "fischer"),
                ("initial_minutes", "180"),
                ("increment_seconds", "60"),
            ],
            vec![
                ("clock", "fischer"),
                ("initial_ms", "10800000"),
                ("increment_ms", "60000"),
            ],
        ),
        (
            vec![
                ("clock", "bronstein"),
                ("initial_minutes", "1"),
                ("delay_seconds", "0"),
            ],
            vec![
                ("clock", "bronstein"),
                ("initial_ms", "60000"),
                ("delay_ms", "0"),
            ],
        ),
    ] {
        let config = game.normalize(&local(2, &pairs)).unwrap();
        let mut args = vec![
            ("game", "com.tabula.chess"),
            ("mode", "local"),
            ("seats", "2"),
        ];
        args.extend(expected);
        assert_eq!(
            config.launch_args(),
            args.into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect::<Vec<_>>()
        );
        let handoff = crate::launch::resolve_with_locale(
            RuntimeBinding::bound("/play/"),
            &config,
            crate::Locale::Vi,
        )
        .unwrap();
        assert!(handoff.url.starts_with("/play/local/?"));
        assert!(handoff.url.ends_with(
            "&source=tabula&return_to=%2Fgames%2Fcom.tabula.chess%3Fsetup%3D1&locale=vi"
        ));
    }
}

#[test]
fn draft_redirect_and_locale_fields_cannot_override_registry_handoff_metadata() {
    let catalog = crate::catalog(&Stub::vi());
    let game = catalog.get(&chess_id()).unwrap().game();
    let config = game
        .normalize(&local(
            2,
            &[
                ("clock", "untimed"),
                ("source", "other"),
                ("return_to", "https://other.example"),
                ("locale", "other"),
            ],
        ))
        .unwrap();
    assert_eq!(resolve(RuntimeBinding::bound("/play"), &config).unwrap().url,
        "/play/local/?game=com.tabula.chess&mode=local&seats=2&clock=untimed&source=tabula&return_to=%2Fgames%2Fcom.tabula.chess%3Fsetup%3D1&locale=en");
}

#[test]
fn binding_one_runtime_does_not_launch_an_unimplemented_game_or_change_its_modes() {
    let catalog = crate::catalog(&Stub::vi());
    let game = catalog.get(&tiles_id()).unwrap().game();
    assert!(game
        .modes()
        .iter()
        .any(|support| support.mode == LaunchMode::LocalBots && support.is_available()));
    let config = game.normalize(&local(2, &[("deadline", "none")])).unwrap();
    assert_eq!(
        resolve(RuntimeBinding::bound("/play"), &config),
        Err(UnavailableReason::NoModeRuntime)
    );
}

/// Executable registry-to-document boundary corpus. A caller can pipe the
/// marker lines from `--nocapture` into the real browser launch parser rather
/// than trusting two independently handwritten sets of query fixtures.
#[test]
fn export_local_handoff_boundary_urls() {
    let catalog = crate::catalog(&Stub::vi());
    let game = catalog.get(&chess_id()).unwrap().game();
    let mut drafts = vec![draft(&[("clock", "untimed")])];
    for (clock, adjustment_field) in [
        ("fischer", "increment_seconds"),
        ("bronstein", "delay_seconds"),
    ] {
        for initial in ["1", "180"] {
            for adjustment in ["0", "60"] {
                drafts.push(draft(&[
                    ("clock", clock),
                    ("initial_minutes", initial),
                    (adjustment_field, adjustment),
                ]));
            }
        }
    }
    let mut exported = 0;
    for locale in crate::Locale::ALL {
        for draft in &drafts {
            let config = game
                .normalize(&SetupRequest {
                    mode: LaunchMode::LocalHotSeat,
                    seats: 2,
                    bot_level: None,
                    draft: draft.clone(),
                })
                .unwrap();
            let handoff =
                crate::launch::resolve_with_locale(RuntimeBinding::bound("/play"), &config, locale)
                    .unwrap();
            println!("TABULA_LAUNCH_URL={}", handoff.url);
            exported += 1;
        }
    }
    assert_eq!(
        exported, 18,
        "both locales and all timed clock endpoints execute"
    );
}

#[test]
fn every_key_the_catalog_hands_the_shell_exists_in_both_locales() {
    use crate::{config::FieldKind, erased::bot_level_label_key, i18n::Locale};

    let catalog = crate::catalog(&Stub::vi());
    let mut required: Vec<&'static str> = vec![
        "setup.summary.mode",
        "setup.summary.seats",
        "summary.time.untimed",
        "summary.time.increment",
        "summary.time.delay",
        "summary.seats",
        "unit.minutes",
        "unit.seconds",
    ];
    for reason in [
        RejectionReason::Missing,
        RejectionReason::NotANumber,
        RejectionReason::OutOfRange { min: 0, max: 1 },
        RejectionReason::UnknownChoice,
        RejectionReason::SeatCount,
        RejectionReason::ModuleField,
        RejectionReason::Unsupported,
    ] {
        required.push(reason.message_key());
    }
    for entry in catalog.entries() {
        let game = entry.game();
        let metadata = game.metadata();
        required.push(leaked(metadata.name_key().as_str()));
        required.push(leaked(metadata.tagline_key().as_str()));
        required.push(leaked(metadata.description_key().as_str()));
        for field in game.form().fields {
            required.push(field.label_key);
            if let Some(hint) = field.hint_key {
                required.push(hint);
            }
            if let FieldKind::Choice { options } = field.kind {
                for option in options {
                    required.push(option.label_key);
                }
            }
        }
        for support in game.modes() {
            required.push(support.mode.label_key());
            required.push(support.mode.consequence_key());
            if let Some(reason) = support.unavailable_reason() {
                required.push(reason.reason_key());
                required.push(reason.recovery_key());
            }
        }
        for level in game.bot_levels() {
            required.push(bot_level_label_key(level));
        }
        for reason in [
            UnavailableReason::NoGameplayRuntime,
            UnavailableReason::NoBotRuntime,
            UnavailableReason::NoModeRuntime,
            UnavailableReason::NavigationFailed,
        ] {
            required.push(reason.reason_key());
            required.push(reason.recovery_key());
        }
    }

    for locale in Locale::ALL {
        let table = catalog.messages(locale);
        for key in &required {
            assert!(
                table
                    .iter()
                    .any(|(candidate, text)| candidate == key && !text.is_empty()),
                "{key} has no {} copy",
                locale.tag()
            );
        }
    }
}

/// The metadata keys are owned by `&'static GameMetadata`, so their text is
/// already `'static`; this only re-borrows it for the assertion list.
fn leaked(value: &'static str) -> &'static str {
    value
}
