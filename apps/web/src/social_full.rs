//! One shell-owned, snapshot-only social stream (ADR-0044, doc 04 §2).
//! Route tickets fence rendering; the cookie-authenticated server owns permission.

#[cfg(target_arch = "wasm32")]
use crate::account::AccountStatus;
use crate::account::{AccountController, DocumentAccountTicket};
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use tabula_session_http::accounts::social::PresenceObservation;
use tabula_session_http::accounts::social::SocialSnapshot;

struct Runtime {
    binding: Option<(AccountController, DocumentAccountTicket)>,
    subject: Option<String>,
    scope: Option<String>,
    revision: u64,
    waiting_scope: bool,
    request_outstanding: bool,
    request_ticket: Option<DocumentAccountTicket>,
    #[cfg(target_arch = "wasm32")]
    browser: browser::Transport,
}

/// App-owned transport and permitted projection, never a second socket per view.
#[derive(Clone, Copy)]
pub(crate) struct SocialOwner {
    runtime: StoredValue<Runtime, LocalStorage>,
    snapshot: RwSignal<Option<SocialSnapshot>>,
    stale: RwSignal<bool>,
}

pub(crate) fn provide_social() {
    let owner = SocialOwner {
        runtime: StoredValue::new_local(Runtime {
            binding: None,
            subject: None,
            scope: None,
            revision: 0,
            waiting_scope: true,
            request_outstanding: false,
            request_ticket: None,
            #[cfg(target_arch = "wasm32")]
            browser: browser::Transport::default(),
        }),
        snapshot: RwSignal::new(None),
        stale: RwSignal::new(true),
    };
    provide_context(owner);
    #[cfg(target_arch = "wasm32")]
    browser::listen(owner);
    on_cleanup(move || owner.close());
}

pub(crate) fn use_social() -> SocialOwner {
    use_context::<SocialOwner>().expect("App provides one social owner")
}

/// All account routes acquire fresh context before binding the shared stream.
pub(crate) fn bind_account(account: AccountController) {
    let owner = use_social();
    #[cfg(target_arch = "wasm32")]
    // Native account tests deliberately have no browser executor or transport.
    Effect::new(move |_| {
        let state = account.state.try_get();
        if !account.current() {
            owner.detach(account);
            return;
        }
        if let Some(ticket) = account
            .document_ticket()
            .filter(|_| account.social_available())
        {
            owner.bind(account, ticket);
        } else if state.is_some_and(|state| {
            matches!(
                state.status,
                AccountStatus::SignedOut
                    | AccountStatus::Expired
                    | AccountStatus::LogoutPending
                    | AccountStatus::LogoutContextChanged
                    | AccountStatus::LogoutStorageUnavailable
                    | AccountStatus::Disconnected
            )
        }) {
            owner.close();
        } else {
            owner.mask();
        }
    });
    on_cleanup(move || owner.detach(account));
}

impl SocialOwner {
    #[cfg(target_arch = "wasm32")]
    fn bind(self, account: AccountController, ticket: DocumentAccountTicket) {
        let same = self
            .runtime
            .try_with_value(|runtime| {
                runtime
                    .binding
                    .as_ref()
                    .is_some_and(|(_, old)| *old == ticket)
            })
            .unwrap_or(true);
        if same {
            return;
        }
        let changed = self
            .runtime
            .try_with_value(|runtime| {
                runtime
                    .subject
                    .as_deref()
                    .is_some_and(|old| old != ticket.subject())
            })
            .unwrap_or(true);
        if changed {
            self.close();
        }
        self.mask();
        self.runtime.try_update_value(|runtime| {
            runtime.subject = Some(ticket.subject().to_owned());
            runtime.binding = Some((account, ticket));
            runtime.waiting_scope = true;
        });
        #[cfg(target_arch = "wasm32")]
        browser::connect_or_resync(self);
    }

    fn detach(self, account: AccountController) {
        let matches = self
            .runtime
            .try_with_value(|runtime| {
                runtime
                    .binding
                    .as_ref()
                    .is_some_and(|(bound, _)| bound.state == account.state)
            })
            .unwrap_or(false);
        if matches {
            self.mask();
            self.runtime
                .try_update_value(|runtime| runtime.binding = None);
        }
    }

