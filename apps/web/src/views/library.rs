//! 01 — the game Library at `/games`.
//!
//! Search and filters live in the address so Back restores them. Every card
//! links to detail and starts nothing (docs/ui/screens/01-library.md).

use leptos::prelude::*;
use leptos_router::{
    components::A,
    hooks::{use_navigate, use_query_map},
    NavigateOptions,
};
use tabula_registry::{Catalog, CatalogEntry, CatalogQuery, LaunchMode};

use crate::{
    i18n::{shell, Messages},
    query::{
        category_label_key, category_value, complexity_label_key, complexity_value, library_href,
        parse, ParsedQuery, CATEGORIES, COMPLEXITIES,
    },
    views::use_locale,
};

const QUERY_KEYS: [&str; 6] = ["q", "category", "players", "duration", "complexity", "mode"];

/// Read the catalog constraints out of the current address.
pub fn current_query(map: &leptos_router::params::ParamsMap) -> ParsedQuery {
    let owned: Vec<(String, String)> = QUERY_KEYS
        .iter()
        .filter_map(|key| map.get(key).map(|value| ((*key).to_owned(), value)))
        .collect();
    parse(
        owned
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    )
}

#[component]
pub fn Library() -> impl IntoView {
    let locale = use_locale();
    let query_map = use_query_map();
    let parsed = Memo::new(move |_| current_query(&query_map.get()));

    // Locale owns the copy/catalog. URL changes update properties and results
    // below, never rebuild this toolbar: keyboard focus must survive filtering.
    view! {
        <section class="section catalog">
            {move || {
                let (messages, catalog) = shell(locale.get());
                library_body(&messages, &catalog, parsed)
            }}
        </section>
    }
}

fn library_body(messages: &Messages, catalog: &Catalog, parsed: Memo<ParsedQuery>) -> AnyView {
    let count_messages = messages.clone();
    let count_catalog = catalog.clone();
    let status_messages = messages.clone();
    let result_messages = messages.clone();
    let result_catalog = catalog.clone();

    view! {
        <header class="catalog__heading">
            <h1 class="section__title">{messages.text("library.heading")}</h1>
            {search(messages, parsed)}
        </header>
        <super::home::ContinueRegion/>
        {filters(messages, catalog, parsed, navigator())}
        <div class="catalog__errors">
            {move || parsed.get().invalid.into_iter().map(|axis| {
                let axis_label = status_messages.text(axis);
                view! {
                    <p class="banner banner--error" role="status">
                        {status_messages.format("library.filter.invalid", &[&axis_label])}
                    </p>
                }
            }).collect_view()}
        </div>
        <div class="catalog__results-heading">
            <h2 class="section__subtitle" id="results">
                {messages.text("library.results.heading")}
            </h2>
            <p class="results__count" aria-live="polite" aria-atomic="true">
                {move || {
                    let total = count_catalog.entries().len();
                    let count = count_catalog.query(&parsed.get().query, &count_messages).len();
                    count_messages.format("library.results.count", &[&count.to_string(), &total.to_string()])
                }}
            </p>
        </div>
        {move || {
            let query = parsed.get().query;
            let results = result_catalog.query(&query, &result_messages);
            results_body(&result_messages, &result_catalog, &results)
        }}
    }
    .into_any()
}

fn results_body(messages: &Messages, catalog: &Catalog, results: &[&CatalogEntry]) -> AnyView {
    let messages = messages.clone();
    if catalog.is_empty() {
        return view! {
            <div class="catalog__empty" role="status">
                {messages.text("library.empty.catalog")}
            </div>
        }
        .into_any();
    }
    if results.is_empty() {
        return view! {
            <div class="empty catalog__empty">
                <p role="status">{messages.text("library.empty.filtered")}</p>
                <A href="/games" attr:class="btn btn--tonal">
                    {messages.text("library.filter.reset")}
                </A>
            </div>
        }
        .into_any();
    }
    view! {
        <ul class="cards" aria-labelledby="results">
            {results.iter().map(|entry| card(&messages, entry)).collect_view()}
        </ul>
    }
    .into_any()
}

