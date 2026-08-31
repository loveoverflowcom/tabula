#![allow(clippy::doc_markdown)]

//! Bot self-play — the primary fuzzer. (doc 02 §11.3)
//!
//! The runner is a small imperative shell around the same `GameRules` contract
//! used by production. It owns scheduling and effect interpretation; it never
//! inspects canonical game fields to choose a bot action. Bots receive only the
//! `View` returned by `GameRules::project`.
//!
//! @ai.role deterministic-harness
//! @ai.domain testkit.selfplay
//! @ai.invariant bot-consumes-projection-only
//! @ai.invariant rejected-input-preserves-state
//! @ai.invariant timer-order-is-logical
//! @ai.invariant end-match-is-sole-terminal-authority
//! @ai.law same-setup-and-seed-reproduce-semantic-trace
//! @ai.evidence selfplay::tests::same_setup_and_seed_is_semantically_stable
//! @ai.evidence selfplay::tests::timer_queue_rearms_and_cancels
//! @ai.evidence selfplay::tests::max_inputs_returns_a_structured_failure
//!
//! # Cadence
//!
//! - Per PR: small focused runs and approximately 1000 matches per game.
//! - Nightly/manual: 100k matches per game.
//!
//! A failure is returned as data with `(base_seed, match_index, input_index)`
//! coordinates. This module never writes files or mutates Git history; replay
//! artifact orchestration belongs at the `xtask`/CI boundary.

use std::{collections::BTreeMap, time::Instant};

use serde::Serialize;
use tabula_core::{
    canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, Occupant, RuleErrorCode,
    SeatChange, SeatId, SeatRoster, SpectatorTier, StateHash, TimerId, Viewer,
};
use tabula_game_api::{AdminInput, Budget, Ctx, Effect, GameBot, GameModule, GameRules, Input};

/// Setup data that a game-specific boundary must provide before self-play.
///
/// The generic runner deliberately does not invent a config or a roster. This
/// keeps game-specific defaults in the game package or CLI while allowing the
/// simulation core to work for future cards, tiles, and social games.
#[derive(Clone)]
pub struct SelfPlaySetup<R: GameRules> {
    pub config: R::Config,
    pub roster: SeatRoster,
}

impl<R: GameRules> core::fmt::Debug for SelfPlaySetup<R> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SelfPlaySetup")
            .field("seats", &self.roster.len())
            .finish_non_exhaustive()
    }
}

/// Options controlling a deterministic batch of self-play matches.
#[derive(Clone, Debug)]
pub struct SelfPlayConfig {
    pub matches: u32,
    /// Base seed. Match `n` derives its own [`MatchSeed`] from this value and
    /// the absolute match index.
    pub base_seed: [u8; 32],
    /// Fraction of input attempts that receive a generic hostile input.
    /// Values must be finite and in `0.0..=1.0`.
    pub hostile_fraction: f32,
    /// Fail a match that has not terminated after this many attempted inputs.
    pub max_inputs: u32,
    pub check_projections: bool,
    /// Absolute index of the first match in this batch. This makes a failing
    /// match reproducible without pretending every run starts at index zero.
    pub start_match_index: u32,
}

impl Default for SelfPlayConfig {
    fn default() -> Self {
        Self {
            matches: 1_000,
            base_seed: [0u8; 32],
            hostile_fraction: 0.05,
            max_inputs: 10_000,
            check_projections: true,
            start_match_index: 0,
        }
    }
}

/// Why one deterministic match failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelfPlayFailureKind {
    InvalidSetup,
    CreateFailed,
    BotUnavailable,
    BotProducedNoCommand,
    DidNotTerminate,
    TransactionalViolation,
    MultipleEndMatch,
    Diverged,
    CanonicalEncoding,
}

impl core::fmt::Display for SelfPlayFailureKind {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match self {
            Self::InvalidSetup => "invalid setup",
            Self::CreateFailed => "create failed",
            Self::BotUnavailable => "bot unavailable",
            Self::BotProducedNoCommand => "bot produced no command",
            Self::DidNotTerminate => "did not terminate",
            Self::TransactionalViolation => "transactional violation",
            Self::MultipleEndMatch => "multiple EndMatch effects",
            Self::Diverged => "determinism divergence",
            Self::CanonicalEncoding => "canonical encoding failed",
        };
        formatter.write_str(name)
    }
}

/// A reproducible self-play failure. The seed is the user-supplied base seed;
/// the per-match seed is derived from it and `match_index`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfPlayFailure {
    pub match_index: u32,
    pub input_index: Option<u64>,
    pub base_seed: [u8; 32],
    pub kind: SelfPlayFailureKind,
    pub reason: String,
}

/// Aggregate results for one self-play batch.
#[derive(Clone, Debug, Default)]
pub struct SelfPlayReport {
    pub matches_run: u32,
    pub terminated: u32,
    pub inputs_total: u64,
    pub failures: Vec<SelfPlayFailure>,
    pub determinism_failures: u32,
    pub transactional_failures: u32,
    pub max_input_failures: u32,
    /// Observational only. It is never part of semantic trace comparison.
    pub p99_apply_micros: u32,
}

impl SelfPlayReport {
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.failures.is_empty() && self.terminated == self.matches_run
    }
}

