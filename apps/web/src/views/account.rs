//! Compact account state and immutable account identity under ADR-0036.
//!
//! The isolated adapter owns authority and cleanup. This view only renders its
//! structured dispositions; provider sign-in uses only a top-level redirect.
//! Separate ADR-0044 controls render a confirmed v2 self profile when available
//! (docs/ui/screens/account-state-isolated.md). Fixed escapes deliberately ignore
//! return parameters rather than interpreting a URL as authorization.

use leptos::{html, prelude::*};
use leptos_router::components::A;

#[cfg(target_arch = "wasm32")]
use super::GuardedShow as Show;

use crate::{
    account::{use_account, AccountOperation, AccountSnapshot, AccountStatus},
    i18n::Messages,
    views::{
        parts::{translated, NeutralAvatar},
        use_locale,
    },
};

/// Teardown can queue an old render before its arena values are disposed.
/// Retired bindings select public pending output, never the last private facts.
fn presentation_snapshot(state: ReadSignal<AccountSnapshot>) -> AccountSnapshot {
    state.try_get().unwrap_or(AccountSnapshot {
        status: AccountStatus::Resolving,
        busy: None,
        presentation_generation: 0,
        login_available: false,
    })
}

/// Unlike `NodeRef::on_load`, this queued effect tolerates route-owner retirement.
pub(super) fn focus_on_arrival<T>(node: NodeRef<T>)
where
    T: leptos::tachys::html::element::ElementType + 'static,
    T::Output: AsRef<web_sys::HtmlElement> + wasm_bindgen::JsCast + Clone + 'static,
{
    let focused = StoredValue::new(false);
    Effect::new(move |_| {
        if !focused.try_with_value(|focused| !*focused).unwrap_or(false) {
            return;
        }
        if let Some(element) = node.try_get().flatten() {
            focused.try_update_value(|focused| *focused = true);
            let target: &web_sys::HtmlElement = element.as_ref();
            let _ = target.focus();
        }
    });
}

/// Current account state at `/account`, without an identity-provider form.
#[component]
pub fn AccountPage() -> impl IntoView {
    view! { <AccountTask task=AccountTaskKind::Account/> }
}

/// Current self task at `/me`; separate v2 controls require a fresh profile read.
#[component]
pub fn SelfProfile() -> impl IntoView {
    view! { <AccountTask task=AccountTaskKind::Profile/> }
}

/// The route's task changes presentation, never session permissions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AccountTaskKind {
    Account,
    Login,
    Profile,
}

impl AccountTaskKind {
    const fn keys(self) -> (&'static str, &'static str) {
        match self {
            Self::Account => ("accounts.title", "accounts.intro.account"),
            Self::Login => ("accounts.login.title", "accounts.intro.login"),
            Self::Profile => ("accounts.profile.self", "accounts.intro.profile"),
        }
    }
}

