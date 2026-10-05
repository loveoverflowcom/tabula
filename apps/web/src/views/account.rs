//! Compact account state and immutable self-profile under ADR-0036.
//!
//! The isolated adapter owns authority and cleanup. This view only renders its
//! structured dispositions; unavailable provider/social routes collect nothing
//! (docs/ui/screens/account-state-isolated.md). Fixed escapes deliberately ignore
//! return parameters rather than interpreting a URL as authorization.

use leptos::{html, prelude::*};
use leptos_router::components::A;

use crate::{
    account::{use_account, AccountOperation, AccountSnapshot, AccountStatus},
    i18n::Messages,
    views::use_locale,
};

/// Current account state at `/account`, without an identity-provider form.
#[component]
pub fn AccountPage() -> impl IntoView {
    view! { <AccountTask title_key="accounts.title"/> }
}

/// The permitted read-only self task at `/me`; never an other-profile lookup.
#[component]
pub fn SelfProfile() -> impl IntoView {
    view! { <AccountTask title_key="accounts.profile.self"/> }
}

#[component]
fn AccountTask(title_key: &'static str) -> impl IntoView {
    let locale = use_locale();
    let controller = use_account();
    let state = controller.state;
    let confirming = RwSignal::new(false);
    let heading = NodeRef::<html::H1>::new();
    let logout_invoker = NodeRef::<html::Button>::new();
    heading.on_load(|element| {
        let _ = element.focus();
    });

    // A permission/lifecycle transition dismisses a merely local confirmation.
    // It cannot execute a logout or preserve a former viewer's private data.
    Effect::new(move |_| {
        if profile_for(&state.get()).is_none() {
            confirming.set(false);
        }
    });

    view! {
        <section class="section account" aria-labelledby="account-title">
            <h1 id="account-title" class="section__title" tabindex="-1" node_ref=heading>
                {move || Messages::new(locale.get()).text(title_key)}
            </h1>
            <p class="section__body">
                {move || Messages::new(locale.get()).text("accounts.scope")}
            </p>
            <div class="account__state" aria-busy=move || state.get().busy.is_some()>
                <p class="status" role="status" aria-atomic="true">
                    {move || Messages::new(locale.get()).text(status_key(&state.get()))}
                </p>
                <Show when=move || failure_key(&state.get()).is_some()>
                    <p class="banner banner--error" role="alert">
                        {move || {
                            failure_key(&state.get())
                                .map(|key| Messages::new(locale.get()).text(key))
                        }}
                    </p>
                </Show>
            </div>

            // A fresh generation key forces new private nodes, including after a
            // restored document was synchronously masked by the adapter.
            <For
                each={move || profile_for(&state.get()).into_iter().collect::<Vec<_>>()}
                key={|(generation, account_id)| (*generation, account_id.clone())}
                children={move |(_, account_id)| view! { <ProfileFacts account_id/> }}
            />

            <div class="actions account__actions">
                <button
                    type="button"
                    class="btn btn--filled btn--principal"
                    disabled=move || state.get().busy.is_some()
                    on:click=move |_| {
                        confirming.set(false);
                        if let Some(element) = heading.get() {
                            let _ = element.focus();
                        }
                        controller.recheck();
                    }
                >
                    {move || Messages::new(locale.get()).text("accounts.action.check")}
                </button>
                <Show when=move || profile_for(&state.get()).is_some()>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        on:click=move |_| {
                            confirming.set(false);
                            if let Some(element) = heading.get() {
                                let _ = element.focus();
                            }
                            controller.refresh();
                        }
                    >
                        {move || Messages::new(locale.get()).text("accounts.action.refresh")}
                    </button>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        node_ref=logout_invoker
                        aria-expanded=move || confirming.get()
                        aria-controls=move || confirming.get().then_some("logout-confirmation")
                        on:click=move |_| confirming.set(true)
                    >
                        {move || Messages::new(locale.get()).text("accounts.action.logout")}
                    </button>
                </Show>
                <Show when=move || {
                    logout_retry_allowed(&state.get())
                }>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        on:click=move |_| {
                            if let Some(element) = heading.get() {
                                let _ = element.focus();
                            }
                            controller.logout();
                        }
                    >
                        {move || Messages::new(locale.get()).text("accounts.action.logout_retry")}
                    </button>
                </Show>
                <Show when=move || state.get().busy.is_some()>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        on:click=move |_| {
                            controller.cancel();
                            if let Some(element) = heading.get() {
                                let _ = element.focus();
                            }
                        }
                    >
                        {move || Messages::new(locale.get()).text("accounts.action.cancel")}
                    </button>
                </Show>
            </div>

            <Show when=move || confirming.get() && profile_for(&state.get()).is_some()>
                <LogoutConfirmation
                    on_cancel=move || {
                        confirming.set(false);
                        if let Some(element) = logout_invoker.get() {
                            let _ = element.focus();
                        }
                    }
                    on_confirm=move || {
                        confirming.set(false);
                        if let Some(element) = heading.get() {
                            let _ = element.focus();
                        }
                        controller.logout();
                    }
                />
            </Show>

            <p class="section__body">
                {move || Messages::new(locale.get()).text("accounts.local.explanation")}
            </p>
            <div class="actions account__actions">
                <A
                    href="/games"
                    attr:class="btn btn--tonal"
                    on:click=move |_| controller.cancel()
                >
                    {move || Messages::new(locale.get()).text("accounts.action.library")}
                </A>
                <A href="/" attr:class="btn btn--tonal" on:click=move |_| controller.cancel()>
                    {move || Messages::new(locale.get()).text("nav.home")}
                </A>
            </div>
            <UnavailableLinks/>
        </section>
    }
}

