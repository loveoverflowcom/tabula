//! Native v2 enrollment and permitted profile documents (ADR-0043).
//! Drafts and pending writes are document-local; DTOs never establish authority.
#![allow(clippy::too_many_lines)] // Leptos expands each native form/list route into one lifecycle-bound function.

use leptos::{html, prelude::*};
use leptos_router::{components::A, hooks::use_params_map};

#[cfg(target_arch = "wasm32")]
use super::GuardedShow as Show;
use tabula_session_http::accounts::social::{
    FriendRequestStatus, PresenceObservation, SocialAction, SocialMutation, SocialMutationResponse,
    SocialRelationship, SocialSearchResponse,
};
use tabula_session_http::accounts::{
    self, EnrollmentContextResponse, EnrollmentDisposition, EnrollmentStartResponse,
    OtherAccountProfileResponse, ProfileUpdateRequest, ProfileVisibility, RegistrationDisposition,
    RegistrationRequest, RegistrationResponse, SelfAccountProfileResponse,
};

use crate::{
    account::{use_account, AccountController},
    accounts_full::{operation_id, RequestScope, RequestSlot, ResourcePhase},
    i18n::Messages,
    views::{parts::translated, use_locale},
};

fn focus_field(id: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = id;
    #[cfg(target_arch = "wasm32")]
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(id))
        .and_then(|element| wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlElement>(element).ok())
    {
        let _ = element.focus();
    }
}

fn focus_heading() {
    focus_field("account-title");
}

#[component]
fn FullStatus(phase: ReadSignal<ResourcePhase>) -> impl IntoView {
    let locale = use_locale();
    view! {
        <div class="account__state" aria-busy=super::static_text(Signal::derive(move || if phase.try_get().is_some_and(ResourcePhase::busy) { "true" } else { "false" }), "false")>
            <p role="status" class="status" aria-atomic="true">
                {super::text(Signal::derive(move || Messages::new(locale.get()).text(phase.try_get().unwrap_or(ResourcePhase::Unavailable).key())), "")}
            </p>
            <Show when=move || phase.try_get().is_some_and(|phase| matches!(phase, ResourcePhase::Error | ResourcePhase::Unknown | ResourcePhase::Conflict))>
                <p class="banner banner--error" role="alert">{translated("accounts.full.safe_error")}</p>
            </Show>
        </div>
    }
}

#[component]
fn Escapes(principal: impl Fn() -> bool + Copy + Send + Sync + 'static) -> impl IntoView {
    view! { <aside class="account__local"><p>{translated("accounts.local.explanation")}</p>
    <div class="actions account__actions"><A href="/games" attr:class=super::static_text(Signal::derive(move || if principal() { "btn btn--filled btn--principal" } else { "btn btn--tonal" }), "btn btn--tonal")>{translated("accounts.action.library")}</A>
        <A href="/account" attr:class="btn btn--text">{translated("accounts.action.back_account")}</A>
    </div></aside> }
}