#[component]
fn AccountTask(task: AccountTaskKind) -> impl IntoView {
    let (title_key, intro_key) = task.keys();
    let locale = use_locale();
    let controller = use_account();
    let state = controller.state;
    let snapshot = move || presentation_snapshot(state);
    let confirming = RwSignal::new(false);
    let full_profile = RwSignal::new(false);
    let heading = NodeRef::<html::H1>::new();
    let logout_invoker = NodeRef::<html::Button>::new();
    focus_on_arrival(heading);

    // A permission/lifecycle transition dismisses a merely local confirmation.
    // It cannot execute a logout or preserve a former viewer's private data.
    Effect::new(move |_| {
        if profile_for(&snapshot()).is_none() {
            confirming.try_set(false);
        }
    });

    #[cfg(feature = "account-social")]
    let profile_controls = if task == AccountTaskKind::Profile {
        Some(
            view! { <super::accounts_full::ProfileControls account=controller verified=full_profile/> },
        )
    } else {
        None
    };
    #[cfg(not(feature = "account-social"))]
    let profile_controls = ();

    let page = view! {
        <section class="section account" aria-labelledby="account-title">
            <h1 id="account-title" class="section__title" tabindex="-1" node_ref=heading>
                {translated(title_key)}
            </h1>
            <p class="section__body">
                {translated(intro_key)}
            </p>
            <div class="account__state" aria-busy=super::static_text(Signal::derive(move || if snapshot().busy.is_some() { "true" } else { "false" }), "false")>
                <p class="status" role="status" aria-atomic="true">
                    {super::text(Signal::derive(move || Messages::new(locale.get()).text(status_key(&snapshot()))), "")}
                </p>
                <Show when=move || failure_key(&snapshot()).is_some()>
                    <p class="banner banner--error" role="alert">
                        {super::optional_text(Signal::derive(move || {
                            failure_key(&snapshot())
                                .map(|key| Messages::new(locale.get()).text(key))
                        }))}
                    </p>
                </Show>
            </div>

            // A fresh generation key forces new private nodes, including after a
            // restored document was synchronously masked by the adapter.
            <For
                each={move || profile_for(&snapshot()).into_iter().collect::<Vec<_>>()}
                key={|(generation, account_id)| (*generation, account_id.clone())}
                children={move |(_, account_id)| {
                    if task == AccountTaskKind::Login {
                        view! { <SignedInDestination/> }.into_any()
                    } else {
                        view! { <ProfileFacts account_id/> }.into_any()
                    }
                }}
            />
            {profile_controls}

            <Show when=move || login_allowed(&snapshot())>
                <p class="section__body">
                    {translated("accounts.login.invited")}
                </p>
            </Show>
            <Show when=move || profile_for(&snapshot()).is_some()>
                <p class="section__body">
                    {translated("accounts.login.switch")}
                </p>
            </Show>
            <Show when=move || {
                let state = snapshot();
                state.busy.is_none() && !state.login_available
                    && matches!(state.status, AccountStatus::SignedOut | AccountStatus::Expired)
            }>
                <p class="section__body">
                    {translated("accounts.login.unavailable")}
                </p>
            </Show>
            <div class="actions account__actions">
                <Show when=move || login_allowed(&snapshot())>
                    <LoginAction on_start=move || {
                        confirming.try_set(false);
                        if let Some(element) = heading.try_get().flatten() {
                            let _ = element.focus();
                        }
                        controller.login();
                    }/>
                </Show>
                <button
                    type="button"
                    class=super::static_text(Signal::derive(move || if full_profile.try_get().unwrap_or(false) || login_allowed(&snapshot())
                        || (task == AccountTaskKind::Login && profile_for(&snapshot()).is_some())
                    { "btn btn--tonal" } else { "btn btn--filled btn--principal" }), "btn btn--tonal")
                    disabled=super::boolean(Signal::derive(move || snapshot().busy.is_some()), true)
                    on:click=move |_| {
                        confirming.try_set(false);
                        if let Some(element) = heading.try_get().flatten() {
                            let _ = element.focus();
                        }
                        controller.recheck();
                    }
                >
                    {translated("accounts.action.check")}
                </button>
                <Show when=move || profile_for(&snapshot()).is_some()>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        data-account-private=""
                        on:click=move |_| {
                            confirming.try_set(false);
                            if let Some(element) = heading.try_get().flatten() {
                                let _ = element.focus();
                            }
                            controller.refresh();
                        }
                    >
                        {translated("accounts.action.refresh")}
                    </button>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        data-account-private=""
                        node_ref=logout_invoker
                        aria-expanded=super::static_text(Signal::derive(move || if confirming.try_get().unwrap_or(false) { "true" } else { "false" }), "false")
                        aria-controls=super::optional_text(Signal::derive(move || confirming.try_get().unwrap_or(false).then(|| "logout-confirmation".to_owned())))
                        on:click=move |_| { confirming.try_set(true); }
                    >
                        {translated("accounts.action.logout")}
                    </button>
                </Show>
                <Show when=move || {
                    logout_retry_allowed(&snapshot())
                }>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        on:click=move |_| {
                            if let Some(element) = heading.try_get().flatten() {
                                let _ = element.focus();
                            }
                            controller.logout();
                        }
                    >
                        {translated("accounts.action.logout_retry")}
                    </button>
                </Show>
                <Show when=move || snapshot().busy.is_some()>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        on:click=move |_| {
                            controller.cancel();
                            if let Some(element) = heading.try_get().flatten() {
                                let _ = element.focus();
                            }
                        }
                    >
                        {translated("accounts.action.cancel")}
                    </button>
                </Show>
            </div>

            <Show when=move || confirming.try_get().unwrap_or(false) && profile_for(&snapshot()).is_some()>
                <LogoutConfirmation
                    on_cancel=move || {
                        confirming.try_set(false);
                        if let Some(element) = logout_invoker.try_get().flatten() {
                            let _ = element.focus();
                        }
                    }
                    on_confirm=move || {
                        confirming.try_set(false);
                        if let Some(element) = heading.try_get().flatten() {
                            let _ = element.focus();
                        }
                        controller.logout();
                    }
                />
            </Show>

            <LocalEscape on_leave=move || controller.cancel()/>
            <UnavailableLinks controller/>
        </section>
    };
    // Routes can rebuild identical DOM shapes before disposing the previous
    // owner. A different task must remount its owner-bound reactive closures,
    // rather than retain bindings to the previous route's disposed signals.
    match task {
        AccountTaskKind::Account => leptos::either::EitherOf3::A(page),
        AccountTaskKind::Login => leptos::either::EitherOf3::B(page),
        AccountTaskKind::Profile => leptos::either::EitherOf3::C(page),
    }
}