/// Errors that prevent a batch from being meaningfully started.
#[derive(Debug, thiserror::Error)]
pub enum SelfPlayError {
    #[error("invalid self-play configuration: {0}")]
    InvalidConfig(String),
    #[error("invalid self-play setup: {0}")]
    InvalidSetup(String),
}

/// Run a deterministic batch of bot matches and verify each semantic trace by
/// running it twice from the same setup and derived match seed.
///
/// The two runs compare input bytes, logical times, accepted/rejected results,
/// per-input state hashes, events, effects, terminal outcome, and final state
/// bytes. Timing measurements are collected only from the first run and never
/// influence the second run or the rules context.
///
/// @ai.role deterministic-transition
/// @ai.domain testkit.selfplay.run
/// @ai.invariant rejected-input-preserves-state
/// @ai.invariant same-setup-and-seed-reproduce-semantic-trace
/// @ai.ensures failures-carry-reproduction-coordinates
/// @ai.evidence selfplay::tests::same_setup_and_seed_is_semantically_stable
/// @ai.evidence selfplay::tests::hostile_rejections_are_checked_transactionally
pub fn run<M: GameModule>(
    setup: &SelfPlaySetup<M::Rules>,
    cfg: &SelfPlayConfig,
) -> Result<SelfPlayReport, SelfPlayError> {
    let hostile_threshold = validate_config::<M>(setup, cfg)?;

    let mut report = SelfPlayReport::default();
    let mut latency = LatencyHistogram::default();

    for offset in 0..cfg.matches {
        let match_index = cfg
            .start_match_index
            .checked_add(offset)
            .ok_or_else(|| SelfPlayError::InvalidConfig("match index overflow".to_owned()))?;
        report.matches_run += 1;

        let match_seed = derive_match_seed(&cfg.base_seed, match_index);
        let first = simulate::<M>(
            setup,
            cfg,
            hostile_threshold,
            match_index,
            &match_seed,
            true,
        );
        report.inputs_total += first.trace.steps.len() as u64;
        latency.merge(&first.latency);
        if first.terminated {
            report.terminated += 1;
        }

        if let Some(failure) = first.failure {
            record_failure(&mut report, cfg.base_seed, match_index, failure);
            continue;
        }

        let second = simulate::<M>(
            setup,
            cfg,
            hostile_threshold,
            match_index,
            &match_seed,
            false,
        );
        if let Some(failure) = second.failure {
            record_failure(
                &mut report,
                cfg.base_seed,
                match_index,
                InternalFailure {
                    kind: SelfPlayFailureKind::Diverged,
                    input_index: failure.input_index,
                    reason: format!("second semantic run failed: {}", failure.reason),
                },
            );
        } else if let Some((input_index, reason)) = first_divergence(&first.trace, &second.trace) {
            record_failure(
                &mut report,
                cfg.base_seed,
                match_index,
                InternalFailure {
                    kind: SelfPlayFailureKind::Diverged,
                    input_index,
                    reason,
                },
            );
        }
    }

    report.determinism_failures = count_failures(&report, SelfPlayFailureKind::Diverged);
    report.transactional_failures =
        count_failures(&report, SelfPlayFailureKind::TransactionalViolation);
    report.max_input_failures = count_failures(&report, SelfPlayFailureKind::DidNotTerminate);
    report.p99_apply_micros = latency.p99_micros();
    Ok(report)
}

fn count_failures(report: &SelfPlayReport, kind: SelfPlayFailureKind) -> u32 {
    u32::try_from(
        report
            .failures
            .iter()
            .filter(|failure| failure.kind == kind)
            .count(),
    )
    .unwrap_or(u32::MAX)
}

fn validate_config<M: GameModule>(
    setup: &SelfPlaySetup<M::Rules>,
    cfg: &SelfPlayConfig,
) -> Result<u64, SelfPlayError> {
    if cfg.max_inputs == 0 {
        return Err(SelfPlayError::InvalidConfig(
            "max_inputs must be greater than zero".to_owned(),
        ));
    }
    cfg.start_match_index
        .checked_add(cfg.matches)
        .ok_or_else(|| SelfPlayError::InvalidConfig("match index overflow".to_owned()))?;
    let hostile_threshold = fraction_threshold(cfg.hostile_fraction)
        .map_err(|reason| SelfPlayError::InvalidConfig(reason.to_owned()))?;

    for entry in &setup.roster {
        if !matches!(entry.occupant, Occupant::Bot { .. }) {
            return Err(SelfPlayError::InvalidSetup(
                "every self-play seat must have a bot occupant".to_owned(),
            ));
        }
    }
    M::validate_config(&setup.config, &setup.roster)
        .map_err(|error| SelfPlayError::InvalidSetup(format!("game config rejected: {error:?}")))?;
    Ok(hostile_threshold)
}