    fn mask(self) {
        #[cfg(target_arch = "wasm32")]
        browser::retire_focus(false);
        self.snapshot.try_set(None);
        self.stale.try_set(true);
        // Retire the intent while its serialized reply may still be in flight.
        self.runtime
            .try_update_value(|runtime| runtime.request_ticket = None);
    }

    fn close(self) {
        self.mask();
        self.runtime.try_update_value(|runtime| {
            runtime.binding = None;
            runtime.subject = None;
            runtime.scope = None;
            runtime.revision = 0;
            runtime.waiting_scope = true;
            runtime.request_outstanding = false;
            runtime.request_ticket = None;
            #[cfg(target_arch = "wasm32")]
            runtime.browser.close();
        });
    }

    pub(crate) fn resync(self) {
        self.mask();
        self.runtime
            .try_update_value(|runtime| runtime.waiting_scope = true);
        #[cfg(target_arch = "wasm32")]
        browser::connect_or_resync(self);
    }

    pub(crate) fn current(self, account: AccountController) -> Option<SocialSnapshot> {
        // Subscribe even before the first binding exists. A fail-closed initial
        // render must still observe the subsequently validated projection.
        let snapshot = self.snapshot.try_get().flatten();
        let _ = account.state.try_get();
        let allowed = self
            .runtime
            .try_with_value(|runtime| {
                runtime.binding.as_ref().is_some_and(|(bound, ticket)| {
                    bound.state == account.state && account.ticket_current(ticket)
                })
            })
            .unwrap_or(false);
        allowed.then_some(snapshot).flatten()
    }

    pub(crate) fn stale(self) -> bool {
        self.stale.try_get().unwrap_or(true)
    }