/// Already signed-in Login has one read-only destination, with no ID in copy.
/// This node shares the same synchronous lifecycle mask as private facts.
#[component]
fn SignedInDestination() -> impl IntoView {
    view! {
        <div class="account__profile" data-account-private="">
            <p class="section__body">{translated("accounts.login.already_signed_in")}</p>
            <div class="actions account__actions">
                <A href="/me" attr:class="btn btn--filled btn--principal">
                    {translated("accounts.action.profile")}
                </A>
            </div>
        </div>
    }
}

/// Account-independent escape shared by every account task (doc 04 §2).
#[component]
fn LocalEscape(
    on_leave: impl Fn() + Send + Sync + Copy + 'static,
    #[prop(default = false)] principal: bool,
) -> impl IntoView {
    view! {
        <aside class="account__local" aria-labelledby="account-local-title">
            <h2 id="account-local-title" class="field__label">{translated("accounts.local.title")}</h2>
            <p class="section__body">{translated("accounts.local.explanation")}</p>
            <div class="actions account__actions">
                <A href="/games"
                    attr:class=if principal { "btn btn--filled btn--principal" } else { "btn btn--tonal" }
                    on:click=move |_| on_leave()>
                    {translated("accounts.action.library")}
                </A>
                <A href="/" attr:class="btn btn--text" on:click=move |_| on_leave()>
                    {translated("nav.home")}
                </A>
            </div>
        </aside>
    }
}

/// Explicit provider continuation; no credential fields or direct provider URL.
#[component]
fn LoginAction(on_start: impl Fn() + Send + Sync + Copy + 'static) -> impl IntoView {
    view! {
        <button type="button" class="btn btn--filled btn--principal" on:click=move |_| on_start()>
            {translated("accounts.action.login")}
        </button>
    }
}

/// Immutable ID facts do not establish availability of v2 profile metadata.
/// The separate profile controls require their own validated permitted response.
#[component]
fn ProfileFacts(account_id: String) -> impl IntoView {
    view! {
        <section class="account__profile" data-account-private="" aria-labelledby="self-facts-title">
            <h2 id="self-facts-title" class="section__subtitle">
                <NeutralAvatar/>
                {translated("accounts.profile.id")}
            </h2>
            <dl class="facts account__facts">
                <dt>{translated("accounts.profile.id")}</dt>
                <dd class="account__id">{account_id}</dd>
            </dl>
            <p class="section__body">
                {translated("accounts.profile.only_id")}
            </p>
        </section>
    }
}