/// One compact discovery card, also consumed by Home. Only game-owned,
/// compile-time cover SVG enters this DOM; no gameplay bundle/assets load here.
/// Detail controls never contain another interactive control (doc 04 §3.2).
pub fn card(messages: &Messages, entry: &CatalogEntry) -> AnyView {
    let messages = messages.clone();
    let game = entry.game();
    let metadata = game.metadata();
    let name = messages.text(metadata.name_key().as_str());
    let details = messages.format("card.details", &[&name]);
    let seats = messages.format(
        "card.players",
        &[&seat_label(&messages, game.capabilities())],
    );
    let duration = messages.format(
        "detail.duration.range",
        &[
            &metadata.estimated_minutes().min().to_string(),
            &metadata.estimated_minutes().max().to_string(),
        ],
    );
    let complexity = messages.text(complexity_label_key(metadata.complexity()));
    let category = metadata
        .categories()
        .first()
        .map(|category| messages.text(category_label_key(*category)));
    let href = format!("/games/{}", metadata.id().as_str());
    let cover = game.catalog_cover_svg();
    let modes = entry
        .modes()
        .iter()
        .filter(|support| support.is_available())
        .map(|support| messages.text(support.mode.label_key()))
        .collect::<Vec<_>>()
        .join(" · ");
    let startable = entry.startable();

    view! {
        <li class="card">
            // The decorative cover shares the title's destination, but adds no
            // duplicate keyboard/screen-reader stop. It is still touchable.
            <A href=href.clone() attr:class="card__cover" attr:tabindex="-1" attr:aria-hidden="true">
                {cover.map_or_else(neutral_cover, |svg| view! { <div class="card__art" inner_html=svg></div> }.into_any())}
                {category.map(|label| view! { <span class="card__category">{label}</span> })}
            </A>
            <div class="card__text">
                <h3 class="card__title">
                    <A href=href attr:class="card__action" attr:aria-label=details>
                        {name}
                        <svg class="card__arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                            <path d="M5 12h14m-5-5 5 5-5 5"/>
                        </svg>
                    </A>
                </h3>
                <ul class="card__metadata">
                    <li title=messages.text("detail.seats")>
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                            <circle cx="9" cy="7" r="3"/><path d="M3 21v-3a6 6 0 0 1 12 0v3M16 4a3 3 0 0 1 0 6m5 11v-3a6 6 0 0 0-4-5"/>
                        </svg>
                        {seats}
                    </li>
                    <li title=messages.text("detail.duration")>
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                            <circle cx="12" cy="12" r="9"/><path d="M12 6v6l4 2"/>
                        </svg>
                        {duration}
                    </li>
                </ul>
                <p class="card__support">
                    {complexity}
                    {(!modes.is_empty()).then(|| format!(" · {modes}"))}
                </p>
                {(!startable).then(|| view! { <p class="reason card__reason">{messages.text("card.unavailable")}</p> })}
            </div>
        </li>
    }
    .into_any()
}

/// Decorative fallback for a linked module that declares no lightweight cover.
/// It has no fictitious gameplay, title, profile, or availability information.
fn neutral_cover() -> AnyView {
    view! {
        <div class="card__art card__art--neutral">
            <svg viewBox="0 0 364 160" aria-hidden="true" focusable="false">
                <g fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="120" y="20" width="94" height="120" rx="12" transform="rotate(-16 167 80)"/>
                    <rect x="154" y="20" width="94" height="120" rx="12" transform="rotate(14 201 80)"/>
                    <path d="M181 54v52m-26-26h52"/>
                    <circle cx="74" cy="58" r="10"/><circle cx="285" cy="117" r="14"/>
                    <path d="M292 33v14m-7-7h14M60 117v10m-5-5h10"/>
                </g>
            </svg>
        </div>
    }.into_any()
}