    #[cfg(target_arch = "wasm32")]
    fn receive(self, snapshot: SocialSnapshot) {
        if snapshot.validate().is_err() {
            self.close();
            return;
        }
        let mut accepted = false;
        let mut gap = false;
        self.runtime.try_update_value(|runtime| {
            if runtime.subject.as_deref() != Some(snapshot.viewer_id.as_str()) {
                return;
            }
            if runtime.waiting_scope {
                if !runtime.request_outstanding
                    || snapshot.revision != 1
                    || runtime.scope.as_ref() == Some(&snapshot.scope_id)
                {
                    return;
                }
                runtime.scope = Some(snapshot.scope_id.clone());
                runtime.revision = 1;
                runtime.request_outstanding = false;
                let current = runtime.binding.as_ref().is_some_and(|(account, ticket)| {
                    runtime.request_ticket.as_ref() == Some(ticket)
                        && account.ticket_current(ticket)
                });
                runtime.request_ticket = None;
                if current {
                    runtime.waiting_scope = false;
                    accepted = true;
                } else {
                    gap = runtime
                        .binding
                        .as_ref()
                        .is_some_and(|(account, ticket)| account.ticket_current(ticket));
                }
            } else if runtime.scope.as_ref() == Some(&snapshot.scope_id) {
                if !runtime
                    .binding
                    .as_ref()
                    .is_some_and(|(account, ticket)| account.ticket_current(ticket))
                {
                    return;
                }
                if snapshot.revision <= runtime.revision {
                    return;
                }
                if runtime.revision.checked_add(1) != Some(snapshot.revision) {
                    gap = true;
                    return;
                }
                runtime.revision = snapshot.revision;
                accepted = true;
            }
            if accepted {
                runtime.browser.received_at = js_sys::Date::now();
            }
        });
        if gap {
            self.resync();
        }
        if accepted {
            self.snapshot.try_set(Some(snapshot));
            self.stale.try_set(false);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn disconnected(self) {
        self.mask();
        let account = self
            .runtime
            .try_with_value(|runtime| runtime.binding.as_ref().map(|(account, _)| *account))
            .flatten();
        self.runtime.try_update_value(|runtime| {
            runtime.waiting_scope = true;
            runtime.request_outstanding = false;
            runtime.request_ticket = None;
        });
        if let Some(account) = account.filter(|account| account.current()) {
            account.recheck();
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn lose_stream(self) {
        browser::retire_focus(true);
        self.stale.try_set(true);
        self.snapshot.try_update(|snapshot| {
            if let Some(snapshot) = snapshot {
                for friend in &mut snapshot.friends {
                    friend.presence = match &friend.presence {
                        PresenceObservation::Online { as_of_ms } => PresenceObservation::Stale {
                            as_of_ms: Some(*as_of_ms),
                            last_seen_ms: None,
                        },
                        PresenceObservation::Offline {
                            as_of_ms,
                            last_seen_ms,
                        } => PresenceObservation::Stale {
                            as_of_ms: Some(*as_of_ms),
                            last_seen_ms: *last_seen_ms,
                        },
                        other => other.clone(),
                    };
                }
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::SocialOwner;
    use leptos::prelude::*;
    use tabula_session_http::accounts::social::{
        SocialClientMessage, SocialServerMessage, SOCIAL_CONTRACT_VERSION,
    };
    use wasm_bindgen::{closure::Closure, JsCast};

    #[derive(Default)]
    pub(super) struct Transport {
        socket: Option<web_sys::WebSocket>,
        callbacks: Vec<Closure<dyn FnMut(web_sys::Event)>>,
        listeners: Vec<Listener>,
        timer: Option<(i32, Closure<dyn FnMut()>)>,
        pub(super) received_at: f64,
        sequence: u64,
    }
    type Listener = (
        web_sys::EventTarget,
        &'static str,
        Closure<dyn FnMut(web_sys::Event)>,
    );
    impl Transport {
        pub(super) fn close(&mut self) {
            self.sequence = self.sequence.saturating_add(1);
            if let Some(socket) = self.socket.take() {
                socket.set_onopen(None);
                socket.set_onmessage(None);
                socket.set_onerror(None);
                socket.set_onclose(None);
                let _ = socket.close_with_code_and_reason(1000, "retired");
            }
            self.callbacks.clear();
        }
    }

    pub(super) fn retire_focus(buttons_only: bool) {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let retiring = document.active_element().is_some_and(|element| {
            (!buttons_only || element.tag_name() == "BUTTON")
                && element
                    .closest("[data-social-stream]")
                    .ok()
                    .flatten()
                    .is_some()
        });
        if retiring {
            if let Some(heading) = document
                .get_element_by_id("account-title")
                .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = heading.focus();
            }
        }
    }

    fn send(socket: &web_sys::WebSocket, message: &SocialClientMessage) {
        if let Ok(message) = serde_json::to_string(message) {
            let _ = socket.send_with_str(&message);
        }
    }

    fn begin_request(owner: SocialOwner) -> bool {
        let mut begin = false;
        owner.runtime.try_update_value(|runtime| {
            if !runtime.request_outstanding {
                runtime.request_outstanding = true;
                runtime.request_ticket = runtime
                    .binding
                    .as_ref()
                    .filter(|(account, ticket)| account.ticket_current(ticket))
                    .map(|(_, ticket)| ticket.clone());
                begin = true;
            }
        });
        begin
    }

    #[allow(clippy::too_many_lines)] // Four socket callbacks share one transport sequence fence.
    pub(super) fn connect_or_resync(owner: SocialOwner) {
        let Some(window) = web_sys::window() else {
            owner.close();
            return;
        };
        if window.location().protocol().ok().as_deref() != Some("https:")
            || !window.navigator().on_line()
            || window.document().is_none_or(|document| document.hidden())
        {
            owner.close();
            return;
        }
        let existing = owner
            .runtime
            .try_with_value(|runtime| runtime.browser.socket.clone())
            .flatten();
        if let Some(socket) = existing {
            if socket.ready_state() == web_sys::WebSocket::OPEN {
                if begin_request(owner) {
                    send(&socket, &SocialClientMessage::Resync);
                }
                return;
            }
            if socket.ready_state() == web_sys::WebSocket::CONNECTING {
                return;
            }
        }
        let Ok(host) = window.location().host() else {
            owner.close();
            return;
        };
        let Ok(socket) = web_sys::WebSocket::new_with_str(
            &format!("wss://{host}/api/v2/lobby/ws"),
            "tabula-social.v2.json",
        ) else {
            owner.close();
            return;
        };
        let mut sequence = 0;
        owner.runtime.try_update_value(|runtime| {
            runtime.browser.close();
            runtime.request_outstanding = false;
            runtime.request_ticket = None;
            sequence = runtime.browser.sequence;
            runtime.browser.socket = Some(socket.clone());
        });
        let current = move || {
            owner
                .runtime
                .try_with_value(|runtime| runtime.browser.sequence == sequence)
                .unwrap_or(false)
        };
        let open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if current() {
                if let Some(socket) = owner
                    .runtime
                    .try_with_value(|runtime| runtime.browser.socket.clone())
                    .flatten()
                {
                    if begin_request(owner) {
                        send(
                            &socket,
                            &SocialClientMessage::Hello {
                                version: SOCIAL_CONTRACT_VERSION,
                            },
                        );
                    }
                }
            }
        });
        let message = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            if !current() {
                return;
            }
            let Some(text) = event
                .dyn_ref::<web_sys::MessageEvent>()
                .and_then(|event| event.data().as_string())
                .filter(|text| text.len() <= 512 * 1024)
            else {
                owner.close();
                return;
            };
            match crate::json::decode::<SocialServerMessage>(text.as_bytes()) {
                Ok(SocialServerMessage::Snapshot { snapshot }) => owner.receive(snapshot),
                Err(_) => owner.close(),
            }
        });
        let closed = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if current() {
                owner.disconnected();
            }
        });
        let failed = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if current() {
                owner.disconnected();
            }
        });
        socket.set_onopen(Some(open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(closed.as_ref().unchecked_ref()));
        socket.set_onerror(Some(failed.as_ref().unchecked_ref()));
        owner.runtime.try_update_value(|runtime| {
            runtime.browser.callbacks = vec![open, message, closed, failed];
        });
    }

