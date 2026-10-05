//! Shared chrome: the top bar, the locale control, and the route fallback.

use leptos::prelude::*;
use leptos_router::components::A;
use tabula_registry::Locale;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::MediaQueryList;

use crate::{i18n::Messages, views::use_locale};

#[component]
pub fn TopBar() -> impl IntoView {
    let locale = use_locale();
    let scheme = system_scheme();
    // The document's language follows the shell's, so assistive technology
    // announces the copy in the language it is written in. The scheme follows
    // the viewer's own system settings: all four generated schemes are
    // reachable, and none of them is a choice this screen owns (screen 13).
    Effect::new(move |_| {
        let messages = Messages::new(locale.get());
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("lang", messages.locale().tag());
            let _ = root.set_attribute("data-theme", scheme.get());
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
                <A href="/account" attr:class="topbar__link">
                    {move || Messages::new(locale.get()).text("nav.account")}
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

/// The generated scheme that matches the viewer's system preferences, and
/// keeps matching them.
///
/// `tokens.css` keys its dark and high-contrast schemes off `data-theme`, so
/// something has to map the media queries onto it. This is that mapping and
/// nothing more: it chooses no colour and defines no token.
///
/// It is a signal rather than a value because the preference is not read once.
/// A viewer who turns on dark mode, or turns on more contrast, while the
/// document is open changes the answer; reading it only at mount would leave
/// them on the scheme that happened to be current at load, until a reload they
/// have no reason to perform.
fn system_scheme() -> ReadSignal<&'static str> {
    let dark = media_query("(prefers-color-scheme: dark)");
    let contrast = media_query("(prefers-contrast: more)");
    let read = {
        let dark = dark.clone();
        let contrast = contrast.clone();
        move || scheme_for(asked(dark.as_ref()), asked(contrast.as_ref()))
    };
    let (scheme, set_scheme) = signal(read());

    for list in [dark, contrast].into_iter().flatten() {
        let read = read.clone();
        let on_change = Closure::<dyn FnMut()>::new(move || set_scheme.set(read()));
        list.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        // The shell owns both listeners for the lifetime of the document: it
        // is never unmounted, so there is nothing to drop them on.
        on_change.forget();
    }

    scheme
}

/// One media query, or `None` where the browser will not answer it.
fn media_query(query: &str) -> Option<MediaQueryList> {
    window().match_media(query).ok().flatten()
}

/// Whether a preference is set. A query the browser will not answer reads as
/// unset, which is what the generated default scheme already assumes.
fn asked(list: Option<&MediaQueryList>) -> bool {
    list.is_some_and(MediaQueryList::matches)
}

/// The generated scheme named by a pair of preferences.
///
/// Separated from the browser so the mapping itself is checkable: the domain
/// is four cases and `tokens.css` defines exactly four schemes.
const fn scheme_for(dark: bool, more_contrast: bool) -> &'static str {
    match (dark, more_contrast) {
        (true, true) => "hc-dark",
        (true, false) => "dark",
        (false, true) => "hc-light",
        (false, false) => "light",
    }
}

#[cfg(test)]
mod tests {
    use super::scheme_for;

    /// Every combination of the two preferences names a distinct scheme, and
    /// each one is a scheme the generated stylesheet actually defines. A
    /// mapping that collapsed two cases would leave a viewer on a scheme they
    /// did not ask for, with no symptom the shell could report.
    #[test]
    fn each_pair_of_preferences_names_its_own_generated_scheme() {
        let named = [
            (false, false, "light"),
            (true, false, "dark"),
            (false, true, "hc-light"),
            (true, true, "hc-dark"),
        ];
        for (dark, contrast, expected) in named {
            assert_eq!(scheme_for(dark, contrast), expected, "{dark} {contrast}");
        }

        let all: Vec<&str> = named.iter().map(|(.., scheme)| *scheme).collect();
        let generated = include_str!("../../style/tokens.css");
        for scheme in &all {
            assert!(
                *scheme == "light" || generated.contains(&format!(r#"[data-theme="{scheme}"]"#)),
                "tokens.css defines no {scheme} scheme"
            );
        }
        assert_eq!(
            all.iter().collect::<std::collections::BTreeSet<_>>().len(),
            4,
            "two preference pairs share a scheme"
        );
    }
}
