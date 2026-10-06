//! Document-only v2 account request mechanism (ADR-0043).
//! Server authority remains at every read, commit and publication boundary.

use leptos::prelude::*;

use crate::account::{AccountController, DocumentAccountTicket};

/// A response must match the route's current validated document context.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum RequestScope {
    Viewer(DocumentAccountTicket),
    Enrollment(u64),
}

impl RequestScope {
    pub(crate) fn current(&self, account: AccountController) -> bool {
        match self {
            Self::Viewer(ticket) => account.ticket_current(ticket),
            Self::Enrollment(generation) => account.public_generation() == Some(*generation),
        }
    }
}

/// No infrastructure response text or private data is rendered as an error.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) enum RequestFailure {
    Unavailable,
    Transport,
    Protocol,
}

/// Bounded response bytes, still requiring the v2 DTO's shape/subject validation.
pub(crate) struct JsonResponse {
    pub(crate) status: u16,
    pub(crate) body: Vec<u8>,
}

impl JsonResponse {
    #[cfg(target_arch = "wasm32")]
    fn unauthenticated(&self) -> bool {
        self.status == 401
            && crate::json::decode::<tabula_session_http::PublicProblem>(&self.body)
                .is_ok_and(|problem| matches!(problem.version, 1 | 2) && problem.status == 401)
    }
    pub(crate) fn rejected(&self) -> bool {
        matches!(self.status, 400 | 401 | 403 | 404 | 409)
            && crate::json::decode::<tabula_session_http::PublicProblem>(&self.body).is_ok_and(
                |problem| matches!(problem.version, 1 | 2) && problem.status == self.status,
            )
    }
    pub(crate) fn conflict(&self) -> bool {
        self.status == 409
            && crate::json::decode::<tabula_session_http::PublicProblem>(&self.body).is_ok_and(
                |problem| {
                    matches!(problem.version, 1 | 2)
                        && problem.status == 409
                        && problem.code == "revision_conflict"
                },
            )
    }
}

struct SlotRuntime {
    alive: bool,
    sequence: u64,
    #[cfg(target_arch = "wasm32")]
    abort: Option<web_sys::AbortController>,
}

/// One single-flight route slot; retirement invalidates completion effects.
#[derive(Clone, Copy)]
pub(crate) struct RequestSlot(StoredValue<SlotRuntime, LocalStorage>);

impl RequestSlot {
    pub(crate) fn new() -> Self {
        let slot = Self(StoredValue::new_local(SlotRuntime {
            alive: true,
            sequence: 0,
            #[cfg(target_arch = "wasm32")]
            abort: None,
        }));
        on_cleanup(move || {
            slot.retire();
            slot.0.try_update_value(|runtime| runtime.alive = false);
        });
        slot
    }

    pub(crate) fn retire(self) {
        self.0.try_update_value(|runtime| {
            runtime.sequence = runtime.sequence.saturating_add(1);
            #[cfg(target_arch = "wasm32")]
            if let Some(abort) = runtime.abort.take() {
                abort.abort();
            }
        });
    }

    #[allow(clippy::too_many_arguments)] // One bounded HTTP dispatch, not domain policy.
    pub(crate) fn run(
        self,
        account: AccountController,
        scope: RequestScope,
        method: &'static str,
        path: String,
        csrf: Option<String>,
        body: Option<String>,
        complete: impl FnOnce(Result<JsonResponse, RequestFailure>) + 'static,
    ) {
        self.dispatch(account, scope, method, path, csrf, body, Box::new(complete));
    }

