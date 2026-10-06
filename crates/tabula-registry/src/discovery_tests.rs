//! Differential evidence for the additive discovery vtable and legacy API.

#[cfg(any(feature = "game-chess", feature = "game-tiles"))]
use crate::{
    BotLevel, CatalogQuery, ConfigDraft, DiscoveryCatalog, FieldKind, GameId, LaunchMode,
    SetupRequest,
};
use crate::{Catalog, I18nKey, Locale, Localizer, RuntimeBinding};
use std::collections::BTreeMap;

struct CopyTable(BTreeMap<String, String>);
impl Localizer for CopyTable {
    fn text(&self, key: &I18nKey) -> Option<&str> {
        self.0.get(key.as_str()).map(String::as_str)
    }
}

#[test]
fn legacy_bare_catalog_constructors_clone_default_and_function_pointer_remain_valid() {
    use crate::*;
    let table = CopyTable(BTreeMap::new());
    let default_catalog = Catalog::default();
    assert!(default_catalog.is_empty());
    let empty_catalog = Catalog::new(vec![], &table);
    assert!(empty_catalog.clone().is_empty());
    let constructor: fn(Vec<std::sync::Arc<dyn ErasedGame>>, &dyn Localizer) -> Catalog =
        Catalog::new;
    assert!(constructor(vec![], &table).is_empty());
    let _method: fn(&RuntimeBinding, &dyn ErasedGame) -> bool = RuntimeBinding::supports_direct;
    let _resolve: fn(
        RuntimeBinding,
        &dyn ErasedGame,
        &str,
        Locale,
    ) -> Result<crate::LaunchHandoff, UnavailableReason> = crate::launch::resolve_direct;
    #[cfg(feature = "game-chess")]
    {
        // Existing wildcard consumers see only the original erased trait.
        let adapter = Adapter::<games::chess::ChessSetup>::new();
        assert!(!adapter.metadata().id().as_str().is_empty());
    }
}

#[cfg(any(feature = "game-chess", feature = "game-tiles"))]
fn bindings() -> [RuntimeBinding; 8] {
    [
        RuntimeBinding::unbound(),
        RuntimeBinding::bound("/play"),
        RuntimeBinding::bound("/play/"),
        RuntimeBinding::direct_online("/play"),
        RuntimeBinding::direct_online("/play/"),
        RuntimeBinding::bound("https://invalid.example/play"),
        RuntimeBinding::bound("/play?next=elsewhere"),
        RuntimeBinding::direct_online("/play/../elsewhere"),
    ]
}

#[cfg(any(feature = "game-chess", feature = "game-tiles"))]
fn drafts(form: &crate::ConfigForm) -> Vec<ConfigDraft> {
    let defaults = ConfigDraft::with_defaults(form);
    let mut bases = vec![defaults.clone(), ConfigDraft::new()];
    for field in form.fields {
        if let FieldKind::Choice { options } = field.kind {
            for option in options {
                let mut next = defaults.clone();
                next.set(field.key, option.value);
                bases.push(next);
            }
        }
    }
    let mut cases = bases.clone();
    for base in bases {
        let mut unknown = base.clone();
        unknown.set("unexpected_field", "not a config field");
        cases.push(unknown);
        for field in form.fields {
            let mut values = vec![
                "".to_owned(),
                "unknown-choice".to_owned(),
                "-1".to_owned(),
                "1.5".to_owned(),
                " 1 ".to_owned(),
                "18446744073709551616".to_owned(),
            ];
            match field.kind {
                FieldKind::Choice { options } => {
                    values.extend(options.iter().map(|option| option.value.to_owned()))
                }
                FieldKind::Integer { min, max, default } => {
                    values.extend([
                        min.to_string(),
                        max.to_string(),
                        default.to_string(),
                        u64::MAX.to_string(),
                    ]);
                    values.extend(min.checked_sub(1).map(|value| value.to_string()));
                    values.extend(max.checked_add(1).map(|value| value.to_string()));
                }
            }
            for value in values {
                let mut next = base.clone();
                next.set(field.key, value);
                cases.push(next);
            }
        }
    }
    cases
}

