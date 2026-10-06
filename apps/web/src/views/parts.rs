//! Shared chrome: the top bar, the locale control, and the route fallback.

use leptos::{prelude::*, tachys::html::InertElement};
use leptos_router::components::A;
use tabula_design::AvatarFallback;
use tabula_registry::Locale;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::MediaQueryList;

use crate::{i18n::Messages, views::use_locale};

/// One reactive text renderer for static translation keys. Sharing this
/// callable type avoids duplicating tachys effect/state code for each label;
/// every use still tracks locale changes independently.
pub fn translated(key: &'static str) -> impl Fn() -> String + Copy + Send + Sync + 'static {
    let locale = use_locale();
    move || Messages::new(locale.get()).text(key)
}

/// Persistent Design 01 chrome. The neutral avatar is a shared decorative fallback,
/// never a claim that this document has confirmed an account (issue #87 / #84).
#[component]
pub fn TopBar() -> impl IntoView {
    let locale = use_locale();
    let scheme = system_scheme();
    let location = leptos_router::hooks::use_location();
    let menu = NodeRef::<leptos::html::Dialog>::new();
    let menu_open = RwSignal::new(false);
    track_document_preferences(locale, scheme);
    let close_menu = move || {
        if let Some(dialog) = menu.get() {
            dialog.close();
        }
        menu_open.set(false);
    };
    close_menu_on_navigation(location.pathname, location.search, menu, menu_open);
    view! {
        <aside class="sidebar" aria-label=translated("shell.navigation")>
            <Brand/>
            <ShellNavigation/>
            <div class="sidebar__spacer"></div>
            <div class="sidebar__note">
                <Icon kind="leaf"/>
                <p>{translated("shell.note")}</p>
                <small>{translated("shell.note.detail")}</small>
            </div>
            <A href="/account" attr:class="sidebar__profile">
                <NeutralAvatar/>
                <span>{translated("nav.account")}</span>
                <Icon kind="arrow"/>
            </A>
        </aside>
        <header class="topbar">
            <div class="topbar__brand"><Brand/></div>
            <div class="topbar__context">
                <Icon kind="grid"/>
                <span>{translated("shell.context")}</span>
                <span aria-hidden="true">"/"</span>
                <span>{move || {
                    let path = location.pathname.get();
                    Messages::new(locale.get()).text(route_context_key(&path))
                }}</span>
            </div>
            <div class="topbar__actions">
                <div class="topbar__locale">
                    <label class="field__label" for="locale">
                        {translated("app.locale")}
                    </label>
                    <select id="locale" class="field__control" on:change=move |event| {
                        if let Some(next) = Locale::parse(&event_target_value(&event)) {
                            locale.set(next);
                        }
                    }>
                        {Locale::ALL.into_iter().map(|candidate| view! {
                            <option value=candidate.tag() selected=move || locale.get() == candidate>
                                {candidate.label()}
                            </option>
                        }).collect_view()}
                    </select>
                </div>
                <A href="/account" attr:class="topbar__profile" attr:aria-label=translated("nav.account")>
                    <NeutralAvatar/>
                </A>
                <button id="menu-toggle" type="button" class="icon-button mobile-menu-button"
                    aria-label=translated("shell.menu.open")
                    aria-controls="shell-menu" aria-haspopup="dialog" aria-expanded=move || menu_open.get()
                    on:click=move |_| {
                        if let Some(dialog) = menu.get() {
                            if dialog.show_modal().is_ok() { menu_open.set(true); }
                        }
                    }>
                    <Icon kind="menu"/>
                </button>
            </div>
        </header>
        <dialog id="shell-menu" class="shell-menu" node_ref=menu
            on:click=move |event| {
                if event.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                    .and_then(|element| element.closest("a").ok().flatten()).is_some() { close_menu(); }
            }
            aria-label=translated("shell.navigation")
            on:close=move |_: leptos::ev::Event| menu_open.set(false)
            on:cancel=move |_: leptos::ev::Event| menu_open.set(false)
            on:keydown=move |event| trap_dialog_tab(menu, &event)>
            <div class="shell-menu__header">
                <Brand/>
                <button type="button" class="icon-button" autofocus=true
                    aria-label=translated("shell.menu.close")
                    on:click=move |_| close_menu()><Icon kind="close"/></button>
            </div>
            <ShellNavigation/>
        </dialog>
        <nav class="bottom-nav" aria-label=translated("shell.navigation")>
            <ShellLinks/>
        </nav>
    }
}

/// A known document names its own task; unknown addresses claim no account context.
fn route_context_key(pathname: &str) -> &'static str {
    match pathname {
        "/" => "nav.home",
        "/games" => "nav.library",
        "/account" => "nav.account",
        "/me" => "accounts.profile.self",
        "/login" => "accounts.login.title",
        "/register" => "accounts.register.title",
        "/friends" => "accounts.friends.title",
        path if path.starts_with("/games/") => "shell.context.game",
        path if path.starts_with("/u/") => "accounts.profile.title",
        _ => "app.title",
    }
}

