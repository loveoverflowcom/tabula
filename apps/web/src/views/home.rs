//! 01 — original Design 01 discovery composition at `/` (issue #87).
//!
//! The hero is discovery, never an unsupported Tutor/AI action. The continue
//! region has no resume-list adapter and labels that unavailable state explicitly.
//! Registry data owns the featured selection; `/games` owns full search/filtering.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::{
    i18n::shell,
    views::{
        library::card,
        parts::{translated, Icon},
        use_locale,
    },
};

#[component]
pub fn Home() -> impl IntoView {
    let locale = use_locale();
    view! {
        <section class="section home">
            <div class="page-heading">
                <p class="eyebrow">{translated("home.eyebrow")}</p>
                <h1 class="section__title">{translated("home.heading")}</h1>
                <p class="section__body">{translated("home.lead")}</p>
            </div>
            <section class="feature-hero" aria-labelledby="hero-heading">
                <div class="hero-copy">
                    <p class="eyebrow"><Icon kind="spark"/>{translated("home.hero.eyebrow")}</p>
                    <h2 id="hero-heading">{translated("home.hero.heading")}</h2>
                    <p>{translated("home.hero.body")}</p>
                    <A href="/games" attr:class="btn btn--filled"><Icon kind="arrow"/><span>{translated("home.hero.action")}</span></A>
                </div>
                // The original ornamental token composition is a design illustration.
                // It is not a game offer, an engine preview, or a runtime resource.
                <div class="hero-art" aria-hidden="true">
                    <div class="hero-art__grid"></div><span class="hero-art__ring"></span>
                    <span class="hero-art__token hero-art__token--main">"帥"</span>
                    <span class="hero-art__token hero-art__token--other">"馬"</span>
                    <span class="hero-art__token hero-art__token--small">"兵"</span>
                    <span class="hero-art__spark">"✧"</span>
                </div>
            </section>
            <ContinueRegion/>
            <div class="section-heading">
                <h2 class="section__subtitle" id="featured">{translated("home.featured")}</h2>
                <A href="/games" attr:class="text-link">{translated("home.browse")}<Icon kind="arrow"/></A>
            </div>
            {move || {
                let (messages, catalog) = shell(locale.get());
                let available: Vec<_> = catalog.entries().iter().filter(|entry| entry.startable()).collect();
                if available.is_empty() {
                    view! { <p class="banner" role="status">{messages.text(if catalog.is_empty() { "library.empty.catalog" } else { "card.unavailable" })}</p> }.into_any()
                } else {
                    view! { <ul class="cards" aria-labelledby="featured">{available.iter().map(|entry| card(&messages, entry)).collect_view()}</ul> }.into_any()
                }
            }}
            <p class="gentle-note"><Icon kind="leaf"/>{translated("accounts.local.explanation")}</p>
        </section>
    }
}

/// One truthful unavailable continue state, shared by Home and Library.
/// Account/session validation exists; it does not supply a saved-match list.
#[component]
pub fn ContinueRegion() -> impl IntoView {
    view! {
        <section class="continue-strip" aria-labelledby="continue-heading" data-state="unavailable">
            <span class="continue-strip__icon" aria-hidden="true"><Icon kind="history"/></span>
            <div>
                <h2 id="continue-heading" class="eyebrow">{translated("home.continue.heading")}</h2>
                <p role="status">{translated("home.continue.status")}</p>
                <small>{translated("home.continue.body")}</small>
            </div>
        </section>
    }
}
