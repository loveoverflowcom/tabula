//! 03 — new match setup, the `?setup=1` substate of detail.
//!
//! The shell renders descriptors and owns the lifecycle; the registry owns
//! meaning and validation. Nothing here knows what a field means, which is why
//! the same code renders every game (docs/ui/screens/03-new-match.md).

use leptos::prelude::*;
use leptos_router::components::A;
use tabula_registry::discovery::ErasedDiscoveryGame as ErasedGame;
use tabula_registry::{
    bot_level_label_key, DiscoveryCatalog as Catalog, FieldKind, GameId, LaunchMode, ModeSupport,
    RuntimeBinding, SummaryLine, SummaryValue, TimeControlKind,
};
use wasm_bindgen::{closure::Closure, JsCast};

use crate::{
    i18n::{shell, Messages},
    setup::{Phase, SetupState},
    views::{library::seat_label, parts::Reason, use_locale},
};

/// Where this build's gameplay document lives.
///
/// ADR-011 makes gameplay a separate document, so the shell cannot start a
/// session by mounting anything: it needs a deployed bundle to navigate to.
/// `TABULA_PLAY_BASE=/play` binds one at build time; without it the Start action
/// stays unavailable with its reason, and never reports a started match.
fn runtime_binding() -> RuntimeBinding {
    match option_env!("TABULA_PLAY_BASE") {
        Some(base) => RuntimeBinding::bound(base),
        None => RuntimeBinding::unbound(),
    }
}

#[component]
pub fn NewMatch(id: String, preselected: Option<LaunchMode>) -> impl IntoView {
    let locale = use_locale();
    let (_, catalog) = shell(locale.get_untracked());
    let game_id = GameId::new(id.clone()).expect("detail validated this id");
    let state = RwSignal::new(SetupState::new(
        catalog
            .get(&game_id)
            .expect("detail resolved this game")
            .game(),
        preselected,
    ));
    restore_after_document_navigation(state);

    view! {
        <div class="setup">
            {move || {
                let (messages, catalog) = shell(locale.get());
                let Some(entry) = catalog.get(&game_id) else {
                    return ().into_any();
                };
                body(&messages, entry.game(), state, &id)
            }}
        </div>
    }
}

/// Own one listener for this setup's lifetime. Browser Back may restore the
/// suspended shell rather than mount a new one; a completed handoff must not
/// leave its controls locked or reactivate an old launch.
fn restore_after_document_navigation(state: RwSignal<SetupState>) {
    let browser = window();
    let on_show = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
        state.try_update(SetupState::navigation_returned);
    });
    if browser
        .add_event_listener_with_callback("pageshow", on_show.as_ref().unchecked_ref())
        .is_ok()
    {
        let listener = StoredValue::new_local(Some((browser, on_show)));
        on_cleanup(move || {
            listener.try_update_value(|listener| {
                if let Some((browser, on_show)) = listener.take() {
                    let _ = browser.remove_event_listener_with_callback(
                        "pageshow",
                        on_show.as_ref().unchecked_ref(),
                    );
                }
            });
        });
    }
}

fn body(
    messages: &Messages,
    game: &dyn ErasedGame,
    state: RwSignal<SetupState>,
    id: &str,
) -> AnyView {
    let messages = messages.clone();
    let metadata = game.metadata();
    let back = format!("/games/{id}");
    let phase_label = messages.text(state.get().phase().label_key());
    let back_label = messages.text("setup.back");
    let heading = messages.text("setup.heading");
    let game_name = messages.text(metadata.name_key().as_str());

    view! {
        <>
            <A href=back attr:class="btn btn--tonal">{back_label}</A>
            <span class="setup__revision" hidden data-revision=state.get().revision().to_string()></span>
            <h1 class="section__title">{heading}</h1>
            <p class="section__lead">{game_name}</p>

            {mode_group(&messages, game, state)}
            {seat_control(&messages, game, state)}
            {bot_control(&messages, game, state)}
            {option_fields(&messages, game, state)}

            <p class="status" aria-live="polite" data-phase=phase_key(state)>
                {phase_label}
            </p>
            {rejection_summary(&messages, state)}
            {summary(&messages, game, state)}
            {actions(&messages, game, state)}
        </>
    }
    .into_any()
}

