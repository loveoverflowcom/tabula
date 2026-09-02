//! # `tabula-game-client` — the Macroquad gameplay runtime
//!
//! Phase 2 owns one local imperative shell around the deterministic game
//! contract. Presenters receive projections and local state only; every
//! authoritative mutation returns through [`GameRules::apply`].

#![forbid(unsafe_code)]

use std::{collections::BTreeMap, marker::PhantomData};

#[cfg(test)]
use tabula_core::StateHash;
use tabula_core::{
    Audience, DetRng, InputIndex, LogicalTime, MatchOutcome, MatchSeed, Millis, RuleError, SeatId,
    TimerId, Viewer,
};
use tabula_game_api::{Budget, Ctx, Effect, GameRules, InitError, Input, Notice};
use tabula_presentation::{
    AudioCues, Dpi, FrameCtx, GamePresentation, InputEvent, RenderList, Viewport,
};

/// Validates platform-reported display dimensions and device-pixel scale.
///
/// Returns `None` if the platform measurement is non-positive or non-finite
/// (for example during initial canvas attach, window minimization, or rapid browser resizing).
/// The caller should skip or defer rendering for that frame rather than constructing an invalid
/// frame context.
#[must_use]
pub fn resolve_display_geometry(
    width: f32,
    height: f32,
    dpi_scale: f32,
) -> Option<(Viewport, Dpi)> {
    let viewport = Viewport::new(glam::Vec2::new(width, height)).ok()?;
    let dpi = Dpi::new(dpi_scale).ok()?;
    Some((viewport, dpi))
}

/// One recorded canonical input attempt, ready for the replay writer in the
/// next local-runtime increment (doc 00 §3.1).
///
/// The typed input is deterministic replay data: `Input` and its game command
/// are canonical serializable values. This runtime deliberately does not choose
/// a replay file format.
#[derive(Clone, Debug)]
pub struct RecordedInput<C> {
    /// The unique input-log/RNG-domain ordinal consumed by this attempt.
    pub index: InputIndex,
    /// The monotonic logical time supplied to the rules transition.
    pub now: LogicalTime,
    /// The complete canonical input, including timer and bot-originated input.
    pub input: Input<C>,
}

/// A bot request emitted by rules and awaiting a local executor.
///
/// A caller that chooses to execute this request must feed its answer through
/// [`LocalMatch::submit_bot_move`], never mutate match state directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalBotRequest {
    pub seat: SeatId,
    pub deadline: Millis,
}

/// A non-authoritative local notice together with the audience selected by rules.
#[derive(Clone, Debug)]
pub struct LocalNotice {
    pub audience: Audience,
    pub notice: Notice,
}

/// The explicit local result of interpreting a game effect.
///
/// This audit trail makes deliberate Phase 2 no-ops visible rather than
/// silently dropping effects that later phases must execute.
#[derive(Clone, Debug)]
pub enum LocalEffect {
    TimerSet {
        id: TimerId,
        deadline: LogicalTime,
    },
    TimerCancelled {
        id: TimerId,
    },
    MatchEnded {
        outcome: MatchOutcome,
    },
    BotRequested(LocalBotRequest),
    Notified {
        notice: LocalNotice,
    },
    ChatScopesIgnored,
    VoiceScopesIgnored,
    CheckpointIgnored,
    /// A future additive effect has no local Phase 2 executor yet.
    UnknownEffectIgnored,
}

/// Failure raised by the local imperative shell.
#[derive(Clone, Debug)]
pub enum LocalMatchError {
    /// The finite canonical input-index domain cannot allocate another RNG root.
    InputIndexExhausted,
    /// A presentation command was produced for a viewer that cannot occupy a seat.
    ViewerCannotSubmitPlayerInput,
    /// Rules rejected an attempted authoritative input. The attempt remains recorded.
    Rejected(RuleError),
    /// Rules emitted `EndMatch`; no later gameplay input may enter the stream.
    MatchEnded,
    /// Rules emitted a second terminal authority, contrary to the effect contract.
    MultipleEndMatch,
}

/// Failure raised while constructing a local match.
#[derive(Debug)]
pub enum LocalMatchInitError {
    /// Rules rejected the requested config or roster.
    Rules(InitError),
    /// Init effects violated the local effect contract.
    Effects(LocalMatchError),
}

