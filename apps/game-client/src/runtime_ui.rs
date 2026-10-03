//! Honest feedback for the synchronous local shell. (doc 04 §4, §10)
//!
//! This adapter consumes stable error classes, never state or error details.
//! It has no pending/network/loading state: local inputs finish synchronously.
//! Recovery starts a fresh local match; it never claims to resume a stopped one.

#![allow(clippy::float_arithmetic)]

use tabula_core::RuleErrorCode;
use tabula_design::{Positive, ThemeKind};
use tabula_game_api::GameRules;
use tabula_presentation::{
    ActionButton, Align, ButtonInteraction, ButtonTone, Camera2D, Corners, FocusGraph, FocusId,
    FocusModality, FocusNode, FocusState, FrameCtx, GamePresentation, InputEvent, Key, Layer,
    NavigationAction, Paint, PointerButton, PointerPhase, Rect, RenderCmd, RenderList,
    RenderListBuilder, RenderListError, TextStyleToken, Vec2,
};

use crate::{LocalMatch, LocalMatchError};

const RECOVERY: FocusId = FocusId::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Feedback {
    Rejected(RuleErrorCode),
    CannotSubmit,
    Stopped(StopReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopReason {
    InputLimit,
    InvalidEffects,
    Timer,
    Renderer,
}

/// The shell action selected by a feedback interaction. (doc 04 §10)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackInput {
    /// Input outside the nonmodal feedback remains available to the board.
    PassThrough,
    /// Feedback handled this input without changing canonical state.
    Consumed,
    /// Feedback took a board gesture; cancel its local press without authority input.
    CancelGameplayPointer,
    /// The shell must construct a fresh local match with its selected options.
    NewLocalGame,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PointerOwner {
    #[default]
    Idle,
    Board,
    Feedback,
}

/// Local-only feedback and recovery focus, separate from every game's view. (I-10/I-12)
#[derive(Clone, Debug, Default)]
pub struct LocalFeedback {
    feedback: Option<Feedback>,
    interaction: ButtonInteraction,
    focus: FocusState,
    activation_guard: [bool; 2],
    pointer_owners: [PointerOwner; 3],
}

impl LocalFeedback {
    /// Records a stable class without copying developer detail into visible UI.
    ///
    /// A terminal rejection is not a fatal runtime failure. Stopped sessions stay
    /// stopped until the caller constructs a fresh match (doc 04 §4).
    pub fn note_error(&mut self, error: &LocalMatchError) {
        let next = match error {
            LocalMatchError::Rejected(error) => Feedback::Rejected(error.code),
            LocalMatchError::ViewerCannotSubmitPlayerInput => Feedback::CannotSubmit,
            LocalMatchError::InputIndexExhausted => Feedback::Stopped(StopReason::InputLimit),
            LocalMatchError::MultipleEndMatch => Feedback::Stopped(StopReason::InvalidEffects),
            LocalMatchError::MatchEnded => {
                if !self.is_stopped() {
                    self.clear();
                }
                return;
            }
        };
        self.set(next);
    }

    /// Stops after an unsuccessful due timer: it was consumed and cannot be retried safely.
    pub fn note_timer_error(&mut self, error: &LocalMatchError) {
        if matches!(error, LocalMatchError::MatchEnded) {
            self.note_error(error);
        } else {
            self.set(Feedback::Stopped(StopReason::Timer));
        }
    }

    /// Records a render failure without exposing backend diagnostics or payloads.
    pub fn note_render_error(&mut self) {
        self.set(Feedback::Stopped(StopReason::Renderer));
    }

    /// Clears a recoverable warning only after the shell observes real rules acceptance.
    pub fn note_accepted_input(&mut self) {
        if !self.is_stopped() {
            self.clear();
        }
    }

    /// Blocks authority input after either a terminal outcome or a local shell failure.
    #[must_use]
    pub fn allows_gameplay(&self, match_ended: bool) -> bool {
        !match_ended && !self.is_stopped()
    }

    /// Returns the controlled public message; arbitrary `RuleError::detail` is absent.
    #[must_use]
    pub fn message(&self) -> Option<&'static str> {
        self.feedback.map(|feedback| match feedback {
            Feedback::Rejected(code) => rejection_message(code),
            Feedback::CannotSubmit => "This viewer cannot send moves. Choose a player seat.",
            Feedback::Stopped(StopReason::InputLimit) => {
                "This local session reached its input limit."
            }
            Feedback::Stopped(StopReason::InvalidEffects) => {
                "The local session received conflicting end results."
            }
            Feedback::Stopped(StopReason::Timer) => "A local timer could not be completed safely.",
            Feedback::Stopped(StopReason::Renderer) => "The board could not be rendered.",
        })
    }

    /// Suppresses the held key that produced an error until physical release.
    pub fn suppress_opening_input(&mut self, input: &InputEvent) {
        if let InputEvent::Key { key, pressed: true } = input {
            self.interaction.suppress_activation_until_release(*key);
            if let Some(index) = activation_index(*key) {
                self.activation_guard[index] = true;
            }
        }
    }

    /// Routes feedback and cancels an occluded board press only in presentation-local state.
    pub fn route_input<R, P>(
        &mut self,
        local_match: &mut LocalMatch<R, P>,
        input: &InputEvent,
        frame: &FrameCtx,
    ) -> FeedbackInput
    where
        R: GameRules,
        P: GamePresentation<Rules = R>,
    {
        let routed = self.on_input(input, frame);
        if let (
            FeedbackInput::CancelGameplayPointer,
            InputEvent::Pointer {
                position, button, ..
            },
        ) = (routed, input)
        {
            local_match.cancel_presentation_pointer(*position, *button);
        }
        routed
    }

    /// Handles one real recovery control using shared pointer/keyboard mechanics.
    ///
    /// Tab enters a nonmodal banner; Escape dismisses it. Pointer input outside
    /// it returns to the board. Fatal feedback consumes all gameplay input and
    /// Escape never resumes it. Focus loss and canceled presses cannot activate.
    pub fn on_input(&mut self, input: &InputEvent, frame: &FrameCtx) -> FeedbackInput {
        if let InputEvent::Focus(focused) = input {
            self.focus.set_window_focused(*focused);
        }
        if matches!(input, InputEvent::Focus(false)) {
            // A release may occur outside the focused surface. A fresh press on
            // return must remain usable; the unfocused graph still cannot act.
            self.interaction = ButtonInteraction::default();
            self.activation_guard = [false; 2];
            self.pointer_owners = [PointerOwner::Idle; 3];
        }
        if !self.focus.is_window_focused() && !matches!(input, InputEvent::Focus(_)) {
            return FeedbackInput::Consumed;
        }
        if let InputEvent::Key { key, pressed } = input {
            if let Some(index) = activation_index(*key) {
                if self.activation_guard[index] {
                    if *pressed {
                        return FeedbackInput::Consumed;
                    }
                    self.activation_guard[index] = false;
                }
            }
        }
        let pointer_route = self.pointer_route(input, frame);
        let Some(layout) = self.layout(frame) else {
            if let Some(routed) = pointer_route {
                return routed;
            }
            return if self.is_stopped() {
                FeedbackInput::Consumed
            } else {
                FeedbackInput::PassThrough
            };
        };
        let stopped = self.is_stopped();
        let button = self.button(layout, frame);
        let graph = FocusGraph::new(vec![FocusNode::new(RECOVERY, layout.button)])
            .expect("one recovery node is a valid graph");
        let keyboard_focus =
            self.focus.current().is_some() && self.focus.modality() == FocusModality::Keyboard;
        let consume = match input {
            InputEvent::Key {
                key: Key::Escape,
                pressed: true,
            } if !stopped => {
                self.clear();
                return FeedbackInput::Consumed;
            }
            InputEvent::Key {
                key: Key::Tab,
                pressed: true,
            } => {
                self.focus.set_keyboard_focus(Some(RECOVERY));
                true
            }
            // Physical release must also clear any board latch set before Tab
            // moved keyboard focus into this nonmodal banner.
            InputEvent::Key { pressed, .. } => stopped || (*pressed && keyboard_focus),
            InputEvent::Pointer {
                position, phase, ..
            } => {
                if !layout.panel.contains(position.get()) && *phase == PointerPhase::Down {
                    self.focus.set_current(None);
                }
                pointer_route != Some(FeedbackInput::PassThrough)
            }
            InputEvent::Focus(_) => stopped,
        };

        // Even pass-through pointer/focus input cancels a pending button press.
        let action = if consume
            || matches!(
                input,
                InputEvent::Pointer { .. }
                    | InputEvent::Focus(_)
                    | InputEvent::Key { pressed: false, .. }
            ) {
            self.interaction
                .on_input(input, &[button], &graph, &mut self.focus)
        } else {
            // Board arrows must not silently give the banner focus for the next
            // key. Track held activation only to prevent Tab activating mid-press.
            if let InputEvent::Key { key, pressed: true } = input {
                self.interaction.suppress_activation_until_release(*key);
            }
            NavigationAction::None
        };
        if pointer_route == Some(FeedbackInput::CancelGameplayPointer) {
            return FeedbackInput::CancelGameplayPointer;
        }
        if consume && matches!(action, NavigationAction::Activate(RECOVERY)) {
            self.suppress_opening_input(input);
            if stopped {
                return FeedbackInput::NewLocalGame;
            }
            self.clear();
            return FeedbackInput::Consumed;
        }
        if consume {
            FeedbackInput::Consumed
        } else {
            FeedbackInput::PassThrough
        }
    }

    fn pointer_route(&mut self, input: &InputEvent, frame: &FrameCtx) -> Option<FeedbackInput> {
        let InputEvent::Pointer {
            position,
            button,
            phase,
        } = input
        else {
            return None;
        };
        let index = match button {
            PointerButton::Primary => 0,
            PointerButton::Secondary => 1,
            PointerButton::Middle => 2,
        };
        let blocks = self.is_stopped()
            || self
                .layout(frame)
                .is_some_and(|layout| layout.panel.contains(position.get()));
        let owner = &mut self.pointer_owners[index];
        let routed = match (*owner, blocks) {
            (PointerOwner::Board, true) => {
                *owner = PointerOwner::Feedback;
                FeedbackInput::CancelGameplayPointer
            }
            (PointerOwner::Feedback, _) | (_, true) => FeedbackInput::Consumed,
            (_, false) => FeedbackInput::PassThrough,
        };
        if *phase == PointerPhase::Down {
            *owner = if blocks || *owner == PointerOwner::Feedback {
                PointerOwner::Feedback
            } else {
                PointerOwner::Board
            };
        } else if matches!(phase, PointerPhase::Up | PointerPhase::Cancel) {
            *owner = PointerOwner::Idle;
        }
        Some(routed)
    }

    /// Draws feedback with the default screen camera, independently of the board camera.
    ///
    /// Fatal feedback fills the screen and stays visible; recoverable feedback
    /// leaves the HUD and the rest of the board available. (doc 04 §5, §10)
    pub fn present(&self, frame: &FrameCtx) -> Result<RenderList, RenderListError> {
        let mut builder = RenderListBuilder::new(Camera2D::default());
        let Some(layout) = self.layout(frame) else {
            return builder.finish();
        };
        let theme = frame.theme();
        if self.is_stopped() {
            builder.push(RenderCmd::Rect {
                rect: Rect::new(Vec2::ZERO, frame.viewport().size())?,
                radii: Corners::uniform(0.0)?,
                fill: Some(Paint::Solid(theme.color.surface)),
                border: None,
                layer: Layer::MODAL,
                z: 0,
            })?;
        }
        builder.push(RenderCmd::Rect {
            rect: layout.panel,
            radii: Corners::uniform(theme.shape.lg.get())?,
            fill: Some(Paint::Solid(theme.color.surface_container_high)),
            border: None,
            layer: Layer::MODAL,
            z: 0,
        })?;
        let padding = 16.0;
        let width = Positive::new(layout.panel.size().x - padding * 2.0)
            .map_err(|_| RenderListError::InvalidGeometry)?;
        let top = layout.panel.origin() + Vec2::splat(padding);
        let (title, message_at) = if self.is_stopped() {
            ("Local game stopped", top + Vec2::new(0.0, 32.0))
        } else {
            ("Move rejected", top + Vec2::new(0.0, 28.0))
        };
        for (value, at, style, color) in [
            (title, top, TextStyleToken::TitleMd, theme.color.danger),
            (
                self.message().unwrap_or_default(),
                message_at,
                TextStyleToken::BodyMd,
                theme.color.on_surface,
            ),
        ] {
            builder.push(RenderCmd::Text {
                text: value.into(),
                at,
                style,
                align: Align::Start,
                max_width: Some(width),
                color,
                layer: Layer::MODAL,
                z: 0,
            })?;
        }
        if self.is_stopped() {
            builder.push(RenderCmd::Text {
                text: "Start a fresh local game to continue.".into(),
                at: top + Vec2::new(0.0, 92.0),
                style: TextStyleToken::BodyMd,
                align: Align::Start,
                max_width: Some(width),
                color: theme.color.on_surface,
                layer: Layer::MODAL,
                z: 0,
            })?;
        }
        self.button(layout, frame).draw(
            &mut builder,
            &theme,
            &self.interaction,
            &self.focus,
            Layer::MODAL,
        )?;
        builder.finish()
    }

    fn is_stopped(&self) -> bool {
        matches!(self.feedback, Some(Feedback::Stopped(_)))
    }

    fn set(&mut self, next: Feedback) {
        if self.is_stopped() || self.feedback == Some(next) {
            return;
        }
        let focused = self.focus.is_window_focused();
        self.feedback = Some(next);
        self.interaction = ButtonInteraction::default();
        self.focus = FocusState::new(None, FocusModality::Pointer, focused);
    }

    fn clear(&mut self) {
        self.feedback = None;
        self.interaction = ButtonInteraction::default();
        self.focus.set_current(None);
    }

    fn button(&self, layout: FeedbackLayout, frame: &FrameCtx) -> ActionButton<'static> {
        let label = if self.is_stopped() {
            "New local game"
        } else {
            "Dismiss"
        };
        ActionButton::new(
            RECOVERY,
            layout.button,
            label,
            frame.theme().density.min_target,
        )
        .expect("recovery bounds retain the target floor")
        .tone(if self.is_stopped() {
            ButtonTone::Filled
        } else {
            ButtonTone::Tonal
        })
    }

    fn layout(&self, frame: &FrameCtx) -> Option<FeedbackLayout> {
        self.feedback?;
        let viewport = frame.viewport().size();
        // A minimized/tiny surface cannot fit a usable target; retain the error
        // state until it grows instead of creating clipped geometry or targets.
        if viewport.x < 240.0 || viewport.y < 248.0 {
            return None;
        }
        let width = (viewport.x - 24.0).min(560.0);
        let height = if self.is_stopped() { 224.0 } else { 168.0 };
        let panel = Rect::new(
            (viewport - Vec2::new(width, height)) / 2.0,
            Vec2::new(width, height),
        )
        .ok()?;
        let target = frame.theme().density.min_target.get().max(44.0);
        let button_width = if self.is_stopped() { 176.0 } else { 104.0 };
        let button = Rect::new(
            panel.origin() + Vec2::new(width - button_width - 16.0, height - target - 16.0),
            Vec2::new(button_width, target),
        )
        .ok()?;
        Some(FeedbackLayout { panel, button })
    }
}

