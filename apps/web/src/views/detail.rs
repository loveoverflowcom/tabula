//! 02 — game detail at `/games/:id`, and its setup substate `?setup=1`.
//!
//! Detail resolves one validated `GameId` through the catalog. An unknown but
//! well-formed id is a not-found state and a malformed id is a navigation
//! error; neither falls back to another game
//! (docs/ui/screens/02-game-detail.md).

use leptos::prelude::*;
use leptos_router::{
    components::A,
    hooks::{use_params_map, use_query_map},
};
use tabula_registry::{DiscoveryCatalogEntry as CatalogEntry, GameId, LaunchMode, ModeSupport};

use crate::{
    i18n::{shell, Messages},
    query::complexity_label_key,
    views::{library::seat_label, new_match::NewMatch, parts::Reason, use_locale},
};

#[component]
pub fn GameDetail() -> impl IntoView {
    let locale = use_locale();
    let params = use_params_map();
    let query = use_query_map();

    view! {
        <section class="section">
            {move || {
                let (messages, catalog) = shell(locale.get());
                let raw = params.get().get("id").unwrap_or_default();
                let Ok(id) = GameId::new(raw.clone()) else {
                    return invalid_id(&messages).into_any();
                };
                let Some(entry) = catalog.get(&id) else {
                    return not_found(&messages).into_any();
                };
                let setup = query.get().get("setup").as_deref() == Some("1");
                let preselected = query
                    .get()
                    .get("mode")
                    .and_then(|value| LaunchMode::parse(&value));
                if setup {
                    view! { <NewMatch id=id.as_str().to_owned() preselected=preselected/> }
                        .into_any()
                } else {
                    detail_body(&messages, entry).into_any()
                }
            }}
        </section>
    }
}

fn invalid_id(messages: &Messages) -> AnyView {
    let messages = messages.clone();
    let heading = messages.text("detail.invalid.heading");
    let back = messages.text("detail.back");
    view! {
        <>
            <h1 class="section__title">{heading}</h1>
            <A href="/games" attr:class="btn btn--tonal">{back}</A>
        </>
    }
    .into_any()
}

fn not_found(messages: &Messages) -> AnyView {
    let messages = messages.clone();
    let heading = messages.text("detail.notfound.heading");
    let body = messages.text("detail.notfound.body");
    let back = messages.text("detail.back");
    view! {
        <>
            <h1 class="section__title">{heading}</h1>
            <p class="section__body">{body}</p>
            <A href="/games" attr:class="btn btn--tonal">{back}</A>
        </>
    }
    .into_any()
}

fn detail_body(messages: &Messages, entry: &CatalogEntry) -> AnyView {
    let messages = messages.clone();
    let game = entry.game();
    let metadata = game.metadata();
    let id = metadata.id().as_str().to_owned();
    let startable = entry.startable();
    let setup_href = format!("/games/{id}?setup=1");
    let back_label = messages.text("detail.back");
    let setup_label = messages.text("detail.setup");
    #[cfg(feature = "online")]
    let online = view! { <crate::online::OnlinePanel id=id.clone()/> };
    #[cfg(not(feature = "online"))]
    let online = ();

    view! {
        <>
            <A href="/games" attr:class="btn btn--tonal">{back_label}</A>
            <h1 class="section__title">{messages.text(metadata.name_key().as_str())}</h1>
            <p class="section__lead">{messages.text(metadata.tagline_key().as_str())}</p>
            <p class="section__body">{messages.text(metadata.description_key().as_str())}</p>
            <p class="meta">
                {messages
                    .format(
                        "detail.version",
                        &[
                            metadata.version().as_str(),
                            &metadata.rules_version().0.to_string(),
                        ],
                    )}
            </p>

            {online}
            <h2 class="section__subtitle">{messages.text("detail.capabilities")}</h2>
            {detail_facts(&messages, entry)}
            <p class="meta">{messages.text("detail.declared.hint")}</p>

            <h2 class="section__subtitle" id="modes">{messages.text("detail.modes")}</h2>
            <ul class="rows" aria-labelledby="modes">
                {game.modes().iter().map(|support| mode_row(&messages, *support)).collect_view()}
            </ul>

            <h2 class="section__subtitle">{messages.text("detail.resources")}</h2>
            <p class="section__body">
                {metadata
                    .rules_url_key()
                    .map_or_else(
                        || messages.text("detail.rules.unavailable"),
                        |key| messages.text(key.as_str()),
                    )}
            </p>

            {if startable {
                view! {
                    <A href=setup_href attr:class="btn btn--filled btn--principal">
                        {setup_label}
                    </A>
                }
                    .into_any()
            } else {
                view! {
                    <p class="reason">{messages.text("setup.nomode")}</p>
                }
                    .into_any()
            }}
        </>
    }
    .into_any()
}

