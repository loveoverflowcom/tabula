//! Document-memory browser account controller (ADR-0031 §6 / ADR-0036 PR3).
//!
//! This binary consumes the default DTO contract, never server runtime authority.
//! No credential or private account data is persisted. Only a bounded,
//! non-authorizing logout-suppression fingerprint crosses document reloads.

pub mod core;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
// Browser storage; pure protocol tested natively.
mod suppression;
pub use core::{AccountOperation, AccountSnapshot, AccountStatus};

use leptos::prelude::*;

#[derive(Clone, Copy)]
struct AccountSession {
    core: StoredValue<core::AccountCore>,
    active_route: StoredValue<Option<RouteLease>>,
}

/// Arena identity is unique even when route owners overlap during navigation.
/// Comparing a retired lease never reads its disposed value.
#[derive(Clone, Copy, PartialEq, Eq)]
struct RouteLease(StoredValue<()>);

impl AccountSession {
    fn new() -> Self {
        #[allow(unused_mut)] // WASM restores the non-authorizing saved intent.
        let mut core = core::AccountCore::default();
        #[cfg(target_arch = "wasm32")]
        browser::restore_suppression(&mut core);
        Self {
            core: StoredValue::new(core),
            active_route: StoredValue::new(None),
        }
    }
}

/// Provide once at the application owner. Only non-secret logout suppression
/// survives route owners and document reloads; identity/CSRF clear on every exit.
pub fn provide_account_session() {
    provide_context(AccountSession::new());
}

/// Route-scoped UI handle; requests, listeners and timeout handles are owned here.
#[derive(Clone, Copy)]
pub struct AccountController {
    pub state: ReadSignal<AccountSnapshot>,
    inner: StoredValue<Runtime, LocalStorage>,
}

/// In-memory v2 completion fence, never a credential or serialized authority.
/// Its exact route and context generation must remain current (ADR-0043).
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DocumentAccountTicket {
    lease: RouteLease,
    generation: u64,
    subject: String,
    csrf_token: String,
}

impl DocumentAccountTicket {
    pub(crate) fn subject(&self) -> &str {
        &self.subject
    }

    pub(crate) fn csrf_token(&self) -> &str {
        &self.csrf_token
    }
}

/// Signed-out, in-memory continuation control for enrollment; no identity grant.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EnrollmentNavigationTicket {
    lease: RouteLease,
    generation: u64,
    csrf_token: String,
}

impl EnrollmentNavigationTicket {
    pub(crate) fn csrf_token(&self) -> &str {
        &self.csrf_token
    }
}
struct Runtime {
    session: AccountSession,
    lease: RouteLease,
    state: RwSignal<AccountSnapshot>,
    alive: bool,
    visible: bool,
    connected: bool,
    in_flight: Option<core::AccountRequest>,
    #[cfg(target_arch = "wasm32")]
    abort: Option<web_sys::AbortController>,
    #[cfg(target_arch = "wasm32")]
    listeners: Vec<browser::Listener>,
    #[cfg(target_arch = "wasm32")]
    channel: Option<web_sys::BroadcastChannel>,
}

/// Bootstrap a route, and retire its work on Cancel/Back, route exit or freezing.
pub fn use_account() -> AccountController {
    let session = use_context::<AccountSession>().expect("App provides account session");
    let lease = RouteLease(StoredValue::new(()));
    session
        .active_route
        .update_value(|active| *active = Some(lease));
    let state = RwSignal::new(session.core.with_value(core::AccountCore::snapshot));
    let controller = AccountController {
        state: state.read_only(),
        inner: StoredValue::new_local(Runtime {
            session,
            lease,
            state,
            alive: true,
            visible: true,
            connected: true,
            in_flight: None,
            #[cfg(target_arch = "wasm32")]
            abort: None,
            #[cfg(target_arch = "wasm32")]
            listeners: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            channel: None,
        }),
    };
    #[cfg(target_arch = "wasm32")]
    if !browser::listen(controller) {
        controller.inner.update_value(|runtime| {
            runtime.alive = false;
            runtime.session.core.update_value(|core| {
                core.lifecycle_unavailable();
                runtime.state.try_set(core.snapshot());
            });
        });
    }
    on_cleanup(move || controller.dispose());
    crate::social_full::bind_account(controller);
    #[cfg(target_arch = "wasm32")]
    controller.recover_document(browser::visible());
    #[cfg(not(target_arch = "wasm32"))]
    controller.recover_document(true);
    controller
}
impl AccountController {
    /// Read validated document controls only while this exact idle route is live.
    pub(crate) fn document_ticket(self) -> Option<DocumentAccountTicket> {
        let _ = self.state.try_get();
        if !self.current() || !self.visible() || !self.connected() {
            return None;
        }
        self.inner
            .try_with_value(|runtime| {
                runtime
                    .session
                    .core
                    .try_with_value(|core| {
                        let (subject, csrf_token) = core.current_document_context()?;
                        Some(DocumentAccountTicket {
                            lease: runtime.lease,
                            generation: core.snapshot().presentation_generation,
                            subject,
                            csrf_token,
                        })
                    })
                    .flatten()
            })
            .flatten()
    }

    /// A late v2 response cannot cross a route, subject, context or lifecycle.
    pub(crate) fn ticket_current(self, ticket: &DocumentAccountTicket) -> bool {
        self.document_ticket().as_ref() == Some(ticket)
    }