/// Only the immutable ID is returned by PR2. No name, handle, avatar, statistics,
/// history or edit fields are synthesized from that identity.
#[component]
fn ProfileFacts(account_id: String) -> impl IntoView {
    let locale = use_locale();
    view! {
        <section class="account__profile" data-account-private="" aria-labelledby="self-facts-title">
            <h2 id="self-facts-title" class="section__subtitle">
                {move || Messages::new(locale.get()).text("accounts.profile.read_only")}
            </h2>
            <dl class="facts account__facts">
                <dt>{move || Messages::new(locale.get()).text("accounts.profile.id")}</dt>
                <dd class="account__id">{account_id}</dd>
            </dl>
            <p class="section__body">
                {move || Messages::new(locale.get()).text("accounts.profile.only_id")}
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
    let locale = use_locale();
    let cancel = NodeRef::<html::Button>::new();
    cancel.on_load(|element| {
        let _ = element.focus();
    });
    view! {
        <fieldset
            id="logout-confirmation"
            class="account__confirmation"
            aria-describedby="logout-consequence"
            on:keydown=move |event| {
                if event.key() == "Escape" {
                    event.prevent_default();
                    on_cancel();
                }
            }
        >
            <legend class="field__label">
                {move || Messages::new(locale.get()).text("accounts.logout.title")}
            </legend>
            <p id="logout-consequence" class="section__body">
                {move || Messages::new(locale.get()).text("accounts.logout.explanation")}
            </p>
            <div class="actions account__actions">
                <button type="button" class="btn btn--tonal" node_ref=cancel on:click=move |_| on_cancel()>
                    {move || Messages::new(locale.get()).text("accounts.action.cancel")}
                </button>
                <button type="button" class="btn btn--danger" on:click=move |_| on_confirm()>
                    {move || Messages::new(locale.get()).text("accounts.action.logout")}
                </button>
            </div>
        </fieldset>
    }
}

/// A visibly unavailable route is still a useful explanatory destination.
#[derive(Clone, Copy)]
enum UnavailableTask {
    Login,
    Register,
    Friends,
    OtherProfile,
}

impl UnavailableTask {
    const fn keys(self) -> (&'static str, &'static str) {
        match self {
            Self::Login => ("accounts.login.title", "accounts.login.unavailable"),
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
pub fn LoginUnavailable() -> impl IntoView {
    view! { <UnavailablePage task=UnavailableTask::Login/> }
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
    let locale = use_locale();
    let (title_key, body_key) = task.keys();
    let heading = NodeRef::<html::H1>::new();
    heading.on_load(|element| {
        let _ = element.focus();
    });
    view! {
        <section class="section account">
            <h1 class="section__title" tabindex="-1" node_ref=heading>
                {move || Messages::new(locale.get()).text(title_key)}
            </h1>
            <p class="banner">{move || Messages::new(locale.get()).text(body_key)}</p>
            <p class="section__body">
                {move || Messages::new(locale.get()).text("accounts.local.explanation")}
            </p>
            <div class="actions account__actions">
                <A href="/games" attr:class="btn btn--filled btn--principal">
                    {move || Messages::new(locale.get()).text("accounts.action.library")}
                </A>
                <A href="/account" attr:class="btn btn--tonal">
                    {move || Messages::new(locale.get()).text("accounts.action.back_account")}
                </A>
                <A href="/" attr:class="btn btn--tonal">
                    {move || Messages::new(locale.get()).text("nav.home")}
                </A>
            </div>
        </section>
    }
}

#[component]
fn UnavailableLinks() -> impl IntoView {
    let locale = use_locale();
    view! {
        <nav aria-label=move || Messages::new(locale.get()).text("accounts.features.title")>
            <ul class="rows">
                <li class="row">
                    <A href="/login" attr:class="account__feature-link">
                        {move || Messages::new(locale.get()).text("accounts.login.link_unavailable")}
                    </A>
                </li>
                <li class="row">
                    <A href="/register" attr:class="account__feature-link">
                        {move || Messages::new(locale.get()).text("accounts.register.link_unavailable")}
                    </A>
                </li>
                <li class="row">
                    <A href="/friends" attr:class="account__feature-link">
                        {move || Messages::new(locale.get()).text("accounts.friends.link_unavailable")}
                    </A>
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

fn logout_retry_allowed(snapshot: &AccountSnapshot) -> bool {
    snapshot.busy.is_none() && matches!(snapshot.status, AccountStatus::LogoutPending)
}

fn status_key(snapshot: &AccountSnapshot) -> &'static str {
    if let Some(operation) = snapshot.busy {
        return match operation {
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
    }
}

fn failure_key(snapshot: &AccountSnapshot) -> Option<&'static str> {
    if snapshot.busy.is_some() {
        return None;
    }
    match snapshot.status {
        AccountStatus::Error => Some("accounts.error.generic"),
        AccountStatus::Disconnected => Some("accounts.connection.disconnected"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        failure_key, logout_retry_allowed, profile_for, status_key, AccountOperation,
        AccountSnapshot, AccountStatus, UnavailableTask,
    };
    use crate::i18n::Messages;
    use tabula_registry::Locale;

    fn snapshot(status: AccountStatus, busy: Option<AccountOperation>) -> AccountSnapshot {
        AccountSnapshot {
            status,
            busy,
            presentation_generation: 7,
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
                AccountOperation::Recheck,
                AccountOperation::Refresh,
                AccountOperation::Logout,
            ] {
                let state = snapshot(AccountStatus::Error, Some(operation));
                assert!(messages.lookup(status_key(&state)).is_some());
                assert_eq!(failure_key(&state), None);
            }
            for task in [
                UnavailableTask::Login,
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