fn detail_facts(messages: &Messages, entry: &CatalogEntry) -> impl IntoView {
    let game = entry.game();
    let metadata = game.metadata();
    let capabilities = game.capabilities();
    view! {
        <dl class="facts facts--detail">
            <dt>{messages.text("detail.seats")}</dt>
            <dd>{seat_label(messages, capabilities)}</dd>
            <dt>{messages.text("detail.duration")}</dt>
            <dd>
                {messages
                    .format(
                        "detail.duration.range",
                        &[
                            &metadata.estimated_minutes().min().to_string(),
                            &metadata.estimated_minutes().max().to_string(),
                        ],
                    )}
            </dd>
            <dt>{messages.text("detail.complexity")}</dt>
            <dd>{messages.text(complexity_label_key(metadata.complexity()))}</dd>
            <dt>{messages.text("detail.rating")}</dt>
            <dd>{messages.text(rating_key(metadata.content_rating()))}</dd>
            <dt>{messages.text("detail.hidden_information")}</dt>
            <dd>{yes_no(messages, capabilities.hidden_information())}</dd>
        </dl>
    }
}

fn mode_row(messages: &Messages, support: ModeSupport) -> AnyView {
    let messages = messages.clone();
    let label = messages.text(support.mode.label_key());
    let consequence = messages.text(support.mode.consequence_key());
    view! {
        <li class="row" data-state=if support.is_available() { "enabled" } else { "disabled" }>
            <span class="row__label">{label}</span>
            <span class="row__body">{consequence}</span>
            {support
                .unavailable_reason()
                .map(|reason| {
                    view! {
                        <Reason
                            reason_key=reason.reason_key()
                            recovery_key=reason.recovery_key()
                        />
                    }
                })}
        </li>
    }
    .into_any()
}

fn yes_no(messages: &Messages, value: bool) -> String {
    messages.text(if value { "value.yes" } else { "value.no" })
}

const fn rating_key(rating: tabula_registry::ContentRating) -> &'static str {
    match rating {
        tabula_registry::ContentRating::Everyone => "rating.everyone",
        tabula_registry::ContentRating::Teen => "rating.teen",
        tabula_registry::ContentRating::Mature => "rating.mature",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source guard for the CSS hook, not a browser-layout assertion.
    #[test]
    fn detail_facts_source_keeps_zero_minimum_compact_track_and_wrapping() {
        let stylesheet = include_str!("../../style/app.scss");
        let (_, scoped) = stylesheet
            .split_once("\n.facts--detail {")
            .expect("detail facts have their own rule");
        let (compact, _) = scoped
            .split_once("\n@media (min-width: 37.5rem) {")
            .expect("the wider layout follows the browser's text size");
        for declaration in [
            "grid-template-columns: minmax(0, 1fr);",
            "min-width: 0;",
            "overflow-wrap: anywhere;",
        ] {
            assert!(compact.contains(declaration), "missing {declaration}");
        }
    }

    /// Native HTML proves the semantic reading order and the detail-only style
    /// hook. Actual text-scale reflow still needs the browser acceptance pass.
    #[test]
    fn detail_facts_keep_localized_term_value_pairs_in_reading_order() {
        for locale in tabula_registry::Locale::ALL {
            let (messages, catalog) = shell(locale);
            assert!(!catalog.entries().is_empty());
            for entry in catalog.entries() {
                let html = Owner::new().with(|| detail_facts(&messages, entry).to_html());
                assert!(html.contains("class=\"facts facts--detail\""));
                assert_eq!(html.matches("<dt>").count(), 5);
                assert_eq!(html.matches("<dd>").count(), 5);
                let game = entry.game();
                let metadata = game.metadata();
                let values = [
                    ("detail.seats", seat_label(&messages, game.capabilities())),
                    (
                        "detail.duration",
                        messages.format(
                            "detail.duration.range",
                            &[
                                &metadata.estimated_minutes().min().to_string(),
                                &metadata.estimated_minutes().max().to_string(),
                            ],
                        ),
                    ),
                    (
                        "detail.complexity",
                        messages.text(complexity_label_key(metadata.complexity())),
                    ),
                    (
                        "detail.rating",
                        messages.text(rating_key(metadata.content_rating())),
                    ),
                    (
                        "detail.hidden_information",
                        yes_no(&messages, game.capabilities().hidden_information()),
                    ),
                ];
                let mut rest = html
                    .strip_prefix("<dl class=\"facts facts--detail\">")
                    .expect("detail list retains its scoped style hook");
                for (key, value) in values {
                    let pair = format!("<dt>{}</dt><dd>{value}</dd>", messages.text(key));
                    rest = rest.strip_prefix(&pair).expect(
                        "each localized term is immediately followed by its registry value",
                    );
                }
                assert_eq!(rest, "</dl>");
            }
        }
    }
}