#[cfg(any(feature = "game-chess", feature = "game-tiles"))]
#[test]
fn discovery_registration_copy_catalog_order_and_filters_match_full_authority_descriptors() {
    let full = crate::registered_games();
    let discovery = crate::registered_discovery_games();
    assert!(
        !full.is_empty(),
        "feature-gated selection must reach a registered game"
    );
    assert_eq!(full.len(), discovery.len());
    for (game, facade) in full.iter().zip(&discovery) {
        assert!(std::ptr::eq(game.metadata(), facade.metadata()));
        assert!(std::ptr::eq(game.capabilities(), facade.capabilities()));
        assert!(std::ptr::eq(game.form(), facade.form()));
        assert_eq!(game.modes(), facade.modes());
        assert_eq!(game.catalog_cover_svg(), facade.catalog_cover_svg());
        assert_eq!(game.bot_levels(), facade.bot_levels());
        assert_eq!(game.direct_document(), facade.direct_document());
        assert_eq!(game.direct_host_supported(), facade.direct_host_supported());
        for locale in Locale::ALL {
            assert_eq!(game.messages(locale), facade.messages(locale));
        }
    }
    for locale in Locale::ALL {
        let table = CopyTable(
            full.iter()
                .flat_map(|game| game.messages(locale))
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        let ordinary = Catalog::new(full.clone(), &table);
        let readonly = DiscoveryCatalog::new(discovery.clone(), &table);
        let ids = |entries: &[crate::CatalogEntry]| {
            entries
                .iter()
                .map(|entry| entry.id().as_str())
                .collect::<Vec<_>>()
        };
        let expected = ids(ordinary.entries());
        assert_eq!(
            expected,
            readonly
                .entries()
                .iter()
                .map(|entry| entry.id().as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(ordinary.messages(locale), readonly.messages(locale));
        assert_eq!(readonly.clone().entries().len(), ordinary.entries().len());
        let mut queries = vec![CatalogQuery::default()];
        for players in [0, 1, 2, 3, 4, 5, 16, u8::MAX] {
            queries.push(CatalogQuery {
                players: Some(players),
                ..Default::default()
            });
        }
        for duration in [0, 1, u16::MAX] {
            queries.push(CatalogQuery {
                max_minutes: Some(duration),
                ..Default::default()
            });
        }
        for mode in LaunchMode::ALL {
            queries.push(CatalogQuery {
                mode: Some(mode),
                ..Default::default()
            });
        }
        for entry in ordinary.entries() {
            let metadata = entry.game().metadata();
            for category in metadata.categories() {
                queries.push(CatalogQuery {
                    category: Some(*category),
                    ..Default::default()
                });
            }
            queries.push(CatalogQuery {
                complexity: Some(metadata.complexity()),
                ..Default::default()
            });
            for key in [metadata.name_key(), metadata.tagline_key()] {
                let text = table
                    .text(key)
                    .expect("module-owned visible copy must exist")
                    .to_owned();
                queries.push(CatalogQuery {
                    text: Some(text),
                    ..Default::default()
                });
            }
            queries.push(CatalogQuery {
                players: Some(entry.game().capabilities().seats().allowed().min()),
                mode: Some(LaunchMode::LocalHotSeat),
                complexity: Some(metadata.complexity()),
                ..Default::default()
            });
            assert!(readonly.get(entry.id()).is_some());
        }
        let absent = GameId::new("org.example.absent").expect("valid absent identifier");
        assert!(ordinary.get(&absent).is_none());
        assert!(readonly.get(&absent).is_none());
        assert!(!queries.is_empty());
        for query in queries {
            assert_eq!(
                ordinary
                    .query(&query, &table)
                    .iter()
                    .map(|entry| entry.id().as_str())
                    .collect::<Vec<_>>(),
                readonly
                    .query(&query, &table)
                    .iter()
                    .map(|entry| entry.id().as_str())
                    .collect::<Vec<_>>(),
                "query {query:?}"
            );
        }
    }
}

#[cfg(any(feature = "game-chess", feature = "game-tiles"))]
#[test]
fn discovery_normalization_success_errors_bytes_summaries_and_handoffs_match_full_games() {
    let full = crate::registered_games();
    let discovery = crate::registered_discovery_games();
    assert!(!full.is_empty());
    assert_eq!(full.len(), discovery.len());
    let mut comparisons = 0;
    let mut successes = 0;
    let mut rejections = 0;
    let mut direct_successes = 0;
    for (game, facade) in full.iter().zip(&discovery) {
        let cases = drafts(game.form());
        assert!(cases.len() > 1);
        let mut game_successes = 0;
        for draft in &cases {
            for seats in [0, 1, 2, 3, 4, 5, 16, u8::MAX] {
                assert_eq!(
                    game.normalize_direct(seats, draft),
                    facade.normalize_direct(seats, draft),
                    "direct {} seats={seats} {draft:?}",
                    game.metadata().id().as_str()
                );
                direct_successes += usize::from(facade.normalize_direct(seats, draft).is_ok());
                for mode in LaunchMode::ALL {
                    for bot_level in [
                        None,
                        Some(BotLevel::Trivial),
                        Some(BotLevel::Easy),
                        Some(BotLevel::Medium),
                        Some(BotLevel::Hard),
                    ] {
                        let request = SetupRequest {
                            mode,
                            seats,
                            bot_level,
                            draft: draft.clone(),
                        };
                        let expected = game.normalize(&request);
                        let actual = facade.normalize(&request);
                        assert_eq!(
                            expected,
                            actual,
                            "{} {request:?}",
                            game.metadata().id().as_str()
                        );
                        comparisons += 1;
                        match (expected, actual) {
                            (Ok(left), Ok(right)) => {
                                assert!(!left.canonical_config.is_empty());
                                successes += 1;
                                game_successes += 1;
                                // Result equality above includes bytes, summaries, launch args and return target.
                                for binding in bindings() {
                                    for locale in Locale::ALL {
                                        assert_eq!(
                                            crate::launch::resolve_with_locale(
                                                binding, &left, locale
                                            ),
                                            crate::launch::resolve_with_locale(
                                                binding, &right, locale
                                            )
                                        );
                                    }
                                }
                            }
                            (Err(_), Err(_)) => rejections += 1,
                            _ => unreachable!("assert_eq rejected divergent variants"),
                        }
                    }
                }
            }
        }
        assert!(
            game_successes > 0,
            "must reach valid configuration for each linked game"
        );
        for binding in bindings() {
            assert_eq!(
                binding.supports_direct(game.as_ref()),
                binding.supports_discovery_direct(facade.as_ref())
            );
            for match_id in [
                "00000000000000000000000000000001",
                "ffffffffffffffffffffffffffffffff",
                "00000000000000000000000000000000",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "1",
                "../private?token=x",
                "",
            ] {
                for locale in Locale::ALL {
                    assert_eq!(
                        crate::launch::resolve_direct(binding, game.as_ref(), match_id, locale),
                        crate::launch::resolve_discovery_direct(
                            binding,
                            facade.as_ref(),
                            match_id,
                            locale
                        )
                    );
                }
            }
        }
    }
    assert!(comparisons > 100 && successes > 0 && rejections > 0);
    #[cfg(feature = "game-chess")]
    assert!(
        direct_successes > 0,
        "direct-eligible game must reach accepted normalization"
    );
    #[cfg(not(feature = "game-chess"))]
    assert_eq!(
        direct_successes, 0,
        "currently linked non-direct game advertises no direct normalization"
    );
}
