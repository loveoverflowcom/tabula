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

    view! {
        <section class="section">
            {move || {
                let (messages, catalog) = shell(locale.get());
                let parsed = current_query(&query_map.get());
                library_body(&messages, &catalog, &parsed)
            }}
        </section>
    }
}

fn library_body(messages: &Messages, catalog: &Catalog, parsed: &ParsedQuery) -> AnyView {
    let messages = messages.clone();
    let results = catalog.query(&parsed.query, &messages);
    let total = catalog.entries().len();
    let count = messages.format(
        "library.results.count",
        &[&results.len().to_string(), &total.to_string()],
    );

    view! {
        <h1 class="section__title">{messages.text("library.heading")}</h1>
        {toolbar(&messages, catalog, &parsed.query)}
        {parsed
            .invalid
            .iter()
            .map(|axis| {
                let axis_label = messages.text(axis);
                view! {
                    <p class="banner banner--error" role="status">
                        {messages.format("library.filter.invalid", &[&axis_label])}
                    </p>
                }
            })
            .collect_view()}
        <h2 class="section__subtitle" id="results">
            {messages.text("library.results.heading")}
        </h2>
        <p class="results__count" aria-live="polite">
            {count}
        </p>
        {results_body(&messages, catalog, &results, &parsed.query)}
    }
    .into_any()
}

fn results_body(
    messages: &Messages,
    catalog: &Catalog,
    results: &[&CatalogEntry],
    query: &CatalogQuery,
) -> AnyView {
    let messages = messages.clone();
    if catalog.is_empty() {
        return view! {
            <p class="banner" role="status">{messages.text("library.empty.catalog")}</p>
        }
        .into_any();
    }
    if results.is_empty() {
        return view! {
            <div class="empty">
                <p class="banner" role="status">{messages.text("library.empty.filtered")}</p>
                <a class="btn btn--tonal" href="/games">
                    {messages.text("library.filter.reset")}
                </a>
            </div>
        }
        .into_any();
    }
    let _ = query;
    view! {
        <ul class="cards" aria-labelledby="results">
            {results
                .iter()
                .map(|entry| card(&messages, entry))
                .collect_view()}
        </ul>
    }
    .into_any()
}