struct MatchExecution {
    trace: SemanticTrace,
    failure: Option<InternalFailure>,
    terminated: bool,
    latency: LatencyHistogram,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SemanticTrace {
    initial_events: Vec<Vec<u8>>,
    initial_effects: Vec<Vec<u8>>,
    steps: Vec<StepTrace>,
    final_state: Vec<u8>,
    final_hash: StateHash,
    terminal_outcome: Option<Vec<u8>>,
}

impl Default for SemanticTrace {
    fn default() -> Self {
        Self {
            initial_events: Vec::new(),
            initial_effects: Vec::new(),
            steps: Vec::new(),
            final_state: Vec::new(),
            final_hash: StateHash([0; 32]),
            terminal_outcome: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StepTrace {
    input: Vec<u8>,
    logical_time: LogicalTime,
    result: AttemptResult,
    state_hash: StateHash,
    events: Vec<Vec<u8>>,
    effects: Vec<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttemptResult {
    Accepted,
    Rejected(RuleErrorCode),
}

struct InternalFailure {
    kind: SelfPlayFailureKind,
    input_index: Option<u64>,
    reason: String,
}

impl InternalFailure {
    fn at(kind: SelfPlayFailureKind, input_index: u64, reason: impl Into<String>) -> Self {
        Self {
            kind,
            input_index: Some(input_index),
            reason: reason.into(),
        }
    }

    fn global(kind: SelfPlayFailureKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            input_index: None,
            reason: reason.into(),
        }
    }
}

fn record_failure(
    report: &mut SelfPlayReport,
    base_seed: [u8; 32],
    match_index: u32,
    failure: InternalFailure,
) {
    report.failures.push(SelfPlayFailure {
        match_index,
        input_index: failure.input_index,
        base_seed,
        kind: failure.kind,
        reason: failure.reason,
    });
}

// The loop is deliberately kept together so the ordering policy, hostile-input
// budget, and terminal authority can be audited as one state machine.
#[allow(clippy::too_many_lines)]
fn simulate<M: GameModule>(
    setup: &SelfPlaySetup<M::Rules>,
    cfg: &SelfPlayConfig,
    hostile_threshold: u64,
    match_index: u32,
    seed: &MatchSeed,
    measure: bool,
) -> MatchExecution {
    let mut trace = SemanticTrace::default();
    let mut latency = LatencyHistogram::default();
    let mut bots = Vec::new();
    for entry in &setup.roster {
        let Occupant::Bot { level } = entry.occupant else {
            return MatchExecution {
                trace,
                failure: Some(InternalFailure::global(
                    SelfPlayFailureKind::InvalidSetup,
                    "self-play roster contains a non-bot occupant",
                )),
                terminated: false,
                latency,
            };
        };
        let Some(bot) = M::bot(level) else {
            return MatchExecution {
                trace,
                failure: Some(InternalFailure::global(
                    SelfPlayFailureKind::BotUnavailable,
                    format!(
                        "no bot is available for seat {:?} at level {level:?}",
                        entry.seat
                    ),
                )),
                terminated: false,
                latency,
            };
        };
        bots.push(BotSlot {
            seat: entry.seat,
            bot,
        });
    }

    let mut create_rng = DetRng::for_input(seed, InputIndex(0));
    let mut create_ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut create_rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    };
    let init = match M::Rules::create(&setup.config, &setup.roster, &mut create_ctx) {
        Ok(init) => init,
        Err(error) => {
            return MatchExecution {
                trace,
                failure: Some(InternalFailure::global(
                    SelfPlayFailureKind::CreateFailed,
                    format!("match {match_index} create failed: {error:?}"),
                )),
                terminated: false,
                latency,
            };
        }
    };

    let mut state = init.state;
    trace.initial_events = match encoded_values(&init.events) {
        Ok(values) => values,
        Err(failure) => {
            return MatchExecution {
                trace,
                failure: Some(failure),
                terminated: false,
                latency,
            };
        }
    };
    exercise_view_events::<M::Rules>(&state, &init.events, &setup.roster, cfg.check_projections);
    trace.initial_effects = match encoded_values(&init.effects) {
        Ok(values) => values,
        Err(failure) => {
            return MatchExecution {
                trace,
                failure: Some(failure),
                terminated: false,
                latency,
            };
        }
    };

    let mut timers = TimerQueue::default();
    let mut terminal_outcome = None;
    if let Err(failure) = interpret_effects(
        &init.effects,
        LogicalTime::ZERO,
        &mut timers,
        &mut terminal_outcome,
    ) {
        return MatchExecution {
            trace,
            failure: Some(failure),
            terminated: terminal_outcome.is_some(),
            latency,
        };
    }
    trace.terminal_outcome.clone_from(&terminal_outcome);

    let mut now = LogicalTime::ZERO;
    let mut hostile_since_progress = false;
    let mut last_timer = None;

    while trace.terminal_outcome.is_none() {
        if trace.steps.len() >= usize::try_from(cfg.max_inputs).unwrap_or(usize::MAX) {
            let input_index = trace.steps.len() as u64;
            let failure = InternalFailure::at(
                SelfPlayFailureKind::DidNotTerminate,
                input_index,
                format!(
                    "match {match_index} reached max_inputs={} without an EndMatch effect",
                    cfg.max_inputs
                ),
            );
            let final_state = canonical_encode(&state).unwrap_or_default();
            trace.final_state = final_state;
            trace.final_hash = M::Rules::state_hash(&state);
            return MatchExecution {
                trace,
                failure: Some(failure),
                terminated: false,
                latency,
            };
        }

        let next_index = trace.steps.len() as u64 + 1;
        let bot_action =
            next_bot_action::<M>(&state, now, &bots, seed, next_index, cfg.check_projections);
        let Some(scheduled) = choose_scheduled(bot_action, timers.next()) else {
            return finish_execution::<M::Rules>(
                trace,
                &state,
                Some(InternalFailure::at(
                    SelfPlayFailureKind::BotProducedNoCommand,
                    next_index,
                    "all bot projections produced no command and no timer was pending",
                )),
                terminal_outcome.as_ref(),
                latency,
            );
        };

        if !hostile_since_progress && should_inject_hostile(seed, next_index, hostile_threshold) {
            let mut hostile_rng =
                DetRng::for_input(seed, InputIndex(next_index)).stream(HOSTILE_DOMAIN);
            let hostile = hostile_input(
                &setup.roster,
                &timers,
                last_timer,
                scheduled.bot_player(),
                &mut hostile_rng,
            );
            if let Some(input) = hostile {
                let failure = {
                    let mut context = ApplyContext {
                        trace: &mut trace,
                        timers: &mut timers,
                        terminal_outcome: &mut terminal_outcome,
                        roster: &setup.roster,
                        check_projections: cfg.check_projections,
                        latency: &mut latency,
                        measure,
                    };
                    apply_one::<M::Rules>(
                        &mut state,
                        input,
                        now,
                        InputIndex(next_index),
                        seed,
                        &mut context,
                    )
                };
                if let Some(failure) = failure {
                    return finish_execution::<M::Rules>(
                        trace,
                        &state,
                        Some(failure),
                        terminal_outcome.as_ref(),
                        latency,
                    );
                }
                hostile_since_progress = true;
                trace.terminal_outcome.clone_from(&terminal_outcome);
                continue;
            }
        }

        if let Some(timer) = scheduled.timer {
            timers.remove(timer);
            last_timer = Some(timer);
        }
        now = now.max(scheduled.at);
        let failure = {
            let mut context = ApplyContext {
                trace: &mut trace,
                timers: &mut timers,
                terminal_outcome: &mut terminal_outcome,
                roster: &setup.roster,
                check_projections: cfg.check_projections,
                latency: &mut latency,
                measure,
            };
            apply_one::<M::Rules>(
                &mut state,
                scheduled.input,
                now,
                InputIndex(next_index),
                seed,
                &mut context,
            )
        };
        if let Some(failure) = failure {
            return finish_execution::<M::Rules>(
                trace,
                &state,
                Some(failure),
                terminal_outcome.as_ref(),
                latency,
            );
        }
        trace.terminal_outcome.clone_from(&terminal_outcome);
        hostile_since_progress = false;
    }

    finish_execution::<M::Rules>(trace, &state, None, terminal_outcome.as_ref(), latency)
}

fn finish_execution<R: GameRules>(
    mut trace: SemanticTrace,
    state: &R::State,
    failure: Option<InternalFailure>,
    terminal_outcome: Option<&Vec<u8>>,
    latency: LatencyHistogram,
) -> MatchExecution {
    trace.terminal_outcome = terminal_outcome.cloned();
    let (final_state, encoding_failure) = match canonical_encode(state) {
        Ok(bytes) => (bytes, None),
        Err(error) => (
            Vec::new(),
            Some(InternalFailure::global(
                SelfPlayFailureKind::CanonicalEncoding,
                format!("final state encoding failed: {error}"),
            )),
        ),
    };
    trace.final_state = final_state;
    trace.final_hash = R::state_hash(state);
    MatchExecution {
        terminated: terminal_outcome.is_some(),
        trace,
        failure: failure.or(encoding_failure),
        latency,
    }
}

struct ApplyContext<'a> {
    trace: &'a mut SemanticTrace,
    timers: &'a mut TimerQueue,
    terminal_outcome: &'a mut Option<Vec<u8>>,
    roster: &'a SeatRoster,
    check_projections: bool,
    latency: &'a mut LatencyHistogram,
    measure: bool,
}

// This wrapper intentionally keeps the canonical-before/apply/canonical-after
// transaction check in one auditable block.
#[allow(clippy::too_many_lines)]
fn apply_one<R: GameRules>(
    state: &mut R::State,
    input: Input<R::Command>,
    now: LogicalTime,
    index: InputIndex,
    seed: &MatchSeed,
    context: &mut ApplyContext<'_>,
) -> Option<InternalFailure> {
    let input_encoded = match canonical_encode(&input) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Some(InternalFailure::at(
                SelfPlayFailureKind::CanonicalEncoding,
                index.0,
                format!("input encoding failed: {error}"),
            ));
        }
    };
    let before = match canonical_encode(state) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Some(InternalFailure::at(
                SelfPlayFailureKind::CanonicalEncoding,
                index.0,
                format!("state encoding before input failed: {error}"),
            ));
        }
    };
    let hash_before = R::state_hash(state);
    let mut rng = DetRng::for_input(seed, index);
    let mut ctx = Ctx {
        now,
        index,
        rng: &mut rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    };
    let started = context.measure.then(Instant::now);
    let result = R::apply(state, input, &mut ctx);
    if let Some(started) = started {
        context.latency.record(started.elapsed());
    }

    let mut step = StepTrace {
        input: input_encoded,
        logical_time: now,
        result: AttemptResult::Accepted,
        state_hash: hash_before,
        events: Vec::new(),
        effects: Vec::new(),
    };

    match result {
        Err(error) => {
            let after = match canonical_encode(state) {
                Ok(bytes) => bytes,
                Err(encoding_error) => {
                    return Some(InternalFailure::at(
                        SelfPlayFailureKind::CanonicalEncoding,
                        index.0,
                        format!("state encoding after rejection failed: {encoding_error}"),
                    ));
                }
            };
            if before != after {
                let hash_after = R::state_hash(state);
                return Some(InternalFailure::at(
                    SelfPlayFailureKind::TransactionalViolation,
                    index.0,
                    format!(
                        "rule error code {:?}; state hash before {:02x?}; state hash after {:02x?}",
                        error.code, hash_before.0, hash_after.0
                    ),
                ));
            }
            step.result = AttemptResult::Rejected(error.code);
        }
        Ok(outcome) => {
            step.events = match encoded_values(&outcome.events) {
                Ok(values) => values,
                Err(mut failure) => {
                    failure.input_index = Some(index.0);
                    return Some(failure);
                }
            };
            step.effects = match encoded_values(&outcome.effects) {
                Ok(values) => values,
                Err(mut failure) => {
                    failure.input_index = Some(index.0);
                    return Some(failure);
                }
            };
            exercise_view_events::<R>(
                state,
                &outcome.events,
                context.roster,
                context.check_projections,
            );
            if let Err(failure) = interpret_effects(
                &outcome.effects,
                now,
                context.timers,
                context.terminal_outcome,
            ) {
                return Some(InternalFailure {
                    input_index: Some(index.0),
                    ..failure
                });
            }
        }
    }

    step.state_hash = R::state_hash(state);
    context.trace.steps.push(step);
    None
}