/// Provider-proven enrollment; this document never collects provider credentials.
#[allow(clippy::too_many_lines)] // One route binds its native form and lifecycle.
#[component]
pub fn RegisterPage() -> impl IntoView {
    let account = use_account();
    let slot = RequestSlot::new();
    let phase = RwSignal::new(ResourcePhase::Checking);
    let grant = RwSignal::new(None::<EnrollmentContextResponse>);
    let accepted = RwSignal::new(false);
    let rejected = RwSignal::new(false);
    let handle = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let invalid = RwSignal::new(false);
    let composing = RwSignal::new(false);
    let last_generation = StoredValue::new(None::<u64>);
    let submitted = StoredValue::new(None::<RegistrationRequest>);
    let heading = NodeRef::<html::H1>::new();
    super::account::focus_on_arrival(heading);
    let read = move || {
        let Some(generation) = account.public_generation() else {
            return;
        };
        phase.try_set(ResourcePhase::Checking);
        slot.run(
            account,
            RequestScope::Enrollment(generation),
            "GET",
            "/api/v2/auth/enrollment".into(),
            None,
            None,
            move |result| {
                let context = result
                    .ok()
                    .filter(|response| matches!(response.status, 200 | 503))
                    .and_then(|response| {
                        crate::json::decode::<EnrollmentContextResponse>(&response.body).ok()
                    })
                    .filter(|context| context.validate().is_ok());
                match context {
                    Some(context) if context.disposition == EnrollmentDisposition::Ready => {
                        let old_operation = submitted
                            .try_with_value(|value| {
                                value.as_ref().map(|request| request.operation_id.clone())
                            })
                            .flatten();
                        if old_operation.is_some() && old_operation != context.operation_id {
                            grant.try_set(None);
                            phase.try_set(ResourcePhase::Unknown);
                        } else {
                            grant.try_set(Some(context));
                            phase.try_set(ResourcePhase::Ready);
                        }
                    }
                    Some(context) => {
                        grant.try_set(None);
                        submitted.try_set_value(None);
                        accepted.try_set(
                            context.disposition == EnrollmentDisposition::AcceptedWithoutSession,
                        );
                        rejected.try_set(context.disposition == EnrollmentDisposition::Rejected);
                        phase.try_set(
                            if context.disposition == EnrollmentDisposition::AcceptedWithoutSession
                            {
                                ResourcePhase::Ready
                            } else {
                                ResourcePhase::Unavailable
                            },
                        );
                    }
                    None => {
                        grant.try_set(None);
                        phase.try_set(ResourcePhase::Error);
                    }
                }
            },
        );
    };
    Effect::new(move |_| {
        let _ = account.state.try_get();
        let generation = account.public_generation();
        if last_generation
            .try_with_value(|old| *old == generation)
            .unwrap_or(true)
        {
            return;
        }
        last_generation.try_set_value(generation);
        slot.retire();
        grant.try_set(None);
        if generation.is_some() && account.enrollment_navigation_ticket().is_some() {
            read();
        } else {
            phase.try_set(ResourcePhase::Unavailable);
        }
    });
    let submit = move || {
        if composing.try_get().unwrap_or(true) || phase.try_get().is_none_or(ResourcePhase::busy) {
            return;
        }
        let Some(context) = grant.try_get().flatten() else {
            return;
        };
        let Some(generation) = account.public_generation() else {
            return;
        };
        let request = RegistrationRequest {
            operation_id: context.operation_id.unwrap_or_default(),
            handle: handle.try_get().unwrap_or_default(),
            display_name: name.try_get().unwrap_or_default().trim().to_owned(),
        };
        if request.validate().is_err() {
            invalid.try_set(true);
            focus_field(if accounts::valid_handle(&request.handle) {
                "register-display-name"
            } else {
                "register-handle"
            });
            return;
        }
        if submitted
            .try_with_value(|old| old.as_ref().is_some_and(|old| old != &request))
            .unwrap_or(true)
        {
            phase.try_set(ResourcePhase::Unknown);
            return;
        }
        submitted.try_set_value(Some(request.clone()));
        invalid.try_set(false);
        phase.try_set(ResourcePhase::Pending);
        focus_heading();
        slot.run(
            account,
            RequestScope::Enrollment(generation),
            "POST",
            "/api/v2/auth/register".into(),
            context.csrf_token,
            serde_json::to_string(&request).ok(),
            move |result| {
                if result
                    .as_ref()
                    .is_ok_and(super::super::accounts_full::JsonResponse::rejected)
                {
                    submitted.try_set_value(None);
                    grant.try_set(None);
                    rejected.try_set(true);
                    phase.try_set(ResourcePhase::Unavailable);
                    return;
                }
                let response = result
                    .ok()
                    .filter(|response| response.status == 200)
                    .and_then(|response| {
                        crate::json::decode::<RegistrationResponse>(&response.body).ok()
                    })
                    .filter(|response| response.validate().is_ok());
                if let Some(response) = response {
                    accepted.try_set(
                        response.disposition == RegistrationDisposition::AcceptedWithoutSession,
                    );
                    rejected.try_set(response.disposition == RegistrationDisposition::Rejected);
                    grant.try_set(None);
                    submitted.try_set_value(None);
                    phase.try_set(
                        if response.disposition == RegistrationDisposition::AcceptedWithoutSession {
                            ResourcePhase::Ready
                        } else {
                            ResourcePhase::Unavailable
                        },
                    );
                } else {
                    phase.try_set(ResourcePhase::Unknown);
                }
            },
        );
    };
    view! { <section class="section account" aria-labelledby="account-title">
        <h1 id="account-title" class="section__title" tabindex="-1" node_ref=heading>{translated("accounts.register.title")}</h1>
        <p class="section__body">{translated("accounts.full.enrollment_intro")}</p>
        <FullStatus phase=phase.read_only()/>
        <Show when=move || accepted.try_get().unwrap_or(false)>
            <p class="banner" role="status" data-testid="registration-accepted">{translated("accounts.full.accepted_without_session")}</p>
        </Show>
        <Show when=move || rejected.try_get().unwrap_or(false)><p role="alert" class="banner banner--error">{translated("accounts.full.register_rejected")}</p></Show>
        <Show when=move || grant.try_get().flatten().is_some()>
            <form class="account__form" data-account-private="" novalidate on:submit=move |event| { event.prevent_default(); submit(); }
                on:compositionstart=move |_| { composing.try_set(true); } on:compositionend=move |_| { composing.try_set(false); }>
                <Show when=move || invalid.try_get().unwrap_or(false)><p role="alert" class="field__error" id="registration-error">{translated("accounts.full.fields_invalid")}</p></Show>
                <label class="field" for="register-handle"><span class="field__label">{translated("accounts.full.handle")}</span>
                    <input id="register-handle" name="handle" type="text" required=true autocomplete="username" autocapitalize="none" spellcheck="false" class="field__control"
                        minlength="3" maxlength="32" aria-describedby="handle-help registration-error" aria-invalid=super::optional_text(Signal::derive(move || invalid.try_get().unwrap_or(false).then(|| "true".to_owned())))
                        disabled=super::boolean(Signal::derive(move || phase.try_get().is_some_and(|phase| phase.busy() || phase == ResourcePhase::Unknown)), true) prop:value=super::text(Signal::derive(move || handle.try_get().unwrap_or_default()), "")
                        on:input=move |event| { handle.try_set(event_target_value(&event)); }/>
                    <span class="field__hint" id="handle-help">{translated("accounts.full.handle_help")}</span>
                </label>
                <label class="field" for="register-display-name"><span class="field__label">{translated("accounts.full.display_name")}</span>
                    <input id="register-display-name" name="display_name" type="text" required=true autocomplete="nickname" class="field__control" maxlength="256"
                        aria-describedby="name-help registration-error" aria-invalid=super::optional_text(Signal::derive(move || invalid.try_get().unwrap_or(false).then(|| "true".to_owned())))
                        disabled=super::boolean(Signal::derive(move || phase.try_get().is_some_and(|phase| phase.busy() || phase == ResourcePhase::Unknown)), true) prop:value=super::text(Signal::derive(move || name.try_get().unwrap_or_default()), "")
                        on:input=move |event| { name.try_set(event_target_value(&event)); }/>
                    <span class="field__hint" id="name-help">{translated("accounts.full.name_help")}</span>
                </label>
                <button type="submit" data-testid="register-submit" class="btn btn--filled btn--principal"
                    disabled=super::boolean(Signal::derive(move || phase.try_get().is_some_and(ResourcePhase::busy)), true)>{translated("accounts.register.title")}</button>
            </form>
        </Show>
        <Show when=move || grant.try_get().flatten().is_none() && !accepted.try_get().unwrap_or(false)
            && !phase.try_get().is_some_and(|phase| matches!(phase, ResourcePhase::Unknown | ResourcePhase::Pending | ResourcePhase::Checking))
            && account.enrollment_navigation_ticket().is_some()>
            <button type="button" data-testid="enrollment-start" class="btn btn--filled btn--principal" on:click=move |_| {
                let Some(ticket) = account.enrollment_navigation_ticket() else { return; };
                let Some(generation) = account.public_generation() else { return; };
                let guard = ticket.clone();
                phase.try_set(ResourcePhase::Pending); focus_heading();
                slot.run(account, RequestScope::Enrollment(generation), "POST", "/api/v2/auth/enrollment/start".into(), Some(ticket.csrf_token().to_owned()), Some("{}".into()), move |result| {
                    let target = result.ok().filter(|response| response.status == 200)
                        .and_then(|response| crate::json::decode::<EnrollmentStartResponse>(&response.body).ok())
                        .filter(|response| response.validate().is_ok());
                    if let Some(target) = target {
                        #[cfg(target_arch = "wasm32")]
                        if account.enrollment_navigation_current(&guard) {
                            if let Some(window) = web_sys::window() {
                                if window.top().ok().flatten().is_some_and(|top| js_sys::Object::is(top.as_ref(), window.as_ref()))
                                    && window.location().assign(&target.authorization_url).is_ok() { return; }
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        let _ = (target, guard);
                    }
                    phase.try_set(ResourcePhase::Error);
                });
            }>{translated("accounts.full.enrollment_start")}</button>
        </Show>
        <Show when=move || phase.try_get() == Some(ResourcePhase::Unknown)>
            <button type="button" class="btn btn--tonal" on:click=move |_| read()>{translated("accounts.full.reconcile")}</button>
        </Show>
        <A href="/login" attr:class=super::static_text(Signal::derive(move || if accepted.try_get().unwrap_or(false) { "btn btn--filled btn--principal" } else { "btn btn--tonal" }), "btn btn--tonal")>{translated("accounts.action.signin")}</A>
        <Escapes principal=move || !accepted.try_get().unwrap_or(false) && grant.try_get().flatten().is_none() && account.enrollment_navigation_ticket().is_none()/>
    </section> }
}

/// Self-only metadata editing shares the v1 route's document authority owner.
#[allow(clippy::too_many_lines)] // One self-only draft and request lifecycle.
#[component]
pub fn ProfileControls(account: AccountController, verified: RwSignal<bool>) -> impl IntoView {
    let slot = RequestSlot::new();
    let locale = use_locale();
    let profile = RwSignal::new(None::<SelfAccountProfileResponse>);
    Effect::new(move |_| {
        verified
            .try_set(account.document_ticket().is_some() && profile.try_get().flatten().is_some());
    });
    on_cleanup(move || {
        verified.try_set(false);
    });
    let phase = RwSignal::new(ResourcePhase::Unavailable);
    let editing = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let visibility = RwSignal::new(ProfileVisibility::Private);
    let composing = RwSignal::new(false);
    let invalid = RwSignal::new(false);
    let missing = RwSignal::new(false);
    let previous = StoredValue::new(None::<crate::account::DocumentAccountTicket>);
    let pending = StoredValue::new(None::<ProfileUpdateRequest>);
    let write_acknowledged = RwSignal::new(false);
    let last_subject = StoredValue::new(None::<String>);
    let reload = move |saved: bool| {
        let Some(ticket) = account.document_ticket() else {
            return;
        };
        let expected = ticket.subject().to_owned();
        phase.try_set(ResourcePhase::Checking);
        slot.run(
            account,
            RequestScope::Viewer(ticket),
            "GET",
            "/api/v2/profiles/me".into(),
            None,
            None,
            move |result| {
                let absent = result
                    .as_ref()
                    .is_ok_and(|response| response.status == 404 && response.rejected());
                let current = result
                    .ok()
                    .filter(|response| response.status == 200)
                    .and_then(|response| {
                        crate::json::decode::<SelfAccountProfileResponse>(&response.body).ok()
                    })
                    .filter(|response| response.validate().is_ok())
                    .filter(|response| response.account_id == expected);
                if let Some(current) = current {
                    if !editing.try_get().unwrap_or(false) {
                        name.try_set(current.display_name.clone());
                        visibility.try_set(current.visibility);
                    }
                    missing.try_set(false);
                    profile.try_set(Some(current));
                    if saved {
                        pending.try_set_value(None);
                        editing.try_set(false);
                        write_acknowledged.try_set(false);
                    }
                    phase.try_set(if saved {
                        ResourcePhase::Saved
                    } else if pending.try_with_value(Option::is_some).unwrap_or(false) {
                        ResourcePhase::Unknown
                    } else {
                        ResourcePhase::Ready
                    });
                } else {
                    profile.try_set(None);
                    missing.try_set(absent);
                    phase.try_set(if saved {
                        ResourcePhase::Unknown
                    } else if absent {
                        ResourcePhase::Unavailable
                    } else {
                        ResourcePhase::Error
                    });
                }
            },
        );
    };
    Effect::new(move |_| {
        let _ = account.state.try_get();
        let ticket = account.document_ticket();
        if previous
            .try_with_value(|old| *old == ticket)
            .unwrap_or(true)
        {
            return;
        }
        previous.try_set_value(ticket.clone());
        slot.retire();
        profile.try_set(None);
        editing.try_set(false);
        name.try_set(String::new());
        if let Some(ticket) = ticket {
            if last_subject
                .try_with_value(|old| {
                    old.as_deref()
                        .is_some_and(|subject| subject != ticket.subject())
                })
                .unwrap_or(true)
            {
                pending.try_set_value(None);
                write_acknowledged.try_set(false);
            }
            last_subject.try_set_value(Some(ticket.subject().to_owned()));
            reload(write_acknowledged.try_get().unwrap_or(false));
        } else {
            phase.try_set(ResourcePhase::Unavailable);
        }
    });
    let save = move || {
        if composing.try_get().unwrap_or(true) || phase.try_get().is_none_or(ResourcePhase::busy) {
            return;
        }
        let Some(ticket) = account.document_ticket() else {
            return;
        };
        let request = pending.try_with_value(Clone::clone).flatten().or_else(|| {
            Some(ProfileUpdateRequest {
                operation_id: operation_id()?,
                expected_revision: profile.try_get().flatten()?.revision,
                display_name: name.try_get()?.trim().to_owned(),
                visibility: visibility.try_get()?,
            })
        });
        let Some(request) = request else {
            phase.try_set(ResourcePhase::Error);
            return;
        };
        if request.validate().is_err() {
            invalid.try_set(true);
            focus_field("profile-display-name");
            return;
        }
        pending.try_set_value(Some(request.clone()));
        invalid.try_set(false);
        phase.try_set(ResourcePhase::Pending);
        focus_heading();
        let expected = ticket.subject().to_owned();
        slot.run(
            account,
            RequestScope::Viewer(ticket.clone()),
            "PATCH",
            "/api/v2/profiles/me".into(),
            Some(ticket.csrf_token().to_owned()),
            serde_json::to_string(&request).ok(),
            move |result| match result {
                Ok(response) if response.status == 204 => {
                    write_acknowledged.try_set(true);
                    reload(true);
                }
                Ok(response) if response.conflict() => {
                    pending.try_set_value(None);
                    phase.try_set(ResourcePhase::Conflict);
                }
                Ok(response) if response.rejected() => {
                    pending.try_set_value(None);
                    phase.try_set(ResourcePhase::Error);
                }
                Ok(response) if response.status == 200 => {
                    let current = crate::json::decode::<SelfAccountProfileResponse>(&response.body)
                        .ok()
                        .filter(|current| current.validate().is_ok())
                        .filter(|current| current.account_id == expected);
                    if let Some(current) = current {
                        profile.try_set(Some(current));
                        pending.try_set_value(None);
                        editing.try_set(false);
                        phase.try_set(ResourcePhase::Saved);
                    } else {
                        phase.try_set(ResourcePhase::Unknown);
                    }
                }
                _ => {
                    phase.try_set(ResourcePhase::Unknown);
                }
            },
        );
    };
    view! { <Show when=move || account.document_ticket().is_some()><section class="account__details" data-account-private="">
        <Show when=move || account.document_ticket().is_some() && profile.try_get().flatten().is_none() && missing.try_get().unwrap_or(false)>
            <p class="section__body">{translated("accounts.full.missing_profile")}</p>
            <A href="/register" attr:class="btn btn--text">{translated("accounts.register.title")}</A>
        </Show>
        <Show when=move || profile.try_get().flatten().is_some()>
            <h2 class="section__subtitle">{translated("accounts.full.profile_details")}</h2>
            <dl class="facts"><dt>{translated("accounts.full.handle")}</dt><dd>{super::optional_text(Signal::derive(move || profile.try_get().flatten().map(|profile| profile.handle)))}</dd>
                <dt>{translated("accounts.full.display_name")}</dt><dd>{super::optional_text(Signal::derive(move || profile.try_get().flatten().map(|profile| profile.display_name)))}</dd>
                <dt>{translated("accounts.full.visibility")}</dt><dd>{super::optional_text(Signal::derive(move || profile.try_get().flatten().map(|profile| Messages::new(locale.get()).text(visibility_key(profile.visibility)))))}</dd>
            </dl>
            <Show when=move || !editing.try_get().unwrap_or(false)>
                <button type="button" data-testid="profile-edit" class="btn btn--filled btn--principal" on:click=move |_| {
                    if let Some(current) = profile.try_get().flatten() { name.try_set(current.display_name); visibility.try_set(current.visibility); editing.try_set(true); }
                }>{translated("accounts.full.edit")}</button>
            </Show>
            <Show when=move || editing.try_get().unwrap_or(false)>
                <form class="account__form" novalidate on:submit=move |event| { event.prevent_default(); save(); }
                    on:compositionstart=move |_| { composing.try_set(true); } on:compositionend=move |_| { composing.try_set(false); }>
                    <label class="field" for="profile-display-name"><span class="field__label">{translated("accounts.full.display_name")}</span>
                        <input id="profile-display-name" name="display_name" autocomplete="nickname" type="text" required=true class="field__control" maxlength="256"
                            aria-describedby="profile-name-help profile-name-error" aria-invalid=super::optional_text(Signal::derive(move || invalid.try_get().unwrap_or(false).then(|| "true".to_owned())))
                            disabled=super::boolean(Signal::derive(move || phase.try_get().is_some_and(|phase| phase.busy() || phase == ResourcePhase::Unknown)), true)
                            prop:value=super::text(Signal::derive(move || name.try_get().unwrap_or_default()), "") on:input=move |event| { name.try_set(event_target_value(&event)); }/>
                        <span class="field__hint" id="profile-name-help">{translated("accounts.full.name_help")}</span>
                    </label>
                    <Show when=move || invalid.try_get().unwrap_or(false)><p id="profile-name-error" role="alert" class="field__error">{translated("accounts.full.fields_invalid")}</p></Show>
                    <label class="field" for="profile-visibility"><span class="field__label">{translated("accounts.full.visibility")}</span>
                        <select id="profile-visibility" name="visibility" class="field__control" disabled=super::boolean(Signal::derive(move || phase.try_get().is_some_and(|phase| phase.busy() || phase == ResourcePhase::Unknown)), true)
                            on:change=move |event| { visibility.try_set(match event_target_value(&event).as_str() { "public" => ProfileVisibility::Public, "friends" => ProfileVisibility::Friends, _ => ProfileVisibility::Private }); }>
                            <option value="private" selected=super::boolean(Signal::derive(move || visibility.try_get() == Some(ProfileVisibility::Private)), false)>{translated("accounts.full.visibility.private")}</option>
                            <option value="friends" selected=super::boolean(Signal::derive(move || visibility.try_get() == Some(ProfileVisibility::Friends)), false)>{translated("accounts.full.visibility.friends")}</option>
                            <option value="public" selected=super::boolean(Signal::derive(move || visibility.try_get() == Some(ProfileVisibility::Public)), false)>{translated("accounts.full.visibility.public")}</option>
                        </select>
                    </label>
                    <div class="actions account__actions"><button type="submit" data-testid="profile-save" class="btn btn--filled btn--principal"
                        disabled=super::boolean(Signal::derive(move || phase.try_get().is_some_and(|phase| phase.busy() || phase == ResourcePhase::Conflict)), true)>{translated("accounts.full.save")}</button>
                        <button type="button" class="btn btn--text" disabled=super::boolean(Signal::derive(move || phase.try_get().is_some_and(|phase| phase.busy() || phase == ResourcePhase::Unknown)), true)
                            on:click=move |_| { editing.try_set(false); invalid.try_set(false); focus_heading(); }>{translated("accounts.action.cancel")}</button></div>
                </form>
            </Show>

        </Show>
        <FullStatus phase=phase.read_only()/>
            <Show when=move || phase.try_get() == Some(ResourcePhase::Conflict)>
                <button type="button" class="btn btn--tonal" on:click=move |_| reload(false)>{translated("accounts.full.refetch")}</button>
            </Show>
            <Show when=move || phase.try_get() == Some(ResourcePhase::Unknown)>
                <button type="button" class="btn btn--tonal" on:click=move |_| if write_acknowledged.try_get().unwrap_or(false) { reload(true); } else { save(); }>{translated("accounts.full.retry_same")}</button>
            </Show>
        <Show when=move || phase.try_get() == Some(ResourcePhase::Error)>
            <button type="button" class="btn btn--tonal" on:click=move |_| reload(false)>{translated("accounts.full.refetch")}</button>
        </Show>
    </section></Show> }
}

fn visibility_key(visibility: ProfileVisibility) -> &'static str {
    match visibility {
        ProfileVisibility::Public => "accounts.full.visibility.public",
        ProfileVisibility::Friends => "accounts.full.visibility.friends",
        ProfileVisibility::Private => "accounts.full.visibility.private",
    }
}

/// Participant-authorized directory and requests, with one shell-owned stream.
#[allow(clippy::too_many_lines)] // One participant-scoped native search/list task.
#[component]
pub fn FriendsPage() -> impl IntoView {
    let account = use_account();
    let owner = crate::social_full::use_social();
    let search_slot = RequestSlot::new();
    let write_slot = RequestSlot::new();
    let phase = RwSignal::new(ResourcePhase::Unavailable);
    let query = RwSignal::new(String::new());
    let results = RwSignal::new(None::<SocialSearchResponse>);
    let composing = RwSignal::new(false);
    let invalid = RwSignal::new(false);
    let previous = StoredValue::new(None::<crate::account::DocumentAccountTicket>);
    let pending = StoredValue::new(None::<SocialMutation>);
    let pending_subject = StoredValue::new(None::<String>);
    let heading = NodeRef::<html::H1>::new();
    super::account::focus_on_arrival(heading);
    Effect::new(move |_| {
        let _ = account.state.try_get();
        let current = account
            .document_ticket()
            .filter(|_| account.social_available());
        if previous
            .try_with_value(|old| *old == current)
            .unwrap_or(true)
        {
            return;
        }
        previous.try_set_value(current.clone());
        search_slot.retire();
        write_slot.retire();
        results.try_set(None);
        query.try_set(String::new());
        if let Some(current) = current {
            if pending_subject
                .try_with_value(|subject| {
                    subject
                        .as_deref()
                        .is_some_and(|subject| subject != current.subject())
                })
                .unwrap_or(true)
            {
                pending.try_set_value(None);
            }
            phase.try_set(
                if pending.try_with_value(Option::is_some).unwrap_or(false) {
                    ResourcePhase::Unknown
                } else {
                    ResourcePhase::Ready
                },
            );
        } else {
            phase.try_set(ResourcePhase::Unavailable);
        }
    });
    let search = move || {
        if composing.try_get().unwrap_or(true)
            || phase
                .try_get()
                .is_none_or(|phase| phase.busy() || phase == ResourcePhase::Unknown)
        {
            return;
        }
        let Some(ticket) = account
            .document_ticket()
            .filter(|_| account.social_available())
        else {
            return;
        };
        let requested = query.try_get().unwrap_or_default().trim().to_owned();
        if !requested.is_empty()
            && (requested.len() > 32
                || !requested
                    .bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'_'))
        {
            invalid.try_set(true);
            focus_field("friends-query");
            return;
        }
        let expected = (ticket.subject().to_owned(), requested.clone());
        invalid.try_set(false);
        phase.try_set(ResourcePhase::Checking);
        results.try_set(None);
        search_slot.run(
            account,
            RequestScope::Viewer(ticket),
            "GET",
            format!("/api/v2/social/search?q={requested}"),
            None,
            None,
            move |result| {
                let response = result
                    .ok()
                    .filter(|response| response.status == 200)
                    .and_then(|response| {
                        crate::json::decode::<SocialSearchResponse>(&response.body).ok()
                    })
                    .filter(|response| response.validate().is_ok())
                    .filter(|response| {
                        response.viewer_id == expected.0 && response.query == expected.1
                    });
                if let Some(response) = response {
                    results.try_set(Some(response));
                    phase.try_set(ResourcePhase::Ready);
                } else {
                    results.try_set(None);
                    phase.try_set(ResourcePhase::Error);
                }
            },
        );
    };
    let mutate = move |action: SocialAction| {
        if phase.try_get().is_none_or(ResourcePhase::busy) {
            return;
        }
        let Some(ticket) = account
            .document_ticket()
            .filter(|_| account.social_available())
        else {
            return;
        };
        let request = pending.try_with_value(Clone::clone).flatten().or_else(|| {
            Some(SocialMutation {
                operation_id: operation_id()?,
                action: action.clone(),
            })
        });
        let Some(request) =
            request.filter(|request| request.action == action && request.validate().is_ok())
        else {
            return;
        };
        pending.try_set_value(Some(request.clone()));
        pending_subject.try_set_value(Some(ticket.subject().to_owned()));
        let expected = (ticket.subject().to_owned(), request.operation_id.clone());
        let expected_action = request.action.clone();
        phase.try_set(ResourcePhase::Pending);
        focus_heading();
        write_slot.run(
            account,
            RequestScope::Viewer(ticket.clone()),
            "POST",
            "/api/v2/social/mutate".into(),
            Some(ticket.csrf_token().to_owned()),
            serde_json::to_string(&request).ok(),
            move |result| {
                if result
                    .as_ref()
                    .is_ok_and(super::super::accounts_full::JsonResponse::rejected)
                {
                    let conflict = result.as_ref().is_ok_and(|response| response.status == 409);
                    pending.try_set_value(None);
                    results.try_set(None);
                    phase.try_set(if conflict {
                        ResourcePhase::Conflict
                    } else {
                        ResourcePhase::Error
                    });
                    owner.resync();
                    return;
                }
                let response = result
                    .ok()
                    .filter(|response| response.status == 200)
                    .and_then(|response| {
                        crate::json::decode::<SocialMutationResponse>(&response.body).ok()
                    })
                    .filter(|response| response.validate().is_ok())
                    .filter(|response| {
                        response.viewer_id == expected.0
                            && response.operation_id == expected.1
                            && match &expected_action {
                                SocialAction::Send { target_user_id } => {
                                    response.request.sender.user_id == expected.0
                                        && response.request.recipient.user_id == *target_user_id
                                }
                                SocialAction::Accept { request_id, .. }
                                | SocialAction::Decline { request_id, .. }
                                | SocialAction::Cancel { request_id, .. } => {
                                    response.request.request_id == *request_id
                                }
                            }
                    });
                if response.is_some() {
                    pending.try_set_value(None);
                    results.try_set(None);
                    phase.try_set(ResourcePhase::Saved);
                    owner.resync();
                } else {
                    phase.try_set(ResourcePhase::Unknown);
                }
            },
        );
    };
    let disabled = super::boolean(
        Signal::derive(move || {
            phase
                .try_get()
                .is_none_or(|phase| phase.busy() || phase == ResourcePhase::Unknown)
        }),
        true,
    );
    let locale = use_locale();
    view! { <section class="section account" aria-labelledby="account-title">
        <h1 id="account-title" class="section__title" tabindex="-1" node_ref=heading>{translated("accounts.friends.title")}</h1>
        <p class="section__body">{translated("accounts.full.friends_intro")}</p><FullStatus phase=phase.read_only()/>
        <Show when=move || account.social_available()>
            <section data-account-private="">
                <form class="account__form" novalidate on:submit=move |event| { event.prevent_default(); search(); }
                    on:compositionstart=move |_| { composing.try_set(true); } on:compositionend=move |_| { composing.try_set(false); }>
                    <label class="field" for="friends-query"><span class="field__label">{translated("accounts.full.handle")}</span>
                        <input id="friends-query" name="query" type="search" autocomplete="off" autocapitalize="none" spellcheck="false" maxlength="32" class="field__control"
                            aria-describedby="friends-search-help friends-search-error" aria-invalid=super::optional_text(Signal::derive(move || invalid.try_get().unwrap_or(false).then(|| "true".to_owned())))
                            disabled=disabled prop:value=super::text(Signal::derive(move || query.try_get().unwrap_or_default()), "") on:input=move |event| { query.try_set(event_target_value(&event)); }/>
                        <span id="friends-search-help" class="field__hint">{translated("accounts.full.search_help")}</span>
                    </label>
                    <Show when=move || invalid.try_get().unwrap_or(false)><p id="friends-search-error" class="field__error" role="alert">{translated("accounts.full.search_help")}</p></Show>
                    <button type="submit" data-testid="friends-search" class="btn btn--filled btn--principal" disabled=disabled>{translated("accounts.full.search")}</button>
                </form>
                <Show when=move || results.try_get().flatten().is_some_and(|response| response.results.is_empty())><p role="status">{translated("accounts.full.empty")}</p></Show>
                <ul class="account__social-list" aria-label=translated("accounts.full.search")>
                    <For each=move || results.try_get().flatten().map(|result| result.results.into_iter().map(|row| row.identity.user_id).collect::<Vec<_>>()).unwrap_or_default()
                        key=|id| id.clone() children=move |id| {
                            let id = StoredValue::new(id);
                            let row = move || results.try_get().flatten().and_then(|result| result.results.into_iter().find(|row| id.try_with_value(|id| *id == row.identity.user_id).unwrap_or(false)));
                            view! { <li class="account__social-row" data-user-id=super::text(Signal::derive(move || id.try_get_value().unwrap_or_default()), "")>
                                <A href=super::text(Signal::derive(move || row().map_or_else(|| "/account".into(), |row| format!("/u/{}", row.identity.handle))), "/account")>{super::optional_text(Signal::derive(move || row().map(|row| row.identity.handle)))}</A>
                                <p>{super::optional_text(Signal::derive(move || row().and_then(|row| row.identity.display_name)))}</p>
                                <p class="status">{super::optional_text(Signal::derive(move || row().map(|row| Messages::new(locale.get()).text(relationship_key(row.relationship)))))}</p>
                                <Show when=move || row().is_some_and(|row| matches!(row.relationship, SocialRelationship::None | SocialRelationship::Declined | SocialRelationship::Expired | SocialRelationship::Cancelled))>
                                    <button type="button" data-testid="friend-send" class="btn btn--tonal" disabled=disabled on:click=move |_| {
                                        if let Some(row) = row() { mutate(SocialAction::Send { target_user_id: row.identity.user_id }); }
                                    }>{translated("accounts.full.send")}</button>
                                </Show>
                            </li> }
                        }/>
                </ul>
                <h2 class="section__subtitle">{translated("accounts.full.friends_list")}</h2>
                <Show when=move || owner.stale()><p class="status" role="status">{translated("accounts.full.presence.stale")}</p></Show>
                <Show when=move || owner.current(account).is_some_and(|snapshot| snapshot.friends.is_empty())><p>{translated("accounts.full.empty")}</p></Show>
                <ul class="account__social-list">
                    <For each=move || owner.current(account).map(|snapshot| snapshot.friends.into_iter().map(|friend| friend.identity.user_id).collect::<Vec<_>>()).unwrap_or_default()
                        key=|id| id.clone() children=move |id| {
                            let id = StoredValue::new(id);
                            let friend = move || owner.current(account).and_then(|snapshot| snapshot.friends.into_iter().find(|friend| id.try_with_value(|id| *id == friend.identity.user_id).unwrap_or(false)));
                            view! { <li class="account__social-row" data-social-stream="" data-user-id=super::text(Signal::derive(move || id.try_get_value().unwrap_or_default()), "")>
                                <A href=super::text(Signal::derive(move || friend().map_or_else(|| "/account".into(), |friend| format!("/u/{}", friend.identity.handle))), "/account")>{super::optional_text(Signal::derive(move || friend().map(|friend| friend.identity.handle)))}</A>
                                <p>{super::optional_text(Signal::derive(move || friend().and_then(|friend| friend.identity.display_name)))}</p>
                                <p class="status">{super::optional_text(Signal::derive(move || friend().map(|friend| Messages::new(locale.get()).text(presence_key(&friend.presence)))))}</p>
                                <PresenceTimes presence=move || friend().map(|friend| friend.presence)/>
                            </li> }
                        }/>
                </ul>
                <h2 class="section__subtitle">{translated("accounts.full.requests")}</h2>
                <Show when=move || owner.current(account).is_some_and(|snapshot| snapshot.requests.is_empty())><p>{translated("accounts.full.empty")}</p></Show>
                <ul class="account__social-list">
                    <For each=move || owner.current(account).map(|snapshot| snapshot.requests.into_iter().map(|request| request.request_id).collect::<Vec<_>>()).unwrap_or_default()
                        key=|id| id.clone() children=move |id| {
                            let id = StoredValue::new(id);
                            let request = move || owner.current(account).and_then(|snapshot| snapshot.requests.into_iter().find(|request| id.try_with_value(|id| *id == request.request_id).unwrap_or(false)));
                            let incoming = move || request().is_some_and(|request| account.document_ticket().is_some_and(|ticket| request.recipient.user_id == ticket.subject()));
                            let actionable = move || request().is_some_and(|request| request.status == FriendRequestStatus::Pending && owner.current(account).is_some_and(|snapshot| request.expires_at_ms > snapshot.generated_at_ms)) && !owner.stale();
                            view! { <li class="account__social-row" data-social-stream="" data-request-id=super::text(Signal::derive(move || id.try_get_value().unwrap_or_default()), "")>
                                <p>{super::optional_text(Signal::derive(move || request().map(|request| if incoming() { request.sender.handle } else { request.recipient.handle })))}</p>
                                <p class="status">{super::optional_text(Signal::derive(move || request().map(|request| Messages::new(locale.get()).text(request_status_key(request.status)))))}</p>
                                <Show when=move || actionable() && incoming()><div class="actions account__actions">
                                    <button type="button" data-testid="friend-accept" class="btn btn--tonal" disabled=disabled on:click=move |_| { if let Some(request) = request() { mutate(SocialAction::Accept { request_id: request.request_id, expected_revision: request.revision }); } }>{translated("accounts.full.accept")}</button>
                                    <button type="button" data-testid="friend-decline" class="btn btn--text" disabled=disabled on:click=move |_| { if let Some(request) = request() { mutate(SocialAction::Decline { request_id: request.request_id, expected_revision: request.revision }); } }>{translated("accounts.full.decline")}</button>
                                </div></Show>
                                <Show when=move || actionable() && !incoming()><button type="button" data-testid="friend-cancel" class="btn btn--tonal" disabled=disabled on:click=move |_| { if let Some(request) = request() { mutate(SocialAction::Cancel { request_id: request.request_id, expected_revision: request.revision }); } }>{translated("accounts.full.cancel_request")}</button></Show>
                            </li> }
                        }/>
                </ul>
                <Show when=move || phase.try_get() == Some(ResourcePhase::Unknown)><button type="button" class="btn btn--tonal" on:click=move |_| {
                    if let Some(request) = pending.try_with_value(Clone::clone).flatten() { mutate(request.action); }
                }>{translated("accounts.full.retry_same")}</button></Show>
                <button type="button" class="btn btn--tonal" disabled=super::boolean(Signal::derive(move || phase.try_get().is_some_and(ResourcePhase::busy)), true) on:click=move |_| { account.recheck(); }>{translated("accounts.full.resync")}</button>
                <A href="/me" attr:class="btn btn--text">{translated("accounts.action.profile")}</A>
            </section>
        </Show>
        <A href="/login" attr:class="btn btn--tonal">{translated("accounts.action.signin")}</A>
        <Escapes principal=move || !account.social_available()/>
    </section> }
}