#[derive(Clone, Copy)]
struct FeedbackLayout {
    panel: Rect,
    button: Rect,
}

fn activation_index(key: Key) -> Option<usize> {
    match key {
        Key::Enter => Some(0),
        Key::Space => Some(1),
        _ => None,
    }
}

fn rejection_message(code: RuleErrorCode) -> &'static str {
    match code {
        RuleErrorCode::NotYourTurn => "It is another seat's turn. Wait for your turn.",
        RuleErrorCode::IllegalMove => "Choose a legal action and try again.",
        RuleErrorCode::WrongPhase => "That action is unavailable in this phase.",
        RuleErrorCode::UnknownCommand => "This game does not support that action.",
        RuleErrorCode::MatchOver => "The game has ended. No more moves can be played.",
        RuleErrorCode::Unsupported => "This action is unavailable in the local session.",
        RuleErrorCode::NoSuchSeat => "That seat is unavailable in this local game.",
        _ => "The action was rejected. Choose another action.",
    }
}

/// Parses the supported local appearance names without introducing a settings backend.
#[must_use]
pub fn parse_local_theme(name: &str) -> Option<ThemeKind> {
    match name {
        "light" => Some(ThemeKind::Light),
        "dark" => Some(ThemeKind::Dark),
        "hc-light" => Some(ThemeKind::HighContrastLight),
        "hc-dark" => Some(ThemeKind::HighContrastDark),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::RuleError;
    use tabula_design::Theme;
    use tabula_presentation::{Dpi, PointerButton, PointerPosition, Viewport};

    fn frame(kind: ThemeKind, size: Vec2) -> FrameCtx {
        FrameCtx::new(
            Viewport::new(size).unwrap(),
            Dpi::new(1.0).unwrap(),
            0,
            Theme::by_kind(kind),
        )
    }

    fn key(key: Key, pressed: bool) -> InputEvent {
        InputEvent::Key { key, pressed }
    }

    fn pointer(at: Vec2, phase: PointerPhase) -> InputEvent {
        InputEvent::Pointer {
            position: PointerPosition::new(at).unwrap(),
            button: PointerButton::Primary,
            phase,
        }
    }

    fn rejected() -> LocalFeedback {
        let mut feedback = LocalFeedback::default();
        feedback.note_error(&LocalMatchError::Rejected(RuleError::with_detail(
            RuleErrorCode::IllegalMove,
            "private test sentinel",
        )));
        feedback
    }

    #[test]
    fn rejection_is_non_authoritative_and_never_displays_detail() {
        let frame = frame(ThemeKind::Light, Vec2::new(320.0, 568.0));
        let mut feedback = rejected();
        assert!(feedback.allows_gameplay(false));
        let list = feedback.present(&frame).unwrap();
        assert!(list.commands().iter().any(|cmd| matches!(cmd, RenderCmd::Text { text, .. } if text == "Choose a legal action and try again.")));
        assert!(!list
            .commands()
            .iter()
            .any(|cmd| matches!(cmd, RenderCmd::Text { text, .. } if text.contains("sentinel"))));
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::PassThrough
        );
        assert_eq!(
            feedback.on_input(&key(Key::Tab, true), &frame),
            FeedbackInput::Consumed
        );
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::Consumed
        );
        // A key already held by the board cannot activate the newly focused banner.
        assert!(feedback.message().is_some());
        feedback.on_input(&key(Key::Enter, false), &frame);
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::Consumed
        );
        assert!(feedback.message().is_none());
        assert!(feedback.allows_gameplay(false));
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::Consumed
        );
        feedback.on_input(&key(Key::Enter, false), &frame);
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::PassThrough
        );
    }

    #[test]
    fn fatal_is_latched_and_only_the_new_game_action_recovers() {
        let frame = frame(ThemeKind::Dark, Vec2::new(900.0, 720.0));
        let mut feedback = LocalFeedback::default();
        feedback.note_error(&LocalMatchError::MultipleEndMatch);
        assert!(!feedback.allows_gameplay(false));
        assert_eq!(
            feedback.on_input(&key(Key::Escape, true), &frame),
            FeedbackInput::Consumed
        );
        feedback.note_error(&LocalMatchError::Rejected(RuleError::code(
            RuleErrorCode::IllegalMove,
        )));
        feedback.note_error(&LocalMatchError::MatchEnded);
        assert_eq!(
            feedback.message(),
            Some("The local session received conflicting end results.")
        );
        feedback.on_input(&key(Key::Tab, true), &frame);
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::NewLocalGame
        );
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::Consumed
        );
        assert!(
            !feedback.allows_gameplay(false),
            "only replacing the session recovers authority"
        );
    }

    #[test]
    fn board_arrows_do_not_steal_feedback_focus_and_blur_does_not_lose_a_fresh_press() {
        let frame = frame(ThemeKind::Light, Vec2::new(320.0, 568.0));
        let mut feedback = rejected();
        assert_eq!(
            feedback.on_input(&key(Key::ArrowRight, true), &frame),
            FeedbackInput::PassThrough
        );
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::PassThrough
        );
        assert!(feedback.message().is_some());
        feedback.on_input(&key(Key::Enter, false), &frame);
        feedback.note_render_error();
        feedback.on_input(&key(Key::Tab, true), &frame);
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::NewLocalGame
        );
        feedback.on_input(&InputEvent::Focus(false), &frame);
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::Consumed
        );
        // No release is delivered while unfocused. Reentry still needs a fresh
        // usable press; the interrupted press itself never recovers the game.
        feedback.on_input(&InputEvent::Focus(true), &frame);
        assert_eq!(
            feedback.on_input(&key(Key::Enter, true), &frame),
            FeedbackInput::NewLocalGame
        );
    }

    #[test]
    fn rejected_timer_stops_and_terminal_outcome_never_becomes_fatal() {
        let mut feedback = rejected();
        feedback.note_accepted_input();
        assert!(feedback.message().is_none());
        feedback = rejected();
        feedback.note_error(&LocalMatchError::MatchEnded);
        assert!(feedback.message().is_none());
        assert!(
            !feedback.allows_gameplay(true),
            "every later event in the batch is gated"
        );
        assert!(feedback.allows_gameplay(false));
        feedback.note_timer_error(&LocalMatchError::Rejected(RuleError::code(
            RuleErrorCode::IllegalMove,
        )));
        assert!(!feedback.allows_gameplay(false));
        feedback.note_accepted_input();
        assert!(
            !feedback.allows_gameplay(false),
            "acceptance cannot repair a failed shell"
        );
    }

    #[test]
    fn recovery_requires_matching_pointer_release_and_focus() {
        let frame = frame(ThemeKind::Light, Vec2::new(320.0, 568.0));
        let mut feedback = rejected();
        let layout = feedback.layout(&frame).unwrap();
        let at = layout.button.origin() + layout.button.size() / 2.0;
        for interruption in [
            pointer(Vec2::ZERO, PointerPhase::Move),
            pointer(at, PointerPhase::Cancel),
            InputEvent::Focus(false),
        ] {
            feedback.on_input(&pointer(at, PointerPhase::Down), &frame);
            feedback.on_input(&interruption, &frame);
            feedback.on_input(&InputEvent::Focus(true), &frame);
            feedback.on_input(&pointer(at, PointerPhase::Up), &frame);
            assert!(feedback.message().is_some());
        }
        assert_eq!(
            feedback.on_input(&pointer(Vec2::ZERO, PointerPhase::Down), &frame),
            FeedbackInput::PassThrough
        );
        feedback.on_input(&pointer(at, PointerPhase::Down), &frame);
        assert_eq!(
            feedback.on_input(&pointer(at, PointerPhase::Up), &frame),
            FeedbackInput::Consumed
        );
        assert!(feedback.message().is_none());
    }

    #[test]
    fn render_and_recovery_keep_screen_camera_tokens_and_target_floor_in_every_theme() {
        for kind in [
            ThemeKind::Light,
            ThemeKind::Dark,
            ThemeKind::HighContrastLight,
            ThemeKind::HighContrastDark,
        ] {
            for size in [Vec2::new(320.0, 568.0), Vec2::new(900.0, 720.0)] {
                let frame = frame(kind, size);
                let mut feedback = rejected();
                for stopped in [false, true] {
                    if stopped {
                        feedback.note_render_error();
                    }
                    let layout = feedback.layout(&frame).unwrap();
                    assert!(layout.button.size().min_element() >= 44.0);
                    assert!(layout
                        .panel
                        .contains(layout.button.origin() + layout.button.size()));
                    let list = feedback.present(&frame).unwrap();
                    assert_eq!(list.camera(), Camera2D::default());
                    assert!(list.commands().iter().any(|cmd| matches!(cmd, RenderCmd::Text { color, .. } if *color == frame.theme().color.danger)));
                }
            }
        }
    }

    #[test]
    fn all_existing_rejection_codes_have_controlled_copy_and_theme_names_are_exact() {
        for code in [
            RuleErrorCode::NotYourTurn,
            RuleErrorCode::IllegalMove,
            RuleErrorCode::WrongPhase,
            RuleErrorCode::UnknownCommand,
            RuleErrorCode::MatchOver,
            RuleErrorCode::Unsupported,
            RuleErrorCode::NoSuchSeat,
        ] {
            let mut feedback = LocalFeedback::default();
            feedback.note_error(&LocalMatchError::Rejected(RuleError::with_detail(
                code, "sentinel",
            )));
            assert!(!feedback.message().unwrap().contains("sentinel"));
        }
        assert_eq!(parse_local_theme("light"), Some(ThemeKind::Light));
        assert_eq!(parse_local_theme("dark"), Some(ThemeKind::Dark));
        assert_eq!(
            parse_local_theme("hc-light"),
            Some(ThemeKind::HighContrastLight)
        );
        assert_eq!(
            parse_local_theme("hc-dark"),
            Some(ThemeKind::HighContrastDark)
        );
        for invalid in ["", "auto", "Dark", "unknown"] {
            assert_eq!(parse_local_theme(invalid), None);
        }
    }
}