fn exercise_view_events<R: GameRules>(
    state_after: &R::State,
    events: &[R::Event],
    roster: &SeatRoster,
    check_projections: bool,
) {
    for event in events {
        for entry in roster {
            let _ = R::view_event(state_after, event, Viewer::Seat(entry.seat));
        }
        if check_projections {
            let _ = R::view_event(state_after, event, Viewer::Spectator(SpectatorTier::Live));
            let _ = R::view_event(
                state_after,
                event,
                Viewer::Spectator(SpectatorTier::Delayed {
                    by: tabula_core::Millis(30_000),
                }),
            );
            let _ = R::view_event(state_after, event, Viewer::Audit);
        }
    }
}

fn next_bot_action<M: GameModule>(
    state: &<M::Rules as GameRules>::State,
    now: LogicalTime,
    bots: &[BotSlot<M::Rules>],
    seed: &MatchSeed,
    input_index: u64,
    check_projections: bool,
) -> Option<BotAction<<M::Rules as GameRules>::Command>> {
    if check_projections {
        let _ = M::Rules::project(state, Viewer::Spectator(SpectatorTier::Live));
        let _ = M::Rules::project(
            state,
            Viewer::Spectator(SpectatorTier::Delayed {
                by: tabula_core::Millis(30_000),
            }),
        );
        let _ = M::Rules::project(state, Viewer::Audit);
    }

    let mut selected: Option<BotAction<<M::Rules as GameRules>::Command>> = None;
    for slot in bots {
        // This is the security boundary: `choose` receives the projected View
        // and never receives `state`, even though this function owns the state.
        let view = M::Rules::project(state, Viewer::Seat(slot.seat));
        let mut bot_rng = DetRng::for_input(seed, InputIndex(input_index))
            .stream(BOT_DOMAIN + u32::from(slot.seat.0));
        let Some(command) = slot.bot.choose(&view, slot.seat, &mut bot_rng) else {
            continue;
        };
        let candidate = BotAction {
            seat: slot.seat,
            ready_at: now.plus(slot.bot.think_time(&view)),
            command,
        };
        let replace = match selected.as_ref() {
            None => true,
            Some(current) => {
                (candidate.ready_at, candidate.seat) < (current.ready_at, current.seat)
            }
        };
        if replace {
            selected = Some(candidate);
        }
    }
    selected
}