fn request_status_key(status: FriendRequestStatus) -> &'static str {
    match status {
        FriendRequestStatus::Pending => "accounts.full.pending_request",
        FriendRequestStatus::Accepted => "accounts.full.accepted_request",
        FriendRequestStatus::Declined => "accounts.full.declined_request",
        FriendRequestStatus::Expired => "accounts.full.expired_request",
        FriendRequestStatus::Cancelled => "accounts.full.cancelled_request",
    }
}
fn relationship_key(relationship: SocialRelationship) -> &'static str {
    match relationship {
        SocialRelationship::None => "accounts.full.ready",
        SocialRelationship::IncomingPending | SocialRelationship::OutgoingPending => {
            "accounts.full.pending_request"
        }
        SocialRelationship::Accepted => "accounts.full.accepted_request",
        SocialRelationship::Declined => "accounts.full.declined_request",
        SocialRelationship::Expired => "accounts.full.expired_request",
        SocialRelationship::Cancelled => "accounts.full.cancelled_request",
    }
}
fn presence_key(presence: &PresenceObservation) -> &'static str {
    match presence {
        PresenceObservation::Unknown => "accounts.full.presence.unknown",
        PresenceObservation::Online { .. } => "accounts.full.presence.online",
        PresenceObservation::Offline { .. } => "accounts.full.presence.offline",
        PresenceObservation::Stale { .. } => "accounts.full.presence.stale",
    }
}
fn presence_times(presence: Option<&PresenceObservation>) -> (Option<u64>, Option<u64>) {
    match presence {
        Some(PresenceObservation::Online { as_of_ms }) => (Some(*as_of_ms), None),
        Some(PresenceObservation::Offline {
            as_of_ms,
            last_seen_ms,
        }) => (Some(*as_of_ms), *last_seen_ms),
        Some(PresenceObservation::Stale {
            as_of_ms,
            last_seen_ms,
        }) => (*as_of_ms, *last_seen_ms),
        _ => (None, None),
    }
}

