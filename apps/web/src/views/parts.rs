//! Shared chrome: the top bar, the locale control, and the route fallback.

use leptos::prelude::*;
use leptos_router::components::A;
use tabula_registry::Locale;

use crate::{i18n::Messages, views::use_locale};

#[component]
pub fn TopBar() -> impl IntoView {
    let locale = use_locale();
    // The document's language follows the shell's, so assistive technology
    // announces the copy in the language it is written in. The scheme follows
    // the viewer's own system settings: all four generated schemes are
    // reachable, and none of them is a choice this screen owns (screen 13).
    Effect::new(move |_| {
        let messages = Messages::new(locale.get());
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("lang", messages.locale().tag());
            let _ = root.set_attribute("data-theme", system_scheme());
        }
    });
    view! {
        <header class="topbar">
            <nav class="topbar__nav" aria-label="Tabula">
                <A href="/" exact=true attr:class="topbar__link">
                    {move || Messages::new(locale.get()).text("nav.home")}
                </A>
                <A href="/games" attr:class="topbar__link">
                    {move || Messages::new(locale.get()).text("nav.library")}
                </A>
            </nav>
            <div class="topbar__locale">
                <label class="field__label" for="locale">
                    {move || Messages::new(locale.get()).text("app.locale")}
                </label>
                <select
                    id="locale"
                    class="field__control"
                    on:change=move |event| {
                        let value = event_target_value(&event);
                        if let Some(next) = Locale::parse(&value) {
                            locale.set(next);
                        }
                    }
                >
                    {Locale::ALL
                        .into_iter()
                        .map(|candidate| {
                            view! {
                                <option
                                    value=candidate.tag()
                                    selected=move || locale.get() == candidate
                                >
                                    {candidate.label()}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </div>
        </header>
    }
}

/// A URL no route owns.
#[component]
pub fn RouteNotFound() -> impl IntoView {
    let locale = use_locale();
    view! {
        <section class="section">
            <h1 class="section__title">
                {move || Messages::new(locale.get()).text("detail.notfound.heading")}
            </h1>
            <p class="section__body">
                {move || Messages::new(locale.get()).text("detail.notfound.body")}
            </p>
            <A href="/games" attr:class="btn btn--tonal">
                {move || Messages::new(locale.get()).text("detail.back")}
            </A>
        </section>
    }
}

/// A reason and its recovery, at full contrast and never only a tooltip.
#[component]
pub fn Reason(reason_key: &'static str, recovery_key: &'static str) -> impl IntoView {
    let locale = use_locale();
    view! {
        <p class="reason">
            <span class="reason__why">
                {move || Messages::new(locale.get()).text(reason_key)}
            </span>
            " "
            <span class="reason__how">
                {move || Messages::new(locale.get()).text(recovery_key)}
            </span>
        </p>
    }
}

/// The generated scheme that matches the viewer's system preferences.
///
/// `tokens.css` keys its dark and high-contrast schemes off `data-theme`, so
/// something has to map the media queries onto it. This is that mapping and
/// nothing more: it chooses no colour and defines no token.
fn system_scheme() -> &'static str {
    let matches = |query: &str| {
        window()
            .match_media(query)
            .ok()
            .flatten()
            .is_some_and(|list| list.matches())
    };
    match (
        matches("(prefers-color-scheme: dark)"),
        matches("(prefers-contrast: more)"),
    ) {
        (true, true) => "hc-dark",
        (true, false) => "dark",
        (false, true) => "hc-light",
        (false, false) => "light",
    }
}