/// A generic local executor for one deterministic game and one presentation.
///
/// `state` is intentionally private. The only values crossing into `P` are a
/// projected `View`, view events, local presentation state, and frame facts
/// (I-5, I-6, I-10). The shell owns input numbering, logical-time clamping,
/// timers, and interpreting returned effects; it has no game-specific branches.
///
/// @ai.role imperative-shell
/// @ai.domain client.local-match
/// @ai.pure false
/// @ai.invariant projection-only-presenter-input
/// @ai.invariant monotonic-logical-time
/// @ai.invariant input-index-per-attempt
/// @ai.invariant timers-reenter-through-canonical-input-stream
#[allow(clippy::doc_markdown)]
pub struct LocalMatch<R, P>
where
    R: GameRules,
    P: GamePresentation<Rules = R>,
{
    state: R::State,
    view: R::View,
    local: P::Local,
    seed: MatchSeed,
    viewer: Viewer,
    now: LogicalTime,
    next_input_index: Option<InputIndex>,
    timers: BTreeMap<TimerId, LogicalTime>,
    ended: Option<MatchOutcome>,
    recorded_inputs: Vec<RecordedInput<R::Command>>,
    effects: Vec<LocalEffect>,
    bot_requests: Vec<LocalBotRequest>,
    notices: Vec<LocalNotice>,
    _presentation: PhantomData<P>,
}

impl<R, P> core::fmt::Debug for LocalMatch<R, P>
where
    R: GameRules,
    P: GamePresentation<Rules = R>,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LocalMatch")
            .field("viewer", &self.viewer)
            .field("now", &self.now)
            .field("next_input_index", &self.next_input_index)
            .field("timer_count", &self.timers.len())
            .field("is_ended", &self.ended.is_some())
            .field("recorded_input_count", &self.recorded_inputs.len())
            .finish_non_exhaustive()
    }
}