fn interpret_effects(
    effects: &[Effect],
    now: LogicalTime,
    timers: &mut TimerQueue,
    terminal_outcome: &mut Option<Vec<u8>>,
) -> Result<(), InternalFailure> {
    for effect in effects {
        match effect {
            Effect::SetTimer { id, delay } => timers.set(*id, now.plus(*delay)),
            Effect::CancelTimer { id } => timers.remove(*id),
            Effect::EndMatch { outcome } => {
                if terminal_outcome.is_some() {
                    return Err(InternalFailure::global(
                        SelfPlayFailureKind::MultipleEndMatch,
                        "more than one EndMatch effect was emitted for one logical match",
                    ));
                }
                *terminal_outcome = Some(canonical_encode(outcome).map_err(|error| {
                    InternalFailure::global(
                        SelfPlayFailureKind::CanonicalEncoding,
                        format!("terminal outcome encoding failed: {error}"),
                    )
                })?);
            }
            // These effects are durable/platform concerns. Recording them in the
            // semantic trace is enough for this mini runtime; no external side
            // effect is allowed to influence the next input. If a future game
            // requests a bot move, the projection-driven bot scan remains the
            // only source of its command.
            _ => {}
        }
    }
    Ok(())
}

#[derive(Default)]
struct TimerQueue {
    deadlines: BTreeMap<TimerId, LogicalTime>,
}

impl TimerQueue {
    /// Replace/re-arm the timer id with the newest absolute logical deadline.
    fn set(&mut self, id: TimerId, deadline: LogicalTime) {
        self.deadlines.insert(id, deadline);
    }

