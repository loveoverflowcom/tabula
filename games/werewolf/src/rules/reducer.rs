//! Pure transactional ClassicV1 reducer (doc 02 §3, W-D3–W-D17).
use super::{
    checked_deadline, create_initial_state, projection, scopes, Ballot, Config, Event, NightChoice,
    Phase, PhaseDuration, PlayerStatus, RawState, Role, State, VoteMode,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tabula_core::{
    LogicalTime, MatchOutcome, OutcomeKind, RuleError, RuleErrorCode, SeatChange, SeatId, Standing,
    TimerId, Viewer,
};
use tabula_game_api::{
    AdminInput, Ctx, Effect, GameRules, Init, InitError, Input, LegalCommands, Outcome,
};

/// A private Night choice or replaceable public ballot (W-D3–W-D7).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Night(NightChoice),
    Vote(Ballot),
    Unvote,
}

/// Complete deterministic ClassicV1 rules authority; no transport or bot policy.
#[derive(Debug)]
pub struct WerewolfRules;

impl GameRules for WerewolfRules {
    type State = State;
    type Command = Command;
    type Event = Event;
    type View = projection::View;
    type ViewEvent = projection::ViewEvent;
    type Config = Config;
    const RULES_VERSION: tabula_core::RulesVersion = super::RULES_VERSION;
    const RULES_HASH: [u8; 32] = super::RULES_HASH;

    fn create(
        config: &Config,
        roster: &tabula_core::SeatRoster,
        ctx: &mut Ctx<'_>,
    ) -> Result<Init<Self>, InitError> {
        let (state, events) = create_initial_state(config, roster, ctx.now, ctx.rng)
            .map_err(|_| InitError::Config("werewolf.creation".into()))?;
        let effects = smallvec::smallvec![
            Effect::SetTimer {
                id: state.current_timer,
                delay: tabula_core::Millis(state.config.phase_durations.night.millis())
            },
            Effect::SetChatScopes(scopes::chat_scopes(&state))
        ];
        Ok(Init {
            state,
            events: events.into_iter().collect(),
            effects,
        })
    }

    fn apply(
        state: &mut State,
        input: Input<Command>,
        ctx: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        if state.phase == Phase::Ended {
            return Err(reject(RuleErrorCode::MatchOver));
        }
        // Small bounded state clone is the commit boundary (R2). No apply RNG draws.
        let mut next = state.clone();
        let mut out = Outcome::empty();
        match input {
            Input::Player { seat, command } => player(&mut next, seat, command, ctx.now, &mut out)?,
            Input::Timer { timer } => timer_expired(&mut next, timer, ctx.now, &mut out)?,
            Input::Seat { seat, change } => lifecycle(&mut next, seat, &change, &mut out)?,
            Input::Admin(AdminInput::Cancel { reason }) => {
                let outcome = MatchOutcome::new(
                    OutcomeKind::Aborted { reason },
                    smallvec::smallvec![],
                    "werewolf.cancelled".into(),
                )
                .map_err(|_| reject(RuleErrorCode::IllegalMove))?;
                finish(&mut next, outcome, &mut out);
            }
            Input::Admin(AdminInput::ForceEnd { outcome }) => {
                // Reconstruct through State's roster barrier before accepting operator result.
                let mut raw = RawState::from(next.clone());
                raw.phase = Phase::Ended;
                raw.outcome = Some(outcome.clone());
                raw.night_choices.clear();
                raw.votes.clear();
                State::try_from(raw).map_err(|_| reject(RuleErrorCode::IllegalMove))?;
                finish(&mut next, outcome, &mut out);
            }
            Input::Admin(AdminInput::Pause | AdminInput::Resume) => {
                return Err(reject(RuleErrorCode::Unsupported))
            }
        }
        next.history.extend(
            out.events
                .iter()
                .filter(|event| {
                    !matches!(
                        event,
                        Event::BallotChanged { .. } | Event::SeatStatusChanged { .. }
                    )
                })
                .cloned(),
        );
        State::try_from(RawState::from(next.clone()))
            .map_err(|_| reject(RuleErrorCode::IllegalMove))?;
        *state = next;
        Ok(out)
    }