    pub(super) fn listen(owner: SocialOwner) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };
        for (target, kind) in [
            (window.clone().into(), "offline"),
            (window.clone().into(), "pagehide"),
            (document.clone().into(), "freeze"),
            (document.clone().into(), "visibilitychange"),
        ] {
            let target: web_sys::EventTarget = target;
            let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                if kind != "visibilitychange"
                    || web_sys::window()
                        .and_then(|window| window.document())
                        .is_none_or(|document| document.hidden())
                {
                    owner.close();
                }
            });
            if target
                .add_event_listener_with_callback(kind, callback.as_ref().unchecked_ref())
                .is_ok()
            {
                owner.runtime.try_update_value(|runtime| {
                    runtime.browser.listeners.push((target, kind, callback));
                });
            }
        }
        #[allow(clippy::float_arithmetic)]
        // Browser wall time measures presentation freshness only.
        let callback = Closure::<dyn FnMut()>::new(move || {
            if owner
                .runtime
                .try_with_value(|runtime| {
                    runtime.browser.received_at > 0.0
                        && js_sys::Date::now() - runtime.browser.received_at >= 5_000.0
                })
                .unwrap_or(false)
            {
                owner.lose_stream();
            }
        });
        if let Ok(timer) = window.set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            250,
        ) {
            owner
                .runtime
                .try_update_value(|runtime| runtime.browser.timer = Some((timer, callback)));
        }
        on_cleanup(move || {
            owner.runtime.try_update_value(|runtime| {
                if let Some((timer, _)) = runtime.browser.timer.take() {
                    window.clear_interval_with_handle(timer);
                }
                for (target, kind, callback) in runtime.browser.listeners.drain(..) {
                    let _ = target.remove_event_listener_with_callback(
                        kind,
                        callback.as_ref().unchecked_ref(),
                    );
                }
            });
        });
    }
}