fn formatted_timestamp(value: u64, locale: tabula_registry::Locale) -> Option<(String, String)> {
    if value > 8_640_000_000_000_000 {
        return None;
    }
    #[cfg(target_arch = "wasm32")]
    {
        #[allow(clippy::cast_precision_loss)]
        // The validated Date range is within exact integer f64 representation.
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(value as f64));
        let options = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&options, &"dateStyle".into(), &"medium".into());
        let _ = js_sys::Reflect::set(&options, &"timeStyle".into(), &"medium".into());
        let label = date
            .to_locale_string(
                match locale {
                    tabula_registry::Locale::En => "en-US",
                    tabula_registry::Locale::Vi => "vi-VN",
                },
                &options,
            )
            .as_string()?;
        Some((date.to_iso_string().as_string()?, label))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = locale;
        Some((
            String::new(),
            format!("{value} milliseconds since Unix epoch"),
        ))
    }
}

#[component]
fn PresenceTimes(
    presence: impl Fn() -> Option<PresenceObservation> + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let locale = use_locale();
    let observed = move || {
        presence_times(presence().as_ref())
            .0
            .and_then(|value| formatted_timestamp(value, locale.get()))
    };
    let last_seen = move || {
        presence_times(presence().as_ref())
            .1
            .and_then(|value| formatted_timestamp(value, locale.get()))
    };
    view! { <div class="field__hint">
        <Show when=move || observed().is_some()><p>{translated("accounts.full.observed_at")} " "
            <time datetime=super::optional_text(Signal::derive(move || observed().map(|value| value.0)))>{super::optional_text(Signal::derive(move || observed().map(|value| value.1)))}</time>
        </p></Show>
        <Show when=move || last_seen().is_some()><p>{translated("accounts.full.last_seen")} " "
            <time datetime=super::optional_text(Signal::derive(move || last_seen().map(|value| value.0)))>{super::optional_text(Signal::derive(move || last_seen().map(|value| value.1)))}</time>
        </p></Show>
    </div> }
}