/// An in-document confirmation, not a modal: ordinary Tab order stays usable.
/// Cancel and Escape retire only this local prompt, before any request is sent.
#[component]
fn LogoutConfirmation(
    on_cancel: impl Fn() + Send + Sync + Copy + 'static,
    on_confirm: impl Fn() + Send + Sync + Copy + 'static,
) -> impl IntoView {
    let cancel = NodeRef::<html::Button>::new();
    focus_on_arrival(cancel);
    view! {
        <fieldset
            id="logout-confirmation"
            class="account__confirmation"
            data-account-private=""
            aria-describedby="logout-consequence"
            on:keydown=move |event| {
                if event.key() == "Escape" {
                    event.prevent_default();
                    on_cancel();
                }
            }
        >
            <legend class="field__label">
                {translated("accounts.logout.title")}
            </legend>
            <p id="logout-consequence" class="section__body">
                {translated("accounts.logout.explanation")}
            </p>
            <div class="actions account__actions">
                <button type="button" class="btn btn--tonal" node_ref=cancel on:click=move |_| on_cancel()>
                    {translated("accounts.action.cancel")}
                </button>
                <button type="button" class="btn btn--danger" on:click=move |_| on_confirm()>
                    {translated("accounts.action.logout")}
                </button>
            </div>
        </fieldset>
    }
}

/// A visibly unavailable route is still a useful explanatory destination.
#[derive(Clone, Copy)]
enum UnavailableTask {
    Register,
    Friends,
    OtherProfile,
}

impl UnavailableTask {
    const fn keys(self) -> (&'static str, &'static str) {
        match self {
            Self::Register => ("accounts.register.title", "accounts.register.unavailable"),
            Self::Friends => ("accounts.friends.title", "accounts.friends.unavailable"),
            Self::OtherProfile => (
                "accounts.profile.title",
                "accounts.profile.other_unavailable",
            ),
        }
    }
}

#[component]
pub fn LoginPage() -> impl IntoView {
    // Availability comes from fresh backend capabilities; no credential form.
    view! { <AccountTask task=AccountTaskKind::Login/> }
}

#[component]
pub fn RegisterUnavailable() -> impl IntoView {
    view! { <UnavailablePage task=UnavailableTask::Register/> }
}

#[component]
pub fn FriendsUnavailable() -> impl IntoView {
    view! { <UnavailablePage task=UnavailableTask::Friends/> }
}

#[component]
pub fn OtherProfileUnavailable() -> impl IntoView {
    // The URL text is not resolved identity and does not select the self route.
    view! { <UnavailablePage task=UnavailableTask::OtherProfile/> }
}

#[component]
fn UnavailablePage(task: UnavailableTask) -> impl IntoView {
    let (title_key, body_key) = task.keys();
    let heading = NodeRef::<html::H1>::new();
    focus_on_arrival(heading);
    view! {
        <section class="section account" aria-labelledby="account-title">
            <h1 id="account-title" class="section__title" tabindex="-1" node_ref=heading>
                {translated(title_key)}
            </h1>
            <div class="account__state">
                <p class="status">{translated("accounts.features.unavailable")}</p>
                <p class="section__body">{translated(body_key)}</p>
                <div class="actions account__actions">
                    <A href="/login" attr:class="btn btn--tonal">
                        {translated("accounts.action.signin")}
                    </A>
                    <A href="/account" attr:class="btn btn--text">
                        {translated("accounts.action.back_account")}
                    </A>
                </div>
            </div>
            <LocalEscape on_leave=|| {} principal=true/>
        </section>
    }
}