    fn project(state: &State, viewer: Viewer) -> Self::View {
        projection::project(state, viewer)
    }
    fn view_event(state: &State, event: &Event, viewer: Viewer) -> Option<Self::ViewEvent> {
        projection::view_event(state, event, viewer)
    }
    fn legal_commands(state: &State, seat: SeatId) -> LegalCommands<Command> {
        let commands = commands(state, seat);
        if commands.is_empty() {
            LegalCommands::None
        } else {
            LegalCommands::Enumerated(commands)
        }
    }
    fn describe(state: &State, viewer: Viewer) -> tabula_game_api::A11yDescription {
        let view = projection::project(state, viewer);
        tabula_game_api::A11yDescription {
            status: format!(
                "werewolf.phase.{:?}; werewolf.round.{}; werewolf.alive.{}",
                view.phase,
                view.round,
                view.roster.iter().filter(|seat| seat.alive).count()
            ),
            regions: vec![],
            actions: view
                .legal_commands
                .iter()
                .enumerate()
                .map(|(index, command)| tabula_game_api::A11yAction {
                    id: tabula_game_api::ActionId(format!("werewolf.action.{index}")),
                    label: format!("{command:?}"),
                    enabled: true,
                })
                .collect(),
        }
    }
}

fn reject(code: RuleErrorCode) -> RuleError {
    RuleError::code(code)
}

fn player(
    state: &mut State,
    seat: SeatId,
    command: Command,
    now: LogicalTime,
    out: &mut Outcome<WerewolfRules>,
) -> Result<(), RuleError> {
    let Some(status) = state.player_status.get(&seat) else {
        return Err(reject(RuleErrorCode::NoSuchSeat));
    };
    if !state.alive.contains(&seat) || !status.can_act() {
        return Err(reject(RuleErrorCode::NotYourTurn));
    }
    if now >= state.phase_ends_at {
        return Err(reject(RuleErrorCode::WrongPhase));
    }
    match command {
        Command::Night(choice) => {
            if state.phase != Phase::Night {
                return Err(reject(RuleErrorCode::WrongPhase));
            }
            validate_night(state, seat, choice)?;
            if let Some(potions) = &mut state.witch_potions {
                match choice {
                    NightChoice::WitchHeal(Some(_)) => potions.heal = false,
                    NightChoice::WitchPoison(Some(_)) => potions.poison = false,
                    _ => {}
                }
            }
            state.night_choices.insert(seat, choice);
            if state.roles[&seat] == Role::Hunter {
                state.hunter_mark = choice.target();
            }
            out.events.push(Event::NightActionSubmitted {
                seat,
                choice,
                round: state.round,
            });
        }
        Command::Vote(ballot) => {
            if state.phase != Phase::Vote {
                return Err(reject(RuleErrorCode::WrongPhase));
            }
            if let Ballot::Target(target) = ballot {
                if target == seat || !state.alive.contains(&target) {
                    return Err(reject(RuleErrorCode::IllegalMove));
                }
            }
            state.votes.insert(seat, ballot);
            out.events.push(Event::BallotChanged {
                seat,
                ballot: Some(ballot),
            });
        }
        Command::Unvote => {
            if state.phase != Phase::Vote {
                return Err(reject(RuleErrorCode::WrongPhase));
            }
            state.votes.remove(&seat);
            out.events.push(Event::BallotChanged { seat, ballot: None });
        }
    }
    Ok(())
}