/// One application owner synchronizes language and generated system scheme.
fn track_document_preferences(locale: super::LocaleSignal, scheme: ReadSignal<&'static str>) {
    Effect::new(move |_| {
        let messages = Messages::new(locale.get());
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("lang", messages.locale().tag());
            let _ = root.set_attribute("data-theme", scheme.get());
        }
    });
}

/// Back/Forward and programmatic route changes also retire a modal drawer.
fn close_menu_on_navigation(
    pathname: Memo<String>,
    search: Memo<String>,
    menu: NodeRef<leptos::html::Dialog>,
    menu_open: RwSignal<bool>,
) {
    Effect::new(move |_| {
        let _ = (pathname.get(), search.get());
        if menu_open.get_untracked() {
            if let Some(dialog) = menu.get() {
                dialog.close();
            }
            menu_open.set(false);
        }
    });
}

/// Wrap Tab at the native dialog boundary, including reverse traversal.
fn trap_dialog_tab(menu: NodeRef<leptos::html::Dialog>, event: &leptos::ev::KeyboardEvent) {
    if event.key() != "Tab" {
        return;
    }
    let Some(dialog) = menu.get() else {
        return;
    };
    let Ok(targets) = dialog.query_selector_all("a[href], button:not([disabled]), select:not([disabled]), input:not([disabled]), [tabindex=\"0\"]") else { return; };
    let Some(first) = targets
        .item(0)
        .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
    else {
        return;
    };
    let Some(last) = targets
        .item(targets.length().saturating_sub(1))
        .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
    else {
        return;
    };
    let active = document().active_element();
    if event.shift_key()
        && active
            .as_ref()
            .is_some_and(|active| active.is_same_node(Some(first.as_ref())))
    {
        event.prevent_default();
        let _ = last.focus();
    } else if !event.shift_key()
        && active
            .as_ref()
            .is_some_and(|active| active.is_same_node(Some(last.as_ref())))
    {
        event.prevent_default();
        let _ = first.focus();
    }
}

#[component]
fn ShellLinks() -> impl IntoView {
    view! {
        <A href="/" exact=true attr:class="shell-link"><Icon kind="home"/><span>{translated("nav.home")}</span></A>
        <A href="/games" attr:class="shell-link"><Icon kind="grid"/><span>{translated("nav.library")}</span></A>
        <A href="/account" attr:class="shell-link"><Icon kind="user"/><span>{translated("nav.account")}</span></A>
    }
}

#[component]
fn ShellNavigation() -> impl IntoView {
    view! {
        <nav class="sidebar__nav"><ShellLinks/></nav>
        <p class="sidebar__caption">{translated("shell.upcoming")}</p>
        <div class="sidebar__unavailable">
            <span><Icon kind="history"/>{translated("shell.history")}</span>
            <span><Icon kind="users"/>{translated("shell.rooms")}</span>
            <small>{translated("shell.upcoming.reason")}</small>
        </div>
    }
}

#[component]
fn Brand() -> impl IntoView {
    view! {
        <A href="/" attr:class="brand" attr:aria-label="Tabula">
            {InertElement::new(include_str!("../../../../assets/brand/generated/tabula-lockup.svg"))}
        </A>
    }
}