/// The allowed seat counts, as the module declares them.
pub fn seat_label(messages: &Messages, capabilities: &tabula_registry::GameCapabilities) -> String {
    let allowed = capabilities.seats().allowed();
    let counts: Vec<u8> = (allowed.min()..=allowed.max())
        .filter(|count| allowed.contains(*count))
        .collect();
    match counts.as_slice() {
        [] => String::new(),
        [only] => messages.format("seats.exact", &[&only.to_string()]),
        counts if counts.len() == usize::from(allowed.max() - allowed.min()) + 1 => messages
            .format(
                "seats.range",
                &[&allowed.min().to_string(), &allowed.max().to_string()],
            ),
        counts => counts
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn search(messages: &Messages, parsed: Memo<ParsedQuery>) -> AnyView {
    let go = navigator();
    let draft = RwSignal::new(parsed.get_untracked().query.text.unwrap_or_default());
    // Preserve trailing spaces while typing multiword queries. Only a distinct
    // address (e.g. Back/Forward) replaces the local native input's draft.
    Effect::new(move |_| {
        let address = parsed.get().query.text.unwrap_or_default();
        if let Some(replacement) = search_address_replacement(&draft.get_untracked(), &address) {
            draft.set(replacement);
        }
    });
    let on_search = move |event: leptos::ev::Event| {
        let value = event_target_value(&event);
        draft.set(value.clone());
        let mut next = parsed.get_untracked().query;
        next.text = (!value.trim().is_empty()).then(|| value.trim().to_owned());
        go(library_href(&next));
    };

    view! {
        <div class="catalog__search field" role="search">
            <label class="field__label" for="search">
                {messages.text("library.search.label")}
            </label>
            <div class="catalog__search-control">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                    <circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>
                </svg>
                <input id="search" class="field__control" type="search"
                    placeholder=messages.text("library.search.label")
                    prop:value=move || draft.get()
                    on:input=on_search
                />
            </div>
        </div>
    }.into_any()
}

/// A new address replaces the draft; the address's normalized form of the
/// same in-progress input does not eat the space before the next search word.
fn search_address_replacement(draft: &str, address: &str) -> Option<String> {
    (draft.trim() != address).then(|| address.to_owned())
}

/// Replace the address with the given href, keeping scroll position: typing
/// and filtering must not fill the history stack (shared discovery contract).
fn navigator() -> impl Fn(String) + Clone + 'static {
    let navigate = use_navigate();
    move |href: String| {
        navigate(
            &href,
            NavigateOptions {
                replace: true,
                scroll: false,
                ..NavigateOptions::default()
            },
        );
    }
}

/// One labeled native control per independent axis; constraints combine with
/// AND. Their DOM identity persists while URL-selected properties change.
fn filters(
    messages: &Messages,
    catalog: &Catalog,
    parsed: Memo<ParsedQuery>,
    go: impl Fn(String) + Clone + 'static,
) -> AnyView {
    let any = messages.text("library.filter.any");
    let category_options = CATEGORIES
        .iter()
        .map(|category| {
            (
                category_value(*category).to_owned(),
                messages.text(category_label_key(*category)),
            )
        })
        .collect();
    let player_options = player_counts(catalog)
        .into_iter()
        .map(|count| (count.to_string(), count.to_string()))
        .collect();
    let duration_options = duration_budgets(catalog)
        .into_iter()
        .map(|minutes| {
            (
                minutes.to_string(),
                messages.format("unit.minutes", &[&minutes.to_string()]),
            )
        })
        .collect();
    let complexity_options = COMPLEXITIES
        .iter()
        .map(|complexity| {
            (
                complexity_value(*complexity).to_owned(),
                messages.text(complexity_label_key(*complexity)),
            )
        })
        .collect();
    let mode_options = available_modes(catalog)
        .into_iter()
        .map(|mode| (mode.as_str().to_owned(), messages.text(mode.label_key())))
        .collect();

    let on_category = axis(parsed, go.clone(), |next, value| {
        next.category = CATEGORIES
            .into_iter()
            .find(|candidate| category_value(*candidate) == value);
    });
    let on_players = axis(parsed, go.clone(), |next, value| {
        next.players = value.parse().ok();
    });
    let on_duration = axis(parsed, go.clone(), |next, value| {
        next.max_minutes = value.parse().ok();
    });
    let on_complexity = axis(parsed, go.clone(), |next, value| {
        next.complexity = COMPLEXITIES
            .into_iter()
            .find(|candidate| complexity_value(*candidate) == value);
    });
    let on_mode = axis(parsed, go.clone(), |next, value| {
        next.mode = LaunchMode::parse(&value);
    });
    let reset = move |_| go("/games".to_owned());

    view! {
        <fieldset class="filters catalog__filters">
            <legend class="filters__legend">{messages.text("library.filter.heading")}</legend>
            {select(messages, "filter-category", "library.filter.category", &any, category_options,
                move || parsed.get().query.category.map(category_value).unwrap_or_default().to_owned(), on_category)}
            {select(messages, "filter-players", "library.filter.players", &any, player_options,
                move || parsed.get().query.players.map(|value| value.to_string()).unwrap_or_default(), on_players)}
            {select(messages, "filter-duration", "library.filter.duration", &any, duration_options,
                move || parsed.get().query.max_minutes.map(|value| value.to_string()).unwrap_or_default(), on_duration)}
            {select(messages, "filter-complexity", "library.filter.complexity", &any, complexity_options,
                move || parsed.get().query.complexity.map(complexity_value).unwrap_or_default().to_owned(), on_complexity)}
            {select(messages, "filter-mode", "library.filter.mode", &any, mode_options,
                move || parsed.get().query.mode.map(|value| value.as_str().to_owned()).unwrap_or_default(), on_mode)}
            <button type="button" class="btn catalog__reset" on:click=reset>
                {messages.text("library.filter.reset")}
            </button>
        </fieldset>
    }.into_any()
}

/// Read the latest address at interaction time, so rapidly combining filters
/// never applies one axis to a stale snapshot of another.
fn axis(
    parsed: Memo<ParsedQuery>,
    go: impl Fn(String) + 'static,
    apply: impl Fn(&mut CatalogQuery, String) + 'static,
) -> impl Fn(String) + 'static {
    move |value: String| {
        let mut next = parsed.get_untracked().query;
        apply(&mut next, value);
        go(library_href(&next));
    }
}