#[component]
fn UnavailableLinks(controller: crate::account::AccountController) -> impl IntoView {
    let locale = use_locale();
    view! {
        <nav aria-label=translated("accounts.features.title")>
            <ul class="rows">
                <li class="row">
                    <A href="/register" attr:class="account__feature-link">
                        {super::text(Signal::derive(move || Messages::new(locale.get()).text(if controller.enrollment_available() { "accounts.register.title" } else { "accounts.register.link_unavailable" })), "")}
                    </A>
                    <p class="row__body">{super::text(Signal::derive(move || Messages::new(locale.get()).text(if controller.enrollment_available() { "accounts.full.register_available" } else { "accounts.register.reason" })), "")}</p>
                </li>
                <li class="row">
                    <A href="/friends" attr:class="account__feature-link">
                        {super::text(Signal::derive(move || Messages::new(locale.get()).text(if controller.social_available() { "accounts.friends.title" } else { "accounts.friends.link_unavailable" })), "")}
                    </A>
                    <p class="row__body">{super::text(Signal::derive(move || Messages::new(locale.get()).text(if controller.social_available() { "accounts.full.friends_available" } else { "accounts.friends.reason" })), "")}</p>
                </li>
            </ul>
        </nav>
    }
}

/// Presentation is fail-closed even if an adapter accidentally retains an old
/// authenticated status while an operation is pending. IDs never enter alerts.
fn profile_for(snapshot: &AccountSnapshot) -> Option<(u64, String)> {
    match &snapshot.status {
        AccountStatus::Authenticated { account_id } if snapshot.busy.is_none() => {
            Some((snapshot.presentation_generation, account_id.clone()))
        }
        _ => None,
    }
}

fn login_allowed(snapshot: &AccountSnapshot) -> bool {
    snapshot.login_available
        && snapshot.busy.is_none()
        && matches!(
            snapshot.status,
            AccountStatus::SignedOut | AccountStatus::Expired
        )
}

fn logout_retry_allowed(snapshot: &AccountSnapshot) -> bool {
    snapshot.busy.is_none() && matches!(snapshot.status, AccountStatus::LogoutPending)
}

fn status_key(snapshot: &AccountSnapshot) -> &'static str {
    if snapshot.status == AccountStatus::LoginRedirecting {
        return "accounts.login.redirecting";
    }
    if let Some(operation) = snapshot.busy {
        return match operation {
            AccountOperation::Login => "accounts.login.starting",
            AccountOperation::Recheck => "accounts.session.checking",
            AccountOperation::Refresh => "accounts.session.refreshing",
            AccountOperation::Logout => "accounts.logout.pending",
        };
    }
    match snapshot.status {
        AccountStatus::Resolving => "accounts.session.checking",
        AccountStatus::SignedOut => "accounts.session.signed_out",
        AccountStatus::Authenticated { .. } => "accounts.session.confirmed",
        AccountStatus::Expired => "accounts.session.ended",
        AccountStatus::Unavailable => "accounts.service.unavailable",
        AccountStatus::Error | AccountStatus::Disconnected => "accounts.session.unconfirmed",
        AccountStatus::LogoutPending => "accounts.logout.unknown",
        AccountStatus::LogoutContextChanged => "accounts.logout.context_changed",
        AccountStatus::Cancelled => "accounts.operation.cancelled",
        AccountStatus::LoginRedirecting => "accounts.login.redirecting",
        AccountStatus::LoginUnavailable => "accounts.login.failed",
        AccountStatus::LogoutStorageUnavailable => "accounts.logout.storage_unavailable",
    }
}