    fn remove(&mut self, id: TimerId) {
        self.deadlines.remove(&id);
    }

    fn contains(&self, id: TimerId) -> bool {
        self.deadlines.contains_key(&id)
    }

    /// Select by `(deadline, timer id)`, so equal deadlines are stable too.
    fn next(&self) -> Option<(TimerId, LogicalTime)> {
        self.deadlines
            .iter()
            .map(|(id, deadline)| (*id, *deadline))
            .min_by_key(|(id, deadline)| (*deadline, *id))
    }
}

struct BotSlot<R: GameRules> {
    seat: SeatId,
    bot: Box<dyn GameBot<R>>,
}

struct BotAction<C> {
    seat: SeatId,
    ready_at: LogicalTime,
    command: C,
}

struct Scheduled<C> {
    input: Input<C>,
    at: LogicalTime,
    timer: Option<TimerId>,
}

/// Select the next real input. Timer deadlines win when equal to bot readiness;
/// Chess's exact-zero clock rule makes that boundary explicit and replay-stable.
fn choose_scheduled<C>(
    bot: Option<BotAction<C>>,
    timer: Option<(TimerId, LogicalTime)>,
) -> Option<Scheduled<C>> {
    match (bot, timer) {
        (Some(bot), Some((timer, deadline))) if deadline <= bot.ready_at => Some(Scheduled {
            input: Input::Timer { timer },
            at: deadline,
            timer: Some(timer),
        }),
        (Some(bot), _) => Some(Scheduled {
            input: Input::Player {
                seat: bot.seat,
                command: bot.command,
            },
            at: bot.ready_at,
            timer: None,
        }),
        (None, Some((timer, deadline))) => Some(Scheduled {
            input: Input::Timer { timer },
            at: deadline,
            timer: Some(timer),
        }),
        (None, None) => None,
    }
}

impl<C> Scheduled<C> {
    fn bot_player(&self) -> Option<(SeatId, &C)> {
        match &self.input {
            Input::Player { seat, command } => Some((*seat, command)),
            Input::Timer { .. } | Input::Seat { .. } | Input::Admin(_) => None,
        }
    }
}

const BOT_DOMAIN: u32 = 1;
const HOSTILE_DOMAIN: u32 = 2;

fn hostile_input<C: Clone>(
    roster: &SeatRoster,
    timers: &TimerQueue,
    last_timer: Option<TimerId>,
    bot_player: Option<(SeatId, &C)>,
    rng: &mut DetRng,
) -> Option<Input<C>> {
    match rng.below(5) {
        0 => Some(Input::Timer {
            timer: last_timer.unwrap_or_else(|| unknown_timer(timers)),
        }),
        1 => Some(Input::Seat {
            seat: unused_seat(roster),
            change: SeatChange::Disconnected,
        }),
        2 => Some(Input::Admin(AdminInput::Pause)),
        3 => bot_player.and_then(|(bot_seat, command)| {
            let other = roster.iter().find(|entry| entry.seat != bot_seat)?;
            Some(Input::Player {
                seat: other.seat,
                command: command.clone(),
            })
        }),
        _ => Some(Input::Timer {
            timer: unknown_timer(timers),
        }),
    }
}

fn unknown_timer(timers: &TimerQueue) -> TimerId {
    for raw in [0, u16::MAX, u16::MAX - 1, 1, 2] {
        let id = TimerId(raw);
        if !timers.contains(id) {
            return id;
        }
    }
    TimerId(u16::MAX)
}

fn unused_seat(roster: &SeatRoster) -> SeatId {
    (0..=u8::MAX)
        .rev()
        .map(SeatId)
        .find(|seat| roster.get(*seat).is_none())
        .unwrap_or(SeatId(u8::MAX))
}

fn should_inject_hostile(seed: &MatchSeed, input_index: u64, threshold: u64) -> bool {
    let mut rng = DetRng::for_input(seed, InputIndex(input_index)).stream(HOSTILE_DOMAIN);
    u64::from(rng.next_u32()) < threshold
}

/// Convert the retained public `f32` setting to an integer probability without
/// doing floating-point arithmetic in the semantic scheduler. This also makes
/// NaN, infinity, and negative values explicit configuration errors.
fn fraction_threshold(fraction: f32) -> Result<u64, &'static str> {
    let bits = fraction.to_bits();
    let sign = bits >> 31;
    let exponent = (bits >> 23) & 0xff;
    let mantissa = bits & 0x7f_ff_ff;
    if sign != 0 || exponent == 0xff {
        return Err("hostile_fraction must be finite and non-negative");
    }
    if exponent > 127 || (exponent == 127 && mantissa != 0) {
        return Err("hostile_fraction must be at most 1.0");
    }
    if exponent == 0 {
        return Ok(0);
    }

    let significand = (1_u64 << 23) | u64::from(mantissa);
    let threshold = if exponent >= 118 {
        significand << (exponent - 118)
    } else {
        significand >> (118 - exponent)
    };
    Ok(threshold.min(1_u64 << 32))
}

fn derive_match_seed(base_seed: &[u8; 32], match_index: u32) -> MatchSeed {
    let base = MatchSeed::from_bytes(*base_seed);
    let mut rng = DetRng::for_input(&base, InputIndex(u64::from(match_index)));
    let mut bytes = [0_u8; 32];
    for chunk in bytes.chunks_exact_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    MatchSeed::from_bytes(bytes)
}