/// One catalog entry. The whole card is not a link: the detail link is its own
/// control, so a trailing action can never be nested inside it.
pub fn card(messages: &Messages, entry: &CatalogEntry) -> AnyView {
    let messages = messages.clone();
    let game = entry.game();
    let metadata = game.metadata();
    let name = messages.text(metadata.name_key().as_str());
    let seats = seat_label(&messages, game.capabilities());
    let duration = messages.format(
        "detail.duration.range",
        &[
            &metadata.estimated_minutes().min().to_string(),
            &metadata.estimated_minutes().max().to_string(),
        ],
    );
    let complexity = messages.text(complexity_label_key(metadata.complexity()));
    let href = format!("/games/{}", metadata.id().as_str());
    let startable = entry.startable();
    let modes: Vec<String> = entry
        .modes()
        .iter()
        .filter(|support| support.is_available())
        .map(|support| messages.text(support.mode.label_key()))
        .collect();

    view! {
        <li class="card">
            <div class="card__art" aria-hidden="true"></div>
            <div class="card__text">
                <h3 class="card__title">{name.clone()}</h3>
                <p class="card__tagline">{messages.text(metadata.tagline_key().as_str())}</p>
                <ul class="badges">
                    <li class="badge">{messages.text("detail.seats")} ": " {seats}</li>
                    <li class="badge">{duration}</li>
                    <li class="badge">{complexity}</li>
                    {modes
                        .into_iter()
                        .map(|label| view! { <li class="badge badge--mode">{label}</li> })
                        .collect_view()}
                </ul>
                {(!startable)
                    .then(|| {
                        view! { <p class="reason">{messages.text("card.unavailable")}</p> }
                    })}
            </div>
            <A href=href attr:class="btn btn--tonal card__action">
                {messages.format("card.details", &[&name])}
            </A>
        </li>
    }
    .into_any()
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

fn toolbar(messages: &Messages, catalog: &Catalog, query: &CatalogQuery) -> AnyView {
    let messages = messages.clone();
    let go = navigator();
    let search_value = query.text.clone().unwrap_or_default();
    let on_search = {
        let query = query.clone();
        let go = go.clone();
        move |event: leptos::ev::Event| {
            let mut next = query.clone();
            let value = event_target_value(&event);
            next.text = (!value.trim().is_empty()).then_some(value);
            go(library_href(&next));
        }
    };

    view! {
        <div class="toolbar" role="search">
            <div class="field">
                <label class="field__label" for="search">
                    {messages.text("library.search.label")}
                </label>
                <input
                    id="search"
                    class="field__control"
                    type="search"
                    value=search_value
                    on:change=on_search
                />
            </div>
            {filters(&messages, catalog, query, go)}
        </div>
    }
    .into_any()
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

/// One labeled control per independent axis; constraints combine with AND.
fn filters(
    messages: &Messages,
    catalog: &Catalog,
    query: &CatalogQuery,
    go: impl Fn(String) + Clone + 'static,
) -> AnyView {
    let messages = messages.clone();
    let any = messages.text("library.filter.any");
    let reset = messages.text("library.filter.reset");
    let heading = messages.text("library.filter.heading");

    let category_options = CATEGORIES
        .iter()
        .map(|category| {
            (
                category_value(*category).to_owned(),
                messages.text(category_label_key(*category)),
                query.category == Some(*category),
            )
        })
        .collect();
    let player_options = player_counts(catalog)
        .into_iter()
        .map(|count| {
            (
                count.to_string(),
                count.to_string(),
                query.players == Some(count),
            )
        })
        .collect();
    let duration_options = duration_budgets(catalog)
        .into_iter()
        .map(|minutes| {
            (
                minutes.to_string(),
                messages.format("unit.minutes", &[&minutes.to_string()]),
                query.max_minutes == Some(minutes),
            )
        })
        .collect();
    let complexity_options = COMPLEXITIES
        .iter()
        .map(|complexity| {
            (
                complexity_value(*complexity).to_owned(),
                messages.text(complexity_label_key(*complexity)),
                query.complexity == Some(*complexity),
            )
        })
        .collect();
    let mode_options = available_modes(catalog)
        .into_iter()
        .map(|mode| {
            (
                mode.as_str().to_owned(),
                messages.text(mode.label_key()),
                query.mode == Some(mode),
            )
        })
        .collect();

    let on_category = axis(query, go.clone(), |next, value| {
        next.category = CATEGORIES
            .into_iter()
            .find(|candidate| category_value(*candidate) == value);
    });
    let on_players = axis(query, go.clone(), |next, value| {
        next.players = value.parse().ok();
    });
    let on_duration = axis(query, go.clone(), |next, value| {
        next.max_minutes = value.parse().ok();
    });
    let on_complexity = axis(query, go.clone(), |next, value| {
        next.complexity = COMPLEXITIES
            .into_iter()
            .find(|candidate| complexity_value(*candidate) == value);
    });
    let on_mode = axis(query, go, |next, value| {
        next.mode = LaunchMode::parse(&value);
    });

    view! {
        <fieldset class="filters">
            <legend class="filters__legend">{heading}</legend>
            {select(&messages, "filter-category", "library.filter.category", &any, category_options, on_category)}
            {select(&messages, "filter-players", "library.filter.players", &any, player_options, on_players)}
            {select(&messages, "filter-duration", "library.filter.duration", &any, duration_options, on_duration)}
            {select(&messages, "filter-complexity", "library.filter.complexity", &any, complexity_options, on_complexity)}
            {select(&messages, "filter-mode", "library.filter.mode", &any, mode_options, on_mode)}
            <a class="btn btn--tonal" href="/games">{reset}</a>
        </fieldset>
    }
    .into_any()
}

/// Build one axis handler: apply the change to a copy of the current
/// constraints and navigate to the result.
fn axis(
    query: &CatalogQuery,
    go: impl Fn(String) + 'static,
    apply: impl Fn(&mut CatalogQuery, String) + 'static,
) -> impl Fn(String) + 'static {
    let query = query.clone();
    move |value: String| {
        let mut next = query.clone();
        apply(&mut next, value);
        go(library_href(&next));
    }
}

/// A labeled native select. One axis, one control, always with a visible label.
fn select(
    messages: &Messages,
    id: &'static str,
    label_key: &'static str,
    any_label: &str,
    options: Vec<(String, String, bool)>,
    on_change: impl Fn(String) + 'static,
) -> AnyView {
    let any_label = any_label.to_owned();
    view! {
        <div class="field">
            <label class="field__label" for=id>{messages.text(label_key)}</label>
            <select
                id=id
                class="field__control"
                on:change=move |event| on_change(event_target_value(&event))
            >
                <option value="" selected=options.iter().all(|(_, _, chosen)| !chosen)>
                    {any_label}
                </option>
                {options
                    .into_iter()
                    .map(|(value, label, chosen)| {
                        view! { <option value=value selected=chosen>{label}</option> }
                    })
                    .collect_view()}
            </select>
        </div>
    }
    .into_any()
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