fn phase_key(state: RwSignal<SetupState>) -> &'static str {
    match state.get().phase() {
        Phase::Unavailable { .. } => "unavailable",
        Phase::Editing => "editing",
        Phase::Validating => "validating",
        Phase::Ready(_) => "ready",
        Phase::Rejected(_) => "rejected",
        Phase::Pending(_) | Phase::Handoff(_) => "pending",
    }
}

/// Modes, as a connected single-selection group. Unavailable choices keep a
/// readable reason and cannot be selected.
fn mode_group(messages: &Messages, game: &dyn ErasedGame, state: RwSignal<SetupState>) -> AnyView {
    let messages = messages.clone();
    let selected = state.get().mode();
    let locked = state.get().phase().locks_fields();
    view! {
        <fieldset class="group">
            <legend class="group__legend">{messages.text("setup.mode")}</legend>
            {game
                .modes()
                .iter()
                .map(|support: &ModeSupport| {
                    let mode = support.mode;
                    let available = support.is_available();
                    let input_id = format!("mode-{}", mode.as_str());
                    view! {
                        <div class="group__option" data-state=if available { "enabled" } else { "disabled" }>
                            <input
                                type="radio"
                                name="mode"
                                id=input_id.clone()
                                class="group__input"
                                value=mode.as_str()
                                checked=selected == Some(mode)
                                disabled=!available || locked
                                on:change=move |_| state.update(|draft| draft.set_mode(mode))
                            />
                            <label class="group__label" for=input_id>
                                {messages.text(mode.label_key())}
                            </label>
                            <span class="group__body">{messages.text(mode.consequence_key())}</span>
                            {support
                                .unavailable_reason()
                                .map(|reason| {
                                    view! {
                                        <Reason
                                            reason_key=reason.reason_key()
                                            recovery_key=reason.recovery_key()
                                        />
                                    }
                                })}
                        </div>
                    }
                })
                .collect_view()}
        </fieldset>
    }
    .into_any()
}

/// Seats. A game with one legal count shows static text rather than a control
/// that cannot change anything.
fn seat_control(
    messages: &Messages,
    game: &dyn ErasedGame,
    state: RwSignal<SetupState>,
) -> AnyView {
    let messages = messages.clone();
    let allowed = game.capabilities().seats().allowed();
    let counts: Vec<u8> = (allowed.min()..=allowed.max())
        .filter(|count| allowed.contains(*count))
        .collect();
    if counts.len() <= 1 {
        return view! {
            <p class="facts__static">
                {messages.text("setup.seats")} ": " {seat_label(&messages, game.capabilities())}
            </p>
        }
        .into_any();
    }
    let selected = state.get().seats();
    let locked = state.get().phase().locks_fields();
    view! {
        <div class="field">
            <label class="field__label" for="seats">{messages.text("setup.seats")}</label>
            <select
                id="seats"
                class="field__control"
                disabled=locked
                on:change=move |event| {
                    if let Ok(count) = event_target_value(&event).parse::<u8>() {
                        state.update(|draft| draft.set_seats(count));
                    }
                }
            >
                {counts
                    .into_iter()
                    .map(|count| {
                        view! {
                            <option value=count.to_string() selected=count == selected>
                                {count.to_string()}
                            </option>
                        }
                    })
                    .collect_view()}
            </select>
        </div>
    }
    .into_any()
}

