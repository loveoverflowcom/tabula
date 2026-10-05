//! Opt-in direct-match controls; game meaning remains in erased registry data.
use crate::{
    i18n::{shell, Messages},
    views::use_locale,
};
use leptos::prelude::*;
use tabula_registry::{GameId, RuntimeBinding};

/// Only the explicitly built online panel can assert this deployment binding.
/// The ordinary local setup keeps `RuntimeBinding::bound` and its default gates.
pub(crate) fn runtime_binding() -> RuntimeBinding {
    option_env!("TABULA_PLAY_BASE").map_or(RuntimeBinding::unbound(), RuntimeBinding::direct_online)
}

#[component]
pub fn OnlinePanel(id: String) -> impl IntoView {
    let locale = use_locale();
    let code = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let status = RwSignal::new("online.ready".to_owned());
    let admission = RwSignal::new(None::<tabula_match_http::MatchAdmission>);
    let runtime = StoredValue::new_local(Runtime::default());
    on_cleanup(move || {
        runtime.update_value(|r| {
            r.alive = false;
            #[cfg(target_arch = "wasm32")]
            if let Some(abort) = r.abort.take() {
                abort.abort();
            }
        });
    });
    let create_id = id.clone();
    let enter_id = id;
    view! {
        <section class="online-panel" aria-labelledby="online-heading">
            <h2 id="online-heading" class="section__subtitle">{move || Messages::new(locale.get()).text("online.heading")}</h2>
            <p class="section__body">{move || Messages::new(locale.get()).text("online.scope")}</p>
            <button type="button" class="btn btn--filled" data-testid="online-create" disabled=move || busy.get() on:click=move |_| {
                if busy.get_untracked() { return; }
                busy.set(true); status.set("online.pending".into()); admission.set(None);
                dispatch(runtime, busy, status, admission, Operation::Create(create_id.clone()));
            }>{move || Messages::new(locale.get()).text("online.create")}</button>
            <div class="field">
                <label for="online-join-code" class="field__label">{move || Messages::new(locale.get()).text("online.code.label")}</label>
                <input id="online-join-code" data-testid="online-join-code" class="field__control" type="text" maxlength="12" autocomplete="off" autocapitalize="characters" spellcheck="false" prop:value=move || code.get() disabled=move || busy.get() on:input=move |event| code.set(event_target_value(&event).to_ascii_uppercase())/>
                <button type="button" class="btn btn--tonal" data-testid="online-join" disabled=move || busy.get() || code.get().is_empty() on:click=move |_| {
                    if busy.get_untracked() { return; }
                    busy.set(true); status.set("online.pending".into()); admission.set(None);
                    dispatch(runtime, busy, status, admission, Operation::Join(code.get_untracked()));
                }>{move || Messages::new(locale.get()).text("online.join")}</button>
            </div>
            <p class="status" role="status" aria-live="polite" data-testid="online-status">{move || Messages::new(locale.get()).text(&status.get())}</p>
            {move || admission.get().map(|a| {
                let messages = Messages::new(locale.get());
                let join_code = a.join_code().filter(|code| !code.is_empty()).map(str::to_owned);
                let match_id = a.match_id().to_owned();
                let game_id = a.game_id().to_owned();
                let expected = enter_id.clone();
                view! {
                    <div class="online-panel__admission">
                        {join_code.map(|code| view! { <p>{messages.text("online.code.share")}<strong data-testid="online-code">{code}</strong></p> })}
                        <p data-testid="online-seat">{format!("{} {}", messages.text("online.seat"), a.seat() + 1)}</p>
                        <button type="button" class="btn btn--filled" data-testid="online-enter" on:click=move |_| open_admission(&game_id, &expected, &match_id, locale.get_untracked(), status)>{messages.text("online.enter")}</button>
                    </div>
                }
            })}
            <a href="/account" class="btn btn--text">{move || Messages::new(locale.get()).text("online.account")}</a>
        </section>
    }
}
fn open_admission(
    game_id: &str,
    expected: &str,
    match_id: &str,
    locale: tabula_registry::Locale,
    status: RwSignal<String>,
) {
    if game_id != expected {
        status.set("online.incompatible".into());
        return;
    }
    let (_, catalog) = shell(locale);
    let binding = runtime_binding();
    let target = GameId::new(game_id.to_owned())
        .ok()
        .and_then(|id| catalog.get(&id))
        .filter(|entry| binding.supports_direct(entry.game()));
    if let Some(entry) = target {
        if let Ok(handoff) =
            tabula_registry::launch::resolve_direct(binding, entry.game(), match_id, locale)
        {
            navigate(&handoff.url);
            return;
        }
    }
    status.set("online.incompatible".into());
}
struct Runtime {
    alive: bool,
    #[cfg(target_arch = "wasm32")]
    abort: Option<web_sys::AbortController>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            alive: true,
            #[cfg(target_arch = "wasm32")]
            abort: None,
        }
    }
}
#[derive(Clone)]
enum Operation {
    Create(String),
    Join(String),
}
#[cfg(target_arch = "wasm32")]
fn navigate(url: &str) {
    let _ = window().location().assign(url);
}
#[cfg(not(target_arch = "wasm32"))]
fn navigate(_url: &str) {}
#[cfg(not(target_arch = "wasm32"))]
fn dispatch(
    _runtime: StoredValue<Runtime, LocalStorage>,
    busy: RwSignal<bool>,
    status: RwSignal<String>,
    _admission: RwSignal<Option<tabula_match_http::MatchAdmission>>,
    operation: Operation,
) {
    match operation {
        Operation::Create(id) | Operation::Join(id) => drop(id),
    }
    busy.set(false);
    status.set("online.unavailable".into());
}
#[cfg(target_arch = "wasm32")]
fn dispatch(
    runtime: StoredValue<Runtime, LocalStorage>,
    busy: RwSignal<bool>,
    status: RwSignal<String>,
    admission: RwSignal<Option<tabula_match_http::MatchAdmission>>,
    operation: Operation,
) {
    let Ok(abort) = web_sys::AbortController::new() else {
        busy.set(false);
        status.set("online.unavailable".into());
        return;
    };
    runtime.update_value(|r| r.abort = Some(abort.clone()));
    leptos::task::spawn_local(async move {
        let result = browser::run(operation, &abort).await;
        if !runtime.try_with_value(|r| r.alive).unwrap_or(false) {
            return;
        }
        busy.set(false);
        match result {
            Ok(value) => {
                status.set(
                    if value.ready() {
                        "online.joined"
                    } else {
                        "online.waiting"
                    }
                    .into(),
                );
                admission.set(Some(value));
            }
            Err(key) => status.set(key.into()),
        }
    });
}
// Join denials deliberately do not reveal whether a room/code exists. Only a
// real authentication failure should send the user back to sign in.
#[cfg(any(target_arch = "wasm32", test))]
fn response_error_key(status: u16, joining: bool) -> Option<&'static str> {
    match status {
        200 | 201 => None,
        403 if joining => Some("online.invalid_code"),
        401 | 403 => Some("online.signin"),
        404 | 409 => Some("online.invalid_code"),
        _ => Some("online.unavailable"),
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{shell, GameId, Operation};
    use js_sys::Uint8Array;
    use leptos::prelude::window;
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        AbortController, ReadableStreamDefaultReader, Request, RequestCache, RequestCredentials,
        RequestInit, RequestMode, RequestRedirect, Response,
    };
    pub async fn run(
        operation: Operation,
        abort: &AbortController,
    ) -> Result<tabula_match_http::MatchAdmission, &'static str> {
        let context = fetch("/api/v1/auth/context", None, None, abort).await?;
        let context: tabula_session_http::ContextResponse =
            serde_json::from_slice(&context).map_err(|_| "online.unavailable")?;
        context
            .validate_for_browser()
            .map_err(|_| "online.unavailable")?;
        if context.disposition != tabula_session_http::SessionDisposition::Authenticated {
            return Err("online.signin");
        }
        let csrf = context.csrf_token.ok_or("online.signin")?;
        let (path, body) = match operation {
            Operation::Create(id) => {
                let (_, catalog) = shell(tabula_registry::Locale::En);
                let id = GameId::new(id).map_err(|_| "online.incompatible")?;
                let game = catalog.get(&id).ok_or("online.incompatible")?.game();
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
            Operation::Join(code) => {
                let request = tabula_match_http::MatchJoinRequest::new(code)
                    .map_err(|_| "online.invalid_code")?;
                (
                    "/api/v1/matches/join",
                    serde_json::to_string(&request).map_err(|_| "online.unavailable")?,
                )
            }
        };
        serde_json::from_slice(&fetch(path, Some(&body), Some(&csrf), abort).await?)
            .map_err(|_| "online.unavailable")
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
    ) -> Result<Vec<u8>, &'static str> {
        let window = window();
        if window.location().protocol().ok().as_deref() != Some("https:") {
            return Err("online.unavailable");
        }
        let cancel = abort.clone();
        let callback = Closure::<dyn FnMut()>::new(move || cancel.abort());
        let handle = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                20_000,
            )
            .map_err(|_| "online.unavailable")?;
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
        let request =
            Request::new_with_str_and_init(path, &init).map_err(|_| "online.unavailable")?;
        request
            .headers()
            .set("Accept", "application/json")
            .map_err(|_| "online.unavailable")?;
        if let Some(csrf) = csrf {
            request
                .headers()
                .set("Content-Type", "application/json")
                .map_err(|_| "online.unavailable")?;
            request
                .headers()
                .set("X-Tabula-CSRF", csrf)
                .map_err(|_| "online.unavailable")?;
        }
        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| "online.disconnected")?
            .dyn_into::<Response>()
            .map_err(|_| "online.unavailable")?;
        if let Some(key) =
            super::response_error_key(response.status(), path == "/api/v1/matches/join")
        {
            return Err(key);
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
            return Err("online.unavailable");
        }
        read_body(&response, abort).await
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
    use super::response_error_key;

    #[test]
    fn join_denial_is_code_unavailable_without_misclassifying_authentication() {
        assert_eq!(response_error_key(403, true), Some("online.invalid_code"));
        assert_eq!(response_error_key(401, true), Some("online.signin"));
        assert_eq!(response_error_key(401, false), Some("online.signin"));
        assert_eq!(response_error_key(403, false), Some("online.signin"));
        for status in [404, 409] {
            assert_eq!(
                response_error_key(status, true),
                Some("online.invalid_code")
            );
        }
        assert_eq!(response_error_key(500, true), Some("online.unavailable"));
        assert_eq!(response_error_key(200, true), None);
        assert_eq!(response_error_key(201, false), None);
    }
}