impl<R, P> LocalMatch<R, P>
where
    R: GameRules,
    P: GamePresentation<Rules = R>,
{
    /// Creates a local match at logical time zero and interprets its init effects.
    ///
    /// Match creation owns input index zero; all later canonical attempts start
    /// at one, matching the server/replay input-domain convention.
    pub fn new(
        config: &R::Config,
        roster: &tabula_core::SeatRoster,
        seed: MatchSeed,
        viewer: Viewer,
    ) -> Result<Self, LocalMatchInitError> {
        let mut rng = DetRng::for_input(&seed, InputIndex(0));
        let mut ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut rng,
            budget: Budget::default(),
        };
        let init = R::create(config, roster, &mut ctx).map_err(LocalMatchInitError::Rules)?;
        let view = R::project(&init.state, viewer);
        let mut local_match = Self {
            state: init.state,
            view,
            local: P::Local::default(),
            seed,
            viewer,
            now: LogicalTime::ZERO,
            next_input_index: Some(InputIndex(1)),
            timers: BTreeMap::new(),
            ended: None,
            recorded_inputs: Vec::new(),
            effects: Vec::new(),
            bot_requests: Vec::new(),
            notices: Vec::new(),
            _presentation: PhantomData,
        };
        local_match
            .interpret_effects(&init.effects)
            .map_err(LocalMatchInitError::Effects)?;
        Ok(local_match)
    }

    /// Advances the explicit frame-time boundary and fires all due timers.
    ///
    /// `FrameCtx::now_ms` is presentation time. The runtime converts it once at
    /// this boundary and clamps it with the previous logical time, so a bad or
    /// stalled source can never move canonical time backwards.
    pub fn advance_frame(&mut self, frame: &FrameCtx) -> Result<AudioCues, LocalMatchError> {
        self.advance_to(LogicalTime(frame.now_ms()), frame)
    }

    /// Advances to an already-resolved logical time, clamping regressions.
    pub fn advance_to(
        &mut self,
        requested: LogicalTime,
        frame: &FrameCtx,
    ) -> Result<AudioCues, LocalMatchError> {
        let target = self.now.max(requested);
        let mut cues = AudioCues::new();
        while self.ended.is_none() {
            let Some((id, deadline)) = self.next_due_timer(target) else {
                break;
            };
            // A late frame delivers the timer at its requested logical
            // deadline, matching `tabula-testkit::selfplay` rather than the
            // renderer's sampling cadence.
            self.now = deadline;
            self.timers.remove(&id);
            cues.extend(self.apply_canonical(Input::Timer { timer: id }, frame)?);
        }
        self.now = target;
        Ok(cues)
    }

    /// Passes normalized UI input through the presenter and, when it emits a
    /// command, into the ordinary canonical `Input::Player` stream.
    ///
    /// Due timers are processed before presentation input. UI-only interactions
    /// that produce no intent consume no index.
    pub fn handle_presentation_input(
        &mut self,
        input: &InputEvent,
        frame: &FrameCtx,
    ) -> Result<AudioCues, LocalMatchError> {
        let mut cues = self.advance_frame(frame)?;
        let Some(intent) = P::on_input(input, &self.view, &mut self.local) else {
            return Ok(cues);
        };
        let Some(seat) = self.viewer.seat() else {
            return Err(LocalMatchError::ViewerCannotSubmitPlayerInput);
        };
        cues.extend(self.apply_canonical(
            Input::Player {
                seat,
                command: intent.into_command(),
            },
            frame,
        )?);
        Ok(cues)
    }

    /// Submits a typed canonical input through `GameRules::apply`.
    pub fn submit_input(
        &mut self,
        input: Input<R::Command>,
        frame: &FrameCtx,
    ) -> Result<AudioCues, LocalMatchError> {
        let mut cues = self.advance_frame(frame)?;
        cues.extend(self.apply_canonical(input, frame)?);
        Ok(cues)
    }

    /// Returns a locally executed bot command through the ordinary player path.
    pub fn submit_bot_move(
        &mut self,
        seat: SeatId,
        command: R::Command,
        frame: &FrameCtx,
    ) -> Result<AudioCues, LocalMatchError> {
        self.submit_input(Input::Player { seat, command }, frame)
    }

    /// Switches the authorised projection shown to this local presentation.
    pub fn set_viewer(&mut self, viewer: Viewer) {
        self.viewer = viewer;
        self.rebuild_view();
    }

    /// Builds the renderer-neutral frame from the current projection.
    #[must_use]
    pub fn present(&self, frame: &FrameCtx) -> RenderList {
        P::present(&self.view, &self.local, frame)
    }

    /// The current redacted projection; canonical state is deliberately absent.
    #[must_use]
    pub const fn view(&self) -> &R::View {
        &self.view
    }

    /// The selected viewer used for projection and presentation commands.
    #[must_use]
    pub const fn viewer(&self) -> Viewer {
        self.viewer
    }

    /// Local presentation state, for game-specific frame facts such as viewport.
    pub fn local_mut(&mut self) -> &mut P::Local {
        &mut self.local
    }

    /// The current monotonic canonical logical time.
    #[must_use]
    pub const fn now(&self) -> LogicalTime {
        self.now
    }

    /// Inputs attempted through the one canonical stream, in allocation order.
    #[must_use]
    pub fn recorded_inputs(&self) -> &[RecordedInput<R::Command>] {
        &self.recorded_inputs
    }

    /// Explicit local effect interpretations, in the rules' emitted order.
    #[must_use]
    pub fn effects(&self) -> &[LocalEffect] {
        &self.effects
    }

    /// Drains local bot work requests without granting bots a second mutation path.
    pub fn drain_bot_requests(&mut self) -> impl Iterator<Item = LocalBotRequest> + '_ {
        self.bot_requests.drain(..)
    }

    /// Drains non-authoritative user notices emitted by the rules.
    pub fn drain_notices(&mut self) -> impl Iterator<Item = LocalNotice> + '_ {
        self.notices.drain(..)
    }

    /// The terminal outcome after `Effect::EndMatch`, if rules emitted one.
    #[must_use]
    pub fn ended(&self) -> Option<&MatchOutcome> {
        self.ended.as_ref()
    }

    #[cfg(test)]
    fn state_hash(&self) -> StateHash {
        R::state_hash(&self.state)
    }

    #[cfg(test)]
    fn set_next_input_index_for_test(&mut self, index: Option<InputIndex>) {
        self.next_input_index = index;
    }

    fn apply_canonical(
        &mut self,
        input: Input<R::Command>,
        frame: &FrameCtx,
    ) -> Result<AudioCues, LocalMatchError> {
        if self.ended.is_some() {
            return Err(LocalMatchError::MatchEnded);
        }
        let index = self.take_input_index()?;
        self.recorded_inputs.push(RecordedInput {
            index,
            now: self.now,
            input: input.clone(),
        });
        let mut rng = DetRng::for_input(&self.seed, index);
        let mut ctx = Ctx {
            now: self.now,
            index,
            rng: &mut rng,
            budget: Budget::default(),
        };
        let outcome =
            R::apply(&mut self.state, input, &mut ctx).map_err(LocalMatchError::Rejected)?;

        let mut cues = AudioCues::new();
        for event in &outcome.events {
            if let Some(event) = R::view_event(&self.state, event, self.viewer) {
                cues.extend(P::on_view_event(&event, &mut self.local, frame));
            }
        }
        self.interpret_effects(&outcome.effects)?;
        self.rebuild_view();
        Ok(cues)
    }

    fn take_input_index(&mut self) -> Result<InputIndex, LocalMatchError> {
        let index = self
            .next_input_index
            .take()
            .ok_or(LocalMatchError::InputIndexExhausted)?;
        self.next_input_index = index.0.checked_add(1).map(InputIndex);
        Ok(index)
    }

    fn next_due_timer(&self, target: LogicalTime) -> Option<(TimerId, LogicalTime)> {
        self.timers
            .iter()
            .filter(|(_, deadline)| **deadline <= target)
            .map(|(id, deadline)| (*id, *deadline))
            .min_by_key(|(id, deadline)| (*deadline, *id))
    }

    pub fn interpret_effects(&mut self, effects: &[Effect]) -> Result<(), LocalMatchError> {
        for effect in effects {
            match effect {
                Effect::SetTimer { id, delay } => {
                    let deadline = self.now.plus(*delay);
                    self.timers.insert(*id, deadline);
                    self.effects
                        .push(LocalEffect::TimerSet { id: *id, deadline });
                }
                Effect::CancelTimer { id } => {
                    self.timers.remove(id);
                    self.effects.push(LocalEffect::TimerCancelled { id: *id });
                }
                Effect::EndMatch { outcome } => {
                    if self.ended.is_some() {
                        return Err(LocalMatchError::MultipleEndMatch);
                    }
                    self.ended = Some(outcome.clone());
                    self.effects.push(LocalEffect::MatchEnded {
                        outcome: outcome.clone(),
                    });
                }
                Effect::RequestBotMove { seat, deadline } => {
                    let request = LocalBotRequest {
                        seat: *seat,
                        deadline: *deadline,
                    };
                    self.bot_requests.push(request);
                    self.effects.push(LocalEffect::BotRequested(request));
                }
                Effect::Notify { audience, notice } => {
                    let local_notice = LocalNotice {
                        audience: audience.clone(),
                        notice: notice.clone(),
                    };
                    self.notices.push(local_notice.clone());
                    self.effects.push(LocalEffect::Notified {
                        notice: local_notice,
                    });
                }
                // These effects have no Phase 2 local backend. Retaining an
                // explicit trace is the deliberate behaviour, not a drop.
                Effect::SetChatScopes(_) => self.effects.push(LocalEffect::ChatScopesIgnored),
                Effect::SetVoiceScopes(_) => self.effects.push(LocalEffect::VoiceScopesIgnored),
                Effect::Checkpoint { .. } => self.effects.push(LocalEffect::CheckpointIgnored),
                _ => self.effects.push(LocalEffect::UnknownEffectIgnored),
            }
        }
        Ok(())
    }

    fn rebuild_view(&mut self) {
        self.view = R::project(&self.state, self.viewer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use local_game::{
        presentation::{BoardLayout, ChessPresentation},
        ChessRules, ClockConfig, ClockControl, Config, PieceKind, Square,
    };
    use renderer_macroquad::MacroquadRenderer;
    use tabula_core::{Occupant, SeatEntry, SeatRoster, UserId};
    use tabula_game_api::{ChatScopes, CheckpointLabel, VoiceScopes};
    use tabula_game_chess as local_game; // xtask-allow-game-id: direct Phase 2 local vertical slice test wiring.
    use tabula_presentation::{
        AudioCue, AudioSink, Dpi, PointerButton, PointerPhase, PointerPosition, Viewport,
    };

    type ChessMatch = LocalMatch<ChessRules, ChessPresentation>;

    fn frame(now_ms: u64) -> FrameCtx {
        FrameCtx::new(
            Viewport::new(Vec2::splat(640.0)).expect("test viewport is valid"),
            Dpi::new(1.0).expect("test DPI is valid"),
            now_ms,
            tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light),
        )
    }

    fn roster() -> SeatRoster {
        SeatRoster::new(
            [
                SeatEntry {
                    seat: SeatId(0),
                    occupant: Occupant::Human(UserId(1)),
                    team: None,
                },
                SeatEntry {
                    seat: SeatId(1),
                    occupant: Occupant::Human(UserId(2)),
                    team: None,
                },
            ]
            .into_iter()
            .collect(),
        )
        .expect("local seats are unique")
    }

    fn match_for_rules(config: &Config) -> ChessMatch {
        ChessMatch::new(
            config,
            &roster(),
            MatchSeed::from_bytes([0; 32]),
            Viewer::Seat(SeatId(0)),
        )
        .expect("standard local rules configuration is valid")
    }

    fn timed_config(initial: u64) -> Config {
        Config {
            clock: Some(ClockConfig {
                initial: Millis(initial),
                control: ClockControl::Fischer {
                    increment: Millis::ZERO,
                },
            }),
        }
    }

    fn reference_timeout_hash(deadline: LogicalTime) -> StateHash {
        let seed = MatchSeed::from_bytes([0; 32]);
        let config = timed_config(10);
        let mut create_rng = DetRng::for_input(&seed, InputIndex(0));
        let mut create_ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut create_rng,
            budget: Budget::default(),
        };
        let mut state = ChessRules::create(&config, &roster(), &mut create_ctx)
            .expect("clocked reference match is valid")
            .state;
        let mut timer_rng = DetRng::for_input(&seed, InputIndex(1));
        let mut timer_ctx = Ctx {
            now: deadline,
            index: InputIndex(1),
            rng: &mut timer_rng,
            budget: Budget::default(),
        };
        ChessRules::apply(
            &mut state,
            Input::Timer { timer: TimerId(1) },
            &mut timer_ctx,
        )
        .expect("the reference timer is accepted");
        ChessRules::state_hash(&state)
    }

    fn click(layout: BoardLayout, square: u8) -> InputEvent {
        let square = Square::new(square).expect("test square is valid");
        let rect = layout
            .square_rect(square)
            .expect("test square has geometry");
        InputEvent::Pointer {
            position: PointerPosition::new(rect.origin() + rect.size() * 0.5)
                .expect("test pointer is finite"),
            button: PointerButton::Primary,
            phase: PointerPhase::Up,
        }
    }

    fn prepare(match_: &mut ChessMatch, frame: &FrameCtx) {
        match_.local_mut().set_viewport(frame.viewport());
        match_.advance_frame(frame).expect("frame time is accepted");
    }

    #[test]
    fn presenter_command_goes_through_game_rules_apply() {
        let frame = frame(0);
        let layout = BoardLayout::from_viewport(frame.viewport());
        let mut match_ = match_for_rules(&Config::default());
        prepare(&mut match_, &frame);

        match_
            .handle_presentation_input(&click(layout, 12), &frame)
            .expect("selection is local only");
        match_
            .handle_presentation_input(&click(layout, 28), &frame)
            .expect("legal move is accepted");

        assert_eq!(match_.view().board[12], None);
        assert_eq!(match_.view().board[28].unwrap().kind, PieceKind::Pawn);
        assert_eq!(match_.recorded_inputs().len(), 1);
        assert_eq!(match_.recorded_inputs()[0].index, InputIndex(1));
    }

    #[test]
    fn ui_only_interaction_consumes_no_canonical_input_index() {
        let frame = frame(0);
        let layout = BoardLayout::from_viewport(frame.viewport());
        let mut match_ = match_for_rules(&Config::default());
        prepare(&mut match_, &frame);

        match_
            .handle_presentation_input(&click(layout, 12), &frame)
            .expect("selection has no command");
        assert!(match_.recorded_inputs().is_empty());
    }

    #[test]
    fn accepted_and_rejected_inputs_each_consume_one_index() {
        let frame = frame(0);
        let layout = BoardLayout::from_viewport(frame.viewport());
        let mut match_ = match_for_rules(&Config::default());
        prepare(&mut match_, &frame);

        match_
            .handle_presentation_input(&click(layout, 12), &frame)
            .expect("selection is local only");
        let rejected = match_
            .handle_presentation_input(&click(layout, 36), &frame)
            .expect_err("illegal move must be propagated");
        assert!(matches!(
            rejected,
            LocalMatchError::Rejected(RuleError {
                code: tabula_core::RuleErrorCode::IllegalMove,
                ..
            })
        ));
        match_
            .handle_presentation_input(&click(layout, 12), &frame)
            .expect("selection remains local only");
        match_
            .handle_presentation_input(&click(layout, 28), &frame)
            .expect("legal move is accepted");

        assert_eq!(
            match_
                .recorded_inputs()
                .iter()
                .map(|entry| entry.index)
                .collect::<Vec<_>>(),
            [InputIndex(1), InputIndex(2)]
        );
    }

    #[test]
    fn exhausted_input_index_never_wraps_or_reuses_rng_domain() {
        let frame = frame(0);
        let layout = BoardLayout::from_viewport(frame.viewport());
        let mut match_ = match_for_rules(&Config::default());
        match_.set_next_input_index_for_test(Some(InputIndex(u64::MAX)));
        prepare(&mut match_, &frame);

        match_
            .handle_presentation_input(&click(layout, 12), &frame)
            .expect("selection is local only");
        match_
            .handle_presentation_input(&click(layout, 28), &frame)
            .expect("maximum input index is usable once");
        match_.set_viewer(Viewer::Seat(SeatId(1)));
        match_.local_mut().set_viewport(frame.viewport());
        match_
            .handle_presentation_input(&click(layout, 52), &frame)
            .expect("selection remains local after exhaustion");
        assert!(matches!(
            match_.handle_presentation_input(&click(layout, 36), &frame),
            Err(LocalMatchError::InputIndexExhausted)
        ));
    }

    #[test]
    fn logical_time_is_monotonic_for_stalled_and_backwards_frames() {
        let mut match_ = match_for_rules(&Config::default());
        match_.advance_frame(&frame(12)).expect("time advances");
        match_
            .advance_frame(&frame(12))
            .expect("stalled time is valid");
        match_
            .advance_frame(&frame(7))
            .expect("backwards time clamps");
        assert_eq!(match_.now(), LogicalTime(12));
    }

    #[test]
    fn clock_timer_reenters_rules_as_a_recorded_timer_input() {
        let mut match_ = match_for_rules(&timed_config(10));
        match_
            .advance_frame(&frame(10))
            .expect("due clock timer fires");

        assert!(matches!(
            match_.recorded_inputs(),
            [RecordedInput {
                input: Input::Timer { .. },
                now: LogicalTime(10),
                ..
            }]
        ));
        assert!(match_.ended().is_some());
        assert!(matches!(
            match_.effects().last(),
            Some(LocalEffect::MatchEnded { .. })
        ));
    }

    #[test]
    fn late_frame_replays_the_timer_at_its_deadline_not_frame_arrival() {
        let mut match_ = match_for_rules(&timed_config(10));
        match_
            .advance_frame(&frame(20))
            .expect("late frame processes the due timer");

        assert!(matches!(
            match_.recorded_inputs(),
            [RecordedInput {
                input: Input::Timer { timer: TimerId(1) },
                now: LogicalTime(10),
                ..
            }]
        ));
        assert_eq!(match_.now(), LogicalTime(20));
        // Independent reference scheduling follows the `selfplay` rule: apply
        // the timer at its logical deadline, not the sampled render frame.
        assert_eq!(match_.state_hash(), reference_timeout_hash(LogicalTime(10)));
    }

    #[test]
    fn public_bot_entry_cannot_overtake_a_due_timer() {
        let frame = frame(10);
        let mut match_ = match_for_rules(&timed_config(10));

        assert!(matches!(
            match_.submit_bot_move(
                SeatId(0),
                local_game::Command::Move {
                    from: 12,
                    to: 28,
                    promotion: None,
                },
                &frame,
            ),
            Err(LocalMatchError::MatchEnded)
        ));
        assert!(matches!(
            match_.recorded_inputs(),
            [RecordedInput {
                input: Input::Timer { timer: TimerId(1) },
                now: LogicalTime(10),
                ..
            }]
        ));
    }

    #[test]
    fn public_input_entry_cannot_overtake_a_due_timer() {
        let frame = frame(10);
        let mut match_ = match_for_rules(&timed_config(10));

        assert!(matches!(
            match_.submit_input(
                Input::Player {
                    seat: SeatId(0),
                    command: local_game::Command::Move {
                        from: 12,
                        to: 28,
                        promotion: None,
                    },
                },
                &frame,
            ),
            Err(LocalMatchError::MatchEnded)
        ));
        assert_eq!(match_.recorded_inputs().len(), 1);
        assert!(matches!(
            match_.recorded_inputs()[0],
            RecordedInput {
                input: Input::Timer { timer: TimerId(1) },
                now: LogicalTime(10),
                ..
            }
        ));
    }

    #[test]
    fn cancellation_rearm_and_simultaneous_timer_order_are_stable() {
        let first_frame = frame(10);
        let mut match_ = match_for_rules(&Config::default());
        match_
            .interpret_effects(&[
                Effect::SetTimer {
                    id: TimerId(2),
                    delay: Millis(10),
                },
                Effect::SetTimer {
                    id: TimerId(1),
                    delay: Millis(10),
                },
                Effect::SetTimer {
                    id: TimerId(3),
                    delay: Millis(15),
                },
                Effect::CancelTimer { id: TimerId(3) },
                Effect::SetTimer {
                    id: TimerId(3),
                    delay: Millis(20),
                },
            ])
            .expect("effect sequence is valid");
        match_
            .advance_frame(&first_frame)
            .expect("first timer fires");
        match_
            .advance_frame(&frame(20))
            .expect("re-armed timer fires");

        assert_eq!(
            match_
                .recorded_inputs()
                .iter()
                .filter_map(|entry| match entry.input {
                    Input::Timer { timer } => Some((timer, entry.now)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [
                (TimerId(1), LogicalTime(10)),
                (TimerId(2), LogicalTime(10)),
                (TimerId(3), LogicalTime(20)),
            ]
        );
    }

    #[test]
    fn every_effect_has_an_explicit_local_interpretation_in_emitted_order() {
        let mut match_ = match_for_rules(&Config::default());
        let notice = Notice {
            key: "local.notice".into(),
            #[allow(clippy::default_trait_access)]
            args: Default::default(),
        };
        match_
            .interpret_effects(&[
                Effect::SetTimer {
                    id: TimerId(3),
                    delay: Millis(1),
                },
                Effect::CancelTimer { id: TimerId(3) },
                Effect::RequestBotMove {
                    seat: SeatId(0),
                    deadline: Millis(2),
                },
                Effect::Notify {
                    audience: tabula_core::Audience::Everyone,
                    notice: notice.clone(),
                },
                Effect::SetChatScopes(ChatScopes::default()),
                Effect::SetVoiceScopes(VoiceScopes::default()),
                Effect::Checkpoint {
                    label: CheckpointLabel("local".into()),
                },
            ])
            .expect("effect sequence is valid");

        assert!(matches!(
            match_.effects(),
            [
                LocalEffect::TimerSet { .. },
                LocalEffect::TimerCancelled { .. },
                LocalEffect::BotRequested(_),
                LocalEffect::Notified { .. },
                LocalEffect::ChatScopesIgnored,
                LocalEffect::VoiceScopesIgnored,
                LocalEffect::CheckpointIgnored,
            ]
        ));
        assert_eq!(match_.drain_bot_requests().count(), 1);
        let notices = match_.drain_notices().collect::<Vec<_>>();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].notice.key, notice.key);
    }

    #[test]
    fn duplicate_end_match_is_rejected_without_overwriting_the_first_outcome() {
        let mut match_ = match_for_rules(&timed_config(10));
        match_
            .advance_frame(&frame(10))
            .expect("clock timer ends the match");
        let first = match_
            .ended()
            .expect("first terminal outcome exists")
            .clone();

        assert!(matches!(
            match_.interpret_effects(&[Effect::EndMatch {
                outcome: first.clone(),
            }]),
            Err(LocalMatchError::MultipleEndMatch)
        ));
        assert_eq!(
            match_.ended().expect("first outcome remains").summary(),
            first.summary()
        );
    }

    #[test]
    fn locally_executed_bot_move_reuses_the_player_input_path() {
        let frame = frame(0);
        let mut match_ = match_for_rules(&Config::default());
        let command = local_game::Command::Move {
            from: 12,
            to: 28,
            promotion: None,
        };
        match_
            .submit_bot_move(SeatId(0), command, &frame)
            .expect("bot command uses ordinary rules path");
        assert!(matches!(
            match_.recorded_inputs()[0].input,
            Input::Player {
                seat: SeatId(0),
                ..
            }
        ));
    }

    #[test]
    fn projection_is_rebuilt_from_state_after_accepted_transition() {
        let frame = frame(0);
        let mut match_ = match_for_rules(&Config::default());
        match_
            .submit_bot_move(
                SeatId(0),
                local_game::Command::Move {
                    from: 12,
                    to: 28,
                    promotion: None,
                },
                &frame,
            )
            .expect("move is legal");
        assert_eq!(match_.view().board[12], None);
        assert_eq!(match_.view().board[28].unwrap().kind, PieceKind::Pawn);
    }

    #[test]
    fn presenter_produces_a_macroquad_supported_render_list() {
        let match_ = match_for_rules(&Config::default());
        let frame = frame(0);
        assert_eq!(
            MacroquadRenderer::preflight(&match_.present(&frame), &frame),
            Ok(())
        );
    }

    #[test]
    fn audio_sink_failure_cannot_undo_an_accepted_move() {
        struct UnavailableSink;
        impl AudioSink for UnavailableSink {
            type Error = ();
            fn play(&mut self, _: &AudioCue) -> Result<(), Self::Error> {
                Err(())
            }
        }

        let frame = frame(0);
        let mut match_ = match_for_rules(&Config::default());
        let cues = match_
            .submit_bot_move(
                SeatId(0),
                local_game::Command::Move {
                    from: 12,
                    to: 28,
                    promotion: None,
                },
                &frame,
            )
            .expect("move is accepted before audio playback");
        let mut sink = UnavailableSink;
        for cue in &cues {
            assert_eq!(sink.play(cue), Err(()));
        }
        assert_eq!(match_.view().board[12], None);
    }

    #[test]
    fn display_geometry_accepts_positive_finite_viewport_and_dpi() {
        let geometry = resolve_display_geometry(800.0, 600.0, 2.0);
        let (viewport, dpi) = geometry.expect("positive geometry is valid");
        assert_eq!(viewport.size(), glam::Vec2::new(800.0, 600.0));
        assert!((dpi.get() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn display_geometry_rejects_zero_or_negative_dimensions() {
        assert!(resolve_display_geometry(0.0, 600.0, 1.0).is_none());
        assert!(resolve_display_geometry(800.0, 0.0, 1.0).is_none());
        assert!(resolve_display_geometry(0.0, 0.0, 1.0).is_none());
        assert!(resolve_display_geometry(-100.0, 600.0, 1.0).is_none());
        assert!(resolve_display_geometry(800.0, -100.0, 1.0).is_none());
        assert!(resolve_display_geometry(f32::NAN, 600.0, 1.0).is_none());
        assert!(resolve_display_geometry(800.0, f32::INFINITY, 1.0).is_none());
    }

    #[test]
    fn display_geometry_rejects_non_finite_or_zero_dpi() {
        assert!(resolve_display_geometry(800.0, 600.0, 0.0).is_none());
        assert!(resolve_display_geometry(800.0, 600.0, -1.0).is_none());
        assert!(resolve_display_geometry(800.0, 600.0, f32::NAN).is_none());
        assert!(resolve_display_geometry(800.0, 600.0, f32::INFINITY).is_none());
    }

    #[test]
    fn transient_invalid_viewport_resumes_without_state_mutation() {
        let mut match_ = match_for_rules(&Config::default());
        let initial_hash = match_.state_hash();

        // Simulate transient zero during browser startup / resize.
        let transient_geom = resolve_display_geometry(0.0, 0.0, 1.0);
        assert!(transient_geom.is_none());
        assert_eq!(match_.state_hash(), initial_hash);
        assert!(match_.recorded_inputs().is_empty());

        // Platform geometry becomes valid on next frame.
        let (viewport, dpi) = resolve_display_geometry(640.0, 640.0, 1.0).expect("valid geometry");
        let frame = FrameCtx::new(
            viewport,
            dpi,
            16,
            tabula_design::Theme::by_kind(tabula_design::ThemeKind::Light),
        );
        match_
            .advance_frame(&frame)
            .expect("advancing frame succeeds");
        assert_eq!(match_.now(), LogicalTime(16));
        assert_eq!(match_.state_hash(), initial_hash);
    }
}