fn encoded_values<T: Serialize>(values: &[T]) -> Result<Vec<Vec<u8>>, InternalFailure> {
    values
        .iter()
        .map(|value| {
            canonical_encode(value).map_err(|error| {
                InternalFailure::global(
                    SelfPlayFailureKind::CanonicalEncoding,
                    format!("canonical value encoding failed: {error}"),
                )
            })
        })
        .collect()
}

fn first_divergence(a: &SemanticTrace, b: &SemanticTrace) -> Option<(Option<u64>, String)> {
    if a.initial_events != b.initial_events {
        return Some((Some(0), "initial event stream differed".to_owned()));
    }
    if a.initial_effects != b.initial_effects {
        return Some((Some(0), "initial effect stream differed".to_owned()));
    }
    for (index, (left, right)) in a.steps.iter().zip(&b.steps).enumerate() {
        if left != right {
            return Some((
                Some(index as u64 + 1),
                "input, logical time, result, events, effects, or state hash differed".to_owned(),
            ));
        }
    }
    if a.steps.len() != b.steps.len() {
        return Some((
            Some(a.steps.len().min(b.steps.len()) as u64 + 1),
            "input stream lengths differed".to_owned(),
        ));
    }
    if a.terminal_outcome != b.terminal_outcome {
        return Some((
            Some(a.steps.len() as u64),
            "terminal outcomes differed".to_owned(),
        ));
    }
    if a.final_state != b.final_state || a.final_hash != b.final_hash {
        return Some((
            Some(a.steps.len() as u64),
            "final canonical state or state hash differed".to_owned(),
        ));
    }
    None
}

struct LatencyHistogram {
    buckets: [u64; 65],
    samples: u64,
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self {
            buckets: [0; 65],
            samples: 0,
        }
    }
}

impl LatencyHistogram {
    fn record(&mut self, duration: std::time::Duration) {
        let micros = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
        let bucket = if micros == 0 {
            0
        } else {
            (u64::BITS - micros.leading_zeros()) as usize
        };
        self.buckets[bucket.min(self.buckets.len() - 1)] += 1;
        self.samples += 1;
    }

    fn merge(&mut self, other: &Self) {
        for (left, right) in self.buckets.iter_mut().zip(other.buckets) {
            *left += right;
        }
        self.samples += other.samples;
    }