/// A visible label and the native select preserve platform keyboard/touch UX.
fn select(
    messages: &Messages,
    id: &'static str,
    label_key: &'static str,
    any_label: &str,
    options: Vec<(String, String)>,
    value: impl Fn() -> String + Clone + Send + 'static,
    on_change: impl Fn(String) + 'static,
) -> AnyView {
    let any_label = any_label.to_owned();
    let property_value = value.clone();
    let empty_value = value.clone();
    let option_value = value.clone();
    let selected_messages = messages.clone();
    let known_options = options.clone();
    view! {
        <div class="field">
            <label class="field__label" for=id>{messages.text(label_key)}</label>
            <select id=id class="field__control" prop:value=property_value
                on:change=move |event| on_change(event_target_value(&event))>
                <option value="" prop:selected=move || empty_value().is_empty()>{any_label}</option>
                {options.into_iter().map(|(value, label)| {
                    let selected = option_value.clone();
                    let selected_value = value.clone();
                    view! { <option value=value prop:selected=move || selected() == selected_value>{label}</option> }
                }).collect_view()}
                {move || selected_only_option(&selected_messages, label_key, &value(), &known_options)
                    .map(|(value, label)| view! { <option value=value selected=true>{label}</option> })}
            </select>
        </div>
    }.into_any()
}

/// A valid URL constraint may lie outside the catalog's usual menu inventory.
/// Keep its actual value visible without inventing a supporting game's facts.
fn selected_only_option(
    messages: &Messages,
    label_key: &str,
    value: &str,
    options: &[(String, String)],
) -> Option<(String, String)> {
    if value.is_empty() || options.iter().any(|(known, _)| known == value) {
        return None;
    }
    let label = match label_key {
        "library.filter.duration" => messages.format("unit.minutes", &[value]),
        "library.filter.mode" => LaunchMode::parse(value)
            .map(|mode| messages.text(mode.label_key()))
            .unwrap_or_else(|| value.to_owned()),
        _ => value.to_owned(),
    };
    Some((value.to_owned(), label))
}