/// Other-profile DTO contains only the currently permitted projection.
#[component]
pub fn OtherProfilePage() -> impl IntoView {
    let parameters = use_params_map();
    let account = use_account();
    let slot = RequestSlot::new();
    let profile = RwSignal::new(None::<OtherAccountProfileResponse>);
    let phase = RwSignal::new(ResourcePhase::Unavailable);
    let previous = StoredValue::new(None::<(crate::account::DocumentAccountTicket, String)>);
    let heading = NodeRef::<html::H1>::new();
    super::account::focus_on_arrival(heading);
    Effect::new(move |_| {
        let _ = account.state.try_get();
        let handle = parameters
            .try_get()
            .and_then(|parameters| parameters.get("handle"))
            .unwrap_or_default();
        let current = account
            .document_ticket()
            .map(|ticket| (ticket, handle.clone()));
        if previous
            .try_with_value(|old| *old == current)
            .unwrap_or(true)
        {
            return;
        }
        previous.try_set_value(current.clone());
        slot.retire();
        profile.try_set(None);
        let Some((ticket, handle)) = current.filter(|(_, handle)| accounts::valid_handle(handle))
        else {
            phase.try_set(ResourcePhase::Unavailable);
            return;
        };
        let expected = handle.clone();
        phase.try_set(ResourcePhase::Checking);
        slot.run(
            account,
            RequestScope::Viewer(ticket),
            "GET",
            format!("/api/v2/profiles/by-handle/{handle}"),
            None,
            None,
            move |result| {
                let current = result
                    .ok()
                    .filter(|response| response.status == 200)
                    .and_then(|response| {
                        crate::json::decode::<OtherAccountProfileResponse>(&response.body).ok()
                    })
                    .filter(|current| current.validate().is_ok())
                    .filter(|current| current.handle == expected);
                if let Some(current) = current {
                    profile.try_set(Some(current));
                    phase.try_set(ResourcePhase::Ready);
                } else {
                    phase.try_set(ResourcePhase::Unavailable);
                }
            },
        );
    });
    view! { <section class="section account" aria-labelledby="account-title"><h1 id="account-title" class="section__title" tabindex="-1" node_ref=heading>{translated("accounts.profile.title")}</h1>
        <FullStatus phase=phase.read_only()/>
        <Show when=move || profile.try_get().flatten().is_some()><section class="account__profile" data-account-private="">
            <dl class="facts"><dt>{translated("accounts.full.handle")}</dt><dd>{super::optional_text(Signal::derive(move || profile.try_get().flatten().map(|profile| profile.handle)))}</dd>
                <dt>{translated("accounts.full.display_name")}</dt><dd>{super::optional_text(Signal::derive(move || profile.try_get().flatten().map(|profile| profile.display_name)))}</dd></dl>
        </section></Show><Escapes principal=|| true/>
    </section> }
}