/// Bot level, offered only for a mode that fills seats with bots and only for
/// levels in the package's declared policy inventory. The gameplay host's
/// actual mode support is checked independently before launch.
fn bot_control(messages: &Messages, game: &dyn ErasedGame, state: RwSignal<SetupState>) -> AnyView {
    let messages = messages.clone();
    let current = state.get();
    if !current.mode().is_some_and(LaunchMode::fills_with_bots) {
        return ().into_any();
    }
    let levels = game.bot_levels();
    if levels.is_empty() {
        return ().into_any();
    }
    let selected = current.bot_level();
    let locked = current.phase().locks_fields();
    view! {
        <div class="field">
            <label class="field__label" for="bot">{messages.text("setup.bot")}</label>
            <select
                id="bot"
                class="field__control"
                disabled=locked
                on:change=move |event| {
                    let value = event_target_value(&event);
                    let chosen = levels_by_label(&value);
                    if let Some(level) = chosen {
                        state.update(|draft| draft.set_bot_level(level));
                    }
                }
            >
                {levels
                    .iter()
                    .map(|level| {
                        let key = bot_level_label_key(*level);
                        view! {
                            <option value=format!("{level:?}") selected=selected == Some(*level)>
                                {messages.text(key)}
                            </option>
                        }
                    })
                    .collect_view()}
            </select>
        </div>
    }
    .into_any()
}

/// Parse the debug-rendered level back. The value is produced and consumed by
/// this control only; it never reaches a game or the address bar.
fn levels_by_label(value: &str) -> Option<tabula_registry::BotLevel> {
    use tabula_registry::BotLevel;
    [
        BotLevel::Trivial,
        BotLevel::Easy,
        BotLevel::Medium,
        BotLevel::Hard,
    ]
    .into_iter()
    .find(|level| format!("{level:?}") == value)
}

/// The game's own fields, rendered from descriptors.
fn option_fields(
    messages: &Messages,
    game: &dyn ErasedGame,
    state: RwSignal<SetupState>,
) -> AnyView {
    let messages = messages.clone();
    let current = state.get();
    let locked = current.phase().locks_fields();
    let visible = game.form().visible_fields(current.draft());
    view! {
        <fieldset class="group">
            <legend class="group__legend">{messages.text("setup.options")}</legend>
            {visible
                .into_iter()
                .map(|field| {
                    let key = field.key;
                    let value = current.draft().get(key).unwrap_or_default().to_owned();
                    let error = current
                        .field_error(key)
                        .map(|rejection| rejection_text(&messages, rejection));
                    let invalid = error.is_some();
                    let hint_id = format!("{key}-hint");
                    let hint = field.hint_key.map(|hint| messages.text(hint));
                    let label = messages.text(field.label_key);
                    let option_labels: Vec<String> = match field.kind {
                        FieldKind::Choice { options } => {
                            options.iter().map(|option| messages.text(option.label_key)).collect()
                        }
                        FieldKind::Integer { .. } => Vec::new(),
                    };
                    let control = match field.kind {
                        FieldKind::Choice { options } => {
                            view! {
                                <select
                                    id=key
                                    class="field__control"
                                    disabled=locked
                                    aria-describedby=hint_id.clone()
                                    aria-invalid=invalid.then_some("true")
                                    on:change=move |event| {
                                        let next = event_target_value(&event);
                                        state.update(|draft| draft.set_field(key, next));
                                    }
                                >
                                    {options
                                        .iter()
                                        .zip(option_labels)
                                        .map(|(option, label)| {
                                            view! {
                                                <option
                                                    value=option.value
                                                    selected=option.value == value
                                                >
                                                    {label}
                                                </option>
                                            }
                                        })
                                        .collect_view()}
                                </select>
                            }
                                .into_any()
                        }
                        FieldKind::Integer { min, max, .. } => {
                            view! {
                                <input
                                    id=key
                                    class="field__control"
                                    type="number"
                                    inputmode="numeric"
                                    step="1"
                                    min=min.to_string()
                                    max=max.to_string()
                                    value=value.clone()
                                    disabled=locked
                                    aria-describedby=hint_id.clone()
                                    aria-invalid=invalid.then_some("true")
                                    on:change=move |event| {
                                        let next = event_target_value(&event);
                                        state.update(|draft| draft.set_field(key, next));
                                    }
                                />
                            }
                                .into_any()
                        }
                    };
                    view! {
                        <div class="field" data-state=if invalid { "invalid" } else { "enabled" }>
                            <label class="field__label" for=key>{label}</label>
                            {control}
                            <p class="field__hint" id=hint_id>{hint}</p>
                            {error
                                .map(|text| view! { <p class="field__error">{text}</p> })}
                        </div>
                    }
                })
                .collect_view()}
        </fieldset>
    }
    .into_any()
}

