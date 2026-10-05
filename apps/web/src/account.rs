//! Document-memory browser account controller (ADR-0031 §6 / ADR-0036 PR3).
//!
//! This binary consumes the default DTO contract, never server runtime authority.
//! No login credentials, bearer, persistent account cache or storage API is used.

pub mod core;
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
        Self {
            core: StoredValue::new(core::AccountCore::default()),
            active_route: StoredValue::new(None),
        }
    }
}

/// Provide once at the application owner. Only non-secret logout suppression
/// survives route owners; private identity and CSRF are cleared on every exit.
pub fn provide_account_session() {
    provide_context(AccountSession::new());
}

/// Route-scoped UI handle; requests, listeners and timeout handles are owned here.
#[derive(Clone, Copy)]
pub struct AccountController {
    pub state: ReadSignal<AccountSnapshot>,
    inner: StoredValue<Runtime, LocalStorage>,
}
struct Runtime {
    session: AccountSession,
    lease: RouteLease,
    state: RwSignal<AccountSnapshot>,
    alive: bool,
    visible: bool,
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
    #[cfg(target_arch = "wasm32")]
    controller.recover_document(browser::visible());
    #[cfg(not(target_arch = "wasm32"))]
    controller.recover_document(true);
    controller
}
impl AccountController {
    fn current(self) -> bool {
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
    fn recover_document(self, visible: bool) {
        if !self.current() {
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
        let mut request = None;
        self.inner.try_update_value(|runtime| {
            if runtime.alive {
                runtime.session.core.update_value(|core| {
                    request = action(core);
                    runtime.state.try_set(core.snapshot());
                });
            }
        });
        if let Some(request) = request {
            #[cfg(target_arch = "wasm32")]
            {
                browser::mask_private();
                if request.kind() == core::RequestKind::Logout {
                    browser::hint(self);
                }
            }
            self.dispatch(request);
        }
    }
    /// Refetch authority; unresolved logout intent never restores private output.
    pub fn recheck(self) {
        self.begin(core::AccountCore::recheck);
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
        if !self.visible() {
            self.clear(true);
            return;
        }
        #[cfg(target_arch = "wasm32")]
        let successful_mutation = result.as_ref().is_ok_and(|response| response.status == 204)
            && matches!(
                request.kind(),
                core::RequestKind::Refresh | core::RequestKind::Logout
            );
        let mut next = None;
        let mut accepted = false;
        self.inner.try_update_value(|runtime| {
            if runtime.alive && runtime.in_flight.as_ref() == Some(request) {
                runtime.in_flight = None;
                accepted = true;
                #[cfg(target_arch = "wasm32")]
                {
                    runtime.abort = None;
                }
                runtime.session.core.update_value(|core| {
                    next = core.complete(request, result);
                    runtime.state.try_set(core.snapshot());
                });
            }
        });
        #[cfg(target_arch = "wasm32")]
        if accepted && successful_mutation {
            browser::hint(self);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = accepted;
        if let Some(next) = next {
            self.dispatch(next);
        }
    }
    fn dispatch(self, request: core::AccountRequest) {
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
    use super::{core, AccountController};
    use js_sys::Uint8Array;
    use leptos::prelude::*;
    use wasm_bindgen::{closure::Closure, JsCast};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        AbortController, Event, EventTarget, ReadableStreamDefaultReader, Request, RequestCache,
        RequestCredentials, RequestInit, RequestMode, RequestRedirect, Response, VisibilityState,
    };

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
        let focus = add_listener(controller, window.into(), "focus", move |_| {
            controller.recover_document(visible());
        });
        if !(hide && show && visibility && focus) {
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
        app.with(provide_account_session);
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
