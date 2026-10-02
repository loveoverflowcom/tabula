//! 01 — the resume-first home at `/`.
//!
//! Resume comes before browsing when an adapter supplies it. There is no
//! session service in this build, so the region states that plainly instead of
//! inventing a match, a clock, or a turn (docs/ui/screens/01-library.md).

use leptos::prelude::*;
use leptos_router::components::A;

use crate::{
    i18n::shell,
    views::{library::card, use_locale},
};

#[component]
pub fn Home() -> impl IntoView {
    let locale = use_locale();
    view! {
        <section class="section">
            {move || {
                let (messages, catalog) = shell(locale.get());
                let startable: Vec<_> = catalog
                    .entries()
                    .iter()
                    .filter(|entry| entry.startable())
                    .collect();
                view! {
                    <h1 class="section__title">{messages.text("home.heading")}</h1>
                    <div class="resume">
                        <p class="banner" role="status">
                            {messages.text("home.resume.unavailable")}
                        </p>
                    </div>
                    <h2 class="section__subtitle" id="featured">
                        {messages.text("home.featured")}
                    </h2>
                    <ul class="cards" aria-labelledby="featured">
                        {startable
                            .iter()
                            .map(|entry| card(&messages, entry))
                            .collect_view()}
                    </ul>
                    <A href="/games" attr:class="btn btn--tonal">
                        {messages.text("home.browse")}
                    </A>
                }
                    .into_any()
            }}
        </section>
    }
}