    fn p99_micros(&self) -> u32 {
        if self.samples == 0 {
            return 0;
        }
        let target = self.samples.saturating_mul(99).div_ceil(100);
        let mut seen = 0;
        for (bucket, count) in self.buckets.iter().enumerate() {
            seen += count;
            if seen >= target {
                let upper = if bucket == 0 {
                    0
                } else {
                    1_u64 << bucket.min(31)
                };
                return u32::try_from(upper.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            }
        }
        u32::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use smallvec::smallvec;
    use tabula_core::{BotLevel, MatchOutcome, OutcomeKind, Standing};
    use tabula_game_api::{
        GameCapabilities, GameMetadata, Init, InitError, LegalCommands, Outcome,
    };

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct Config;

    #[derive(Clone, Debug, Serialize, Deserialize)]
    enum Command {
        Step,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct State {
        steps: u8,
    }

    #[derive(Clone, Debug, Serialize)]
    struct View {
        steps: u8,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct Event;

    #[derive(Debug)]
    struct Rules;

    struct Module;

    #[derive(Debug)]
    struct TestBot;

    impl GameBot<Rules> for TestBot {
        fn level(&self) -> BotLevel {
            BotLevel::Trivial
        }

        fn choose(&self, view: &View, _seat: SeatId, _rng: &mut DetRng) -> Option<Command> {
            (view.steps < 2).then_some(Command::Step)
        }

        fn think_time(&self, _view: &View) -> tabula_core::Millis {
            tabula_core::Millis(10)
        }
    }

    impl GameRules for Rules {
        type State = State;
        type Command = Command;
        type Event = Event;
        type View = View;
        type ViewEvent = Event;
        type Config = Config;

        const RULES_VERSION: tabula_core::RulesVersion = tabula_core::RulesVersion(1);

        fn create(_: &Config, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            Ok(Init {
                state: State { steps: 0 },
                events: smallvec![],
                effects: smallvec![],
            })
        }

        fn apply(
            state: &mut State,
            input: Input<Command>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, tabula_core::RuleError> {
            match input {
                Input::Player {
                    command: Command::Step,
                    ..
                } => {
                    state.steps += 1;
                    let mut effects = smallvec![];
                    if state.steps == 2 {
                        effects.push(Effect::EndMatch {
                            outcome: MatchOutcome::new_for_seats(
                                OutcomeKind::Draw,
                                smallvec![Standing {
                                    seat: SeatId(0),
                                    rank: 0,
                                    score: 0,
                                }],
                                "test".into(),
                                &[SeatId(0)],
                            )
                            .expect("fixture outcome is valid"),
                        });
                    }
                    Ok(Outcome {
                        events: smallvec![Event],
                        effects,
                    })
                }
                Input::Timer { .. } | Input::Seat { .. } | Input::Admin(_) => {
                    Err(tabula_core::RuleError::code(RuleErrorCode::Unsupported))
                }
            }
        }

        fn project(state: &State, _: Viewer) -> View {
            View { steps: state.steps }
        }

        fn view_event(_: &State, event: &Event, _: Viewer) -> Option<Event> {
            Some(event.clone())
        }

        fn legal_commands(_: &State, _: SeatId) -> LegalCommands<Command> {
            LegalCommands::Unknown
        }
    }

    impl GameModule for Module {
        type Rules = Rules;

        fn metadata() -> &'static GameMetadata {
            static METADATA: std::sync::LazyLock<GameMetadata> =
                std::sync::LazyLock::new(|| panic!("metadata is not used by self-play fixture"));
            &METADATA
        }

        fn capabilities() -> &'static GameCapabilities {
            static CAPABILITIES: std::sync::LazyLock<GameCapabilities> =
                std::sync::LazyLock::new(|| {
                    panic!("capabilities are not used by self-play fixture")
                });
            &CAPABILITIES
        }

        fn bot(_: BotLevel) -> Option<Box<dyn GameBot<Rules>>> {
            Some(Box::new(TestBot))
        }

        fn validate_config(_: &Config, _: &SeatRoster) -> Result<(), tabula_game_api::ConfigError> {
            Ok(())
        }
    }

    fn setup() -> SelfPlaySetup<Rules> {
        SelfPlaySetup {
            config: Config,
            roster: SeatRoster::new(smallvec![tabula_core::SeatEntry {
                seat: SeatId(0),
                occupant: Occupant::Bot {
                    level: BotLevel::Trivial,
                },
                team: None,
            }])
            .expect("fixture roster is valid"),
        }
    }

    fn cfg() -> SelfPlayConfig {
        SelfPlayConfig {
            matches: 2,
            base_seed: [9; 32],
            hostile_fraction: 0.0,
            max_inputs: 8,
            check_projections: true,
            start_match_index: 0,
        }
    }

    #[test]
    fn same_setup_and_seed_is_semantically_stable() {
        let report = run::<Module>(&setup(), &cfg()).expect("fixture self-play is valid");
        assert!(
            report.is_success(),
            "unexpected self-play failures: {report:?}"
        );
        assert_eq!(report.matches_run, 2);
        assert_eq!(report.terminated, 2);
        assert_eq!(report.inputs_total, 2 * 2);
    }

    #[test]
    fn timer_queue_rearms_and_cancels() {
        let mut queue = TimerQueue::default();
        queue.set(TimerId(7), LogicalTime(100));
        queue.set(TimerId(7), LogicalTime(200));
        queue.set(TimerId(3), LogicalTime(200));
        assert_eq!(queue.next(), Some((TimerId(3), LogicalTime(200))));
        queue.remove(TimerId(3));
        assert_eq!(queue.next(), Some((TimerId(7), LogicalTime(200))));
        queue.remove(TimerId(7));
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn timer_deadline_wins_an_exact_bot_tie() {
        let scheduled = choose_scheduled(
            Some(BotAction {
                seat: SeatId(0),
                ready_at: LogicalTime(10),
                command: Command::Step,
            }),
            Some((TimerId(1), LogicalTime(10))),
        )
        .expect("one timer and one bot are ready");
        assert!(matches!(
            scheduled.input,
            Input::Timer { timer: TimerId(1) }
        ));
    }

    #[test]
    fn multiple_end_match_effects_are_rejected_by_the_runtime() {
        let outcome = MatchOutcome::new_for_seats(
            OutcomeKind::Draw,
            smallvec![Standing {
                seat: SeatId(0),
                rank: 0,
                score: 0,
            }],
            "test".into(),
            &[SeatId(0)],
        )
        .expect("fixture outcome is valid");
        let effects = [
            Effect::EndMatch {
                outcome: outcome.clone(),
            },
            Effect::EndMatch { outcome },
        ];
        let mut queue = TimerQueue::default();
        let mut terminal = None;
        let failure = interpret_effects(&effects, LogicalTime::ZERO, &mut queue, &mut terminal)
            .expect_err("a match may emit EndMatch exactly once");
        assert_eq!(failure.kind, SelfPlayFailureKind::MultipleEndMatch);
    }

    #[test]
    fn hostile_rejections_are_checked_transactionally() {
        let mut config = cfg();
        config.hostile_fraction = 1.0;
        // The fixture rejects hostile timer/admin/seat inputs without mutation;
        // this asserts that the hostile path still reaches the ordinary rules.
        let report = run::<Module>(&setup(), &config).expect("fixture config is valid");
        assert!(
            report.is_success(),
            "unexpected self-play failures: {report:?}"
        );
        assert_eq!(report.transactional_failures, 0);
    }

    #[test]
    fn max_inputs_returns_a_structured_failure() {
        let mut config = cfg();
        config.matches = 1;
        config.max_inputs = 1;
        let report = run::<Module>(&setup(), &config).expect("fixture config is valid");
        assert_eq!(report.max_input_failures, 1);
        assert_eq!(
            report.failures[0].kind,
            SelfPlayFailureKind::DidNotTerminate
        );
        assert_eq!(report.failures[0].input_index, Some(1));
    }

    #[test]
    fn invalid_fraction_is_rejected_without_running_a_match() {
        let mut config = cfg();
        config.hostile_fraction = f32::NAN;
        let error = run::<Module>(&setup(), &config).expect_err("NaN must be rejected");
        assert!(error.to_string().contains("hostile_fraction"));
    }
}
