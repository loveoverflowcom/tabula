//! One module per route (doc 04 §2.1).

pub mod account;
#[cfg(feature = "account-social")]
pub mod accounts_full;
pub mod detail;
pub mod home;
pub mod library;
pub mod new_match;
pub mod parts;

use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};
use tabula_registry::Locale;

use crate::i18n::Messages;

#[cfg(not(feature = "account-social"))]
use account::{
    FriendsUnavailable as FriendsPage, OtherProfileUnavailable as OtherProfilePage,
    RegisterUnavailable as RegisterPage,
};
#[cfg(feature = "account-social")]
use accounts_full::{FriendsPage, OtherProfilePage, RegisterPage};

/// The locale the whole shell renders in.
pub type LocaleSignal = RwSignal<Locale>;

#[must_use]
pub fn use_locale() -> LocaleSignal {
    use_context::<LocaleSignal>().expect("the shell provides its locale")
}

// Account conditionals share a browser rendering implementation, rather than
// specialize the memo/Either/mount machinery for every native form subtree.
// ChildrenFn keeps lazy construction and owner cleanup in Leptos's Show.
#[cfg(target_arch = "wasm32")]
#[component]
fn GuardedShow<W>(when: W, children: ChildrenFn) -> AnyView
where
    W: Fn() -> bool + Send + Sync + 'static,
{
    guarded_show(Signal::derive(when), children)
}

#[cfg(target_arch = "wasm32")]
#[inline(never)]
fn guarded_show(when: Signal<bool>, children: ChildrenFn) -> AnyView {
    view! {
        <Show when=move || when.try_get().unwrap_or(false)>
            {children()}
        </Show>
    }
    .into_any()
}

// Fixed callable types keep reactive text/property effects shared across account
// fields. Derived computations still track their original reads; retirement
// selects the caller's public fallback instead of reading disposed arena data.
fn boolean(signal: Signal<bool>, retired: bool) -> impl Fn() -> bool + Copy + Send + Sync {
    move || signal.try_get().unwrap_or(retired)
}

fn text(signal: Signal<String>, retired: &'static str) -> impl Fn() -> String + Copy + Send + Sync {
    move || signal.try_get().unwrap_or_else(|| retired.to_owned())
}

fn optional_text(
    signal: Signal<Option<String>>,
) -> impl Fn() -> Option<String> + Copy + Send + Sync {
    move || signal.try_get().flatten()
}

fn static_text(
    signal: Signal<&'static str>,
    retired: &'static str,
) -> impl Fn() -> &'static str + Copy + Send + Sync {
    move || signal.try_get().unwrap_or(retired)
}

/// The application shell: skip link, navigation, routed main region.
#[component]
pub fn App() -> impl IntoView {
    let locale: LocaleSignal = RwSignal::new(Locale::En);
    provide_context(locale);
    crate::account::provide_account_session();
    #[cfg(feature = "account-social")]
    crate::social_full::provide_social();

    view! {
        <Router>
            // An explicit same-context target bypasses the router's delegated
            // anchor interception. Native fragment navigation then scrolls AND
            // focuses the tabindex=-1 main, rather than only scrolling it.
            <a class="skip-link" href="#main" target="_self">
                {move || Messages::new(locale.get()).text("app.skip")}
            </a>
            <parts::TopBar/>
            <main id="main" class="main" tabindex="-1" data-shell-profile=if cfg!(feature = "account-social") { "tabula-shell-account-social-v1" } else { "tabula-shell-standard-v1" }>
                <Routes fallback=|| view! { <parts::RouteNotFound/> }>
                    <Route path=path!("/") view=home::Home/>
                    <Route path=path!("/games") view=library::Library/>
                    <Route path=path!("/games/:id") view=detail::GameDetail/>
                    <Route path=path!("/account") view=account::AccountPage/>
                    <Route path=path!("/me") view=account::SelfProfile/>
                    <Route path=path!("/login") view=account::LoginPage/>
                    <Route path=path!("/register") view=RegisterPage/>
                    <Route path=path!("/friends") view=FriendsPage/>
                    <Route path=path!("/u/:handle") view=OtherProfilePage/>
                </Routes>
            </main>
        </Router>
    }
}