fn rejection_text(messages: &Messages, rejection: &tabula_registry::ConfigRejection) -> String {
    match rejection.reason {
        tabula_registry::RejectionReason::OutOfRange { min, max } => messages.format(
            rejection.reason.message_key(),
            &[&min.to_string(), &max.to_string()],
        ),
        _ => messages.text(rejection.reason.message_key()),
    }
}

fn rejection_summary(messages: &Messages, state: RwSignal<SetupState>) -> AnyView {
    let current = state.get();
    if let Some(rejection) = current.summary_error() {
        let text = rejection_text(messages, rejection);
        return view! { <p class="banner banner--error" role="alert">{text}</p> }.into_any();
    }
    match current.phase() {
        Phase::Unavailable { reason, .. } => {
            let reason = *reason;
            let resume_label = messages.text("setup.resume");
            view! {
                <div class="banner banner--error" role="alert">
                    <Reason reason_key=reason.reason_key() recovery_key=reason.recovery_key()/>
                    <button
                        type="button"
                        class="btn btn--tonal"
                        on:click=move |_| state.update(SetupState::resume_editing)
                    >
                        {resume_label}
                    </button>
                </div>
            }
            .into_any()
        }
        _ => ().into_any(),
    }
}

/// The normalized summary, immediately before the action.
fn summary(messages: &Messages, game: &dyn ErasedGame, state: RwSignal<SetupState>) -> AnyView {
    let messages = messages.clone();
    let current = state.get();
    let metadata = game.metadata();
    let summary_heading = messages.text("setup.summary.heading");
    let game_label = messages.text("setup.summary.game");
    let game_name = messages.text(metadata.name_key().as_str());
    let version_label = messages.text("setup.summary.version");
    let version_value = messages.format(
        "detail.version",
        &[
            metadata.version().as_str(),
            &metadata.rules_version().0.to_string(),
        ],
    );
    let not_validated_label = messages.text("setup.state.editing");
    let not_validated_value = messages.text("setup.notvalidated");
    let lines: Option<Vec<SummaryLine>> = match current.phase() {
        Phase::Ready(config) | Phase::Pending(config) => Some(config.summary().to_vec()),
        // A refused start keeps showing exactly what it refused.
        Phase::Unavailable { config, .. } => config.as_ref().map(|c| c.summary().to_vec()),
        _ => None,
    };

    view! {
        <section class="summary" aria-labelledby="summary-heading">
            <h2 class="summary__title" id="summary-heading">{summary_heading}</h2>
            <dl class="facts">
                <dt>{game_label}</dt>
                <dd>{game_name}</dd>
                <dt>{version_label}</dt>
                <dd>{version_value}</dd>
                {lines
                    .map_or_else(
                        || {
                            view! {
                                <>
                                    <dt>{not_validated_label}</dt>
                                    <dd>{not_validated_value}</dd>
                                </>
                            }
                                .into_any()
                        },
                        |lines| {
                            lines
                                .into_iter()
                                .map(|line| {
                                    view! {
                                        <>
                                            <dt>{messages.text(line.label_key)}</dt>
                                            <dd>{summary_value(&messages, &line.value)}</dd>
                                        </>
                                    }
                                })
                                .collect_view()
                                .into_any()
                        },
                    )}
            </dl>
        </section>
    }
    .into_any()
}