fn validate_night(state: &State, seat: SeatId, choice: NightChoice) -> Result<(), RuleError> {
    if state.night_choices.contains_key(&seat) {
        return Err(reject(RuleErrorCode::IllegalMove));
    }
    let role = state.roles[&seat];
    let allowed_role = match choice {
        NightChoice::WolfTarget(_) => role == Role::Werewolf,
        NightChoice::Investigate(_) => role == Role::Seer,
        NightChoice::Protect(_) => role == Role::Doctor,
        NightChoice::WitchHeal(_) | NightChoice::WitchPoison(_) => role == Role::Witch,
        NightChoice::HunterMark(_) => role == Role::Hunter,
        NightChoice::Pass => role != Role::Villager,
    };
    if !allowed_role {
        return Err(reject(RuleErrorCode::IllegalMove));
    }
    if let Some(target) = choice.target() {
        if !state.alive.contains(&target) {
            return Err(reject(RuleErrorCode::IllegalMove));
        }
        match choice {
            NightChoice::WolfTarget(_) if state.roles[&target].is_wolf() => {
                return Err(reject(RuleErrorCode::IllegalMove))
            }
            NightChoice::Investigate(_) | NightChoice::HunterMark(_) if target == seat => {
                return Err(reject(RuleErrorCode::IllegalMove))
            }
            NightChoice::Protect(_) if state.last_doctor_target == Some(target) => {
                return Err(reject(RuleErrorCode::IllegalMove))
            }
            NightChoice::WitchHeal(_)
                if !state.witch_potions.is_some_and(|potions| potions.heal) =>
            {
                return Err(reject(RuleErrorCode::IllegalMove))
            }
            NightChoice::WitchPoison(_)
                if !state.witch_potions.is_some_and(|potions| potions.poison) =>
            {
                return Err(reject(RuleErrorCode::IllegalMove))
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn commands(state: &State, seat: SeatId) -> Vec<Command> {
    if !state.alive.contains(&seat)
        || !state
            .player_status
            .get(&seat)
            .is_some_and(|status| status.can_act())
    {
        return vec![];
    }
    match state.phase {
        Phase::Night => {
            let Some(&role) = state.roles.get(&seat) else {
                return vec![];
            };
            let mut choices = vec![NightChoice::Pass];
            for &target in &state.alive {
                match role {
                    Role::Werewolf => choices.push(NightChoice::WolfTarget(Some(target))),
                    Role::Seer => choices.push(NightChoice::Investigate(target)),
                    Role::Doctor => choices.push(NightChoice::Protect(Some(target))),
                    Role::Hunter => choices.push(NightChoice::HunterMark(Some(target))),
                    Role::Witch => {
                        choices.push(NightChoice::WitchHeal(Some(target)));
                        choices.push(NightChoice::WitchPoison(Some(target)));
                    }
                    Role::Villager => {}
                }
            }
            choices
                .into_iter()
                .filter(|&choice| validate_night(state, seat, choice).is_ok())
                .map(Command::Night)
                .collect()
        }
        Phase::Vote => std::iter::once(Command::Vote(Ballot::Abstain))
            .chain(std::iter::once(Command::Unvote))
            .chain(
                state
                    .alive
                    .iter()
                    .filter(|&&target| target != seat)
                    .map(|&target| Command::Vote(Ballot::Target(target))),
            )
            .collect(),
        _ => vec![],
    }
}

fn lifecycle(
    state: &mut State,
    seat: SeatId,
    change: &SeatChange,
    out: &mut Outcome<WerewolfRules>,
) -> Result<(), RuleError> {
    let status = state
        .player_status
        .get_mut(&seat)
        .ok_or_else(|| reject(RuleErrorCode::NoSuchSeat))?;
    if matches!(
        change,
        SeatChange::Occupied { .. } | SeatChange::OccupantChanged { .. }
    ) {
        return Err(reject(RuleErrorCode::Unsupported));
    }
    if *status == PlayerStatus::PermanentlyAbsent {
        return Err(reject(RuleErrorCode::Unsupported));
    }
    *status = match change {
        SeatChange::Vacated | SeatChange::Abandoned => PlayerStatus::PermanentlyAbsent,
        SeatChange::Disconnected => PlayerStatus::Disconnected,
        SeatChange::WentIdle => PlayerStatus::Idle,
        SeatChange::Reconnected | SeatChange::BecameActive => PlayerStatus::Active,
        SeatChange::Occupied { .. } | SeatChange::OccupantChanged { .. } => {
            return Err(reject(RuleErrorCode::Unsupported))
        }
    };
    out.events.push(Event::SeatStatusChanged {
        seat,
        status: *status,
    });
    Ok(())
}

fn timer_expired(
    state: &mut State,
    timer: TimerId,
    now: LogicalTime,
    out: &mut Outcome<WerewolfRules>,
) -> Result<(), RuleError> {
    if timer != state.current_timer || now < state.phase_ends_at {
        return Err(reject(RuleErrorCode::IllegalMove));
    }
    match state.phase {
        Phase::Night => {
            resolve_night(state, out);
            if !victory(state, out)? {
                begin_phase(state, Phase::Dawn, now, out)?;
            }
        }
        Phase::Dawn => begin_phase(state, Phase::Day, now, out)?,
        Phase::Day => begin_phase(state, Phase::Vote, now, out)?,
        Phase::Vote => {
            resolve_vote(state, out);
            if !victory(state, out)? {
                begin_phase(state, Phase::Dusk, now, out)?;
            }
        }
        Phase::Dusk => {
            if state.round == state.config.max_rounds.get() {
                let result = result(state, None, "werewolf.stalemate")?;
                finish(state, result, out);
            } else {
                state.round += 1;
                state.hunter_mark = None;
                begin_phase(state, Phase::Night, now, out)?;
            }
        }
        Phase::Ended => return Err(reject(RuleErrorCode::MatchOver)),
    }
    Ok(())
}

fn duration(state: &State, phase: Phase) -> PhaseDuration {
    match phase {
        Phase::Night => state.config.phase_durations.night,
        Phase::Dawn => state.config.phase_durations.dawn,
        Phase::Day => state.config.phase_durations.day,
        Phase::Vote => state.config.phase_durations.vote,
        Phase::Dusk | Phase::Ended => state.config.phase_durations.dusk,
    }
}
fn begin_phase(
    state: &mut State,
    phase: Phase,
    now: LogicalTime,
    out: &mut Outcome<WerewolfRules>,
) -> Result<(), RuleError> {
    let timer = TimerId(
        state
            .current_timer
            .0
            .checked_add(1)
            .ok_or_else(|| reject(RuleErrorCode::IllegalMove))?,
    );
    let delay = duration(state, phase);
    let deadline = checked_deadline(now, delay).map_err(|_| reject(RuleErrorCode::IllegalMove))?;
    out.effects.push(Effect::CancelTimer {
        id: state.current_timer,
    });
    state.current_timer = timer;
    state.phase_ends_at = deadline;
    state.phase = phase;
    out.events.push(Event::PhaseChanged {
        phase,
        round: state.round,
        timer_id: timer,
        ends_at: deadline,
    });
    out.effects.push(Effect::SetTimer {
        id: timer,
        delay: tabula_core::Millis(delay.millis()),
    });
    out.effects
        .push(Effect::SetChatScopes(scopes::chat_scopes(state)));
    Ok(())
}

// Unique positive plurality; absence/pass never becomes an elimination candidate.
fn unique_max(tally: &BTreeMap<SeatId, u8>) -> Option<SeatId> {
    let max = tally.values().copied().max()?;
    let mut winners = tally.iter().filter(|(_, count)| **count == max);
    let (&first, _) = winners.next()?;
    winners.next().is_none().then_some(first)
}

fn resolve_night(state: &mut State, out: &mut Outcome<WerewolfRules>) {
    let mut tally = BTreeMap::new();
    let mut protected = None;
    let mut healed = None;
    let mut poisoned = None;
    for (&actor, &choice) in &state.night_choices {
        match choice {
            NightChoice::WolfTarget(Some(target)) => *tally.entry(target).or_insert(0) += 1,
            NightChoice::Protect(target) => protected = target,
            NightChoice::WitchHeal(target) => healed = target,
            NightChoice::WitchPoison(target) => poisoned = target,
            NightChoice::Investigate(target) => {
                let alignment = state.roles[&target].alignment();
                state.seer_history.insert(target, alignment);
                out.events.push(Event::SeerReport {
                    seer: actor,
                    target,
                    alignment,
                    round: state.round,
                });
            }
            NightChoice::HunterMark(_) | NightChoice::Pass | NightChoice::WolfTarget(None) => {}
        }
    }
    state.last_doctor_target = protected;
    let attacked = unique_max(&tally);
    out.events.push(Event::NightResolved {
        round: state.round,
        attacked,
        protected,
        healed,
        poisoned,
    });
    let mut deaths = BTreeSet::new();
    if let Some(target) = attacked {
        if Some(target) != protected && Some(target) != healed {
            deaths.insert(target);
        }
    }
    if let Some(target) = poisoned {
        deaths.insert(target);
    }
    state.night_choices.clear();
    deaths_and_hunter(state, deaths, out);
}

fn resolve_vote(state: &mut State, out: &mut Outcome<WerewolfRules>) {
    let mut tally = BTreeMap::new();
    for ballot in state.votes.values() {
        if let Ballot::Target(target) = ballot {
            *tally.entry(*target).or_insert(0) += 1;
        }
    }
    let eliminated = unique_max(&tally).filter(|seat| {
        state.config.vote_mode == VoteMode::Plurality
            || usize::from(tally[seat]) > state.alive.len() / 2
    });
    out.events.push(Event::VoteResolved {
        round: state.round,
        tally,
        eliminated,
    });
    state.votes.clear();
    deaths_and_hunter(state, eliminated.into_iter().collect(), out);
}

fn deaths_and_hunter(
    state: &mut State,
    mut deaths: BTreeSet<SeatId>,
    out: &mut Outcome<WerewolfRules>,
) {
    if !state.hunter_fired {
        if let Some(&hunter) = deaths
            .iter()
            .find(|seat| state.roles.get(seat) == Some(&Role::Hunter))
        {
            state.hunter_fired = true;
            let target = state
                .hunter_mark
                .filter(|target| state.alive.contains(target) && !deaths.contains(target));
            if let Some(target) = target {
                deaths.insert(target);
            }
            out.events.push(Event::HunterTriggered { hunter, target });
        }
    }
    for seat in deaths {
        if state.alive.remove(&seat) {
            let role = state.roles[&seat];
            state.revealed.insert(seat, role);
            out.events.push(Event::DeathRevealed { seat, role });
        }
    }
}

fn victory(state: &mut State, out: &mut Outcome<WerewolfRules>) -> Result<bool, RuleError> {
    let wolves = state
        .alive
        .iter()
        .filter(|seat| state.roles[seat].is_wolf())
        .count();
    let living = state.alive.len();
    let outcome = if living == 0 {
        Some(result(state, None, "werewolf.no_survivors")?)
    } else if wolves == 0 {
        Some(result(state, Some(false), "werewolf.village_wins")?)
    } else if wolves >= living - wolves {
        Some(result(state, Some(true), "werewolf.wolves_win")?)
    } else {
        None
    };
    if let Some(outcome) = outcome {
        finish(state, outcome, out);
        Ok(true)
    } else {
        Ok(false)
    }
}

fn result(
    state: &State,
    wolf_winner: Option<bool>,
    summary: &str,
) -> Result<MatchOutcome, RuleError> {
    let standings = state
        .roster
        .iter()
        .map(|&seat| Standing {
            seat,
            rank: u8::from(
                wolf_winner.is_some_and(|winner| state.roles[&seat].is_wolf() != winner),
            ),
            score: i64::from(
                wolf_winner.is_some_and(|winner| state.roles[&seat].is_wolf() == winner),
            ),
        })
        .collect();
    MatchOutcome::new_for_seats(
        if wolf_winner.is_some() {
            OutcomeKind::Decisive
        } else {
            OutcomeKind::Draw
        },
        standings,
        summary.into(),
        &state.roster,
    )
    .map_err(|_| reject(RuleErrorCode::IllegalMove))
}
fn finish(state: &mut State, outcome: MatchOutcome, out: &mut Outcome<WerewolfRules>) {
    state.phase = Phase::Ended;
    state.night_choices.clear();
    state.votes.clear();
    state.outcome = Some(outcome.clone());
    out.events.push(Event::MatchEnded {
        outcome: outcome.clone(),
    });
    out.effects.push(Effect::CancelTimer {
        id: state.current_timer,
    });
    out.effects
        .push(Effect::SetChatScopes(scopes::chat_scopes(state)));
    out.effects.push(Effect::EndMatch { outcome });
}