/// Seat counts some linked game actually supports.
fn player_counts(catalog: &Catalog) -> Vec<u8> {
    let mut counts: Vec<u8> = (1..=16)
        .filter(|count| {
            catalog.entries().iter().any(|entry| {
                entry
                    .game()
                    .capabilities()
                    .seats()
                    .allowed()
                    .contains(*count)
            })
        })
        .collect();
    counts.dedup();
    counts
}

/// Budgets taken from the estimates the modules declare, not invented bands.
fn duration_budgets(catalog: &Catalog) -> Vec<u16> {
    let mut budgets: Vec<u16> = catalog
        .entries()
        .iter()
        .map(|entry| entry.game().metadata().estimated_minutes().max())
        .collect();
    budgets.sort_unstable();
    budgets.dedup();
    budgets
}

/// Modes at least one linked game can actually start.
fn available_modes(catalog: &Catalog) -> Vec<LaunchMode> {
    LaunchMode::ALL
        .into_iter()
        .filter(|mode| {
            catalog.entries().iter().any(|entry| {
                entry
                    .modes()
                    .iter()
                    .any(|support| support.mode == *mode && support.is_available())
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{axis, search_address_replacement, selected_only_option};
    use crate::{i18n::Messages, query::ParsedQuery};
    use leptos::prelude::*;
    use std::{cell::RefCell, rc::Rc};
    use tabula_registry::{Category, Locale};

    #[test]
    fn axis_change_combines_with_latest_address_instead_of_mount_snapshot() {
        Owner::new().with(|| {
            let source = RwSignal::new(ParsedQuery::default());
            let parsed = Memo::new(move |_| source.get());
            let destination = Rc::new(RefCell::new(String::new()));
            let recorded = destination.clone();
            let change = axis(
                parsed,
                move |href| *recorded.borrow_mut() = href,
                |next, value| {
                    next.max_minutes = value.parse().ok();
                },
            );
            source.update(|current| {
                current.query.text = Some("classic strategy".to_owned());
                current.query.category = Some(Category::Abstract);
                current.query.players = Some(2);
            });
            change("30".to_owned());
            assert_eq!(
                *destination.borrow(),
                "/games?q=classic%20strategy&category=abstract&players=2&duration=30"
            );
            source.update(|current| current.query.players = Some(5));
            change(String::new());
            assert_eq!(
                *destination.borrow(),
                "/games?q=classic%20strategy&category=abstract&players=5"
            );
        });
    }

    #[test]
    fn search_normalization_preserves_space_before_next_word_but_back_replaces_draft() {
        assert_eq!(search_address_replacement("classic ", "classic"), None);
        assert_eq!(
            search_address_replacement("classic strategy ", "classic strategy"),
            None
        );
        assert_eq!(search_address_replacement("   ", ""), None);
        assert_eq!(
            search_address_replacement("classic strategy", "strategy"),
            Some("strategy".to_owned())
        );
        assert_eq!(
            search_address_replacement("classic strategy", ""),
            Some(String::new())
        );
    }

    #[test]
    fn valid_unlisted_url_constraints_keep_their_actual_selected_labels() {
        let en = Messages::new(Locale::En);
        let vi = Messages::new(Locale::Vi);
        let players = vec![("2".to_owned(), "2".to_owned())];
        assert_eq!(
            selected_only_option(&en, "library.filter.players", "", &players),
            None
        );
        assert_eq!(
            selected_only_option(&en, "library.filter.players", "2", &players),
            None
        );
        assert_eq!(
            selected_only_option(&en, "library.filter.players", "8", &players),
            Some(("8".to_owned(), "8".to_owned()))
        );
        assert_eq!(
            selected_only_option(&en, "library.filter.duration", "15", &[]),
            Some(("15".to_owned(), "15 min".to_owned()))
        );
        assert_eq!(
            selected_only_option(&vi, "library.filter.duration", "15", &[]),
            Some(("15".to_owned(), "15 phút".to_owned()))
        );
        assert_eq!(
            selected_only_option(&en, "library.filter.mode", "bots", &[]),
            Some(("bots".to_owned(), "Against the game's bot".to_owned()))
        );
    }
}