/// Render one normalized value. Distinct semantics render distinctly.
fn summary_value(messages: &Messages, value: &SummaryValue) -> String {
    match value {
        SummaryValue::Key(key) => messages.text(key),
        SummaryValue::Count(count) => count.to_string(),
        SummaryValue::Seats { count } => messages.format("summary.seats", &[&count.to_string()]),
        SummaryValue::Millis(millis) => duration_text(messages, *millis),
        SummaryValue::TimeControl {
            initial_ms,
            control,
        } => match control {
            TimeControlKind::Untimed => messages.text("summary.time.untimed"),
            TimeControlKind::Increment { millis } => messages.format(
                "summary.time.increment",
                &[
                    &duration_text(messages, *initial_ms),
                    &duration_text(messages, *millis),
                ],
            ),
            TimeControlKind::Delay { millis } => messages.format(
                "summary.time.delay",
                &[
                    &duration_text(messages, *initial_ms),
                    &duration_text(messages, *millis),
                ],
            ),
        },
    }
}

fn duration_text(messages: &Messages, millis: u64) -> String {
    if millis >= 60_000 && millis % 60_000 == 0 {
        messages.format("unit.minutes", &[&(millis / 60_000).to_string()])
    } else {
        messages.format("unit.seconds", &[&(millis / 1_000).to_string()])
    }
}

/// Validate and start. Start admits only a Ready revision; the resolver reports
/// a missing or unsupported gameplay deployment as a recoverable reason.
fn actions(messages: &Messages, game: &dyn ErasedGame, state: RwSignal<SetupState>) -> AnyView {
    let current = state.get();
    let ready = matches!(current.phase(), Phase::Ready(_));
    let busy = current.phase().locks_fields();

    // The catalog is re-resolved inside the handler so the closure borrows
    // nothing from this render.
    let locale = use_locale();
    let id = game.metadata().id().as_str().to_owned();

    let validate = move |_| {
        let (_, catalog): (Messages, Catalog) = shell(locale.get_untracked());
        let Ok(game_id) = GameId::new(id.clone()) else {
            return;
        };
        let Some(entry) = catalog.get(&game_id) else {
            return;
        };
        let request = state.try_update(SetupState::begin_validation).flatten();
        if let Some((revision, request)) = request {
            let result = entry.game().normalize(&request);
            state.try_update(|draft| {
                draft.validated(revision, result);
            });
        }
    };

    let start = move |_| {
        let Some((revision, config)) = state.try_update(SetupState::submit).flatten() else {
            return;
        };
        match tabula_registry::resolve_launch_with_locale(
            runtime_binding(),
            &config,
            locale.get_untracked(),
        ) {
            Ok(handoff) => {
                let url = handoff.url.clone();
                // ADR-011: a real document navigation, not a router transition
                // into a canvas mounted in this document.
                match window().location().set_href(&url) {
                    Ok(()) => {
                        state.try_update(|draft| draft.handed_off(revision, handoff));
                    }
                    Err(_) => {
                        state.try_update(|draft| {
                            draft.start_unavailable(
                                revision,
                                tabula_registry::UnavailableReason::NavigationFailed,
                            );
                        });
                    }
                }
            }
            Err(reason) => {
                state.try_update(|draft| draft.start_unavailable(revision, reason));
            }
        }
    };

    view! {
        <div class="actions">
            <button
                type="button"
                class="btn btn--tonal"
                disabled=busy
                on:click=validate
            >
                {messages.text("setup.validate")}
            </button>
            <button
                type="button"
                class="btn btn--filled btn--principal"
                disabled=!ready
                on:click=start
            >
                {messages.text("setup.start")}
            </button>
        </div>
    }
    .into_any()
}
