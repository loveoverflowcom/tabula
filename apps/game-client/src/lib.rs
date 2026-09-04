//! # `tabula-game-client` — the Macroquad gameplay runtime
//!
//! Phase 2 owns one local imperative shell around the deterministic game
//! contract. Presenters receive projections and local state only; every
//! authoritative mutation returns through [`GameRules::apply`].

#![forbid(unsafe_code)]

mod replay_capture;

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

pub use replay_capture::{AcceptedReplayInput, LocalReplayTrace, RecordedInput};

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
    replay: LocalReplayTrace<R::Command>,
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
            .field(
                "accepted_replay_count",
                &self.replay.accepted_inputs().len(),
            )
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
        // Captured before `init.state` moves into the struct: this is replay
        // evidence's own checkpoint, distinguishing a divergence at `create`
        // from a divergence at the first accepted input (this crate's replay
        // contract) — never derived from the presenter-facing `view`.
        let initial_state_hash = R::state_hash(&init.state);
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
            replay: LocalReplayTrace::new(initial_state_hash),
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

    /// Inputs attempted through the one canonical stream, in allocation
    /// order — every attempt, accepted or rejected. See
    /// [`replay_trace`](Self::replay_trace) for accepted-only replay
    /// evidence.
    #[must_use]
    pub fn recorded_inputs(&self) -> &[RecordedInput<R::Command>] {
        &self.recorded_inputs
    }

    /// Deterministic canonical replay evidence recorded so far: only the
    /// inputs `GameRules::apply` actually accepted, each carrying its
    /// post-transition checkpoint hash. Independent of presentation,
    /// rendering, and local effect interpretation — recorded the instant
    /// `apply` returns `Ok`, before either can run.
    #[must_use]
    pub const fn replay_trace(&self) -> &LocalReplayTrace<R::Command> {
        &self.replay
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

    /// Allocates the next canonical input index, records the attempt, and —
    /// only if `GameRules::apply` returns `Ok` — commits accepted replay
    /// evidence before doing anything else with the outcome.
    ///
    /// # Why replay evidence is recorded here, and not later
    ///
    /// Acceptance is a fact about `GameRules::apply` alone. Everything after
    /// it in this function — `view_event` dispatch, local effect
    /// interpretation — is shell-level bookkeeping that can itself fail
    /// (`interpret_effects` returns `Err(LocalMatchError::MultipleEndMatch)`
    /// if rules ever emit two terminal effects in one outcome). A shell-level
    /// failure is a *separate* contract violation; it must never retroactively
    /// make an accepted input disappear from replay evidence, so the
    /// [`AcceptedReplayInput`] is pushed immediately after `apply` succeeds,
    /// before the `?` that can end this function early.
    fn apply_canonical(
        &mut self,
        input: Input<R::Command>,
        frame: &FrameCtx,
    ) -> Result<AudioCues, LocalMatchError> {
        if self.ended.is_some() {
            return Err(LocalMatchError::MatchEnded);
        }
        let index = self.take_input_index()?;
        let attempt_input = input.clone();
        self.recorded_inputs.push(RecordedInput {
            index,
            now: self.now,
            input: attempt_input.clone(),
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

        // Accepted. Commit replay evidence now — see the doc comment above.
        self.replay.record(AcceptedReplayInput::new(
            index,
            self.now,
            attempt_input,
            R::state_hash(&self.state),
        ));

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

    fn interpret_effects(&mut self, effects: &[Effect]) -> Result<(), LocalMatchError> {
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

// Verification ledger for the replay-evidence properties this file proves
// (`rust-verification-testing`, `rust-replay-differential-testing`):
//
// R1  attempt ordering: every canonical attempt gets one unique monotonic
//     InputIndex, accepted or rejected                         (example-tested; preexisting)
// R2  replay-frame eligibility: only an `apply`-accepted attempt becomes an
//     `AcceptedReplayInput`, never derived from `recorded_inputs()`
//                                                                (example-tested)
// R3  original index preservation: an accepted entry keeps the exact index
//     `apply` was called with, even across a rejection gap        (example-tested;
//                                                                 negative-controlled)
// R4  logical-time preservation: a replay entry carries the exact
//     `ctx.now` `apply` used, never a later frame-arrival time    (example-tested;
//                                                                 negative-controlled)
// R5  live/replay equivalence: a fresh `GameRules::create`/`apply`
//     reconstruction from recorded evidence reproduces every checkpoint and
//     the final hash                                     (self-differentially replay-tested)
//
// R5's independence comes from using a separate, deliberately smaller
// reconstruction path (`replay`, below) — never `LocalMatch` itself, never
// its timer scheduler, never its own replay-capture code — not from a
// second `GameRules` implementation of chess or tic-tac-toe.
#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use local_game::{
        presentation::{BoardLayout, ChessPresentation},
        ChessRules, ClockConfig, ClockControl, Config, PieceKind, Square,
    };
    use renderer_macroquad::MacroquadRenderer;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use tabula_core::{Occupant, RulesVersion, SeatEntry, SeatRoster, UserId};
    use tabula_game_api::{
        A11yDescription, ChatScopes, CheckpointLabel, Init, Outcome, VoiceScopes,
    };
    use tabula_game_chess as local_game; // xtask-allow-game-id: direct Phase 2 local vertical slice test wiring.
    use tabula_game_tictactoe as second_game; // xtask-allow-game-id: proves the replay recorder is generic, not chess-shaped.
    use tabula_presentation::{
        AssetPackRef, AudioCue, AudioCues, AudioSink, Camera2D, Dpi, InputEvent, Intent,
        PointerButton, PointerPhase, PointerPosition, RenderListBuilder, Viewport,
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

    /// The self-differential replay oracle (R5). Reconstructs a canonical
    /// state from [`LocalReplayTrace`] evidence, checking every recorded
    /// checkpoint, using ONLY the deterministic core `LocalMatch` is built
    /// on top of (`GameRules::create`/`apply`, `DetRng::for_input`) — never
    /// `LocalMatch` itself, its timer scheduler, or its replay-capture code.
    /// Deliberately smaller and independent, per `rust-replay-differential-testing`.
    ///
    /// Returns the final canonical hash and the terminal `MatchOutcome`, if
    /// creation or an accepted input's outcome carried `Effect::EndMatch`.
    ///
    /// # Panics
    /// If a recorded input is rejected by fresh `apply` (every entry's own
    /// invariant is that live accepted it), or if any checkpoint disagrees
    /// with its recorded post-apply hash — naming the diverging `InputIndex`.
    fn replay<R: GameRules>(
        config: &R::Config,
        roster: &SeatRoster,
        seed: &MatchSeed,
        trace: &LocalReplayTrace<R::Command>,
    ) -> (StateHash, Option<MatchOutcome>) {
        let mut create_rng = DetRng::for_input(seed, InputIndex(0));
        let mut create_ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut create_rng,
            budget: Budget::default(),
        };
        let init = R::create(config, roster, &mut create_ctx)
            .expect("live create succeeded, so an independent reconstruction must too");
        let mut terminal = terminal_from_effects(&init.effects);
        let mut state = init.state;
        assert_eq!(
            R::state_hash(&state),
            trace.initial_state_hash(),
            "replay divergence at create: independent reconstruction disagrees with the live \
             initial hash"
        );

        for entry in trace.accepted_inputs() {
            assert!(
                terminal.is_none(),
                "replay input {:?} follows a terminal outcome",
                entry.index()
            );
            let mut rng = DetRng::for_input(seed, entry.index());
            let mut ctx = Ctx {
                now: entry.now(),
                index: entry.index(),
                rng: &mut rng,
                budget: Budget::default(),
            };
            let outcome = R::apply(&mut state, entry.input().clone(), &mut ctx).expect(
                "live accepted this input, so an independent reconstruction must accept it too",
            );
            assert_eq!(
                R::state_hash(&state),
                entry.state_hash(),
                "replay divergence at input index {:?}",
                entry.index()
            );
            if let Some(input_terminal) = terminal_from_effects(&outcome.effects) {
                assert!(
                    terminal.is_none(),
                    "replay observed more than one terminal outcome"
                );
                terminal = Some(input_terminal);
            }
        }

        (R::state_hash(&state), terminal)
    }

    fn terminal_from_effects(effects: &[Effect]) -> Option<MatchOutcome> {
        let mut terminal = None;
        for effect in effects {
            if let Effect::EndMatch { outcome } = effect {
                assert!(
                    terminal.is_none(),
                    "replay observed multiple EndMatch effects in one effect list"
                );
                terminal = Some(outcome.clone());
            }
        }
        terminal
    }

    fn create_terminal_outcome() -> MatchOutcome {
        MatchOutcome::new(
            tabula_core::OutcomeKind::Aborted {
                reason: tabula_core::AbortReason::OperatorCancelled,
            },
            std::iter::empty().collect(),
            "created terminal".into(),
        )
        .expect("the test terminal outcome is structurally valid")
    }

    struct CreateTerminalRules;

    impl GameRules for CreateTerminalRules {
        type State = u8;
        type Command = ();
        type Event = ();
        type View = ();
        type ViewEvent = ();
        type Config = ();

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        fn create(
            _config: &(),
            _roster: &SeatRoster,
            _ctx: &mut Ctx<'_>,
        ) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: 0,
                events: std::iter::empty().collect(),
                effects: std::iter::once(Effect::EndMatch {
                    outcome: create_terminal_outcome(),
                })
                .collect(),
            })
        }

        fn apply(
            _state: &mut u8,
            _input: Input<()>,
            _ctx: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            Ok(Outcome::empty())
        }

        fn project(_state: &u8, _viewer: Viewer) {}

        fn view_event(_state_after: &u8, _event: &(), _viewer: Viewer) -> Option<()> {
            None
        }
    }

    struct CreateTerminalPresentation;

    impl GamePresentation for CreateTerminalPresentation {
        type Rules = CreateTerminalRules;
        type Local = ();

        fn asset_pack() -> AssetPackRef {
            AssetPackRef::from_static("terminal", "0.0.0")
        }

        fn present(_view: &(), _local: &(), _frame: &FrameCtx) -> RenderList {
            RenderListBuilder::new(Camera2D::default())
                .finish()
                .expect("the empty render list is valid")
        }

        fn on_view_event(_event: &(), _local: &mut (), _frame: &FrameCtx) -> AudioCues {
            AudioCues::new()
        }

        fn on_input(_input: &InputEvent, _view: &(), _local: &mut ()) -> Option<Intent<()>> {
            None
        }

        fn a11y(_view: &(), _local: &()) -> A11yDescription {
            A11yDescription::default()
        }
    }

    /// Deliberately minimal, RNG-sensitive `GameRules` used only to prove
    /// that [`replay`] actually depends on the recorded `InputIndex` for
    /// RNG-domain derivation — a property neither chess nor the second game
    /// can demonstrate honestly, since neither ever draws from `ctx.rng`
    /// (inventing that claim for them would be dishonest; see this PR's own
    /// instructions on not fabricating an RNG-sensitivity claim a real game
    /// does not make). Every accepted transition draws one `u32` from the
    /// context RNG and folds it into the running state, so replaying at the
    /// wrong index draws from a different stream position and diverges.
    ///
    /// Exists purely as a negative-control fixture for R3; it is never
    /// wrapped in a `LocalMatch` (no `GamePresentation` exists for it, and
    /// none is needed — the point under test is the replay oracle's
    /// sensitivity, not `LocalMatch`'s wiring, which the chess and second-game
    /// tests already cover).
    struct RngSensitiveRules;

    impl GameRules for RngSensitiveRules {
        type State = u64;
        type Command = ();
        type Event = ();
        type View = ();
        type ViewEvent = ();
        type Config = ();

        const RULES_VERSION: RulesVersion = RulesVersion(1);

        // `SmallVec::default()` would be clearer per clippy, but naming
        // `SmallVec` here would need a `smallvec` dependency this crate does
        // not otherwise have, just for this one test-only fixture.
        #[allow(clippy::default_trait_access)]
        fn create(
            (): &(),
            _roster: &SeatRoster,
            _ctx: &mut Ctx<'_>,
        ) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: 0,
                events: Default::default(),
                effects: Default::default(),
            })
        }

        fn apply(
            state: &mut u64,
            _input: Input<()>,
            ctx: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, RuleError> {
            *state = state.wrapping_add(u64::from(ctx.rng.next_u32()));
            Ok(Outcome::empty())
        }

        fn project(_state: &u64, _viewer: Viewer) {}

        fn view_event(_state_after: &u64, (): &(), _viewer: Viewer) -> Option<()> {
            None
        }
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

        // The presentation-originated command is real replay evidence too
        // (item 15): one accepted entry, at the same index, independently
        // reconstructible.
        let trace = match_.replay_trace();
        assert_eq!(trace.accepted_inputs().len(), 1);
        assert_eq!(trace.accepted_inputs()[0].index(), InputIndex(1));
        let (final_hash, _) = replay::<ChessRules>(
            &Config::default(),
            &roster(),
            &MatchSeed::from_bytes([0; 32]),
            trace,
        );
        assert_eq!(final_hash, match_.state_hash());
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
        assert!(match_.replay_trace().accepted_inputs().is_empty());
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

        // R2/R3: the attempt log has both indices — the rejected move at 1
        // consumed a real InputIndex — but the accepted REPLAY log has only
        // the one `apply` actually accepted, and it keeps the ORIGINAL,
        // now-gapped index rather than being renumbered to 1 (this PR's
        // mandatory rejection-gap theorem, item 13/27).
        let trace = match_.replay_trace();
        assert_eq!(
            trace
                .accepted_inputs()
                .iter()
                .map(AcceptedReplayInput::index)
                .collect::<Vec<_>>(),
            [InputIndex(2)]
        );

        // Replaying just that one gapped-index entry independently
        // reproduces the exact live state.
        let (final_hash, _) = replay::<ChessRules>(
            &Config::default(),
            &roster(),
            &MatchSeed::from_bytes([0; 32]),
            trace,
        );
        assert_eq!(final_hash, match_.state_hash());
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

        // The accepted entry at the maximum index is valid replay evidence,
        // and exhaustion produced no further one (item 18).
        let trace = match_.replay_trace();
        assert_eq!(trace.accepted_inputs().len(), 1);
        assert_eq!(trace.accepted_inputs()[0].index(), InputIndex(u64::MAX));
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
    fn create_end_match_is_replayed_with_zero_accepted_inputs() {
        let seed = MatchSeed::from_bytes([3; 32]);
        let match_ = LocalMatch::<CreateTerminalRules, CreateTerminalPresentation>::new(
            &(),
            &roster(),
            seed.clone(),
            Viewer::Seat(SeatId(0)),
        )
        .expect("creation-terminal fixture is valid");
        let live_terminal = match_
            .ended()
            .cloned()
            .expect("create EndMatch is interpreted by the live shell");

        assert!(match_.replay_trace().accepted_inputs().is_empty());
        assert!(matches!(match_.effects(), [LocalEffect::MatchEnded { .. }]));

        let (final_hash, replay_terminal) =
            replay::<CreateTerminalRules>(&(), &roster(), &seed, match_.replay_trace());
        assert_eq!(final_hash, match_.state_hash());
        assert_eq!(replay_terminal, Some(live_terminal));
    }

    #[test]
    #[should_panic(expected = "multiple EndMatch effects")]
    fn replay_oracle_rejects_multiple_end_match_effects() {
        let outcome = create_terminal_outcome();
        let effects = [
            Effect::EndMatch {
                outcome: outcome.clone(),
            },
            Effect::EndMatch { outcome },
        ];
        let _ = terminal_from_effects(&effects);
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

        // R4 + R5, joined (item 14): the replay entry must carry the timer's
        // own deadline (10), not the late frame that observed it (20), and
        // an independent reconstruction from that entry alone must land on
        // the same final hash AND the same terminal outcome — without
        // duplicating the scheduler on the replay side.
        let trace = match_.replay_trace();
        assert_eq!(trace.accepted_inputs().len(), 1);
        assert_eq!(trace.accepted_inputs()[0].now(), LogicalTime(10));
        let (final_hash, terminal) = replay::<ChessRules>(
            &timed_config(10),
            &roster(),
            &MatchSeed::from_bytes([0; 32]),
            trace,
        );
        assert_eq!(final_hash, match_.state_hash());
        assert_eq!(
            terminal.as_ref(),
            match_.ended(),
            "replayed terminal outcome must match the live one exactly"
        );
    }

    #[test]
    fn corrupting_the_recorded_timer_logical_time_diverges_replay() {
        // Negative control (item 23, "alternative" option): chess's clock
        // arithmetic genuinely depends on the exact logical time `apply` was
        // given (`ClockState::last_move_at = now`, doc 02 §12.1), so a
        // recorder that captured the late FRAME time instead of the timer's
        // own deadline is a real, catchable defect here — not a fabricated
        // claim.
        let mut match_ = match_for_rules(&timed_config(10));
        match_
            .advance_frame(&frame(20))
            .expect("late frame processes the due timer");

        let trace = match_.replay_trace();
        let entry = &trace.accepted_inputs()[0];
        let mut corrupted = LocalReplayTrace::new(trace.initial_state_hash());
        corrupted.record(AcceptedReplayInput::new(
            entry.index(),
            LogicalTime(20), // WRONG: live actually applied the timer at 10.
            entry.input().clone(),
            entry.state_hash(),
        ));

        let result = catch_unwind(AssertUnwindSafe(|| {
            replay::<ChessRules>(
                &timed_config(10),
                &roster(),
                &MatchSeed::from_bytes([0; 32]),
                &corrupted,
            )
        }));
        assert!(
            result.is_err(),
            "recording frame-arrival time instead of the timer's own deadline must diverge the \
             game's clock arithmetic, but replay silently reproduced the same hash"
        );
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
        // A bot move has no second replay/mutation channel: it is recorded
        // as ordinary accepted replay evidence (item 16), indistinguishable
        // from a human's.
        assert!(matches!(
            match_.replay_trace().accepted_inputs()[0].input(),
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

    #[test]
    fn a_real_multi_move_opening_sequence_replays_to_every_checkpoint_and_the_final_hash() {
        // The primary differential-replay evidence (item 12/27): a genuine
        // multi-input LIVE sequence through `LocalMatch`'s own public entry
        // points, never a hard-coded vector shared between the live and
        // replay sides.
        let frame = frame(0);
        let mut match_ = match_for_rules(&Config::default());

        let moves = [
            (SeatId(0), 12u8, 28u8), // e2-e4
            (SeatId(1), 52, 36),     // e7-e5
            (SeatId(0), 6, 21),      // Ng1-f3
            (SeatId(1), 57, 42),     // Nb8-c6
        ];
        for (seat, from, to) in moves {
            match_
                .submit_input(
                    Input::Player {
                        seat,
                        command: local_game::Command::Move {
                            from,
                            to,
                            promotion: None,
                        },
                    },
                    &frame,
                )
                .expect("scripted opening moves are legal");
        }

        let trace = match_.replay_trace();
        assert_eq!(
            trace
                .accepted_inputs()
                .iter()
                .map(AcceptedReplayInput::index)
                .collect::<Vec<_>>(),
            [InputIndex(1), InputIndex(2), InputIndex(3), InputIndex(4)]
        );

        // `replay` asserts every recorded checkpoint as it walks the trace;
        // reaching this line without panicking IS the "every checkpoint
        // matches" evidence.
        let (final_hash, terminal) = replay::<ChessRules>(
            &Config::default(),
            &roster(),
            &MatchSeed::from_bytes([0; 32]),
            trace,
        );
        assert_eq!(final_hash, match_.state_hash());
        assert_eq!(terminal, None, "this opening does not end the match");
        assert_eq!(match_.ended(), None);
    }

    #[test]
    fn a_second_games_moves_are_recorded_and_replayed_by_the_same_generic_machinery() {
        // Item 16/27: proves the recorder is generic across games, not
        // shaped around chess specifically — a second, structurally
        // different `GameRules`/`GamePresentation` pair runs through the
        // exact same `LocalMatch<R, P>` and `replay` code.
        let frame = frame(0);
        let mut match_ = LocalMatch::<
            second_game::TicTacToeRules,
            second_game::presentation::TicTacToePresentation,
        >::new(
            &second_game::Config::default(),
            &roster(),
            MatchSeed::from_bytes([7; 32]),
            Viewer::Seat(SeatId(0)),
        )
        .expect("the second game's standard local configuration is valid");

        match_
            .submit_input(
                Input::Player {
                    seat: SeatId(0),
                    command: second_game::Command::Place { cell: 4 },
                },
                &frame,
            )
            .expect("the center cell is legal");
        match_
            .submit_input(
                Input::Player {
                    seat: SeatId(1),
                    command: second_game::Command::Place { cell: 0 },
                },
                &frame,
            )
            .expect("a corner cell is legal");

        let trace = match_.replay_trace();
        assert_eq!(trace.accepted_inputs().len(), 2);

        let (final_hash, _) = replay::<second_game::TicTacToeRules>(
            &second_game::Config::default(),
            &roster(),
            &MatchSeed::from_bytes([7; 32]),
            trace,
        );
        assert_eq!(final_hash, match_.state_hash());
    }

    #[test]
    fn a_recorded_input_index_is_load_bearing_for_rng_derived_replay() {
        // Negative control for R3 (item 13/23, "best option"): exercises the
        // REPLAY ORACLE's sensitivity to index corruption directly, using
        // `RngSensitiveRules` — chess and the second game cannot demonstrate
        // this honestly, since neither ever draws from `ctx.rng`. This test
        // deliberately does not go through `LocalMatch` (no
        // `GamePresentation` exists for the fixture, and none is needed).
        let seed = MatchSeed::from_bytes([9; 32]);
        let seat_roster = roster();

        let mut create_rng = DetRng::for_input(&seed, InputIndex(0));
        let mut create_ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut create_rng,
            budget: Budget::default(),
        };
        let init = RngSensitiveRules::create(&(), &seat_roster, &mut create_ctx)
            .expect("the fixture always accepts creation");
        let initial_hash = RngSensitiveRules::state_hash(&init.state);
        let mut state = init.state;

        // The live transition happened at InputIndex(2) — e.g. attempt 1 was
        // rejected by some other command and consumed index 1, exactly the
        // gap `accepted_and_rejected_inputs_each_consume_one_index` (above)
        // demonstrates for real chess.
        let accepted_index = InputIndex(2);
        let accepted_now = LogicalTime(20);
        let accepted_input = Input::Player {
            seat: SeatId(0),
            command: (),
        };
        let mut rng = DetRng::for_input(&seed, accepted_index);
        let mut ctx = Ctx {
            now: accepted_now,
            index: accepted_index,
            rng: &mut rng,
            budget: Budget::default(),
        };
        RngSensitiveRules::apply(&mut state, accepted_input.clone(), &mut ctx)
            .expect("the fixture always accepts its one command");
        let correct_hash = RngSensitiveRules::state_hash(&state);

        let mut faithful_trace = LocalReplayTrace::new(initial_hash);
        faithful_trace.record(AcceptedReplayInput::new(
            accepted_index,
            accepted_now,
            accepted_input.clone(),
            correct_hash,
        ));
        let (replayed_hash, _) =
            replay::<RngSensitiveRules>(&(), &seat_roster, &seed, &faithful_trace);
        assert_eq!(
            replayed_hash, correct_hash,
            "sanity: the correctly-indexed trace must replay clean before the mutant proves \
             anything"
        );

        // THE MUTANT: a recorder that compacted/renumbered indices would
        // have written InputIndex(1) here instead of the original
        // InputIndex(2) — closing the gap left by the rejected attempt.
        let mut corrupted_trace = LocalReplayTrace::new(initial_hash);
        corrupted_trace.record(AcceptedReplayInput::new(
            InputIndex(1), // WRONG — live actually accepted this at index 2.
            accepted_now,
            accepted_input,
            correct_hash, // the hash live actually produced, at the REAL index.
        ));

        let result = catch_unwind(AssertUnwindSafe(|| {
            replay::<RngSensitiveRules>(&(), &seat_roster, &seed, &corrupted_trace)
        }));
        assert!(
            result.is_err(),
            "renumbering the recorded InputIndex must shift the RNG-domain stream and fail \
             replay at the checkpoint, but it silently reproduced the same hash"
        );
    }
}
