//! Opt-in direct-match task hierarchy. Registry owns eligibility/configuration;
//! durable server admission owns match identity and seat (ADR-0041 / I-9).
mod core;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod lifecycle_tests;
#[cfg(any(target_arch = "wasm32", test))]
mod suppression;
use crate::{
    account::{AccountController, AccountStatus, DocumentAccountTicket},
    i18n::{shell, Messages},
    views::use_locale,
};
use core::{Action, EntryState, Phase};
use leptos::prelude::*;
use leptos_router::components::A;
use std::collections::BTreeMap;
use tabula_registry::{GameId, RuntimeBinding};

/// Only the explicitly built online panel can assert this deployment binding.
/// The ordinary local setup keeps `RuntimeBinding::bound` and its default gates.
pub(crate) fn runtime_binding() -> RuntimeBinding {
    option_env!("TABULA_PLAY_BASE").map_or(RuntimeBinding::unbound(), RuntimeBinding::direct_online)
}

#[derive(Clone, Copy)]
struct OnlineSession {
    saved: StoredValue<BTreeMap<String, SavedEntry>>,
    blocked: RwSignal<bool>,
    safety_ready: bool,
}
#[derive(Clone)]
struct SavedEntry {
    state: EntryState,
    subject: Option<String>,
}
/// Preserve public field input across shell routes. The only browser-persisted
/// value is an unresolved-POST boolean; it is not an identity, code or grant.
pub(crate) fn provide_online_session() {
    #[cfg(target_arch = "wasm32")]
    let (blocked, safety_ready) = {
        let restored = browser::restore_pending();
        (restored.unwrap_or(true), restored.is_ok())
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (blocked, safety_ready) = (false, true);
    provide_context(OnlineSession {
        saved: StoredValue::new(BTreeMap::new()),
        blocked: RwSignal::new(blocked),
        safety_ready,
    });
}

#[derive(Clone, Copy)]
struct Controller {
    runtime: StoredValue<Runtime, LocalStorage>,
    state: RwSignal<EntryState>,
    account: AccountController,
    session: OnlineSession,
}
struct Runtime {
    id: String,
    alive: bool,
    ticket: Option<DocumentAccountTicket>,
    admitted_subject: Option<String>,
    #[cfg(target_arch = "wasm32")]
    abort: Option<web_sys::AbortController>,
}
#[component]
pub fn OnlinePanel(id: String) -> AnyView {
    panel_with_binding(id, runtime_binding())
}

fn panel_with_binding(id: String, binding: RuntimeBinding) -> AnyView {
    let locale = use_locale();
    let (_, catalog) = shell(locale.get_untracked());
    let eligible = direct_eligible(&id, binding, &catalog);
    if !eligible {
        return view! { <DirectUnavailable/> }.into_any();
    }
    let controller = use_controller(id);
    let account = controller.account;
    view! {
        <div class="play-entry">
            <section class="online-panel" aria-labelledby="online-heading" aria-busy=move || controller.snapshot().is_some_and(|state| state.busy())>
                <h2 id="online-heading" class="section__subtitle">{move || Messages::new(locale.get()).text("online.heading")}</h2>
                <p class="section__body online-panel__scope">{move || Messages::new(locale.get()).text("online.scope")}</p>
                <button type="button" class="btn btn--filled btn--principal online-panel__create" data-testid="online-create" aria-describedby="online-status" disabled=move || !controller.can_submit(Action::Create) on:click=move |_| controller.submit(Action::Create)>
                    {move || Messages::new(locale.get()).text(if controller.snapshot().is_some_and(|state| state.phase == Phase::Pending(Action::Create)) { "online.creating" } else { "online.create" })}
                </button>
                <form class="online-panel__join field" data-state=move || if controller.snapshot().is_some_and(|state| state.phase == Phase::Error("online.invalid_code")) { "invalid" } else { "enabled" } on:submit=move |event| { event.prevent_default(); controller.submit(Action::Join); }>
                    <label for="online-join-code" class="field__label">{move || Messages::new(locale.get()).text("online.code.label")}</label>
                    <div class="online-panel__join-controls">
                        <input id="online-join-code" data-testid="online-join-code" class="field__control" type="text" maxlength="12" autocomplete="off" autocapitalize="characters" spellcheck="false" aria-describedby="online-status" aria-invalid=move || controller.snapshot().is_some_and(|state| state.phase == Phase::Error("online.invalid_code")) placeholder=move || Messages::new(locale.get()).text("online.code.placeholder") prop:value=move || controller.snapshot().map(|state| state.code).unwrap_or_default() readonly=move || controller.snapshot().is_none_or(|state| state.busy()) on:input=move |event| controller.edit_code(&event_target_value(&event))/>
                        <button type="submit" class="btn btn--tonal" data-testid="online-join" aria-describedby="online-status" disabled=move || !controller.can_submit(Action::Join)>
                            {move || Messages::new(locale.get()).text(if controller.snapshot().is_some_and(|state| state.phase == Phase::Pending(Action::Join)) { "online.joining" } else { "online.join" })}
                        </button>
                    </div>
                </form>
                <p id="online-status" class="status online-panel__status" tabindex="-1" role="status" aria-live="polite" aria-atomic="true" data-testid="online-status">{move || Messages::new(locale.get()).text(controller.status_key())}</p>
                {move || controller.snapshot().is_some_and(|state| state.cleanup_failed).then(|| view! {
                    <p class="field__error" role="status" data-testid="online-safety-cleanup-failed">{Messages::new(locale.get()).text("online.safety_cleanup_failed")}</p>
                })}
                {move || controller.visible_admission().map(|admission| {
                    let messages = Messages::new(locale.get());
                    let code = admission.join_code().filter(|code| !code.is_empty()).map(str::to_owned);
                    view! {
                        <div class="online-panel__admission">
                            {code.map(|code| view! { <p class="online-panel__share">{messages.text("online.code.share")}<strong class="online-panel__code" data-testid="online-code">{code}</strong></p> })}
                            <p class="meta" data-testid="online-seat">{format!("{} {}", messages.text("online.seat"), admission.seat() + 1)}</p>
                            <button type="button" class="btn btn--filled btn--principal" data-testid="online-enter" on:click=move |_| controller.open(locale.get_untracked())>{messages.text("online.enter")}</button>
                        </div>
                    }
                })}
                {move || if let Some(snapshot) = controller.account_snapshot().filter(|snapshot| matches!(snapshot.status, AccountStatus::SignedOut | AccountStatus::Expired)) {
                    view! {
                        <div class="online-panel__session-action">
                            <A href=if snapshot.login_available { "/login" } else { "/account" } attr:class="btn btn--tonal" attr:data-testid="online-signin">{Messages::new(locale.get()).text(if snapshot.login_available { "online.account" } else { "online.check_account" })}</A>
                            {snapshot.login_available.then(|| view! { <p class="meta">{Messages::new(locale.get()).text("online.signin_hint")}</p> })}
                        </div>
                    }.into_any()
                } else if controller.snapshot().is_some_and(|state| !state.busy()) && !controller.blocked() && controller.visible_admission().is_none() {
                    view! { <button type="button" class="btn btn--text" data-testid="online-recheck" on:click=move |_| account.recheck()>{Messages::new(locale.get()).text("online.recheck")}</button> }.into_any()
                } else { ().into_any() }}
            </section>
            <QuickMatchUnavailable/>
        </div>
    }.into_any()
}
fn direct_eligible(
    id: &str,
    binding: RuntimeBinding,
    catalog: &tabula_registry::DiscoveryCatalog,
) -> bool {
    GameId::new(id.to_owned())
        .ok()
        .and_then(|id| catalog.get(&id))
        .is_some_and(|entry| {
            let game = entry.game();
            binding.supports_discovery_direct(game)
                && game
                    .normalize_direct(
                        game.capabilities().seats().allowed().min(),
                        &tabula_registry::ConfigDraft::with_defaults(game.form()),
                    )
                    .is_ok()
        })
}
fn use_controller(id: String) -> Controller {
    let session = use_context::<OnlineSession>().expect("App provides direct-entry lifecycle");
    let saved = session
        .saved
        .with_value(|entries| entries.get(&id).cloned());
    let state = RwSignal::new(
        saved
            .as_ref()
            .map_or_else(EntryState::default, |entry| entry.state.clone()),
    );
    let account = crate::account::use_account();
    let controller = Controller {
        runtime: StoredValue::new_local(Runtime {
            id,
            alive: true,
            ticket: None,
            admitted_subject: saved.and_then(|entry| entry.subject),
            #[cfg(target_arch = "wasm32")]
            abort: None,
        }),
        state,
        account,
        session,
    };
    Effect::new(move |_| {
        if let Some(snapshot) = controller.account_snapshot() {
            controller.context_changed(&snapshot.status);
        }
    });
    on_cleanup(move || controller.dispose());
    controller
}
#[component]
fn DirectUnavailable() -> impl IntoView {
    let locale = use_locale();
    view! {
        <div class="play-entry">
            <section class="online-panel" aria-labelledby="online-heading">
                <h2 id="online-heading" class="section__subtitle">{move || Messages::new(locale.get()).text("online.heading")}</h2>
                <p class="section__body" data-testid="online-status">{move || Messages::new(locale.get()).text("online.incompatible")}</p>
                <p class="meta">{move || Messages::new(locale.get()).text("online.local_hint")}</p>
            </section>
            <QuickMatchUnavailable/>
        </div>
    }
}
#[component]
fn QuickMatchUnavailable() -> impl IntoView {
    let locale = use_locale();
    view! {
        <section class="online-quick" aria-labelledby="online-quick-heading" data-testid="online-quick-unavailable">
            <div class="online-quick__heading">
                <h2 id="online-quick-heading" class="section__subtitle">{move || Messages::new(locale.get()).text("online.quick.heading")}</h2>
                <span class="online-quick__badge">{move || Messages::new(locale.get()).text("online.quick.unavailable")}</span>
            </div>
            <p class="section__body">{move || Messages::new(locale.get()).text("online.quick.reason")}</p>
        </section>
    }
}
impl Controller {
    // Routes cleans up the old owner before Suspend replaces its retained DOM
    // on the next executor tick. Queued render reads must tolerate retirement.
    fn alive(self) -> bool {
        self.runtime
            .try_with_value(|runtime| runtime.alive)
            .unwrap_or(false)
    }
    fn snapshot(self) -> Option<EntryState> {
        self.alive().then(|| self.state.try_get()).flatten()
    }
    fn account_snapshot(self) -> Option<crate::account::AccountSnapshot> {
        self.alive().then(|| self.account.state.try_get()).flatten()
    }
    fn blocked(self) -> bool {
        self.session.blocked.try_get().unwrap_or(true)
    }
    fn can_submit(self, action: Action) -> bool {
        self.snapshot().is_some_and(|state| {
            self.account_snapshot().is_some()
                && self.session.safety_ready
                && state.can_submit(
                    action,
                    self.account.document_ticket().is_some(),
                    self.blocked(),
                )
        })
    }
    fn status_key(self) -> &'static str {
        let Some(state) = self.snapshot() else {
            return "online.context_changed";
        };
        let Some(account) = self.account_snapshot() else {
            return "online.session_unavailable";
        };
        if !self.session.safety_ready {
            return "online.safety_unavailable";
        }
        if self.blocked() && !state.busy() && !state.cleanup_failed {
            return "online.unknown";
        }
        if state.busy() {
            return state.phase.label_key();
        }
        match account.status {
            AccountStatus::SignedOut | AccountStatus::Expired => {
                if account.login_available {
                    "online.signin"
                } else {
                    "online.signin_unavailable"
                }
            }
            AccountStatus::Resolving => "online.session_pending",
            AccountStatus::Authenticated { .. } => {
                state.result_key(self.blocked(), state.cleanup_failed)
            }
            AccountStatus::Disconnected => "online.disconnected",
            _ => "online.session_unavailable",
        }
    }
    fn visible_admission(self) -> Option<tabula_match_http::MatchAdmission> {
        let state = self.snapshot()?;
        self.account_snapshot()?;
        let current = self.account.document_ticket()?;
        let matches = self
            .runtime
            .try_with_value(|runtime| {
                runtime.admitted_subject.as_deref() == Some(current.subject())
            })
            .unwrap_or(false);
        matches
            .then_some(state.admission)
            .flatten()
            .filter(|admission| {
                let Some(id) = self.runtime.try_with_value(|runtime| runtime.id.clone()) else {
                    return false;
                };
                admission_handoff(
                    &id,
                    admission,
                    runtime_binding(),
                    tabula_registry::Locale::En,
                )
                .is_some()
            })
    }
    fn context_changed(self, status: &AccountStatus) {
        let current = self.account.document_ticket();
        let stale = self
            .runtime
            .try_with_value(|runtime| {
                runtime
                    .ticket
                    .as_ref()
                    .is_some_and(|ticket| !self.account.ticket_current(ticket))
            })
            .unwrap_or(false);
        if stale {
            self.retire();
        }
        let incompatible_subject = self
            .runtime
            .try_with_value(|runtime| {
                runtime.admitted_subject.as_ref().is_some_and(|subject| {
                    current
                        .as_ref()
                        .is_some_and(|ticket| ticket.subject() != subject)
                })
            })
            .unwrap_or(false);
        if incompatible_subject
            || matches!(status, AccountStatus::SignedOut | AccountStatus::Expired)
        {
            self.state.try_update(EntryState::clear_admission);
            self.runtime
                .try_update_value(|runtime| runtime.admitted_subject = None);
        }
    }
    fn retire(self) {
        self.state.try_update(|state| {
            if state.retire() {
                self.session.blocked.try_set(true);
            }
        });
        self.runtime.try_update_value(|runtime| {
            runtime.ticket = None;
            #[cfg(target_arch = "wasm32")]
            if let Some(abort) = runtime.abort.take() {
                abort.abort();
            }
        });
    }
    fn dispose(self) {
        self.retire();
        if let Some(state) = self.state.try_get_untracked() {
            self.runtime.try_update_value(|runtime| {
                runtime.alive = false;
                self.session.saved.try_update_value(|saved| {
                    saved.insert(
                        runtime.id.clone(),
                        SavedEntry {
                            state,
                            subject: runtime.admitted_subject.clone(),
                        },
                    );
                });
            });
        }
    }
    fn remember(self) {
        let Some(state) = self.state.try_get_untracked().filter(|state| !state.busy()) else {
            return;
        };
        self.runtime.try_with_value(|runtime| {
            self.session.saved.try_update_value(|saved| {
                saved.insert(
                    runtime.id.clone(),
                    SavedEntry {
                        state,
                        subject: runtime.admitted_subject.clone(),
                    },
                );
            });
        });
    }
    fn edit_code(self, raw: &str) {
        if !self.alive() {
            return;
        }
        self.state.try_update(|state| state.edit_code(raw));
        // A routed replacement can mount before old owner cleanup; cache safe
        // input synchronously rather than relying on a later reactive effect.
        self.remember();
    }
    fn submit(self, action: Action) {
        let Some(ticket) = self.account.document_ticket() else {
            return;
        };
        let mut revision = None;
        self.state.update(|state| {
            revision = state.begin(
                action,
                self.session.safety_ready,
                self.session.blocked.get_untracked(),
            );
        });
        let Some(revision) = revision else {
            return;
        };
        self.runtime
            .update_value(|runtime| runtime.ticket = Some(ticket.clone()));
        dispatch(self, revision, action, ticket);
    }
    fn open(self, locale: tabula_registry::Locale) {
        let Some(admission) = self.visible_admission() else {
            return;
        };
        let Some(id) = self.runtime.try_with_value(|runtime| runtime.id.clone()) else {
            return;
        };
        let target = admission_handoff(&id, &admission, runtime_binding(), locale);
        if target.is_some_and(|url| navigate(&url)) {
            return;
        }
        self.state.update(EntryState::handoff_failed);
        self.remember();
        focus("online-status");
    }
}
fn admission_handoff(
    expected: &str,
    admission: &tabula_match_http::MatchAdmission,
    binding: RuntimeBinding,
    locale: tabula_registry::Locale,
) -> Option<String> {
    if admission.game_id() != expected {
        return None;
    }
    let (_, catalog) = shell(locale);
    let id = GameId::new(expected.to_owned()).ok()?;
    let entry = catalog.get(&id)?;
    let game = entry.game();
    if admission.game_version() != game.metadata().version().as_str()
        || admission.seat() >= game.capabilities().seats().allowed().min()
        || game
            .normalize_direct(
                game.capabilities().seats().allowed().min(),
                &tabula_registry::ConfigDraft::with_defaults(game.form()),
            )
            .is_err()
    {
        return None;
    }
    tabula_registry::launch::resolve_discovery_direct(binding, game, admission.match_id(), locale)
        .ok()
        .map(|handoff| handoff.url)
}
#[cfg(target_arch = "wasm32")]
fn navigate(url: &str) -> bool {
    window().location().assign(url).is_ok()
}
#[cfg(not(target_arch = "wasm32"))]
fn navigate(_url: &str) -> bool {
    false
}
#[cfg(target_arch = "wasm32")]
fn focus(id: &str) {
    use wasm_bindgen::JsCast;
    if let Some(element) = window()
        .document()
        .and_then(|document| document.get_element_by_id(id))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = element.focus();
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn focus(_id: &str) {}
#[cfg(not(target_arch = "wasm32"))]
fn dispatch(
    controller: Controller,
    revision: u64,
    _action: Action,
    _ticket: DocumentAccountTicket,
) {
    controller.state.update(|state| {
        state.complete(revision, Err("online.unavailable"));
    });
}
#[cfg(target_arch = "wasm32")]
fn dispatch(controller: Controller, revision: u64, action: Action, ticket: DocumentAccountTicket) {
    let Ok(abort) = web_sys::AbortController::new() else {
        controller.state.update(|state| {
            state.complete(revision, Err("online.unavailable"));
        });
        return;
    };
    controller
        .runtime
        .update_value(|runtime| runtime.abort = Some(abort.clone()));
    let id = controller.runtime.with_value(|runtime| runtime.id.clone());
    let code = controller.state.get_untracked().code;
    leptos::task::spawn_local(async move {
        if !controller
            .runtime
            .try_with_value(|runtime| runtime.alive)
            .unwrap_or(false)
        {
            return;
        }
        let dispatch_ticket = ticket.clone();
        let result = browser::run(action, &id, code, &ticket, &abort, move || {
            if !controller.account.ticket_current(&dispatch_ticket)
                || !controller
                    .state
                    .try_with(|state| state.current(revision))
                    .unwrap_or(false)
            {
                return false;
            }
            if browser::save_pending(true).is_err() {
                return false;
            }
            controller.session.blocked.set(true);
            let mut dispatched = false;
            controller
                .state
                .update(|state| dispatched = state.dispatched(revision));
            dispatched
        })
        .await;
        complete(controller, revision, &id, result);
    });
}
#[cfg(target_arch = "wasm32")]
fn complete(
    controller: Controller,
    revision: u64,
    id: &str,
    result: Result<tabula_match_http::MatchAdmission, &'static str>,
) {
    if !controller
        .runtime
        .try_with_value(|runtime| runtime.alive)
        .unwrap_or(false)
    {
        return;
    }
    if !controller.account.document_ticket().is_some_and(|current| {
        controller
            .runtime
            .with_value(|runtime| runtime.ticket.as_ref() == Some(&current))
    }) {
        controller.retire();
        return;
    }
    if !controller
        .state
        .try_with(|state| state.current(revision))
        .unwrap_or(false)
    {
        return;
    }
    controller
        .runtime
        .update_value(|runtime| runtime.abort = None);
    let confirmed = !matches!(result, Err("online.unknown"));
    let sent = controller
        .state
        .with_untracked(|state| state.was_dispatched(revision));
    if confirmed && sent {
        if browser::save_pending(false).is_ok() {
            controller.session.blocked.set(false);
            controller
                .state
                .update(|state| state.cleanup_failed = false);
        } else {
            controller.state.update(|state| state.cleanup_failed = true);
        }
    }
    if result.is_ok() {
        controller.runtime.update_value(|runtime| {
            runtime.admitted_subject = runtime
                .ticket
                .as_ref()
                .map(|ticket| ticket.subject().to_owned());
        });
    }
    if result.as_ref().is_ok_and(|admission| {
        admission_handoff(
            id,
            admission,
            runtime_binding(),
            tabula_registry::Locale::En,
        )
        .is_none()
    }) {
        // A known admission cannot launch a different package. Retain it to
        // suppress fresh admissions, but never render its code/seat here.
        controller.state.update(|state| {
            state.complete(revision, result);
            state.phase = Phase::Error("online.incompatible");
        });
        controller.remember();
        focus("online-status");
        return;
    }
    let denied = matches!(result, Err("online.invalid_code"));
    let changed = matches!(result, Err("online.signin" | "online.context_changed"));
    controller.state.update(|state| {
        state.complete(revision, result);
    });
    controller.remember();
    if changed {
        controller.account.recheck();
    }
    focus(if denied {
        "online-join-code"
    } else {
        "online-status"
    });
}
// A denial is public-safe and does not distinguish invalid/expired/full/private.
// Ambiguous transport/5xx/invalid-success results after POST are not a retry license.
#[cfg(any(target_arch = "wasm32", test))]
fn response_error_key(status: u16, joining: bool) -> Option<&'static str> {
    match status {
        200 | 201 => None,
        400 | 403 | 404 | 409 if joining => Some("online.invalid_code"),
        401 | 403 => Some("online.signin"),
        400 | 404 | 409 | 413 | 429 => Some("online.unavailable"),
        _ => Some("online.unknown"),
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{shell, Action, DocumentAccountTicket, GameId};
    use js_sys::Uint8Array;
    use leptos::prelude::window;
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        AbortController, ReadableStreamDefaultReader, Request, RequestCache, RequestCredentials,
        RequestInit, RequestMode, RequestRedirect, Response,
    };

    // Non-authorizing suppression only. Do not serialize an operation, subject,
    // code, routing identity, CSRF control, admission or attachment here.
    fn storage() -> Result<web_sys::Storage, ()> {
        window().session_storage().map_err(|_| ())?.ok_or(())
    }
    struct BrowserStore(web_sys::Storage);
    impl super::suppression::Store for BrowserStore {
        fn get(&self, key: &str) -> Result<Option<String>, ()> {
            self.0.get_item(key).map_err(|_| ())
        }
        fn set(&self, key: &str, value: &str) -> Result<(), ()> {
            self.0.set_item(key, value).map_err(|_| ())
        }
        fn remove(&self, key: &str) -> Result<(), ()> {
            self.0.remove_item(key).map_err(|_| ())
        }
    }
    pub(super) fn restore_pending() -> Result<bool, ()> {
        super::suppression::restore(&BrowserStore(storage()?))
    }
    pub(super) fn save_pending(pending: bool) -> Result<(), ()> {
        super::suppression::save(&BrowserStore(storage()?), pending)
    }
    enum Failure {
        BeforeSend(&'static str),
        Denied(&'static str),
        Failed(&'static str),
    }
    impl Failure {
        fn before_admission(self) -> &'static str {
            match self {
                Self::BeforeSend(key) | Self::Denied(key) | Self::Failed(key) => key,
            }
        }
        fn after_admission(self) -> &'static str {
            match self {
                Self::BeforeSend(key) | Self::Denied(key) => key,
                Self::Failed(_) => "online.unknown",
            }
        }
    }
    pub(super) async fn run(
        action: Action,
        id: &str,
        code: String,
        ticket: &DocumentAccountTicket,
        abort: &AbortController,
        before_post: impl FnOnce() -> bool,
    ) -> Result<tabula_match_http::MatchAdmission, &'static str> {
        let context = fetch("/api/v1/auth/context", None, None, abort, || true)
            .await
            .map_err(Failure::before_admission)?;
        let context: tabula_session_http::ContextResponse =
            crate::json::decode(&context).map_err(|_| "online.session_unavailable")?;
        context
            .validate_for_browser()
            .map_err(|_| "online.session_unavailable")?;
        match context.disposition {
            tabula_session_http::SessionDisposition::SignedOut => return Err("online.signin"),
            tabula_session_http::SessionDisposition::Unavailable => {
                return Err("online.session_unavailable")
            }
            tabula_session_http::SessionDisposition::Authenticated => {}
        }
        if context.account_id.as_deref() != Some(ticket.subject())
            || context.csrf_token.as_deref() != Some(ticket.csrf_token())
        {
            return Err("online.context_changed");
        }
        let (path, body) = match action {
            Action::Create => {
                let (_, catalog) = shell(tabula_registry::Locale::En);
                let id = GameId::new(id.to_owned()).map_err(|_| "online.incompatible")?;
                let game = catalog.get(&id).ok_or("online.incompatible")?.game();
                if !super::runtime_binding().supports_discovery_direct(game) {
                    return Err("online.incompatible");
                }
                let seats = game.capabilities().seats().allowed().min();
                let draft = tabula_registry::ConfigDraft::with_defaults(game.form());
                game.normalize_direct(seats, &draft)
                    .map_err(|_| "online.incompatible")?;
                let values = game
                    .form()
                    .visible_fields(&draft)
                    .iter()
                    .filter_map(|field| {
                        draft
                            .get(field.key)
                            .map(|value| (field.key.to_owned(), value.to_owned()))
                    })
                    .collect();
                let request = tabula_match_http::MatchCreateRequest::new(
                    id.as_str().to_owned(),
                    seats,
                    values,
                )
                .map_err(|_| "online.incompatible")?;
                (
                    "/api/v1/matches",
                    serde_json::to_string(&request).map_err(|_| "online.unavailable")?,
                )
            }
            Action::Join => {
                let request = tabula_match_http::MatchJoinRequest::new(code)
                    .map_err(|_| "online.invalid_code")?;
                (
                    "/api/v1/matches/join",
                    serde_json::to_string(&request).map_err(|_| "online.unavailable")?,
                )
            }
        };
        let body = fetch(
            path,
            Some(&body),
            Some(ticket.csrf_token()),
            abort,
            before_post,
        )
        .await
        .map_err(Failure::after_admission)?;
        crate::json::decode(&body).map_err(|_| "online.unknown")
    }
    struct Timeout {
        handle: i32,
        callback: Closure<dyn FnMut()>,
    }
    impl Drop for Timeout {
        fn drop(&mut self) {
            window().clear_timeout_with_handle(self.handle);
            let _ = &self.callback;
        }
    }
    async fn fetch(
        path: &str,
        body: Option<&str>,
        csrf: Option<&str>,
        abort: &AbortController,
        before_send: impl FnOnce() -> bool,
    ) -> Result<Vec<u8>, Failure> {
        let window = window();
        if window.location().protocol().ok().as_deref() != Some("https:") {
            return Err(Failure::BeforeSend("online.unavailable"));
        }
        let cancel = abort.clone();
        let callback = Closure::<dyn FnMut()>::new(move || cancel.abort());
        let handle = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                20_000,
            )
            .map_err(|_| Failure::BeforeSend("online.unavailable"))?;
        let _timeout = Timeout { handle, callback };
        let init = RequestInit::new();
        init.set_method(if body.is_some() { "POST" } else { "GET" });
        init.set_mode(RequestMode::SameOrigin);
        init.set_cache(RequestCache::NoStore);
        init.set_credentials(RequestCredentials::SameOrigin);
        init.set_redirect(RequestRedirect::Error);
        init.set_signal(Some(&abort.signal()));
        if let Some(body) = body {
            init.set_body(&JsValue::from_str(body));
        }
        let request = Request::new_with_str_and_init(path, &init)
            .map_err(|_| Failure::BeforeSend("online.unavailable"))?;
        request
            .headers()
            .set("Accept", "application/json")
            .map_err(|_| Failure::BeforeSend("online.unavailable"))?;
        if let Some(csrf) = csrf {
            request
                .headers()
                .set("Content-Type", "application/json")
                .map_err(|_| Failure::BeforeSend("online.unavailable"))?;
            request
                .headers()
                .set("X-Tabula-CSRF", csrf)
                .map_err(|_| Failure::BeforeSend("online.unavailable"))?;
        }
        if abort.signal().aborted() {
            return Err(Failure::BeforeSend("online.context_changed"));
        }
        if !before_send() {
            return Err(Failure::BeforeSend("online.safety_unavailable"));
        }
        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| Failure::Failed("online.disconnected"))?
            .dyn_into::<Response>()
            .map_err(|_| Failure::Failed("online.unavailable"))?;
        if let Some(key) =
            super::response_error_key(response.status(), path == "/api/v1/matches/join")
        {
            return Err(if key == "online.unknown" {
                Failure::Failed("online.unavailable")
            } else {
                Failure::Denied(key)
            });
        }
        if response.redirected()
            || !response
                .headers()
                .get("Cache-Control")
                .ok()
                .flatten()
                .is_some_and(|v| {
                    v.split(',')
                        .any(|v| v.trim().eq_ignore_ascii_case("no-store"))
                })
            || response
                .headers()
                .get("Content-Type")
                .ok()
                .flatten()
                .is_none_or(|v| v.split(';').next().map(str::trim) != Some("application/json"))
        {
            return Err(Failure::Failed("online.unavailable"));
        }
        read_body(&response, abort).await.map_err(Failure::Failed)
    }
    async fn read_body(
        response: &Response,
        abort: &AbortController,
    ) -> Result<Vec<u8>, &'static str> {
        if response
            .headers()
            .get("Content-Length")
            .ok()
            .flatten()
            .is_some_and(|value| {
                value
                    .parse::<usize>()
                    .map_or(true, |size| size > tabula_match_http::MAX_RESPONSE_BYTES)
            })
        {
            abort.abort();
            return Err("online.unavailable");
        }
        let body = response.body().ok_or("online.disconnected")?;
        let reader = ReadableStreamDefaultReader::new(&body).map_err(|_| "online.disconnected")?;
        let mut bytes = Vec::new();
        let result = async {
            loop {
                let chunk = JsFuture::from(reader.read())
                    .await
                    .map_err(|_| "online.disconnected")?;
                let done = js_sys::Reflect::get(&chunk, &JsValue::from_str("done"))
                    .map_err(|_| "online.unavailable")?
                    .as_bool();
                if done == Some(true) {
                    return Ok(bytes);
                }
                if done != Some(false) {
                    return Err("online.unavailable");
                }
                let value = js_sys::Reflect::get(&chunk, &JsValue::from_str("value"))
                    .map_err(|_| "online.unavailable")?
                    .dyn_into::<Uint8Array>()
                    .map_err(|_| "online.unavailable")?;
                if value.length() as usize
                    > tabula_match_http::MAX_RESPONSE_BYTES.saturating_sub(bytes.len())
                {
                    abort.abort();
                    return Err("online.unavailable");
                }
                bytes.extend(value.to_vec());
            }
        }
        .await;
        reader.release_lock();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn join_denial_is_public_safe_and_only_real_auth_failure_requests_signin() {
        for status in [400, 403, 404, 409] {
            assert_eq!(
                response_error_key(status, true),
                Some("online.invalid_code")
            );
        }
        assert_eq!(response_error_key(401, true), Some("online.signin"));
        assert_eq!(response_error_key(403, false), Some("online.signin"));
        for status in [500, 502, 503, 504, 204, 302] {
            assert_eq!(response_error_key(status, false), Some("online.unknown"));
        }
        assert_eq!(response_error_key(200, true), None);
        assert_eq!(response_error_key(201, false), None);
    }
    #[test]
    fn unavailable_entry_is_explicit_and_quick_match_is_passive_in_both_locales() {
        for locale in tabula_registry::Locale::ALL {
            let owner = Owner::new();
            let html = owner.with(|| {
                provide_context(RwSignal::new(locale));
                view! { <DirectUnavailable/> }.to_html()
            });
            assert!(html.contains(&Messages::new(locale).text("online.incompatible")));
            assert!(html.contains(&Messages::new(locale).text("online.quick.unavailable")));
            assert!(html.contains("data-testid=\"online-quick-unavailable\""));
            assert!(!html.contains("<button"));
            assert!(!html.contains("<input"));
            assert!(!html.contains("<form"));
            assert!(!html.contains("href="));
        }
    }
    #[test]
    fn handoff_rejects_wrong_package_version_seat_and_missing_document() {
        let (_, catalog) = shell(tabula_registry::Locale::En);
        let binding = RuntimeBinding::direct_online("/play");
        let mut supported = 0;
        for entry in catalog.entries() {
            let game = entry.game();
            if !binding.supports_discovery_direct(game) {
                continue;
            }
            supported += 1;
            let id = game.metadata().id().as_str();
            let admission = |game_id: &str, version: &str, seat| {
                tabula_match_http::MatchAdmission::new(
                    "00000000000000000000000000000001".into(),
                    game_id.into(),
                    version.into(),
                    seat,
                    None,
                    true,
                )
                .unwrap()
            };
            let valid = admission(id, game.metadata().version().as_str(), 0);
            assert!(admission_handoff(id, &valid, binding, tabula_registry::Locale::En).is_some());
            assert!(admission_handoff(
                id,
                &valid,
                RuntimeBinding::unbound(),
                tabula_registry::Locale::En
            )
            .is_none());
            for invalid in [
                admission("test.other", game.metadata().version().as_str(), 0),
                admission(id, "0.0.0", 0),
                admission(id, game.metadata().version().as_str(), 7),
            ] {
                assert!(
                    admission_handoff(id, &invalid, binding, tabula_registry::Locale::En).is_none()
                );
            }
        }
        assert!(supported > 0, "eligible registry packages were exercised");
    }
}
