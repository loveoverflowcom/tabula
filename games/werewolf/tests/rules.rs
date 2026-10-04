//! `ClassicV1` semantic oracles independent from the reducer's algorithms.
use tabula_core::{
    canonical_decode, canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, Occupant,
    OutcomeKind, SeatChange, SeatEntry, SeatId, SeatRoster, TimerId, UserId,
};
use tabula_game_api::{
    AdminInput, Budget, Ctx, Effect, GameModule, GameRules, Input, LegalCommands,
};
use tabula_game_werewolf::{
    Alignment, Ballot, Command, Config, Event, MaxRounds, NightChoice, Phase, RawState, Role,
    State, VoteMode, WerewolfModule, WerewolfRules,
};

fn roster(n: u8) -> SeatRoster {
    SeatRoster::new(
        (0..n)
            .map(|seat| SeatEntry {
                seat: SeatId(seat),
                occupant: Occupant::Human(UserId(u128::from(seat))),
                team: None,
            })
            .collect(),
    )
    .unwrap()
}
fn ctx(now: u64, rng: &mut DetRng) -> Ctx<'_> {
    Ctx {
        now: LogicalTime(now),
        index: InputIndex(1),
        rng,
        budget: Budget::default(),
    }
}
fn seeded(n: u8, config: Config, seed: u8) -> State {
    let mut rng = DetRng::for_input(&MatchSeed::from_bytes([seed; 32]), InputIndex(0));
    WerewolfRules::create(&config, &roster(n), &mut ctx(0, &mut rng))
        .unwrap()
        .state
}
fn initial() -> State {
    let mut raw = RawState::from(seeded(10, Config::default(), 1));
    raw.roles = [
        Role::Werewolf,
        Role::Werewolf,
        Role::Seer,
        Role::Doctor,
        Role::Hunter,
        Role::Witch,
        Role::Villager,
        Role::Villager,
        Role::Villager,
        Role::Villager,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, role)| (SeatId(u8::try_from(index).unwrap()), role))
    .collect();
    raw.history.clear(); // Independent semantic fixture retains no counterfeit assignment.
    State::try_from(raw).unwrap()
}
fn apply(
    state: &mut State,
    input: Input<Command>,
    now: u64,
) -> tabula_game_api::Outcome<WerewolfRules> {
    let mut rng = DetRng::for_input(&MatchSeed::from_bytes([1; 32]), InputIndex(1));
    WerewolfRules::apply(state, input, &mut ctx(now, &mut rng)).unwrap()
}
fn reject(state: &mut State, input: Input<Command>, now: u64) {
    let before = canonical_encode(state).unwrap();
    let mut rng = DetRng::for_input(&MatchSeed::from_bytes([1; 32]), InputIndex(1));
    assert!(WerewolfRules::apply(state, input, &mut ctx(now, &mut rng)).is_err());
    assert_eq!(
        canonical_encode(state).unwrap(),
        before,
        "R2 rejected mutation"
    );
}
fn night(state: &mut State, actor: u8, choice: NightChoice) {
    let now = state.phase_ends_at().0 - 1;
    apply(
        state,
        Input::Player {
            seat: SeatId(actor),
            command: Command::Night(choice),
        },
        now,
    );
}
fn close(state: &mut State) -> tabula_game_api::Outcome<WerewolfRules> {
    let timer = state.current_timer();
    let now = state.phase_ends_at().0;
    apply(state, Input::Timer { timer }, now)
}
fn to_vote(state: &mut State) {
    while state.phase() != Phase::Vote {
        assert!(state.phase().is_playing());
        close(state);
    }
}
fn next_night(state: &mut State) {
    while !(state.phase() == Phase::Night && state.round() > 1) {
        assert!(state.phase().is_playing());
        close(state);
    }
}
fn vote(state: &mut State, actor: u8, ballot: Ballot) {
    let now = state.phase_ends_at().0 - 1;
    apply(
        state,
        Input::Player {
            seat: SeatId(actor),
            command: Command::Vote(ballot),
        },
        now,
    );
}
fn deaths(out: &tabula_game_api::Outcome<WerewolfRules>) -> Vec<SeatId> {
    out.events
        .iter()
        .filter_map(|event| {
            if let Event::DeathRevealed { seat, .. } = event {
                Some(*seat)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn fixed_timers_never_close_early_and_deadline_is_exclusive() {
    let mut state = initial();
    for (seat, role) in state.roles().clone() {
        if role != Role::Villager {
            night(&mut state, seat.0, NightChoice::Pass);
        }
    }
    assert_eq!(state.phase(), Phase::Night);
    assert_eq!(state.phase_ends_at(), LogicalTime(30_000));
    reject(&mut state, Input::Timer { timer: TimerId(1) }, 29_999);
    reject(
        &mut state,
        Input::Player {
            seat: SeatId(0),
            command: Command::Night(NightChoice::Pass),
        },
        30_000,
    );
    close(&mut state);
    assert_eq!(state.phase(), Phase::Dawn);
    assert_eq!(state.phase_ends_at(), LogicalTime(32_000));
    reject(&mut state, Input::Timer { timer: TimerId(1) }, 32_000);
    close(&mut state);
    assert_eq!(state.phase(), Phase::Day);
    assert_eq!(state.phase_ends_at(), LogicalTime(152_000));
    close(&mut state);
    assert_eq!(state.phase(), Phase::Vote);
    assert_eq!(state.phase_ends_at(), LogicalTime(182_000));
    for actor in 0..10 {
        vote(&mut state, actor, Ballot::Abstain);
    }
    assert_eq!(state.phase(), Phase::Vote);
    close(&mut state);
    assert_eq!(state.phase(), Phase::Dusk);
    close(&mut state);
    assert_eq!(state.phase(), Phase::Night);
    assert_eq!(state.round(), 2);
}
#[test]
fn wolf_consensus_tie_pass_and_plurality_are_independent_of_submission_order() {
    let mut tied = initial();
    night(&mut tied, 0, NightChoice::WolfTarget(Some(SeatId(6))));
    night(&mut tied, 1, NightChoice::WolfTarget(Some(SeatId(7))));
    assert!(deaths(&close(&mut tied)).is_empty());
    let mut passed = initial();
    night(&mut passed, 0, NightChoice::Pass);
    assert!(deaths(&close(&mut passed)).is_empty());
    let mut a = initial();
    let mut b = initial();
    for state in [&mut a, &mut b] {
        night(state, 0, NightChoice::WolfTarget(Some(SeatId(6))));
    }
    night(&mut a, 1, NightChoice::WolfTarget(Some(SeatId(6))));
    night(&mut b, 1, NightChoice::Pass);
    assert_eq!(deaths(&close(&mut a)), vec![SeatId(6)]);
    assert_eq!(deaths(&close(&mut b)), vec![SeatId(6)]);
}
#[test]
fn doctor_self_save_repeat_rejection_and_pass_reset() {
    let mut state = initial();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(3))));
    night(&mut state, 3, NightChoice::Protect(Some(SeatId(3))));
    assert!(deaths(&close(&mut state)).is_empty());
    next_night(&mut state);
    let now = state.phase_ends_at().0 - 1;
    reject(
        &mut state,
        Input::Player {
            seat: SeatId(3),
            command: Command::Night(NightChoice::Protect(Some(SeatId(3)))),
        },
        now,
    );
    night(&mut state, 3, NightChoice::Pass);
    close(&mut state);
    while state.phase() != Phase::Night {
        close(&mut state);
    }
    night(&mut state, 3, NightChoice::Protect(Some(SeatId(3))));
}
#[test]
fn witch_blind_heal_consumed_on_submission_and_one_potion_per_night() {
    let mut state = initial();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(6))));
    night(&mut state, 5, NightChoice::WitchHeal(Some(SeatId(7))));
    assert!(!state.witch_potions().unwrap().heal);
    assert!(state.witch_potions().unwrap().poison);
    reject(
        &mut state,
        Input::Player {
            seat: SeatId(5),
            command: Command::Night(NightChoice::WitchPoison(Some(SeatId(6)))),
        },
        1,
    );
    assert_eq!(deaths(&close(&mut state)), vec![SeatId(6)]);
    next_night(&mut state);
    let now = state.phase_ends_at().0 - 1;
    reject(
        &mut state,
        Input::Player {
            seat: SeatId(5),
            command: Command::Night(NightChoice::WitchHeal(Some(SeatId(5)))),
        },
        now,
    );
}
#[test]
fn poison_ignores_protection_and_duplicates_merge_sorted() {
    let mut state = initial();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(6))));
    night(&mut state, 3, NightChoice::Protect(Some(SeatId(6))));
    night(&mut state, 5, NightChoice::WitchPoison(Some(SeatId(6))));
    assert_eq!(deaths(&close(&mut state)), vec![SeatId(6)]);
    let mut state = initial();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(7))));
    night(&mut state, 5, NightChoice::WitchPoison(Some(SeatId(6))));
    assert_eq!(deaths(&close(&mut state)), vec![SeatId(6), SeatId(7)]);
}
#[test]
fn killed_actors_keep_actions_and_seer_receives_dead_target_report() {
    let mut state = initial();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(2))));
    night(&mut state, 2, NightChoice::Investigate(SeatId(5)));
    night(&mut state, 5, NightChoice::WitchPoison(Some(SeatId(5))));
    let out = close(&mut state);
    assert_eq!(deaths(&out), vec![SeatId(2), SeatId(5)]);
    assert_eq!(state.seer_history()[&SeatId(5)], Alignment::Village);
    assert!(out.events.iter().any(|event| matches!(
        event,
        Event::SeerReport {
            seer: SeatId(2),
            target: SeatId(5),
            ..
        }
    )));
}
#[test]
fn hunter_precommit_night_and_vote_death_fire_once_before_victory() {
    let mut state = initial();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(4))));
    night(&mut state, 4, NightChoice::HunterMark(Some(SeatId(0))));
    let out = close(&mut state);
    assert_eq!(deaths(&out), vec![SeatId(0), SeatId(4)]);
    assert!(state.hunter_fired());
    assert_eq!(state.phase(), Phase::Dawn);
    reject(
        &mut state,
        Input::Player {
            seat: SeatId(4),
            command: Command::Night(NightChoice::HunterMark(Some(SeatId(1)))),
        },
        31_000,
    );
    let mut state = initial();
    night(&mut state, 4, NightChoice::HunterMark(Some(SeatId(0))));
    to_vote(&mut state);
    vote(&mut state, 6, Ballot::Target(SeatId(4)));
    assert_eq!(deaths(&close(&mut state)), vec![SeatId(0), SeatId(4)]);
}
#[test]
fn hunter_mark_expires_each_round_and_already_dying_target_is_not_double_killed() {
    let mut state = initial();
    night(&mut state, 4, NightChoice::HunterMark(Some(SeatId(6))));
    next_night(&mut state);
    assert_eq!(state.hunter_mark(), None);
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(4))));
    assert_eq!(deaths(&close(&mut state)), vec![SeatId(4)]);
    let mut state = initial();
    night(&mut state, 4, NightChoice::HunterMark(Some(SeatId(6))));
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(4))));
    night(&mut state, 5, NightChoice::WitchPoison(Some(SeatId(6))));
    assert_eq!(deaths(&close(&mut state)), vec![SeatId(4), SeatId(6)]);
}
#[test]
fn vote_replace_unvote_abstain_tie_and_majority_threshold() {
    let mut state = initial();
    to_vote(&mut state);
    vote(&mut state, 6, Ballot::Target(SeatId(0)));
    vote(&mut state, 6, Ballot::Target(SeatId(1)));
    assert_eq!(state.votes()[&SeatId(6)], Ballot::Target(SeatId(1)));
    let now = state.phase_ends_at().0 - 1;
    apply(
        &mut state,
        Input::Player {
            seat: SeatId(6),
            command: Command::Unvote,
        },
        now,
    );
    assert!(!state.votes().contains_key(&SeatId(6)));
    vote(&mut state, 6, Ballot::Abstain);
    vote(&mut state, 7, Ballot::Target(SeatId(0)));
    vote(&mut state, 8, Ballot::Target(SeatId(1)));
    assert!(deaths(&close(&mut state)).is_empty());
    let mut raw = RawState::from(initial());
    raw.config.vote_mode = VoteMode::AbsoluteMajority;
    let mut state = State::try_from(raw).unwrap();
    to_vote(&mut state);
    for actor in 2..7 {
        vote(&mut state, actor, Ballot::Target(SeatId(0)));
    }
    assert!(deaths(&close(&mut state)).is_empty());
    let mut raw = RawState::from(initial());
    raw.config.vote_mode = VoteMode::AbsoluteMajority;
    let mut state = State::try_from(raw).unwrap();
    to_vote(&mut state);
    for actor in 2..8 {
        vote(&mut state, actor, Ballot::Target(SeatId(0)));
    }
    assert_eq!(deaths(&close(&mut state)), vec![SeatId(0)]);
}
#[test]
fn absence_keeps_choices_reconnect_allows_action_and_substitution_is_forbidden() {
    let mut state = initial();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(6))));
    apply(
        &mut state,
        Input::Seat {
            seat: SeatId(0),
            change: SeatChange::Disconnected,
        },
        1,
    );
    assert!(state.night_choices().contains_key(&SeatId(0)));
    apply(
        &mut state,
        Input::Seat {
            seat: SeatId(3),
            change: SeatChange::WentIdle,
        },
        2,
    );
    reject(
        &mut state,
        Input::Player {
            seat: SeatId(3),
            command: Command::Night(NightChoice::Pass),
        },
        3,
    );
    apply(
        &mut state,
        Input::Seat {
            seat: SeatId(3),
            change: SeatChange::Reconnected,
        },
        4,
    );
    night(&mut state, 3, NightChoice::Pass);
    apply(
        &mut state,
        Input::Seat {
            seat: SeatId(7),
            change: SeatChange::Abandoned,
        },
        5,
    );
    reject(
        &mut state,
        Input::Seat {
            seat: SeatId(7),
            change: SeatChange::Reconnected,
        },
        6,
    );
    reject(
        &mut state,
        Input::Seat {
            seat: SeatId(8),
            change: SeatChange::OccupantChanged {
                from: Occupant::Empty,
                to: Occupant::Empty,
            },
        },
        7,
    );
    assert_eq!(deaths(&close(&mut state)), vec![SeatId(6)]);
    assert!(state.alive().contains(&SeatId(0)));
    assert!(state.alive().contains(&SeatId(7)));
}
#[test]
fn victory_zero_alive_then_village_then_parity_and_dead_team_standings() {
    let mut raw = RawState::from(initial());
    raw.alive = [SeatId(0), SeatId(4)].into_iter().collect();
    raw.revealed = raw
        .roles
        .iter()
        .filter(|(seat, _)| !raw.alive.contains(seat))
        .map(|(&seat, &role)| (seat, role))
        .collect();
    let mut state = State::try_from(raw).unwrap();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(4))));
    night(&mut state, 4, NightChoice::HunterMark(Some(SeatId(0))));
    let out = close(&mut state);
    assert_eq!(state.outcome().unwrap().kind(), OutcomeKind::Draw);
    assert!(state.alive().is_empty());
    assert_eq!(
        out.effects
            .iter()
            .filter(|effect| matches!(effect, Effect::EndMatch { .. }))
            .count(),
        1
    );
    let mut raw = RawState::from(initial());
    raw.alive = [SeatId(0), SeatId(4), SeatId(6), SeatId(7)]
        .into_iter()
        .collect();
    raw.revealed = raw
        .roles
        .iter()
        .filter(|(seat, _)| !raw.alive.contains(seat))
        .map(|(&seat, &role)| (seat, role))
        .collect();
    let mut state = State::try_from(raw).unwrap();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(4))));
    night(&mut state, 4, NightChoice::HunterMark(Some(SeatId(0))));
    close(&mut state);
    assert_eq!(state.outcome().unwrap().summary(), "werewolf.village_wins");
    assert_eq!(
        state
            .outcome()
            .unwrap()
            .standings()
            .iter()
            .find(|standing| standing.seat == SeatId(4))
            .unwrap()
            .rank,
        0
    );
    let mut raw = RawState::from(initial());
    raw.alive = [SeatId(0), SeatId(1), SeatId(6), SeatId(7), SeatId(8)]
        .into_iter()
        .collect();
    raw.revealed = raw
        .roles
        .iter()
        .filter(|(seat, _)| !raw.alive.contains(seat))
        .map(|(&seat, &role)| (seat, role))
        .collect();
    let mut state = State::try_from(raw).unwrap();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(6))));
    close(&mut state);
    assert_eq!(state.outcome().unwrap().summary(), "werewolf.wolves_win");
    assert_eq!(
        state
            .outcome()
            .unwrap()
            .standings()
            .iter()
            .find(|standing| standing.seat == SeatId(2))
            .unwrap()
            .rank,
        1
    );
}
#[test]
fn round_cap_and_terminal_rejections_and_cancel_end_once() {
    let config = Config {
        max_rounds: MaxRounds::new(1).unwrap(),
        ..Config::default()
    };
    let mut state = seeded(6, config, 4);
    for _ in 0..5 {
        close(&mut state);
    }
    assert_eq!(state.phase(), Phase::Ended);
    assert_eq!(state.outcome().unwrap().summary(), "werewolf.stalemate");
    let timer = state.current_timer();
    let now = state.phase_ends_at().0;
    reject(&mut state, Input::Timer { timer }, now);
    let mut state = initial();
    let out = apply(
        &mut state,
        Input::Admin(AdminInput::Cancel {
            reason: tabula_core::AbortReason::OperatorCancelled,
        }),
        1,
    );
    assert_eq!(
        out.effects
            .iter()
            .filter(|effect| matches!(effect, Effect::EndMatch { .. }))
            .count(),
        1
    );
    reject(
        &mut state,
        Input::Admin(AdminInput::Cancel {
            reason: tabula_core::AbortReason::OperatorCancelled,
        }),
        2,
    );
}
#[test]
fn hostile_inputs_and_overflow_are_transactional_without_rng_consumption() {
    let mut state = initial();
    let before = state.clone();
    for command in [
        Command::Night(NightChoice::Investigate(SeatId(0))),
        Command::Night(NightChoice::WolfTarget(Some(SeatId(255)))),
        Command::Vote(Ballot::Target(SeatId(1))),
        Command::Unvote,
    ] {
        reject(
            &mut state,
            Input::Player {
                seat: SeatId(0),
                command,
            },
            1,
        );
    }
    reject(
        &mut state,
        Input::Player {
            seat: SeatId(255),
            command: Command::Night(NightChoice::Pass),
        },
        1,
    );
    reject(&mut state, Input::Admin(AdminInput::Pause), 1);
    assert_eq!(state, before);
    let mut raw = RawState::from(state);
    raw.phase_ends_at = LogicalTime(u64::MAX);
    let mut state = State::try_from(raw).unwrap();
    let timer = state.current_timer();
    reject(&mut state, Input::Timer { timer }, u64::MAX);
    let seed = MatchSeed::from_bytes([4; 32]);
    let mut a = DetRng::for_input(&seed, InputIndex(1));
    let b = DetRng::for_input(&seed, InputIndex(1));
    assert!(WerewolfRules::apply(
        &mut state,
        Input::Player {
            seat: SeatId(255),
            command: Command::Unvote
        },
        &mut ctx(1, &mut a)
    )
    .is_err());
    assert_eq!(a.stream(9000).next_u64(), b.stream(9000).next_u64());
}
#[test]
fn production_module_never_exposes_bots() {
    for level in [
        tabula_core::BotLevel::Trivial,
        tabula_core::BotLevel::Easy,
        tabula_core::BotLevel::Medium,
        tabula_core::BotLevel::Hard,
    ] {
        assert!(WerewolfModule::bot(level).is_none());
    }
    assert!(WerewolfModule::declared_bot_levels().is_empty());
}
#[test]
fn randomized_legal_matches_terminate_deterministically_with_roundtrip_each_phase() {
    for seed in 0..100_u8 {
        let config = Config {
            max_rounds: MaxRounds::new(3).unwrap(),
            ..Config::default()
        };
        let mut a = seeded(6 + seed % 15, config, seed);
        let mut b = a.clone();
        let mut driver = DetRng::for_input(&MatchSeed::from_bytes([seed; 32]), InputIndex(42));
        let mut steps = 0;
        while a.phase().is_playing() {
            for seat in a.roster().to_vec() {
                if let LegalCommands::Enumerated(commands) = WerewolfRules::legal_commands(&a, seat)
                {
                    let command = commands[usize::try_from(
                        driver.next_u64() % u64::try_from(commands.len()).unwrap(),
                    )
                    .unwrap()];
                    let now = a.phase_ends_at().0 - 1;
                    let input = Input::Player { seat, command };
                    let out_a = apply(&mut a, input.clone(), now);
                    let out_b = apply(&mut b, input, now);
                    assert_eq!(
                        canonical_encode(&out_a.events).unwrap(),
                        canonical_encode(&out_b.events).unwrap()
                    );
                }
            }
            let out_a = close(&mut a);
            let out_b = close(&mut b);
            assert_eq!(out_a.events, out_b.events);
            assert_eq!(WerewolfRules::state_hash(&a), WerewolfRules::state_hash(&b));
            assert_eq!(
                canonical_decode::<State>(&canonical_encode(&a).unwrap()).unwrap(),
                a
            );
            steps += 1;
            assert!(steps <= 15);
        }
        assert!(a.outcome().is_some());
        assert_eq!(a.roster().len(), a.outcome().unwrap().standings().len());
    }
}
#[test]
fn public_ballot_spam_does_not_expand_retained_history() {
    let mut state = initial();
    to_vote(&mut state);
    let before = state.history().len();
    for _ in 0..1000 {
        vote(&mut state, 6, Ballot::Abstain);
    }
    assert_eq!(state.history().len(), before);
}