fn failure_key(snapshot: &AccountSnapshot) -> Option<&'static str> {
    if snapshot.busy.is_some() {
        return None;
    }
    match snapshot.status {
        AccountStatus::LoginUnavailable => Some("accounts.login.retry"),
        AccountStatus::LogoutStorageUnavailable => Some("accounts.logout.storage_help"),
        AccountStatus::Error => Some("accounts.error.generic"),
        AccountStatus::Disconnected => Some("accounts.connection.disconnected"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        failure_key, login_allowed, logout_retry_allowed, profile_for, status_key,
        AccountOperation, AccountSnapshot, AccountStatus, UnavailableTask,
    };
    use crate::i18n::Messages;
    use tabula_registry::Locale;

    fn snapshot(status: AccountStatus, busy: Option<AccountOperation>) -> AccountSnapshot {
        AccountSnapshot {
            status,
            busy,
            presentation_generation: 7,
            login_available: false,
        }
    }

    #[test]
    fn login_control_requires_idle_signed_out_provider_capability() {
        for status in [AccountStatus::SignedOut, AccountStatus::Expired] {
            let mut state = snapshot(status, None);
            assert!(!login_allowed(&state));
            state.login_available = true;
            assert!(login_allowed(&state));
            for operation in [
                AccountOperation::Login,
                AccountOperation::Recheck,
                AccountOperation::Logout,
            ] {
                state.busy = Some(operation);
                assert!(!login_allowed(&state));
            }
        }
        for status in [
            AccountStatus::Resolving,
            AccountStatus::Authenticated {
                account_id: "private-id".into(),
            },
            AccountStatus::Unavailable,
            AccountStatus::Disconnected,
            AccountStatus::Error,
            AccountStatus::LogoutPending,
            AccountStatus::LogoutContextChanged,
            AccountStatus::LogoutStorageUnavailable,
            AccountStatus::Cancelled,
            AccountStatus::LoginUnavailable,
            AccountStatus::LoginRedirecting,
        ] {
            let mut state = snapshot(status, None);
            state.login_available = true;
            assert!(
                !login_allowed(&state),
                "even accidental capability retention cannot switch an account"
            );
        }
    }

    #[test]
    fn static_login_action_is_a_labelled_explicit_button_without_credential_collection() {
        use leptos::prelude::*;
        for locale in Locale::ALL {
            let owner = Owner::new();
            let html = owner.with(|| {
                provide_context(crate::views::LocaleSignal::new(locale));
                view! { <super::LoginAction on_start=|| {}/> }.to_html()
            });
            assert!(html.contains(&Messages::new(locale).text("accounts.action.login")));
            assert!(html.contains("type=\"button\""));
            for forbidden in [
                "<input",
                "<textarea",
                "type=\"password\"",
                "authorization_url",
                "return_url",
                "href=",
                "action=",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        }
    }

    #[test]
    fn private_field_is_only_selected_for_idle_current_self() {
        let ready = snapshot(
            AccountStatus::Authenticated {
                account_id: "account-17".into(),
            },
            None,
        );
        assert_eq!(profile_for(&ready), Some((7, "account-17".into())));
        for busy in [
            AccountOperation::Login,
            AccountOperation::Recheck,
            AccountOperation::Refresh,
            AccountOperation::Logout,
        ] {
            assert!(profile_for(&snapshot(ready.status.clone(), Some(busy))).is_none());
        }
        for status in [
            AccountStatus::Resolving,
            AccountStatus::SignedOut,
            AccountStatus::Expired,
            AccountStatus::Unavailable,
            AccountStatus::Error,
            AccountStatus::Disconnected,
            AccountStatus::LogoutPending,
            AccountStatus::LogoutContextChanged,
            AccountStatus::Cancelled,
            AccountStatus::LoginRedirecting,
            AccountStatus::LoginUnavailable,
            AccountStatus::LogoutStorageUnavailable,
        ] {
            assert!(profile_for(&snapshot(status, None)).is_none());
        }
    }

    #[test]
    fn static_self_component_is_read_only_marked_private_and_escapes_identifier() {
        use leptos::prelude::*;
        let owner = Owner::new();
        let html = owner.with(|| {
            provide_context(crate::views::LocaleSignal::new(Locale::En));
            view! { <super::ProfileFacts account_id="<script>private & id</script>".to_owned()/> }
                .to_html()
        });
        assert!(html.contains("data-account-private"));
        assert!(html.contains("<dl"));
        assert!(html.contains("Account ID"));
        assert!(html.contains("&lt;script&gt;private &amp; id&lt;/script&gt;"));
        for forbidden in [
            "<script>",
            "<input",
            "<textarea",
            "contenteditable",
            "aria-live",
            "role=\"status\"",
            "role=\"alert\"",
        ] {
            assert!(!html.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn private_node_key_changes_on_recheck_even_for_same_subject() {
        let first = snapshot(
            AccountStatus::Authenticated {
                account_id: "same-subject".into(),
            },
            None,
        );
        let mut restored = first.clone();
        restored.presentation_generation += 1;
        assert_ne!(profile_for(&first), profile_for(&restored));
    }

    #[test]
    fn changed_session_context_never_offers_the_previous_logout_retry() {
        assert!(logout_retry_allowed(&snapshot(
            AccountStatus::LogoutPending,
            None
        )));
        assert!(!logout_retry_allowed(&snapshot(
            AccountStatus::LogoutContextChanged,
            None
        )));
        assert!(!logout_retry_allowed(&snapshot(
            AccountStatus::LogoutPending,
            Some(AccountOperation::Logout)
        )));
        assert!(!logout_retry_allowed(&snapshot(
            AccountStatus::SignedOut,
            None
        )));
    }

    #[test]
    fn lifecycle_hidden_attribute_has_author_css_priority_over_profile_display() {
        let css = include_str!("../../style/app.scss");
        let private_mask = css
            .split_once("[data-account-private][hidden]")
            .expect("private lifecycle mask has an author CSS rule")
            .1
            .split_once('}')
            .expect("mask rule terminates")
            .0;
        assert!(private_mask.contains("display: none !important"));
    }

    #[test]
    fn dispositions_keep_their_distinct_public_meaning() {
        let cases = [
            (
                AccountStatus::SignedOut,
                "accounts.session.signed_out",
                None,
            ),
            (AccountStatus::Expired, "accounts.session.ended", None),
            (
                AccountStatus::Unavailable,
                "accounts.service.unavailable",
                None,
            ),
            (
                AccountStatus::Disconnected,
                "accounts.session.unconfirmed",
                Some("accounts.connection.disconnected"),
            ),
            (
                AccountStatus::Error,
                "accounts.session.unconfirmed",
                Some("accounts.error.generic"),
            ),
            (
                AccountStatus::LogoutPending,
                "accounts.logout.unknown",
                None,
            ),
            (
                AccountStatus::LogoutContextChanged,
                "accounts.logout.context_changed",
                None,
            ),
        ];
        for (status, expected_status, expected_failure) in cases {
            let state = snapshot(status, None);
            assert_eq!(status_key(&state), expected_status);
            assert_eq!(failure_key(&state), expected_failure);
        }
    }

    #[test]
    fn every_status_and_pending_operation_has_safe_localized_copy() {
        let statuses = [
            AccountStatus::Resolving,
            AccountStatus::SignedOut,
            AccountStatus::Authenticated {
                account_id: "private-id-must-not-appear".into(),
            },
            AccountStatus::Expired,
            AccountStatus::Unavailable,
            AccountStatus::Error,
            AccountStatus::Disconnected,
            AccountStatus::LogoutPending,
            AccountStatus::LogoutContextChanged,
            AccountStatus::Cancelled,
            AccountStatus::LoginRedirecting,
            AccountStatus::LoginUnavailable,
            AccountStatus::LogoutStorageUnavailable,
        ];
        for locale in Locale::ALL {
            let messages = Messages::new(locale);
            for status in statuses.clone() {
                let state = snapshot(status, None);
                for key in [Some(status_key(&state)), failure_key(&state)]
                    .into_iter()
                    .flatten()
                {
                    let text = messages.lookup(key).expect("state copy must be installed");
                    assert!(!text.is_empty());
                    assert!(!text.contains("private-id-must-not-appear"));
                }
            }
            for operation in [
                AccountOperation::Login,
                AccountOperation::Recheck,
                AccountOperation::Refresh,
                AccountOperation::Logout,
            ] {
                let state = snapshot(AccountStatus::Error, Some(operation));
                assert!(messages.lookup(status_key(&state)).is_some());
                assert_eq!(failure_key(&state), None);
            }
            for task in [
                UnavailableTask::Register,
                UnavailableTask::Friends,
                UnavailableTask::OtherProfile,
            ] {
                let (title, body) = task.keys();
                assert!(messages.lookup(title).is_some());
                assert!(messages.lookup(body).is_some());
            }
        }
    }
}