    pub(crate) fn enrollment_navigation_ticket(self) -> Option<EnrollmentNavigationTicket> {
        if !self.current() || !self.visible() || !self.connected() {
            return None;
        }
        self.inner
            .try_with_value(|runtime| {
                runtime
                    .session
                    .core
                    .try_with_value(|core| {
                        Some(EnrollmentNavigationTicket {
                            lease: runtime.lease,
                            generation: core.snapshot().presentation_generation,
                            csrf_token: core.current_pre_auth_control()?,
                        })
                    })
                    .flatten()
            })
            .flatten()
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn enrollment_navigation_current(self, ticket: &EnrollmentNavigationTicket) -> bool {
        self.enrollment_navigation_ticket().as_ref() == Some(ticket)
    }

    pub(crate) fn social_available(self) -> bool {
        self.document_ticket().is_some()
            && self
                .inner
                .try_with_value(|runtime| {
                    runtime
                        .session
                        .core
                        .try_with_value(core::AccountCore::social_available)
                        .unwrap_or(false)
                })
                .unwrap_or(false)
    }

    /// Public enrollment completions still require the same live idle route.
    pub(crate) fn public_generation(self) -> Option<u64> {
        if !self.current() || !self.visible() || !self.connected() {
            return None;
        }
        let snapshot = self.state.try_get()?;
        (snapshot.busy.is_none()
            && matches!(
                snapshot.status,
                AccountStatus::SignedOut | AccountStatus::Expired
            ))
        .then_some(snapshot.presentation_generation)
    }
    pub(crate) fn current(self) -> bool {
        self.inner
            .try_with_value(|runtime| {
                runtime.alive
                    && runtime
                        .session
                        .active_route
                        .try_with_value(|active| *active == Some(runtime.lease))
                        .unwrap_or(false)
            })
            .unwrap_or(false)
    }
    fn visible(self) -> bool {
        #[cfg(target_arch = "wasm32")]
        if !browser::visible() {
            return false;
        }
        self.current()
            && self
                .inner
                .try_with_value(|runtime| runtime.alive && runtime.visible)
                .unwrap_or(false)
    }
    fn connected(self) -> bool {
        #[cfg(target_arch = "wasm32")]
        if !browser::online() {
            return false;
        }
        self.current()
            && self
                .inner
                .try_with_value(|runtime| runtime.connected)
                .unwrap_or(false)
    }
    fn resample_connectivity(self) -> bool {
        #[cfg(target_arch = "wasm32")]
        let connected = browser::online();
        #[cfg(not(target_arch = "wasm32"))]
        let connected = self
            .inner
            .try_with_value(|runtime| runtime.connected)
            .unwrap_or(false);
        self.inner
            .try_update_value(|runtime| runtime.connected = connected);
        connected
    }
    fn recover_document(self, visible: bool) {
        if !self.current() {
            return;
        }
        // A frozen document can miss the online event. Focus/pageshow/manual
        // recovery therefore samples the browser again before fresh authority.
        let connected = self.resample_connectivity();
        self.recover_document_from_hint(visible, connected);
    }
    fn recover_document_from_hint(self, visible: bool, connected: bool) {
        if !self.current() {
            return;
        }
        self.inner
            .try_update_value(|runtime| runtime.connected = connected);
        if !connected {
            self.disconnect();
            return;
        }
        self.clear(true);
        self.inner
            .try_update_value(|runtime| runtime.visible = visible);
        if visible {
            self.recheck();
        }
    }
    fn begin(self, action: fn(&mut core::AccountCore) -> Option<core::AccountRequest>) {
        if !self.visible() {
            self.clear(true);
            return;
        }
        let connected = self.resample_connectivity();
        let mut request = None;
        self.inner.try_update_value(|runtime| {
            if runtime.alive {
                runtime.session.core.update_value(|core| {
                    request = action(core);
                    #[cfg(target_arch = "wasm32")]
                    if !matches!(core.snapshot().status, AccountStatus::Authenticated { .. })
                        || core.snapshot().busy.is_some()
                    {
                        // Also cover local failure before a request is created.
                        browser::mask_private();
                    }
                    runtime.state.try_set(core.snapshot());
                });
            }
        });
        if let Some(request) = request {
            #[cfg(target_arch = "wasm32")]
            {
                browser::mask_private();
                if request.kind() == core::RequestKind::Logout {
                    self.save_suppression();
                    browser::hint(self);
                }
            }
            // A confirmed logout records and saves its exact target above even
            // when connectivity disappeared before the offline event. Reject
            // transport only after preserving that unconfirmed user intent.
            if !connected {
                self.disconnect();
                return;
            }
            self.dispatch(request);
        } else if !connected {
            self.disconnect();
        }
    }
    /// Refetch authority; unresolved logout intent never restores private output.
    pub fn recheck(self) {
        #[cfg(target_arch = "wasm32")]
        if !self.restore_suppression() {
            return;
        }
        self.begin(core::AccountCore::recheck);
    }
    /// Continue an explicitly configured invited-account provider login.
    pub fn login(self) {
        #[cfg(target_arch = "wasm32")]
        if !self.restore_suppression() {
            return;
        }
        self.begin(core::AccountCore::login);
    }
    #[cfg(target_arch = "wasm32")]
    fn restore_suppression(self) -> bool {
        if !self.current() {
            return false;
        }
        let mut available = false;
        self.inner.try_update_value(|runtime| {
            runtime.session.core.update_value(|core| {
                available = browser::restore_suppression(core);
                if !matches!(core.snapshot().status, AccountStatus::Authenticated { .. })
                    || core.snapshot().busy.is_some()
                {
                    browser::mask_private();
                }
                runtime.state.try_set(core.snapshot());
            });
        });
        available
    }
    #[cfg(target_arch = "wasm32")]
    fn save_suppression(self) {
        self.inner.try_update_value(|runtime| {
            runtime.session.core.update_value(|core| {
                for intent in core.logout_intents() {
                    if browser::save_suppression(&intent).is_err() {
                        core.logout_storage_unavailable();
                        break;
                    }
                }
                runtime.state.try_set(core.snapshot());
            });
        });
    }
    /// Rotate a current verifier, then refetch context and self profile.
    pub fn refresh(self) {
        self.begin(core::AccountCore::refresh);
    }
    /// Explicit revocation/retry; only acknowledged durable success clears intent.
    pub fn logout(self) {
        self.begin(core::AccountCore::logout);
    }
    /// Clear private data and invalidate all later completions, without a mutation.
    pub fn cancel(self) {
        self.clear(false);
    }

    /// Retire known-disconnected work synchronously; connectivity is not a
    /// signed-out disposition or permission to reuse a previous profile.
    fn disconnect(self) {
        if !self.current() {
            self.dispose();
            return;
        }
        #[cfg(target_arch = "wasm32")]
        browser::mask_private();
        self.inner.try_update_value(|runtime| {
            runtime.connected = false;
            runtime.in_flight = None;
            #[cfg(target_arch = "wasm32")]
            if let Some(abort) = runtime.abort.take() {
                abort.abort();
            }
            runtime.session.core.update_value(|core| {
                core.disconnect();
                runtime.state.try_set(core.snapshot());
            });
        });
    }

    /// An online hint starts a fresh authority check, never restores private data.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Browser hint/native lifecycle tests.
    fn reconnect(self) {
        if !self.current() {
            return;
        }
        self.inner
            .try_update_value(|runtime| runtime.connected = true);
        #[cfg(target_arch = "wasm32")]
        self.recover_document(browser::visible());
        #[cfg(not(target_arch = "wasm32"))]
        self.recover_document(true);
    }

    fn clear(self, suspended: bool) {
        if !self.current() {
            self.dispose();
            return;
        }
        #[cfg(target_arch = "wasm32")]
        browser::mask_private();
        self.inner.try_update_value(|runtime| {
            runtime.in_flight = None;
            if suspended {
                runtime.visible = false;
            }
            #[cfg(target_arch = "wasm32")]
            if let Some(abort) = runtime.abort.take() {
                abort.abort();
            }
            runtime.session.core.update_value(|core| {
                if suspended {
                    core.suspend();
                } else {
                    core.cancel();
                }
                runtime.state.try_set(core.snapshot());
            });
        });
    }
    fn dispose(self) {
        if !self
            .inner
            .try_with_value(|runtime| runtime.alive)
            .unwrap_or(false)
        {
            return;
        }
        // Routes chooses the replacement view before cleaning up the previous
        // owner. Old cleanup must retire only its own work, never the new core
        // request or the replacement route's private DOM.
        let current = self.current();
        #[cfg(target_arch = "wasm32")]
        if current {
            browser::mask_private();
        }
        self.inner.try_update_value(|runtime| {
            runtime.alive = false;
            runtime.in_flight = None;
            #[cfg(target_arch = "wasm32")]
            {
                if let Some(abort) = runtime.abort.take() {
                    abort.abort();
                }
                runtime.listeners.clear();
                if let Some(channel) = runtime.channel.take() {
                    channel.close();
                }
            }
            runtime.state.try_set(AccountSnapshot {
                status: AccountStatus::Resolving,
                busy: None,
                presentation_generation: 0,
                login_available: false,
            });
            if current {
                runtime
                    .session
                    .active_route
                    .update_value(|active| *active = None);
                runtime
                    .session
                    .core
                    .update_value(core::AccountCore::route_exit);
            }
        });
    }
    fn complete(
        self,
        request: &core::AccountRequest,
        result: Result<core::HttpResponse, core::AccountFailure>,
    ) {
        if !self.connected() {
            self.disconnect();
            return;
        }
        if !self.visible() {
            self.clear(true);
            return;
        }
        #[cfg(target_arch = "wasm32")]
        let successful_mutation = result
            .as_ref()
            .is_ok_and(|response| response.status == 204 && response.body.is_empty())
            && matches!(
                request.kind(),
                core::RequestKind::Refresh | core::RequestKind::Logout
            );
        let mut next = None;
        let mut accepted = false;
        let mut navigation = None;
        self.inner.try_update_value(|runtime| {
            if runtime.alive && runtime.in_flight.as_ref() == Some(request) {
                runtime.in_flight = None;
                accepted = true;
                #[cfg(target_arch = "wasm32")]
                {
                    runtime.abort = None;
                }
                runtime.session.core.update_value(|core| {
                    #[cfg(target_arch = "wasm32")]
                    let logout_intent = request.logout_intent();
                    next = core.complete(request, result);
                    #[cfg(target_arch = "wasm32")]
                    if successful_mutation
                        && request.kind() == core::RequestKind::Logout
                        && logout_intent
                            .as_ref()
                            .is_some_and(|intent| !core.logout_intents().contains(intent))
                    {
                        if browser::clear_suppression(logout_intent.as_deref()).is_err() {
                            core.logout_storage_unavailable();
                        } else {
                            core.logout_storage_ready();
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        // Synchronously observe other target markers before this
                        // completion can publish private data or enable login.
                        browser::restore_suppression(core);
                        if next
                            .as_ref()
                            .is_some_and(|request| !core.request_pending(request))
                        {
                            next = None;
                        }
                    }
                    navigation = core.take_login_navigation();
                    runtime.state.try_set(core.snapshot());
                });
            }
        });
        #[cfg(target_arch = "wasm32")]
        if accepted && successful_mutation {
            browser::hint(self);
        }
        #[cfg(target_arch = "wasm32")]
        if accepted {
            if let Some(navigation) = navigation {
                browser::navigate(self, navigation);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (accepted, navigation);
        if let Some(next) = next {
            self.dispatch(next);
        }
    }
    fn dispatch(self, request: core::AccountRequest) {
        if !self.connected() {
            self.disconnect();
            return;
        }
        if !self.visible() {
            self.clear(true);
            return;
        }
        self.inner.try_update_value(|runtime| {
            if runtime.alive {
                runtime.in_flight = Some(request.clone());
            }
        });
        #[cfg(target_arch = "wasm32")]
        browser::dispatch(self, request);
        #[cfg(not(target_arch = "wasm32"))]
        // Native shell tests inspect lifecycle only; no implicit network fallback.
        {
            self.complete(&request, Err(core::AccountFailure::Unavailable));
            drop(request); // Retire the owned completion witness on this native no-I/O path.
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{core, suppression, AccountController};
    use js_sys::Uint8Array;
    use leptos::prelude::*;
    use wasm_bindgen::{closure::Closure, JsCast};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        AbortController, Event, EventTarget, ReadableStreamDefaultReader, Request, RequestCache,
        RequestCredentials, RequestInit, RequestMode, RequestRedirect, Response, VisibilityState,
    };

    // One immutable target per key avoids read/check/write/remove CAS assumptions.
    // Lifetime is until matching acknowledged revocation, never a client timeout.
    use suppression::{PREFIX as SUPPRESSION_PREFIX, VALUE as SUPPRESSION_VALUE};

    fn storage() -> Result<web_sys::Storage, ()> {
        web_sys::window()
            .ok_or(())?
            .local_storage()
            .map_err(|_| ())?
            .ok_or(())
    }
    struct BrowserStorage(web_sys::Storage);
    impl suppression::MarkerStorage for BrowserStorage {
        fn get(&self, key: &str) -> Result<Option<String>, core::InvalidLogoutIntent> {
            self.0.get_item(key).map_err(|_| core::InvalidLogoutIntent)
        }
        fn set(&self, key: &str, value: &str) -> Result<(), core::InvalidLogoutIntent> {
            self.0
                .set_item(key, value)
                .map_err(|_| core::InvalidLogoutIntent)
        }
        fn remove(&self, key: &str) -> Result<(), core::InvalidLogoutIntent> {
            self.0
                .remove_item(key)
                .map_err(|_| core::InvalidLogoutIntent)
        }
    }
    fn read_suppression(core: &mut core::AccountCore) -> Result<(), ()> {
        let storage = storage()?;
        // Also reject the previous single-slot prototype; it is not an absent hint.
        if storage
            .get_item("tabula-logout-suppression-v1")
            .map_err(|_| ())?
            .is_some()
        {
            return Err(());
        }
        let length = storage.length().map_err(|_| ())?;
        if length > 256 {
            return Err(());
        }
        // Directly read the current target as well: concurrent unrelated removals
        // can shift numeric storage indexes during enumeration.
        if let Some(current) = core.current_context_intent() {
            let key = suppression::key(&current).map_err(|_| ())?;
            match storage.get_item(&key).map_err(|_| ())? {
                Some(value) if value == SUPPRESSION_VALUE => {
                    core.restore_logout_intent(Some(&current)).map_err(|_| ())?;
                }
                Some(_) => return Err(()),
                None => {}
            }
        }
        let mut found = 0;
        for index in 0..length {
            let Some(key) = storage.key(index).map_err(|_| ())? else {
                continue;
            };
            let Some(fingerprint) = key.strip_prefix(SUPPRESSION_PREFIX) else {
                continue;
            };
            if fingerprint.len() != 64 {
                return Err(());
            }
            found += 1;
            if found > core::MAX_LOGOUT_INTENTS {
                return Err(());
            }
            let value = storage.get_item(&key).map_err(|_| ())?;
            if value.is_none() {
                continue;
            } // Concurrent receipt removal is harmless.
            if value.as_deref() != Some(SUPPRESSION_VALUE) {
                return Err(());
            }
            core.restore_logout_intent(Some(&format!("v1:{fingerprint}")))
                .map_err(|_| ())?;
        }
        Ok(())
    }
    pub(super) fn restore_suppression(core: &mut core::AccountCore) -> bool {
        if read_suppression(core).is_ok() {
            core.logout_storage_ready();
            true
        } else {
            core.logout_storage_unavailable();
            false
        }
    }
    pub(super) fn save_suppression(intent: &str) -> Result<(), ()> {
        suppression::save(&BrowserStorage(storage()?), intent).map_err(|_| ())
    }
    pub(super) fn clear_suppression(expected: Option<&str>) -> Result<(), ()> {
        suppression::clear(&BrowserStorage(storage()?), expected.ok_or(())?).map_err(|_| ())
    }
    pub(super) fn navigate(controller: AccountController, navigation: core::LoginNavigation) {
        if !controller.visible()
            || !controller.state.try_get_untracked().is_some_and(|state| {
                state.presentation_generation == navigation.generation()
                    && state.status == super::AccountStatus::LoginRedirecting
            })
        {
            return;
        }
        // DTO validation and backend issuer pinning precede this full document
        // navigation; no return/redirect query or client-supplied provider is used.
        let result = web_sys::window().ok_or(()).and_then(|window| {
            let top = window.top().map_err(|_| ())?.ok_or(())?;
            if !js_sys::Object::is(window.as_ref(), top.as_ref()) {
                // Authenticated shell embedding is closed under ADR-0031 §2.
                return Err(());
            }
            top.location()
                .assign(navigation.authorization_url())
                .map_err(|_| ())
        });
        // Consume the one-use effect even when navigation was rejected.
        drop(navigation);
        if result.is_err() {
            controller.inner.try_update_value(|runtime| {
                runtime.session.core.update_value(|core| {
                    core.login_navigation_failed();
                    runtime.state.try_set(core.snapshot());
                });
            });
        }
    }

    pub(super) struct Listener {
        target: EventTarget,
        event: &'static str,
        callback: Closure<dyn FnMut(Event)>,
    }
    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = self.target.remove_event_listener_with_callback(
                self.event,
                self.callback.as_ref().unchecked_ref(),
            );
        }
    }
    fn add_listener(
        controller: AccountController,
        target: EventTarget,
        event: &'static str,
        callback: impl FnMut(Event) + 'static,
    ) -> bool {
        let callback = Closure::<dyn FnMut(Event)>::new(callback);
        if target
            .add_event_listener_with_callback(event, callback.as_ref().unchecked_ref())
            .is_err()
        {
            return false;
        }
        controller.inner.update_value(|runtime| {
            runtime.listeners.push(Listener {
                target,
                event,
                callback,
            });
        });
        true
    }
    pub(super) fn visible() -> bool {
        web_sys::window()
            .and_then(|window| window.document())
            .is_some_and(|document| document.visibility_state() == VisibilityState::Visible)
    }
    pub(super) fn online() -> bool {
        // This browser hint can suppress account output. A true value never
        // establishes server reachability, session validity or account authority.
        web_sys::window().is_some_and(|window| window.navigator().on_line())
    }
    pub(super) fn listen(controller: AccountController) -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };
        let Some(document) = window.document() else {
            return false;
        };
        let hide = add_listener(controller, window.clone().into(), "pagehide", move |_| {
            controller.clear(true);
        });
        let show = add_listener(controller, window.clone().into(), "pageshow", move |_| {
            controller.recover_document(visible());
        });
        let visibility = add_listener(
            controller,
            document.clone().into(),
            "visibilitychange",
            move |_| {
                if document.visibility_state() == VisibilityState::Hidden {
                    controller.clear(true);
                } else {
                    controller.recover_document(visible());
                }
            },
        );
        let storage = add_listener(controller, window.clone().into(), "storage", move |event| {
            if event
                .dyn_ref::<web_sys::StorageEvent>()
                .is_some_and(|event| {
                    event.key().is_none_or(|key| {
                        key.starts_with(SUPPRESSION_PREFIX) || key == "tabula-logout-suppression-v1"
                    })
                })
            {
                controller.recover_document(visible());
            }
        });
        let offline = add_listener(controller, window.clone().into(), "offline", move |_| {
            controller.disconnect();
        });
        let online = add_listener(controller, window.clone().into(), "online", move |_| {
            controller.reconnect();
        });
        let focus = add_listener(controller, window.into(), "focus", move |_| {
            controller.recover_document(visible());
        });
        if !(hide && show && visibility && focus && storage && offline && online) {
            // Missing cleanup/recovery registration must not retain private output.
            controller.cancel();
            return false;
        }
        if let Ok(channel) = web_sys::BroadcastChannel::new("tabula-account-context-v1") {
            let channel_target: EventTarget = channel.clone().into();
            if add_listener(controller, channel_target, "message", move |_| {
                // A public hint is never identity/permission or a revocation receipt.
                controller.recover_document(visible());
            }) {
                controller
                    .inner
                    .update_value(|runtime| runtime.channel = Some(channel));
            } else {
                channel.close();
            }
        }
        true
    }
    pub(super) fn hint(controller: AccountController) {
        controller.inner.try_with_value(|runtime| {
            if let Some(channel) = &runtime.channel {
                let _ = channel.post_message(&wasm_bindgen::JsValue::from_str("recheck"));
            }
        });
    }
    /// Hide DOM and accessibility output synchronously, before any freeze/frame.
    /// The UI keys replacement private nodes by presentation generation; masked
    /// old nodes are never unhidden, even when the subject ID remains unchanged.
    pub(super) fn mask_private() {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        // Retire focus only when its current control is about to become hidden.
        // Public navigation, locale and recovery controls keep the user's focus.
        let focused_private = document
            .active_element()
            .and_then(|element| element.closest("[data-account-private]").ok().flatten())
            .is_some();
        if focused_private {
            if let Some(heading) = document
                .get_element_by_id("account-title")
                .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = heading.focus();
            }
        }
        let Ok(nodes) = document.query_selector_all("[data-account-private]") else {
            return;
        };
        for index in 0..nodes.length() {
            if let Some(element) = nodes
                .item(index)
                .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
            {
                let _ = element.set_attribute("hidden", "");
                let _ = element.set_attribute("aria-hidden", "true");
                let _ = element.set_attribute("inert", "");
            }
        }
    }