#[test]
fn reconstructed_retained_facts_reject_forged_actors_targets_rounds_and_resources() {
    let base = RawState::from(initial());
    let invalid_events = vec![
        Event::NightActionSubmitted {
            seat: SeatId(255),
            choice: NightChoice::Pass,
            round: 1,
        },
        Event::NightActionSubmitted {
            seat: SeatId(0),
            choice: NightChoice::WolfTarget(Some(SeatId(255))),
            round: 1,
        },
        Event::NightActionSubmitted {
            seat: SeatId(0),
            choice: NightChoice::WolfTarget(Some(SeatId(1))),
            round: 1,
        },
        Event::NightActionSubmitted {
            seat: SeatId(2),
            choice: NightChoice::Investigate(SeatId(2)),
            round: 1,
        },
        Event::NightActionSubmitted {
            seat: SeatId(4),
            choice: NightChoice::HunterMark(Some(SeatId(4))),
            round: 1,
        },
        Event::NightActionSubmitted {
            seat: SeatId(0),
            choice: NightChoice::Pass,
            round: 0,
        },
        Event::NightActionSubmitted {
            seat: SeatId(0),
            choice: NightChoice::Pass,
            round: 2,
        },
        Event::SeerReport {
            seer: SeatId(2),
            target: SeatId(0),
            alignment: Alignment::Village,
            round: 1,
        },
        Event::SeerReport {
            seer: SeatId(3),
            target: SeatId(0),
            alignment: Alignment::Wolf,
            round: 1,
        },
        Event::DeathRevealed {
            seat: SeatId(0),
            role: Role::Werewolf,
        },
        Event::BallotChanged {
            seat: SeatId(0),
            ballot: Some(Ballot::Abstain),
        },
    ];
    for event in invalid_events {
        let mut raw = base.clone();
        raw.history.push(event);
        assert!(State::try_from(raw.clone()).is_err());
        assert!(canonical_decode::<State>(&canonical_encode(&raw).unwrap()).is_err());
    }
    for target in [SeatId(4), SeatId(255)] {
        let mut raw = base.clone();
        raw.hunter_mark = Some(target);
        assert!(State::try_from(raw).is_err());
    }
    let mut raw = base.clone();
    raw.hunter_fired = true;
    assert!(State::try_from(raw).is_err());
    let mut raw = base;
    raw.history = vec![
        Event::PhaseChanged {
            phase: Phase::Night,
            round: 1,
            timer_id: TimerId(1),
            ends_at: LogicalTime(30_000)
        };
        3001
    ];
    assert!(State::try_from(raw).is_err());
}