    // One asynchronous dispatch implementation serves all DTO completions.
    // Keeping this boundary out of line avoids cloning the Fetch/fence future
    // into each registration, profile and social call site under size LTO.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(not(target_arch = "wasm32"), allow(clippy::needless_pass_by_value))]
    fn dispatch(
        self,
        account: AccountController,
        scope: RequestScope,
        method: &'static str,
        path: String,
        csrf: Option<String>,
        body: Option<String>,
        complete: Box<dyn FnOnce(Result<JsonResponse, RequestFailure>)>,
    ) {
        self.retire();
        if !scope.current(account) {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let Ok(abort) = web_sys::AbortController::new() else {
                complete(Err(RequestFailure::Unavailable));
                return;
            };
            let mut sequence = None;
            self.0.try_update_value(|runtime| {
                if runtime.alive && runtime.sequence < u64::MAX {
                    runtime.abort = Some(abort.clone());
                    sequence = Some(runtime.sequence);
                }
            });
            let Some(sequence) = sequence else {
                return;
            };
            leptos::task::spawn_local(async move {
                let response =
                    browser::fetch(method, &path, csrf.as_deref(), body.as_deref(), &abort).await;
                if scope.current(account)
                    && self
                        .0
                        .try_with_value(|runtime| runtime.alive && runtime.sequence == sequence)
                        .unwrap_or(false)
                {
                    self.0.try_update_value(|runtime| runtime.abort = None);
                    let recheck = matches!(scope, RequestScope::Viewer(_))
                        && response.as_ref().is_ok_and(JsonResponse::unauthenticated);
                    complete(response);
                    if recheck {
                        account.recheck();
                    }
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (method, path, csrf, body);
            complete(Err(RequestFailure::Unavailable));
        }
    }
}

/// Non-authorizing operation IDs originate from browser cryptographic randomness.
pub(crate) fn operation_id() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let crypto = web_sys::window()?.crypto().ok()?;
        let mut bytes = [0_u8; 16];
        crypto.get_random_values_with_u8_array(&mut bytes).ok()?;
        if bytes.iter().all(|byte| *byte == 0) {
            return None;
        }
        let hex: &[u8; 16] = b"0123456789abcdef";
        Some(
            bytes
                .iter()
                .flat_map(|byte| {
                    [
                        char::from(hex[usize::from(byte >> 4)]),
                        char::from(hex[usize::from(byte & 15)]),
                    ]
                })
                .collect(),
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

/// Ready resource state is kept separate from failure, pending and unknown writes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourcePhase {
    Checking,
    Ready,
    Unavailable,
    Error,
    Pending,
    Unknown,
    Conflict,
    Saved,
}

impl ResourcePhase {
    pub(crate) const fn busy(self) -> bool {
        matches!(self, Self::Checking | Self::Pending)
    }

    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Checking => "accounts.full.checking",
            Self::Ready => "accounts.full.ready",
            Self::Unavailable => "accounts.service.unavailable",
            Self::Error => "accounts.full.error",
            Self::Pending => "accounts.full.pending",
            Self::Unknown => "accounts.full.unknown",
            Self::Conflict => "accounts.full.conflict",
            Self::Saved => "accounts.full.saved",
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{JsonResponse, RequestFailure};
    use wasm_bindgen::{closure::Closure, JsCast};
    use wasm_bindgen_futures::JsFuture;

    const MAX_ACCOUNT_BODY: usize = 32_768;
    const MAX_SOCIAL_HTTP_BODY: usize = 512 * 1024;

    struct Timeout {
        window: web_sys::Window,
        handle: i32,
        _callback: Closure<dyn FnMut()>,
    }
    impl Drop for Timeout {
        fn drop(&mut self) {
            self.window.clear_timeout_with_handle(self.handle);
        }
    }

    #[allow(clippy::too_many_lines)] // The timeout and bounded body reader share one abort lifetime.
    pub(super) async fn fetch(
        method: &str,
        path: &str,
        csrf: Option<&str>,
        body: Option<&str>,
        abort: &web_sys::AbortController,
    ) -> Result<JsonResponse, RequestFailure> {
        let max_body = if path.starts_with("/api/v2/social") {
            MAX_SOCIAL_HTTP_BODY
        } else {
            MAX_ACCOUNT_BODY
        };
        let window = web_sys::window().ok_or(RequestFailure::Unavailable)?;
        if window.location().protocol().ok().as_deref() != Some("https:")
            || !window.navigator().on_line()
            || !path.starts_with("/api/v2/")
        {
            return Err(RequestFailure::Unavailable);
        }
        let timeout_abort = abort.clone();
        let callback = Closure::<dyn FnMut()>::new(move || timeout_abort.abort());
        let handle = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                8000,
            )
            .map_err(|_| RequestFailure::Unavailable)?;
        let _timeout = Timeout {
            window: window.clone(),
            handle,
            _callback: callback,
        };
        let init = web_sys::RequestInit::new();
        init.set_method(method);
        init.set_mode(web_sys::RequestMode::SameOrigin);
        init.set_credentials(web_sys::RequestCredentials::SameOrigin);
        init.set_cache(web_sys::RequestCache::NoStore);
        init.set_redirect(web_sys::RequestRedirect::Error);
        init.set_signal(Some(&abort.signal()));
        if let Some(body) = body {
            init.set_body(&wasm_bindgen::JsValue::from_str(body));
        }
        let request = web_sys::Request::new_with_str_and_init(path, &init)
            .map_err(|_| RequestFailure::Protocol)?;
        request
            .headers()
            .set("Accept", "application/json")
            .map_err(|_| RequestFailure::Protocol)?;
        if body.is_some() {
            request
                .headers()
                .set("Content-Type", "application/json")
                .map_err(|_| RequestFailure::Protocol)?;
        }
        if let Some(csrf) = csrf {
            request
                .headers()
                .set("X-Tabula-CSRF", csrf)
                .map_err(|_| RequestFailure::Protocol)?;
        }
        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| RequestFailure::Transport)?
            .dyn_into::<web_sys::Response>()
            .map_err(|_| RequestFailure::Protocol)?;
        if response.redirected()
            || !response
                .headers()
                .get("Cache-Control")
                .map_err(|_| RequestFailure::Protocol)?
                .is_some_and(|cache| {
                    cache
                        .split(',')
                        .any(|part| part.trim().eq_ignore_ascii_case("no-store"))
                })
        {
            return Err(RequestFailure::Protocol);
        }
        if response.status() == 204 {
            return Ok(JsonResponse {
                status: 204,
                body: Vec::new(),
            });
        }
        if !response
            .headers()
            .get("Content-Type")
            .map_err(|_| RequestFailure::Protocol)?
            .is_some_and(|value| {
                matches!(
                    value.split(';').next().map(str::trim),
                    Some("application/json" | "application/problem+json")
                )
            })
        {
            return Err(RequestFailure::Protocol);
        }
        if response
            .headers()
            .get("Content-Length")
            .map_err(|_| RequestFailure::Protocol)?
            .is_some_and(|length| {
                length
                    .parse::<usize>()
                    .map_or(true, |length| length > max_body)
            })
        {
            abort.abort();
            return Err(RequestFailure::Protocol);
        }
        let stream = response.body().ok_or(RequestFailure::Transport)?;
        let reader = web_sys::ReadableStreamDefaultReader::new(&stream)
            .map_err(|_| RequestFailure::Protocol)?;
        let result = async {
            let mut bytes = Vec::new();
            loop {
                let chunk = JsFuture::from(reader.read())
                    .await
                    .map_err(|_| RequestFailure::Transport)?;
                let done = js_sys::Reflect::get(&chunk, &"done".into())
                    .ok()
                    .and_then(|done| done.as_bool())
                    .ok_or(RequestFailure::Protocol)?;
                if done {
                    return Ok(bytes);
                }
                let value = js_sys::Reflect::get(&chunk, &"value".into())
                    .map_err(|_| RequestFailure::Protocol)?;
                if !value.is_instance_of::<js_sys::Uint8Array>() {
                    return Err(RequestFailure::Protocol);
                }
                let data = js_sys::Uint8Array::new(&value);
                if data.length() as usize > max_body.saturating_sub(bytes.len()) {
                    abort.abort();
                    return Err(RequestFailure::Protocol);
                }
                bytes.extend_from_slice(&data.to_vec());
            }
        }
        .await;
        reader.release_lock();
        Ok(JsonResponse {
            status: response.status(),
            body: result?,
        })
    }
}
