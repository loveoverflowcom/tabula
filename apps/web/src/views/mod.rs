//! One module per route (doc 04 §2.1).

pub mod account;
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

/// The locale the whole shell renders in.
pub type LocaleSignal = RwSignal<Locale>;

#[must_use]
pub fn use_locale() -> LocaleSignal {
    use_context::<LocaleSignal>().expect("the shell provides its locale")
}

/// The application shell: skip link, navigation, routed main region.
#[component]
pub fn App() -> impl IntoView {
    let locale: LocaleSignal = RwSignal::new(Locale::En);
    provide_context(locale);
    crate::account::provide_account_session();

    view! {
        <Router>
            <a class="skip-link" href="#main">
                {move || Messages::new(locale.get()).text("app.skip")}
            </a>
            <parts::TopBar/>
            <main id="main" class="main" tabindex="-1">
                <Routes fallback=|| view! { <parts::RouteNotFound/> }>
                    <Route path=path!("/") view=home::Home/>
                    <Route path=path!("/games") view=library::Library/>
                    <Route path=path!("/games/:id") view=detail::GameDetail/>
                    <Route path=path!("/account") view=account::AccountPage/>
                    <Route path=path!("/me") view=account::SelfProfile/>
                    <Route path=path!("/login") view=account::LoginPage/>
                    <Route path=path!("/register") view=account::RegisterUnavailable/>
                    <Route path=path!("/friends") view=account::FriendsUnavailable/>
                    <Route path=path!("/u/:handle") view=account::OtherProfileUnavailable/>
                </Routes>
            </main>
        </Router>
    }
}