/// Original Design 01 vector icons, with no icon font or runtime request.
#[component]
pub fn Icon(kind: &'static str) -> impl IntoView {
    let paths = match kind {
        "grid" => {
            r#"<rect x="3" y="3" width="7" height="7" rx="2"/><rect x="14" y="3" width="7" height="7" rx="2"/><rect x="3" y="14" width="7" height="7" rx="2"/><rect x="14" y="14" width="7" height="7" rx="2"/>"#
        }
        "home" => r#"<path d="m3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1z"/>"#,
        "user" => r#"<circle cx="12" cy="8" r="4"/><path d="M4 21v-1a8 8 0 0 1 16 0v1"/>"#,
        "users" => {
            r#"<circle cx="9" cy="8" r="3"/><path d="M3 21v-2a6 6 0 0 1 12 0v2M16 5a3 3 0 0 1 0 6m3 10v-2a6 6 0 0 0-2-4"/>"#
        }
        "history" => r#"<path d="M3 4v5h5M3 9a9 9 0 1 1 0 7m9-9v5l3 2"/>"#,
        "leaf" => r#"<path d="M20 4c-8-2-15 3-15 9a7 7 0 0 0 7 7c6 0 9-8 8-16ZM4 21 17 8"/>"#,
        "spark" => {
            r#"<path d="m12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5zM21 2v4m-2-2h4"/>"#
        }
        "close" => r#"<path d="m6 6 12 12M6 18 18 6"/>"#,
        "menu" => r#"<path d="M4 6h16M4 12h16M4 18h16"/>"#,
        _ => r#"<path d="M4 12h16m-6-6 6 6-6 6"/>"#,
    };
    view! { <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" inner_html=paths></svg> }
}

/// Shared public fallback, without requesting or synthesizing profile fields.
/// Dashboard, header and gameplay use the same semantic occupant marker.
#[component]
pub fn NeutralAvatar(
    #[prop(default = AvatarFallback::Human)] fallback: AvatarFallback,
) -> impl IntoView {
    // Keep both optional children typed: the default CSR build's static HTML
    // checks can render these without an SSR-only erased-view implementation.
    let silhouette = (fallback == AvatarFallback::Human).then(|| {
        view! {
            <svg viewBox="0 0 100 100" fill="currentColor" focusable="false">
                <circle cx="50" cy="34" r="14"/>
                <rect x="23" y="53" width="54" height="25" rx="12"/>
            </svg>
        }
    });
    let glyph = (fallback != AvatarFallback::Human).then(|| fallback.glyph());
    view! {
        <span class="neutral-avatar" data-avatar-kind=fallback.kind() aria-hidden="true">
            {silhouette}
            {glyph}
        </span>
    }
}

/// A URL no route owns.
#[component]
pub fn RouteNotFound() -> impl IntoView {
    view! {
        <section class="section">
            <h1 class="section__title">
                {translated("detail.notfound.heading")}
            </h1>
            <p class="section__body">
                {translated("detail.notfound.body")}
            </p>
            <A href="/games" attr:class="btn btn--tonal">
                {translated("detail.back")}
            </A>
        </section>
    }
}

/// A reason and its recovery, at full contrast and never only a tooltip.
#[component]
pub fn Reason(reason_key: &'static str, recovery_key: &'static str) -> impl IntoView {
    view! {
        <p class="reason">
            <span class="reason__why">
                {translated(reason_key)}
            </span>
            " "
            <span class="reason__how">
                {translated(recovery_key)}
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
    use super::{route_context_key, scheme_for};

    #[test]
    fn chrome_context_names_supported_documents_without_fabricating_account_state() {
        for (path, key) in [
            ("/", "nav.home"),
            ("/games", "nav.library"),
            ("/games/example", "shell.context.game"),
            ("/me", "accounts.profile.self"),
            ("/login", "accounts.login.title"),
            ("/register", "accounts.register.title"),
            ("/friends", "accounts.friends.title"),
            ("/u/example", "accounts.profile.title"),
            ("/unknown", "app.title"),
        ] {
            assert_eq!(route_context_key(path), key);
        }
    }

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
