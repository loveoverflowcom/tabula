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
use tabula_registry::{CatalogEntry, GameId, LaunchMode, ModeSupport};

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
    let capabilities = game.capabilities();
    let id = metadata.id().as_str().to_owned();
    let startable = entry.startable();
    let setup_href = format!("/games/{id}?setup=1");
    let back_label = messages.text("detail.back");
    let setup_label = messages.text("detail.setup");
    #[cfg(feature = "online")]
    let online = crate::online::runtime_binding()
        .supports_direct(game)
        .then(|| view! { <crate::online::OnlinePanel id=id.clone()/> });
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
            <dl class="facts">
                <dt>{messages.text("detail.seats")}</dt>
                <dd>{seat_label(&messages, capabilities)}</dd>
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
                <dd>{yes_no(&messages, capabilities.hidden_information())}</dd>
            </dl>
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