    struct Timeout {
        window: web_sys::Window,
        handle: i32,
        _callback: Closure<dyn FnMut()>,
    }
    impl Timeout {
        fn new(abort: &AbortController) -> Result<Self, core::AccountFailure> {
            let window = web_sys::window().ok_or(core::AccountFailure::Unavailable)?;
            let abort = abort.clone();
            let callback = Closure::<dyn FnMut()>::new(move || abort.abort());
            let handle = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.as_ref().unchecked_ref(),
                    8000,
                )
                .map_err(|_| core::AccountFailure::Unavailable)?;
            Ok(Self {
                window,
                handle,
                _callback: callback,
            })
        }
    }
    impl Drop for Timeout {
        fn drop(&mut self) {
            self.window.clear_timeout_with_handle(self.handle);
        }
    }
    pub(super) fn dispatch(controller: AccountController, request: core::AccountRequest) {
        let Ok(abort) = AbortController::new() else {
            controller.complete(&request, Err(core::AccountFailure::Unavailable));
            return;
        };
        let mut alive = false;
        controller.inner.try_update_value(|runtime| {
            if runtime.alive {
                runtime.abort = Some(abort.clone());
                alive = true;
            }
        });
        if !alive {
            return;
        }
        leptos::task::spawn_local(async move {
            let result = fetch(&request, &abort).await;
            controller.complete(&request, result);
        });
    }
    async fn fetch(
        request: &core::AccountRequest,
        abort: &AbortController,
    ) -> Result<core::HttpResponse, core::AccountFailure> {
        let window = web_sys::window().ok_or(core::AccountFailure::Unavailable)?;
        if !core::trusted_browser_scheme(
            &window
                .location()
                .protocol()
                .map_err(|_| core::AccountFailure::Unavailable)?,
        ) {
            return Err(core::AccountFailure::Unavailable);
        }
        let _timeout = Timeout::new(abort)?;
        let init = RequestInit::new();
        init.set_method(request.method());
        init.set_mode(RequestMode::SameOrigin);
        init.set_cache(RequestCache::NoStore);
        init.set_credentials(RequestCredentials::SameOrigin);
        init.set_redirect(RequestRedirect::Error);
        init.set_signal(Some(&abort.signal()));
        if let Some(body) = request.body() {
            init.set_body(&wasm_bindgen::JsValue::from_str(body));
        }
        let fetch_request = Request::new_with_str_and_init(request.path(), &init)
            .map_err(|_| core::AccountFailure::Protocol)?;
        fetch_request
            .headers()
            .set("Accept", "application/json")
            .map_err(|_| core::AccountFailure::Protocol)?;
        if let Some(token) = request.csrf_token() {
            fetch_request
                .headers()
                .set("Content-Type", "application/json")
                .map_err(|_| core::AccountFailure::Protocol)?;
            fetch_request
                .headers()
                .set("X-Tabula-CSRF", token)
                .map_err(|_| core::AccountFailure::Protocol)?;
        }
        let window = web_sys::window().ok_or(core::AccountFailure::Unavailable)?;
        let response = JsFuture::from(window.fetch_with_request(&fetch_request))
            .await
            .map_err(|_| core::AccountFailure::Transport)?
            .dyn_into::<Response>()
            .map_err(|_| core::AccountFailure::Protocol)?;
        if response.redirected() {
            abort.abort();
            return Err(core::AccountFailure::Protocol);
        }
        let cache = response
            .headers()
            .get("Cache-Control")
            .map_err(|_| core::AccountFailure::Protocol)?;
        if !cache.as_deref().is_some_and(|value| {
            value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case("no-store"))
        }) {
            abort.abort();
            return Err(core::AccountFailure::Protocol);
        }
        if response.status() != 204 {
            let content_type = response
                .headers()
                .get("Content-Type")
                .map_err(|_| core::AccountFailure::Protocol)?;
            if !content_type.as_deref().is_some_and(|value| {
                matches!(
                    value.split(';').next().map(str::trim),
                    Some("application/json" | "application/problem+json")
                )
            }) {
                abort.abort();
                return Err(core::AccountFailure::Protocol);
            }
        }
        let body = read_bounded(&response, abort).await?;
        Ok(core::HttpResponse {
            status: response.status(),
            body,
        })
    }
    async fn read_bounded(
        response: &Response,
        abort: &AbortController,
    ) -> Result<Vec<u8>, core::AccountFailure> {
        if response
            .headers()
            .get("Content-Length")
            .map_err(|_| core::AccountFailure::Protocol)?
            .is_some_and(|value| {
                value
                    .parse::<usize>()
                    .map_or(true, |size| size > core::MAX_RESPONSE_BYTES)
            })
        {
            abort.abort();
            return Err(core::AccountFailure::Protocol);
        }
        let Some(body) = response.body() else {
            return if response.status() == 204 {
                Ok(Vec::new())
            } else {
                Err(core::AccountFailure::Transport)
            };
        };
        let reader =
            ReadableStreamDefaultReader::new(&body).map_err(|_| core::AccountFailure::Transport)?;
        let result = read_chunks(&reader, abort).await;
        reader.release_lock();
        result
    }
    async fn read_chunks(
        reader: &ReadableStreamDefaultReader,
        abort: &AbortController,
    ) -> Result<Vec<u8>, core::AccountFailure> {
        let mut bytes = Vec::new();
        loop {
            let chunk = JsFuture::from(reader.read())
                .await
                .map_err(|_| core::AccountFailure::Transport)?;
            let done = js_sys::Reflect::get(&chunk, &wasm_bindgen::JsValue::from_str("done"))
                .map_err(|_| core::AccountFailure::Protocol)?
                .as_bool();
            if done == Some(true) {
                return Ok(bytes);
            }
            if done != Some(false) {
                abort.abort();
                return Err(core::AccountFailure::Protocol);
            }
            let value = js_sys::Reflect::get(&chunk, &wasm_bindgen::JsValue::from_str("value"))
                .map_err(|_| core::AccountFailure::Protocol)?
                .dyn_into::<Uint8Array>()
                .map_err(|_| core::AccountFailure::Protocol)?;
            let length =
                usize::try_from(value.length()).map_err(|_| core::AccountFailure::Protocol)?;
            if length > core::MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                abort.abort();
                return Err(core::AccountFailure::Protocol);
            }
            bytes.extend(value.to_vec());
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::core::{AccountCore, AccountFailure, AccountRequest, HttpResponse};
    use super::*;

    fn controller() -> AccountController {
        controller_in(AccountSession::new())
    }
    fn controller_in(session: AccountSession) -> AccountController {
        let lease = RouteLease(StoredValue::new(()));
        session
            .active_route
            .update_value(|active| *active = Some(lease));
        let state = RwSignal::new(session.core.with_value(AccountCore::snapshot));
        let controller = AccountController {
            state: state.read_only(),
            inner: StoredValue::new_local(Runtime {
                session,
                lease,
                state,
                alive: true,
                visible: true,
                connected: true,
                in_flight: None,
            }),
        };
        on_cleanup(move || controller.dispose());
        controller
    }
    fn pending(controller: AccountController) -> AccountRequest {
        let mut ticket = None;
        controller.inner.update_value(|runtime| {
            runtime.session.core.update_value(|core| {
                ticket = core.recheck();
                runtime.state.set(core.snapshot());
            });
            runtime.in_flight.clone_from(&ticket);
        });
        ticket.unwrap()
    }
    #[allow(clippy::unnecessary_wraps)] // Mirror the Result-shaped transport seam for fixtures.
    fn signed_out() -> Result<HttpResponse, AccountFailure> {
        Ok(HttpResponse {status:200,body:br#"{"version":1,"disposition":"signed_out","account_id":null,"csrf_token":null,"capabilities":{"login":false,"register":false,"friends":false,"read_self_profile":false}}"#.to_vec()})
    }

    #[test]
    fn replacement_route_bootstraps_before_old_owner_cleanup_and_completes_current_profile() {
        let app = Owner::new();
        app.with(|| {
            provide_account_session();
            crate::social_full::provide_social();
        });
        let old_owner = app.child();
        let old = old_owner.with(use_account);
        let old_request = old_owner.with(|| pending(old));
        // Leptos Routes chooses the replacement view before old_owner.cleanup().
        // use_account must retire the old ticket and acquire the shared core.
        let new_owner = app.child();
        let new = new_owner.with(use_account);
        let new_request = new_owner.with(|| pending(new));
        old_owner.cleanup();

        // Native use_account deliberately makes no requests. Hand the literal
        // context completion into its core, then complete the profile through
        // the controller so the real owner/state fence is exercised.
        let mut profile = None;
        new.inner.update_value(|runtime| {
            runtime.session.core.update_value(|core| {
                profile = core.complete(&new_request, Ok(HttpResponse {
                    status: 200,
                    body: br#"{"version":1,"disposition":"authenticated","account_id":"00000000000000000000000000000001","csrf_token":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","capabilities":{"login":false,"register":false,"friends":false,"read_self_profile":true}}"#.to_vec(),
                }));
                runtime.state.set(core.snapshot());
            });
            runtime.in_flight.clone_from(&profile);
        });
        let profile = profile.expect("old cleanup cannot retire the new context ticket");
        new.complete(
            &profile,
            Ok(HttpResponse {
                status: 200,
                body: br#"{"version":1,"account_id":"00000000000000000000000000000001"}"#.to_vec(),
            }),
        );
        let expected = new.state.get_untracked();
        assert_eq!(
            expected.status,
            AccountStatus::Authenticated {
                account_id: "00000000000000000000000000000001".to_owned(),
            }
        );
        assert!(expected.busy.is_none());
        assert!(new.current());
        old.complete(&old_request, signed_out());
        old.recover_document(true);
        old.cancel();
        old.dispose();
        assert_eq!(new.state.get_untracked(), expected);
        assert_eq!(
            new.inner
                .with_value(|runtime| runtime.session.core.with_value(AccountCore::snapshot)),
            expected
        );
    }

    #[test]
    fn overlapping_old_route_hooks_retire_only_old_work_before_owner_cleanup() {
        Owner::new().with(|| {
            let old = controller();
            let old_request = pending(old);
            let session = old.inner.with_value(|runtime| runtime.session);
            let new = controller_in(session);
            new.recover_document(true);
            let new_request = pending(new);
            let expected = session.core.with_value(AccountCore::snapshot);
            assert!(!old.current());
            old.recover_document(true);
            assert_eq!(session.core.with_value(AccountCore::snapshot), expected);
            old.complete(&old_request, signed_out());
            old.cancel();
            old.dispose();
            assert_eq!(session.core.with_value(AccountCore::snapshot), expected);
            assert!(old
                .inner
                .with_value(|runtime| !runtime.alive && runtime.in_flight.is_none()));
            assert_eq!(old.state.get_untracked().status, AccountStatus::Resolving);
            assert!(new
                .inner
                .with_value(|runtime| runtime.in_flight.as_ref() == Some(&new_request)));
            new.complete(&new_request, signed_out());
            assert_eq!(new.state.get_untracked().status, AccountStatus::SignedOut);
        });
    }

    #[test]
    fn stale_completion_cannot_take_new_request_cancellation_owner() {
        Owner::new().with(|| {
            let controller = controller();
            let old = pending(controller);
            controller.cancel();
            let new = pending(controller);
            controller.complete(&old, signed_out());
            assert!(controller
                .inner
                .with_value(|runtime| runtime.in_flight.as_ref() == Some(&new)));
            assert_eq!(
                controller.state.get_untracked().busy,
                Some(AccountOperation::Recheck)
            );
            controller.complete(&new, signed_out());
            assert!(controller
                .inner
                .with_value(|runtime| runtime.in_flight.is_none()));
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::SignedOut
            );
        });
    }
    #[test]
    fn hidden_hint_or_completion_stays_masked_until_visible_document_recovery() {
        Owner::new().with(|| {
            let controller = controller();
            let old = pending(controller);
            controller.recover_document(false);
            controller.complete(&old, signed_out());
            controller.recheck();
            assert!(controller
                .inner
                .with_value(|runtime| !runtime.visible && runtime.in_flight.is_none()));
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::Resolving
            );
            controller.recover_document(true);
            // Native adaptation deliberately ends unavailable rather than making a request.
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::Unavailable
            );
        });
    }
    #[test]
    fn offline_hint_retires_work_and_reconnect_cannot_restore_a_cached_profile() {
        Owner::new().with(|| {
            let controller = controller();
            let context_request = pending(controller);
            let mut profile_request = None;
            controller.inner.update_value(|runtime| {
                runtime.session.core.update_value(|core| {
                    profile_request = core.complete(&context_request, Ok(HttpResponse {
                        status: 200,
                        body: br#"{"version":1,"disposition":"authenticated","account_id":"00000000000000000000000000000001","csrf_token":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","capabilities":{"login":false,"register":false,"friends":false,"read_self_profile":true}}"#.to_vec(),
                    }));
                    runtime.state.set(core.snapshot());
                });
                runtime.in_flight.clone_from(&profile_request);
            });
            let profile_request = profile_request.unwrap();
            let generation = controller.state.get_untracked().presentation_generation;
            controller.disconnect();
            assert_eq!(controller.state.get_untracked().status, AccountStatus::Disconnected);
            assert!(controller.state.get_untracked().presentation_generation > generation);
            assert!(controller.inner.with_value(|runtime| {
                !runtime.connected && runtime.in_flight.is_none()
            }));
            controller.complete(&profile_request, Ok(HttpResponse {
                status: 200,
                body: br#"{"version":1,"account_id":"00000000000000000000000000000001"}"#.to_vec(),
            }));
            controller.recover_document(true);
            controller.recheck();
            assert_eq!(controller.state.get_untracked().status, AccountStatus::Disconnected);
            assert!(controller.inner.with_value(|runtime| runtime.in_flight.is_none()));
            controller.reconnect();
            // Native dispatch has no authority transport. An online hint must
            // therefore end unavailable, never resurrect the earlier subject.
            assert!(controller.inner.with_value(|runtime| runtime.connected));
            assert_eq!(controller.state.get_untracked().status, AccountStatus::Unavailable);
            controller.complete(&profile_request, Ok(HttpResponse {
                status: 200,
                body: br#"{"version":1,"account_id":"00000000000000000000000000000001"}"#.to_vec(),
            }));
            assert_eq!(controller.state.get_untracked().status, AccountStatus::Unavailable);
        });
    }
    #[test]
    fn recovery_resamples_connectivity_after_a_missed_online_event() {
        Owner::new().with(|| {
            let controller = controller();
            let old = pending(controller);
            controller.disconnect();
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::Disconnected
            );
            assert!(controller.inner.with_value(|runtime| !runtime.connected));
            // This is the current browser hint sampled by pageshow/focus; no
            // online event reached the previously disconnected route owner.
            controller.recover_document_from_hint(true, true);
            assert!(controller.inner.with_value(|runtime| runtime.connected));
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::Unavailable
            );
            controller.complete(&old, signed_out());
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::Unavailable
            );
        });
    }
    #[test]
    fn confirmed_logout_before_offline_event_keeps_target_without_dispatch() {
        Owner::new().with(|| {
            let controller = controller();
            let context_request = pending(controller);
            let mut profile_request = None;
            controller.inner.update_value(|runtime| {
                runtime.session.core.update_value(|core| {
                    profile_request = core.complete(&context_request, Ok(HttpResponse {
                        status: 200,
                        body: br#"{"version":1,"disposition":"authenticated","account_id":"00000000000000000000000000000001","csrf_token":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","capabilities":{"login":false,"register":false,"friends":false,"read_self_profile":true}}"#.to_vec(),
                    }));
                    runtime.state.set(core.snapshot());
                });
                runtime.in_flight.clone_from(&profile_request);
            });
            controller.complete(&profile_request.unwrap(), Ok(HttpResponse {
                status: 200,
                body: br#"{"version":1,"account_id":"00000000000000000000000000000001"}"#.to_vec(),
            }));
            assert!(matches!(controller.state.get_untracked().status, AccountStatus::Authenticated { .. }));
            // Connectivity disappeared while the confirmation was still visible;
            // its offline cleanup event has not retired the current target yet.
            controller.inner.update_value(|runtime| runtime.connected = false);
            controller.logout();
            assert_eq!(controller.state.get_untracked().status, AccountStatus::LogoutPending);
            assert!(controller.inner.with_value(|runtime| runtime.in_flight.is_none()));
            let intents = controller.inner.with_value(|runtime| runtime.session.core.with_value(AccountCore::logout_intents));
            assert_eq!(intents.len(), 1);
            controller.recover_document_from_hint(true, true);
            assert_eq!(controller.state.get_untracked().status, AccountStatus::LogoutPending);
            assert_eq!(controller.inner.with_value(|runtime| runtime.session.core.with_value(AccountCore::logout_intents)), intents);
        });
    }
    #[test]
    fn missed_pagehide_pending_request_is_retired_before_pageshow_recovery() {
        Owner::new().with(|| {
            let controller = controller();
            let old = pending(controller);
            let generation = controller.state.get_untracked().presentation_generation;
            controller.recover_document(true);
            assert!(controller.state.get_untracked().presentation_generation > generation);
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::Unavailable
            );
            controller.complete(&old, signed_out());
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::Unavailable
            );
        });
    }
    #[test]
    fn failed_listener_setup_or_disposed_owner_cannot_bootstrap_or_mutate_new_route_core() {
        Owner::new().with(|| {
            let controller = controller();
            controller.inner.update_value(|runtime| {
                runtime.alive = false;
                runtime.session.core.update_value(|core| {
                    core.lifecycle_unavailable();
                    runtime.state.set(core.snapshot());
                });
            });
            controller.recover_document(true);
            controller.recheck();
            controller.cancel();
            controller.dispose();
            assert_eq!(
                controller.state.get_untracked().status,
                AccountStatus::Unavailable
            );
            assert!(controller
                .inner
                .with_value(|runtime| runtime.in_flight.is_none()));
            // Another route owner uses the same application session after this
            // one is disabled; old callbacks must not retire that new operation.
            let session = controller.inner.with_value(|runtime| runtime.session);
            let mut new_ticket = None;
            session
                .core
                .update_value(|core| new_ticket = core.recheck());
            let new_ticket = new_ticket.unwrap();
            let expected = session.core.with_value(AccountCore::snapshot);
            controller.complete(&new_ticket, signed_out());
            controller.recover_document(true);
            controller.cancel();
            controller.dispose();
            assert_eq!(session.core.with_value(AccountCore::snapshot), expected);
        });
    }
}