fn scope_seats(participants: &tabula_game_api::effect::Participants) -> Vec<SeatId> {
    match participants {
        tabula_game_api::effect::Participants::None => vec![],
        tabula_game_api::effect::Participants::Seats(seats) => seats.to_vec(),
        tabula_game_api::effect::Participants::Everyone => {
            panic!("expected explicit roster-bound scope")
        }
    }
}
#[test]
fn fixed_phase_scope_effects_are_absolute_and_separate_living_dead_wolves() {
    let mut state = initial();
    night(&mut state, 0, NightChoice::WolfTarget(Some(SeatId(6))));
    let night_scopes = tabula_game_werewolf::rules::scopes::chat_scopes(&state);
    assert_eq!(scope_seats(&night_scopes.channels[0].speak), vec![]);
    assert_eq!(
        scope_seats(&night_scopes.channels[1].speak),
        vec![SeatId(0), SeatId(1)]
    );
    for expected in [
        Phase::Dawn,
        Phase::Day,
        Phase::Vote,
        Phase::Dusk,
        Phase::Night,
    ] {
        let out = close(&mut state);
        assert_eq!(state.phase(), expected);
        let scopes = out
            .effects
            .iter()
            .find_map(|effect| {
                if let Effect::SetChatScopes(scopes) = effect {
                    Some(scopes)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(scopes.channels.len(), 3);
        let table = &scopes.channels[0];
        let wolves = &scopes.channels[1];
        let dead = &scopes.channels[2];
        assert_eq!(scope_seats(&dead.speak), vec![SeatId(6)]);
        assert_eq!(scope_seats(&dead.listen), vec![SeatId(6)]);
        let living: Vec<_> = state
            .roster()
            .iter()
            .copied()
            .filter(|seat| *seat != SeatId(6))
            .collect();
        assert_eq!(
            scope_seats(&table.speak),
            if expected == Phase::Day {
                living
            } else {
                vec![]
            }
        );
        assert_eq!(
            scope_seats(&table.listen),
            if matches!(expected, Phase::Dawn | Phase::Dusk) {
                state.roster().to_vec()
            } else if expected == Phase::Day {
                state.alive().iter().copied().collect()
            } else {
                vec![]
            }
        );
        assert_eq!(
            scope_seats(&wolves.speak),
            if expected == Phase::Night {
                vec![SeatId(0), SeatId(1)]
            } else {
                vec![]
            }
        );
        assert_eq!(scope_seats(&wolves.listen), scope_seats(&wolves.speak));
    }
    let now = state.phase_ends_at().0 - 1;
    let out = apply(
        &mut state,
        Input::Admin(AdminInput::Cancel {
            reason: tabula_core::AbortReason::OperatorCancelled,
        }),
        now,
    );
    let scopes = out
        .effects
        .iter()
        .find_map(|effect| {
            if let Effect::SetChatScopes(scopes) = effect {
                Some(scopes)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(scope_seats(&scopes.channels[0].speak), state.roster());
}

#[test]
fn maximum_rounds_with_all_roles_acting_has_bounded_history_and_small_state() {
    let mut state = seeded(20, Config::default(), 7);
    let mut inputs = 0;
    while state.phase().is_playing() {
        if state.phase() == Phase::Night {
            for (&seat, &role) in &state.roles().clone() {
                match role {
                    Role::Villager => {}
                    Role::Seer => {
                        let target = *state
                            .alive()
                            .iter()
                            .find(|&&target| target != seat)
                            .unwrap();
                        night(&mut state, seat.0, NightChoice::Investigate(target));
                        inputs += 1;
                    }
                    _ => {
                        night(&mut state, seat.0, NightChoice::Pass);
                        inputs += 1;
                    }
                }
            }
        }
        close(&mut state);
        inputs += 1;
        assert_eq!(
            canonical_decode::<State>(&canonical_encode(&state).unwrap()).unwrap(),
            state
        );
    }
    assert_eq!(state.round(), 100);
    assert_eq!(inputs, 1400);
    assert_eq!(state.outcome().unwrap().summary(), "werewolf.stalemate");
    assert!(state.history().len() <= 1802);
    let encoded = canonical_encode(&state).unwrap();
    assert!(
        encoded.len() < 30 * 1024,
        "Small state class breached: {} bytes",
        encoded.len()
    );
}

#[test]
fn reconstructed_pending_choices_cannot_restore_potions_repeat_saves_or_change_marks() {
    let base = RawState::from(initial());
    for choice in [
        NightChoice::WitchHeal(Some(SeatId(6))),
        NightChoice::WitchPoison(Some(SeatId(6))),
    ] {
        let mut raw = base.clone();
        raw.night_choices.insert(SeatId(5), choice);
        assert!(State::try_from(raw.clone()).is_err());
        assert!(canonical_decode::<State>(&canonical_encode(&raw).unwrap()).is_err());
        let potions = raw.witch_potions.as_mut().unwrap();
        match choice {
            NightChoice::WitchHeal(_) => potions.heal = false,
            _ => potions.poison = false,
        }
        assert!(State::try_from(raw).is_ok());
    }
    let mut raw = base.clone();
    raw.last_doctor_target = Some(SeatId(6));
    raw.night_choices
        .insert(SeatId(3), NightChoice::Protect(Some(SeatId(6))));
    assert!(State::try_from(raw).is_err());
    let mut raw = base.clone();
    raw.night_choices
        .insert(SeatId(4), NightChoice::HunterMark(Some(SeatId(6))));
    assert!(State::try_from(raw.clone()).is_err());
    raw.hunter_mark = Some(SeatId(6));
    assert!(State::try_from(raw.clone()).is_ok());
    raw.hunter_mark = Some(SeatId(7));
    assert!(State::try_from(raw).is_err());
    let mut raw = base.clone();
    raw.hunter_mark = Some(SeatId(6));
    assert!(State::try_from(raw).is_err());
    let mut raw = base.clone();
    raw.history.push(Event::NightActionSubmitted {
        seat: SeatId(5),
        choice: NightChoice::WitchHeal(Some(SeatId(6))),
        round: 1,
    });
    assert!(State::try_from(raw).is_err());
    let mut raw = base;
    let event = Event::NightActionSubmitted {
        seat: SeatId(0),
        choice: NightChoice::Pass,
        round: 1,
    };
    raw.history.extend([event.clone(), event]);
    assert!(State::try_from(raw).is_err());
}

#[test]
fn disconnect_idle_and_reconnect_preserve_fixed_windows_in_every_phase() {
    for phase in [
        Phase::Night,
        Phase::Dawn,
        Phase::Day,
        Phase::Vote,
        Phase::Dusk,
    ] {
        let mut state = initial();
        while state.phase() != phase {
            close(&mut state);
        }
        if phase == Phase::Night {
            night(&mut state, 0, NightChoice::Pass);
        }
        if phase == Phase::Vote {
            vote(&mut state, 0, Ballot::Target(SeatId(6)));
        }
        let timer = state.current_timer();
        let deadline = state.phase_ends_at();
        let choices = state.night_choices().clone();
        let ballots = state.votes().clone();
        let alive = state.alive().clone();
        for change in [
            SeatChange::Disconnected,
            SeatChange::Reconnected,
            SeatChange::WentIdle,
            SeatChange::BecameActive,
        ] {
            apply(
                &mut state,
                Input::Seat {
                    seat: SeatId(0),
                    change,
                },
                deadline.0 - 1,
            );
            assert_eq!(state.current_timer(), timer);
            assert_eq!(state.phase_ends_at(), deadline);
            assert_eq!(state.night_choices(), &choices);
            assert_eq!(state.votes(), &ballots);
            assert_eq!(state.alive(), &alive);
        }
        apply(
            &mut state,
            Input::Seat {
                seat: SeatId(7),
                change: SeatChange::Disconnected,
            },
            deadline.0,
        );
        apply(
            &mut state,
            Input::Seat {
                seat: SeatId(7),
                change: SeatChange::Reconnected,
            },
            deadline.0,
        );
        if phase == Phase::Vote {
            reject(
                &mut state,
                Input::Player {
                    seat: SeatId(7),
                    command: Command::Vote(Ballot::Abstain),
                },
                deadline.0,
            );
        }
        close(&mut state);
        assert!(state.alive().contains(&SeatId(0)));
        assert!(state.alive().contains(&SeatId(7)));
    }
}
