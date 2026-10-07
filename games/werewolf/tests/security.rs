//! Hidden-information boundaries (I-5/I-6, W-D12/W-D13/W-D15).
//!
//! Whole-structure containment is deliberately coarse: postcard encodes scalar
//! roles and short actions in too few bytes for trustworthy substring scanning.
//! Typed visibility and paired accepted traces cover those gaps. These checks
//! concern game outputs; they do not establish network cursor or voice secrecy.

use tabula_core::{
    canonical_encode, DetRng, InputIndex, LogicalTime, MatchSeed, Millis, Occupant, SeatEntry,
    SeatId, SeatRoster, SpectatorTier, UserId, Viewer,
};
use tabula_game_api::{Budget, Ctx, GameRules, Input, LegalCommands, Outcome};
use tabula_game_werewolf::{
    Command, Config, Event, MaxRounds, NightChoice, Phase, PrivateKnowledge, RawState, Role,
    RoleKnowledge, State, WerewolfRules,
};
use tabula_testkit::{
    assert_projection_differs, assert_projection_noninterference, assert_view_event_differs,
    assert_view_event_noninterference,
};

const SEED: [u8; 32] = [19; 32];
const UNKNOWN: SeatId = SeatId(250);

fn roster(count: u8) -> SeatRoster {
    SeatRoster::new(
        (0..count)
            .map(|index| SeatEntry {
                seat: SeatId(index),
                occupant: Occupant::Human(UserId(u128::from(index) + 1)),
                team: None,
            })
            .collect(),
    )
    .expect("fixture seats are unique")
}

fn initial(count: u8) -> (State, Vec<Event>) {
    initial_with_config(count, Config::default())
}

fn initial_with_config(count: u8, config: Config) -> (State, Vec<Event>) {
    let seed = MatchSeed::from_bytes(SEED);
    let mut rng = DetRng::for_input(&seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget::default(),
    };
    let init = WerewolfRules::create(&config, &roster(count), &mut ctx)
        .expect("fixture creation succeeds");
    (init.state, init.events.into_iter().collect())
}

fn apply(
    state: &mut State,
    input: Input<Command>,
    index: u64,
    now: LogicalTime,
) -> Outcome<WerewolfRules> {
    let seed = MatchSeed::from_bytes(SEED);
    let index = InputIndex(index);
    let mut rng = DetRng::for_input(&seed, index);
    let mut ctx = Ctx {
        now,
        index,
        rng: &mut rng,
        budget: Budget::default(),
    };
    WerewolfRules::apply(state, input, &mut ctx).expect("security fixture input is legal")
}

fn submit(
    state: &mut State,
    seat: SeatId,
    choice: NightChoice,
    index: u64,
) -> Outcome<WerewolfRules> {
    apply(
        state,
        Input::Player {
            seat,
            command: Command::Night(choice),
        },
        index,
        LogicalTime(index * 100),
    )
}

fn expire(state: &mut State, index: u64) -> Outcome<WerewolfRules> {
    let timer = state.current_timer();
    let deadline = state.phase_ends_at();
    apply(state, Input::Timer { timer }, index, deadline)
}

fn outside() -> Vec<Viewer> {
    vec![
        Viewer::Spectator(SpectatorTier::Live),
        Viewer::Spectator(SpectatorTier::Delayed { by: Millis(30_000) }),
        Viewer::Seat(UNKNOWN),
    ]
}

fn clients(state: &State) -> Vec<Viewer> {
    let mut viewers: Vec<_> = state.roster().iter().copied().map(Viewer::Seat).collect();
    viewers.extend(outside());
    viewers
}

fn seat_with(state: &State, role: Role) -> SeatId {
    *state
        .roles()
        .iter()
        .find(|(_, assigned)| **assigned == role)
        .expect("preset includes role")
        .0
}

fn target_other_than(state: &State, seat: SeatId) -> SeatId {
    *state
        .roster()
        .iter()
        .find(|target| **target != seat)
        .unwrap()
}

fn real_choice(state: &State, actor: SeatId, role: Role) -> NightChoice {
    match role {
        Role::Villager => panic!("Villager has no private night choice"),
        Role::Werewolf => NightChoice::WolfTarget(Some(seat_with(state, Role::Villager))),
        Role::Seer => NightChoice::Investigate(target_other_than(state, actor)),
        Role::Doctor => NightChoice::Protect(Some(actor)),
        Role::Witch => NightChoice::WitchHeal(Some(actor)),
        Role::Hunter => NightChoice::HunterMark(Some(target_other_than(state, actor))),
    }
}

fn submission(events: &[Event]) -> &Event {
    events
        .iter()
        .find(|event| matches!(event, Event::NightActionSubmitted { .. }))
        .expect("accepted action emits submission")
}

fn visible_stream(state: &State, events: &[Event], viewer: Viewer) -> Vec<Vec<u8>> {
    events
        .iter()
        .filter_map(|event| WerewolfRules::view_event(state, event, viewer))
        .map(|event| canonical_encode(&event).unwrap())
        .collect()
}

fn assert_affordances_equal(before: &State, after: &State, viewer: Viewer) {
    let left = WerewolfRules::describe(before, viewer);
    let right = WerewolfRules::describe(after, viewer);
    assert_eq!(left.status, right.status);
    assert_eq!(
        format!("{:?}", left.regions),
        format!("{:?}", right.regions)
    );
    assert_eq!(
        format!("{:?}", left.actions),
        format!("{:?}", right.actions)
    );
    if let Viewer::Seat(seat) = viewer {
        assert_eq!(
            format!("{:?}", WerewolfRules::legal_commands(before, seat)),
            format!("{:?}", WerewolfRules::legal_commands(after, seat))
        );
        let reject = |state: &State| {
            let mut state = state.clone();
            let canonical_before = canonical_encode(&state).unwrap();
            let seed = MatchSeed::from_bytes(SEED);
            let index = InputIndex(999);
            let mut rng = DetRng::for_input(&seed, index);
            let mut ctx = Ctx {
                now: LogicalTime(state.phase_ends_at().0.saturating_sub(1)),
                index,
                rng: &mut rng,
                budget: Budget::default(),
            };
            let error = WerewolfRules::apply(
                &mut state,
                Input::Player {
                    seat,
                    command: Command::Night(NightChoice::Protect(Some(UNKNOWN))),
                },
                &mut ctx,
            )
            .expect_err("an unknown protection target is always illegal");
            assert_eq!(canonical_encode(&state).unwrap(), canonical_before);
            (error.code, error.detail)
        };
        assert_eq!(
            reject(before),
            reject(after),
            "a hostile observer probe cannot distinguish hidden facts"
        );
    }
}

#[test]
fn living_knowledge_matrix_covers_every_preset_role_at_six_twelve_twenty_seats() {
    let mut observed = std::collections::BTreeSet::new();
    for count in [6, 12, 20] {
        let (state, _) = initial(count);
        for (&seat, &role) in state.roles() {
            observed.insert(role);
            let view = WerewolfRules::project(&state, Viewer::Seat(seat));
            let PrivateKnowledge::Living {
                choice,
                wolf_team,
                seer_reports,
                doctor_previous,
                witch_potions,
                ..
            } = &view.knowledge
            else {
                panic!("every living known seat receives living knowledge");
            };
            assert!(choice.is_none());
            assert!(seer_reports.is_empty());
            assert!(doctor_previous.is_none());
            assert_eq!(
                *witch_potions,
                if role == Role::Witch {
                    state.witch_potions()
                } else {
                    None
                }
            );
            let expected_team: Vec<_> = if role == Role::Werewolf {
                state
                    .roles()
                    .iter()
                    .filter(|(_, role)| **role == Role::Werewolf)
                    .map(|(&seat, _)| seat)
                    .collect()
            } else {
                Vec::new()
            };
            assert_eq!(*wolf_team, expected_team);
            assert_eq!(view.roster.len(), usize::from(count));
            for entry in &view.roster {
                let assigned = state.roles()[&entry.seat];
                let expected = if entry.seat == seat
                    || (role == Role::Werewolf && assigned == Role::Werewolf)
                {
                    RoleKnowledge::Known(assigned)
                } else {
                    RoleKnowledge::Hidden
                };
                assert_eq!(
                    entry.role, expected,
                    "viewer {seat:?}/{role:?}, target {:?}",
                    entry.seat
                );
                assert!(entry.alive);
            }
        }
        for viewer in outside() {
            let view = WerewolfRules::project(&state, viewer);
            assert!(matches!(view.knowledge, PrivateKnowledge::None));
            assert!(view
                .roster
                .iter()
                .all(|entry| entry.role == RoleKnowledge::Hidden));
        }
    }
    assert_eq!(
        observed.len(),
        6,
        "all six base role branches must be exercised"
    );
}

/// A role permutation at creation preserves the pinned preset multiset and all
/// public facts. No reports or choices exist yet, so there is no history to
/// invalidate by changing an unauthorized role. Validate both reconstruction
/// barriers and keep a changed-secret/authorized-observability control.
#[test]
fn swapping_genuinely_unknown_roles_changes_no_unauthorized_projection() {
    let mut comparisons = 0;
    for count in [6, 12, 20] {
        let (state, _) = initial(count);
        for viewer in clients(&state) {
            let hidden: Vec<_> = state
                .roles()
                .iter()
                .filter(|(seat, role)| match viewer {
                    Viewer::Seat(own) if state.roles().contains_key(&own) => {
                        **seat != own
                            && !(state.roles()[&own] == Role::Werewolf && **role == Role::Werewolf)
                    }
                    _ => true,
                })
                .map(|(&seat, &role)| (seat, role))
                .collect();
            let (first, second) = hidden
                .iter()
                .find_map(|left| {
                    hidden
                        .iter()
                        .find(|right| right.1 != left.1)
                        .map(|right| (*left, *right))
                })
                .expect("each viewer has two distinct unknown role assignments");
            let mut raw = RawState::from(state.clone());
            raw.roles.insert(first.0, second.1);
            raw.roles.insert(second.0, first.1);
            // Initial assignment history must agree with the permuted opening.
            for event in &mut raw.history {
                if let Event::RolesAssigned { roles } = event {
                    *roles = raw.roles.clone();
                }
            }
            let scrambled = State::try_from(raw).expect("permutation preserves validated preset");
            assert_ne!(state.roles(), scrambled.roles());
            assert_projection_noninterference::<WerewolfRules>(
                "unseen role permutation",
                &state,
                &scrambled,
                viewer,
            );
            assert_affordances_equal(&state, &scrambled, viewer);
            assert_projection_differs::<WerewolfRules>(
                "Audit sees swapped roles",
                &state,
                &scrambled,
                Viewer::Audit,
            );
            assert_projection_differs::<WerewolfRules>(
                "owner sees changed own role",
                &state,
                &scrambled,
                Viewer::Seat(first.0),
            );
            comparisons += 1;
        }
    }
    assert_eq!(
        comparisons, 47,
        "all roster seats plus both outside tiers and invalid seat are tested"
    );
}

#[test]
fn a_private_submission_changes_neither_view_affordances_nor_visible_event_existence_for_others() {
    let mut actor_cases = 0;
    let mut hidden_events = 0;
    for count in [6, 12, 20] {
        let (before, _) = initial(count);
        for (&actor, &role) in before.roles() {
            if role == Role::Villager {
                continue;
            }
            let mut after = before.clone();
            let outcome = submit(&mut after, actor, real_choice(&before, actor, role), 1);
            assert_ne!(
                canonical_encode(&before).unwrap(),
                canonical_encode(&after).unwrap()
            );
            assert_eq!(before.phase(), after.phase());
            assert_eq!(before.phase_ends_at(), after.phase_ends_at());
            assert!(
                outcome.effects.is_empty(),
                "private actions must not publish timer/scope changes"
            );
            let event = submission(&outcome.events);
            for viewer in clients(&before)
                .into_iter()
                .filter(|viewer| *viewer != Viewer::Seat(actor))
            {
                assert_projection_noninterference::<WerewolfRules>(
                    "night action existence",
                    &before,
                    &after,
                    viewer,
                );
                assert_affordances_equal(&before, &after, viewer);
                assert!(
                    WerewolfRules::view_event(&after, event, viewer).is_none(),
                    "private event must not exist for {viewer:?}"
                );
                assert_eq!(
                    visible_stream(&before, &[], viewer),
                    visible_stream(&after, &outcome.events, viewer)
                );
                hidden_events += 1;
            }
            for authorized in [Viewer::Seat(actor), Viewer::Audit] {
                assert_projection_differs::<WerewolfRules>(
                    "authorized action observable",
                    &before,
                    &after,
                    authorized,
                );
                assert!(WerewolfRules::view_event(&after, event, authorized).is_some());
            }
            actor_cases += 1;
        }
    }
    assert_eq!(
        actor_cases, 19,
        "3 + 7 + 9 active-role actors are exercised"
    );
    assert!(
        hidden_events > 200,
        "private None assertions must be non-vacuous"
    );
}

#[test]
fn different_private_choices_remain_indistinguishable_to_unauthorized_viewers() {
    for count in [6, 12, 20] {
        let (before, _) = initial(count);
        for (&actor, &role) in before.roles() {
            if role == Role::Villager {
                continue;
            }
            let mut passed = before.clone();
            let pass = submit(&mut passed, actor, NightChoice::Pass, 1);
            let mut acted = before.clone();
            let action = submit(&mut acted, actor, real_choice(&before, actor, role), 1);
            assert_ne!(passed.night_choices(), acted.night_choices());
            for viewer in clients(&before)
                .into_iter()
                .filter(|viewer| *viewer != Viewer::Seat(actor))
            {
                assert_projection_noninterference::<WerewolfRules>(
                    "pass versus private target",
                    &passed,
                    &acted,
                    viewer,
                );
                assert_view_event_noninterference::<WerewolfRules>(
                    "pass versus target event",
                    &passed,
                    submission(&pass.events),
                    &acted,
                    submission(&action.events),
                    viewer,
                );
                assert_affordances_equal(&passed, &acted, viewer);
            }
            for viewer in [Viewer::Seat(actor), Viewer::Audit] {
                assert_projection_differs::<WerewolfRules>(
                    "owner sees own chosen target",
                    &passed,
                    &acted,
                    viewer,
                );
                assert_view_event_differs::<WerewolfRules>(
                    "owner receives chosen action",
                    &passed,
                    submission(&pass.events),
                    &acted,
                    submission(&action.events),
                    viewer,
                );
            }
        }
    }
}

#[test]
fn wolf_teammates_cannot_observe_submission_order_or_each_others_choices() {
    let (before, _) = initial(12);
    let wolves: Vec<_> = before
        .roles()
        .iter()
        .filter(|(_, role)| **role == Role::Werewolf)
        .map(|(&seat, _)| seat)
        .collect();
    let target = seat_with(&before, Role::Villager);
    let mut left = before.clone();
    let mut left_events = Vec::new();
    for (index, &actor) in wolves[..2].iter().enumerate() {
        left_events.extend(
            submit(
                &mut left,
                actor,
                NightChoice::WolfTarget(Some(target)),
                index as u64 + 1,
            )
            .events,
        );
    }
    let mut right = before.clone();
    let mut right_events = Vec::new();
    for (index, &actor) in wolves[..2].iter().rev().enumerate() {
        right_events.extend(
            submit(
                &mut right,
                actor,
                NightChoice::WolfTarget(Some(target)),
                index as u64 + 1,
            )
            .events,
        );
    }
    assert_eq!(left.night_choices(), right.night_choices());
    assert_ne!(
        RawState::from(left.clone()).history,
        RawState::from(right.clone()).history
    );
    for viewer in clients(&before) {
        assert_projection_noninterference::<WerewolfRules>(
            "private submission ordering",
            &left,
            &right,
            viewer,
        );
        assert_eq!(
            visible_stream(&left, &left_events, viewer),
            visible_stream(&right, &right_events, viewer)
        );
    }
    assert_projection_differs::<WerewolfRules>(
        "Audit retains canonical order",
        &left,
        &right,
        Viewer::Audit,
    );
}

#[test]
fn an_effective_save_and_an_unused_save_have_identical_owner_acknowledgements() {
    for role in [Role::Doctor, Role::Witch] {
        let (before, _) = initial(12);
        let owner = seat_with(&before, role);
        let wolf = seat_with(&before, Role::Werewolf);
        let target = seat_with(&before, Role::Villager);
        let saving = match role {
            Role::Doctor => NightChoice::Protect(Some(target)),
            Role::Witch => NightChoice::WitchHeal(Some(target)),
            _ => unreachable!(),
        };
        let mut attacked = before.clone();
        let ack_a = submit(&mut attacked, owner, saving, 1);
        submit(
            &mut attacked,
            wolf,
            NightChoice::WolfTarget(Some(target)),
            2,
        );
        let resolution_a = expire(&mut attacked, 3);
        let mut passed = before.clone();
        let ack_b = submit(&mut passed, owner, saving, 1);
        submit(&mut passed, wolf, NightChoice::Pass, 2);
        let resolution_b = expire(&mut passed, 3);
        assert_eq!(attacked.alive(), passed.alive());
        assert_eq!(attacked.phase(), passed.phase());
        assert_eq!(attacked.revealed(), passed.revealed());
        assert_ne!(
            RawState::from(attacked.clone()).history,
            RawState::from(passed.clone()).history
        );
        let viewer = Viewer::Seat(owner);
        assert_eq!(
            visible_stream(&attacked, &ack_a.events, viewer),
            visible_stream(&passed, &ack_b.events, viewer)
        );
        assert_eq!(
            visible_stream(&attacked, &resolution_a.events, viewer),
            visible_stream(&passed, &resolution_b.events, viewer)
        );
        assert_projection_noninterference::<WerewolfRules>(
            "save success attribution is hidden",
            &attacked,
            &passed,
            viewer,
        );
        assert_affordances_equal(&attacked, &passed, viewer);
        for outside in outside() {
            assert_projection_noninterference::<WerewolfRules>(
                "outside cannot see attack or save cause",
                &attacked,
                &passed,
                outside,
            );
            assert_eq!(
                visible_stream(&attacked, &resolution_a.events, outside),
                visible_stream(&passed, &resolution_b.events, outside)
            );
        }
        assert_projection_differs::<WerewolfRules>(
            "Audit sees different save resolution",
            &attacked,
            &passed,
            Viewer::Audit,
        );
    }
}

#[test]
fn death_grants_full_seat_knowledge_while_outside_receives_only_public_role_reveal() {
    for count in [6, 12, 20] {
        let (before, _) = initial(count);
        let wolf = seat_with(&before, Role::Werewolf);
        let victim = seat_with(&before, Role::Villager);
        let seer = seat_with(&before, Role::Seer);
        let mut after = before.clone();
        submit(&mut after, wolf, NightChoice::WolfTarget(Some(victim)), 1);
        submit(&mut after, seer, NightChoice::Investigate(wolf), 2);
        let resolution = expire(&mut after, 3);
        assert!(!after.alive().contains(&victim));
        assert_eq!(after.phase(), Phase::Dawn);
        let dead = WerewolfRules::project(&after, Viewer::Seat(victim));
        assert!(matches!(dead.knowledge, PrivateKnowledge::Full { .. }));
        for entry in &dead.roster {
            assert_eq!(entry.role, RoleKnowledge::Known(after.roles()[&entry.seat]));
        }
        assert!(matches!(
            WerewolfRules::legal_commands(&after, victim),
            LegalCommands::None
        ));
        let raw = RawState::from(after.clone());
        for event in raw.history.iter().filter(|event| {
            matches!(
                event,
                Event::NightActionSubmitted { .. }
                    | Event::SeerReport { .. }
                    | Event::NightResolved { .. }
            )
        }) {
            assert!(
                WerewolfRules::view_event(&after, event, Viewer::Seat(victim)).is_some(),
                "dead seat receives full retained private event"
            );
            assert!(WerewolfRules::view_event(&after, event, Viewer::Audit).is_some());
            for viewer in outside() {
                assert!(WerewolfRules::view_event(&after, event, viewer).is_none());
            }
        }
        let report = resolution
            .events
            .iter()
            .find(|event| matches!(event, Event::SeerReport { .. }))
            .expect("Seer report delivered at Dawn");
        assert!(WerewolfRules::view_event(&after, report, Viewer::Seat(seer)).is_some());
        for viewer in clients(&after)
            .into_iter()
            .filter(|viewer| *viewer != Viewer::Seat(victim) && *viewer != Viewer::Seat(seer))
        {
            assert!(WerewolfRules::view_event(&after, report, viewer).is_none());
        }
        for viewer in outside() {
            let view = WerewolfRules::project(&after, viewer);
            assert!(matches!(view.knowledge, PrivateKnowledge::None));
            for entry in &view.roster {
                assert_eq!(
                    entry.role,
                    if entry.seat == victim {
                        RoleKnowledge::Known(Role::Villager)
                    } else {
                        RoleKnowledge::Hidden
                    }
                );
            }
            assert_projection_differs::<WerewolfRules>("death is public", &before, &after, viewer);
            assert!(resolution.events.iter().any(
                |event| matches!(event, Event::DeathRevealed { seat, .. } if *seat == victim)
                    && WerewolfRules::view_event(&after, event, viewer).is_some()
            ));
        }
    }
}

#[test]
fn ended_match_declassifies_roles_without_releasing_private_history_to_outside() {
    for count in [6, 12, 20] {
        let config = Config {
            max_rounds: MaxRounds::new(1).unwrap(),
            ..Config::default()
        };
        let (mut state, _) = initial_with_config(count, config);
        let seer = seat_with(&state, Role::Seer);
        let wolf = seat_with(&state, Role::Werewolf);
        submit(&mut state, seer, NightChoice::Investigate(wolf), 1);
        let mut ended_events = Vec::new();
        for index in 2..=6 {
            ended_events = expire(&mut state, index).events.into_iter().collect();
        }
        assert_eq!(state.phase(), Phase::Ended);
        let private: Vec<_> = RawState::from(state.clone())
            .history
            .into_iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::NightActionSubmitted { .. }
                        | Event::SeerReport { .. }
                        | Event::NightResolved { .. }
                )
            })
            .collect();
        assert!(
            private.len() >= 3,
            "the ended fixture retains real private history"
        );
        for viewer in outside() {
            let view = WerewolfRules::project(&state, viewer);
            assert!(matches!(view.knowledge, PrivateKnowledge::None));
            for entry in &view.roster {
                assert_eq!(entry.role, RoleKnowledge::Known(state.roles()[&entry.seat]));
            }
            for event in &private {
                assert!(WerewolfRules::view_event(&state, event, viewer).is_none());
            }
            assert!(ended_events
                .iter()
                .any(|event| matches!(event, Event::MatchEnded { .. })
                    && WerewolfRules::view_event(&state, event, viewer).is_some()));
        }
    }
}

#[test]
fn initial_assignment_event_is_audit_only_even_when_roles_are_later_public() {
    let (mut state, initial_events) = initial_with_config(
        12,
        Config {
            max_rounds: MaxRounds::new(1).unwrap(),
            ..Config::default()
        },
    );
    let assignment = initial_events
        .iter()
        .find(|event| matches!(event, Event::RolesAssigned { .. }))
        .unwrap();
    for index in 1..=5 {
        for viewer in clients(&state) {
            assert!(WerewolfRules::view_event(&state, assignment, viewer).is_none());
        }
        assert!(WerewolfRules::view_event(&state, assignment, Viewer::Audit).is_some());
        expire(&mut state, index);
    }
    assert_eq!(state.phase(), Phase::Ended);
    for viewer in clients(&state) {
        assert!(WerewolfRules::view_event(&state, assignment, viewer).is_none());
    }
}

#[test]
fn unknown_seat_never_acquires_dead_or_audit_privileges() {
    let (mut state, _) = initial(12);
    let wolf = seat_with(&state, Role::Werewolf);
    let victim = seat_with(&state, Role::Villager);
    submit(&mut state, wolf, NightChoice::WolfTarget(Some(victim)), 1);
    expire(&mut state, 2);
    assert_eq!(
        canonical_encode(&WerewolfRules::project(&state, Viewer::Seat(UNKNOWN))).unwrap(),
        canonical_encode(&WerewolfRules::project(
            &state,
            Viewer::Spectator(SpectatorTier::Live)
        ))
        .unwrap()
    );
    assert!(matches!(
        WerewolfRules::legal_commands(&state, UNKNOWN),
        LegalCommands::None
    ));
    for event in RawState::from(state.clone()).history {
        assert_eq!(
            canonical_encode(&WerewolfRules::view_event(
                &state,
                &event,
                Viewer::Seat(UNKNOWN)
            ))
            .unwrap(),
            canonical_encode(&WerewolfRules::view_event(
                &state,
                &event,
                Viewer::Spectator(SpectatorTier::Live)
            ))
            .unwrap()
        );
    }
}

#[cfg(feature = "testkit")]
mod containment {
    use super::*;
    use tabula_game_api::GameModule;
    use tabula_game_werewolf::WerewolfModule;
    use tabula_testkit::{GameTestFixture, HiddenInformationFixture, SecretModel};

    struct WerewolfSecurityFixture<const SEATS: u8>;

    impl<const SEATS: u8> GameTestFixture for WerewolfSecurityFixture<SEATS> {
        type Module = WerewolfModule;
        fn config() -> Config {
            Config::default()
        }
        fn roster() -> SeatRoster {
            super::roster(SEATS)
        }
        fn seed() -> MatchSeed {
            MatchSeed::from_bytes(SEED)
        }
        fn deterministic_script() -> Vec<Input<Command>> {
            let (state, _) = initial(SEATS);
            // The one-second testkit schedule remains inside the default Night
            // window, with a public ballot-free event beside real secrets.
            vec![Input::Player {
                seat: seat_with(&state, Role::Seer),
                command: Command::Night(NightChoice::Investigate(seat_with(
                    &state,
                    Role::Werewolf,
                ))),
            }]
        }
    }

    impl<const SEATS: u8> HiddenInformationFixture for WerewolfSecurityFixture<SEATS> {
        fn game_controlled_spectators() -> Option<Vec<SpectatorTier>> {
            Some(vec![
                SpectatorTier::Live,
                SpectatorTier::Delayed { by: Millis(30_000) },
            ])
        }
    }

    mod six {
        use super::*;
        tabula_testkit::projection_security!(WerewolfSecurityFixture<6>);
    }
    mod twelve {
        use super::*;
        tabula_testkit::projection_security!(WerewolfSecurityFixture<12>);
    }
    mod twenty {
        use super::*;
        tabula_testkit::projection_security!(WerewolfSecurityFixture<20>);
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        !needle.is_empty()
            && haystack
                .windows(needle.len())
                .any(|window| window == needle)
    }

    #[test]
    fn whole_assignment_tokens_detect_injected_fields_without_clean_output_collisions() {
        for count in [6, 12, 20] {
            let (state, events) = initial(count);
            assert!(WerewolfModule::capabilities().hidden_information());
            let secrets = WerewolfRules::secrets(&state);
            assert!(!secrets.is_empty(), "the real assignment must be declared");
            let encoded_roles = canonical_encode(state.roles()).unwrap();
            let role_token = &encoded_roles[2..];
            let matching = secrets.iter().find(|secret| secret.tokens.iter().any(|token| token == role_token)).expect("a token must be the actual nested complete role map, without canonical version prefix");
            for viewer in clients(&state) {
                assert!(!matching.authorized.contains(&viewer));
                let view = WerewolfRules::project(&state, viewer);
                let honest = canonical_encode(&view).unwrap();
                let leaky = canonical_encode(&(view, state.roles())).unwrap();
                assert!(!contains(&honest, role_token));
                assert!(
                    contains(&leaky, role_token),
                    "whole-map injection must be detected"
                );
            }
            let assignment = events
                .iter()
                .find(|event| matches!(event, Event::RolesAssigned { .. }))
                .unwrap();
            let event_secrets = WerewolfRules::event_secrets(&state, assignment);
            assert!(
                event_secrets
                    .iter()
                    .any(|secret| secret.tokens.iter().any(|token| token == role_token)),
                "RolesAssigned must declare its event-local whole-assignment secret"
            );
        }
    }
}

#[test]
fn hunter_retaliation_details_are_visible_only_to_full_vision_viewers() {
    let (mut state, _) = initial(12);
    let hunter = seat_with(&state, Role::Hunter);
    let wolf = seat_with(&state, Role::Werewolf);
    submit(&mut state, hunter, NightChoice::HunterMark(Some(wolf)), 1);
    submit(&mut state, wolf, NightChoice::WolfTarget(Some(hunter)), 2);
    let resolution = expire(&mut state, 3);
    assert!(!state.alive().contains(&hunter));
    assert!(!state.alive().contains(&wolf));
    let trigger = resolution
        .events
        .iter()
        .find(|event| matches!(event, Event::HunterTriggered { .. }))
        .expect("Hunter death triggers its precommitted mark");
    for viewer in clients(&state) {
        let full = viewer.seat().is_some_and(|seat| {
            state.roles().contains_key(&seat) && !state.alive().contains(&seat)
        });
        assert_eq!(
            WerewolfRules::view_event(&state, trigger, viewer).is_some(),
            full,
            "Hunter retaliation visibility for {viewer:?}"
        );
    }
    assert!(WerewolfRules::view_event(&state, trigger, Viewer::Audit).is_some());
}

#[test]
fn all_public_event_classes_remain_visible_to_every_client() {
    let (mut state, initial_events) = initial_with_config(
        12,
        Config {
            max_rounds: MaxRounds::new(1).unwrap(),
            ..Config::default()
        },
    );
    let mut cases: Vec<_> = initial_events
        .into_iter()
        .filter(|event| matches!(event, Event::PhaseChanged { .. }))
        .collect();
    let lifecycle = apply(
        &mut state,
        Input::Seat {
            seat: SeatId(0),
            change: tabula_core::SeatChange::Disconnected,
        },
        1,
        LogicalTime(100),
    );
    cases.extend(lifecycle.events);
    apply(
        &mut state,
        Input::Seat {
            seat: SeatId(0),
            change: tabula_core::SeatChange::Reconnected,
        },
        2,
        LogicalTime(200),
    );
    for index in 3..=5 {
        cases.extend(
            expire(&mut state, index)
                .events
                .into_iter()
                .filter(|event| matches!(event, Event::PhaseChanged { .. })),
        );
    }
    assert_eq!(state.phase(), Phase::Vote);
    let victim = seat_with(&state, Role::Villager);
    let voter = target_other_than(&state, victim);
    let now = LogicalTime(state.phase_ends_at().0 - 1);
    cases.extend(
        apply(
            &mut state,
            Input::Player {
                seat: voter,
                command: Command::Vote(tabula_game_werewolf::Ballot::Target(victim)),
            },
            6,
            now,
        )
        .events,
    );
    cases.extend(expire(&mut state, 7).events.into_iter().filter(|event| {
        matches!(
            event,
            Event::VoteResolved { .. } | Event::DeathRevealed { .. } | Event::PhaseChanged { .. }
        )
    }));
    cases.extend(
        expire(&mut state, 8)
            .events
            .into_iter()
            .filter(|event| matches!(event, Event::MatchEnded { .. })),
    );
    assert_eq!(state.phase(), Phase::Ended);
    assert!(cases
        .iter()
        .any(|event| matches!(event, Event::BallotChanged { .. })));
    assert!(cases
        .iter()
        .any(|event| matches!(event, Event::VoteResolved { .. })));
    assert!(cases
        .iter()
        .any(|event| matches!(event, Event::DeathRevealed { .. })));
    assert!(cases
        .iter()
        .any(|event| matches!(event, Event::SeatStatusChanged { .. })));
    assert!(cases
        .iter()
        .any(|event| matches!(event, Event::MatchEnded { .. })));
    for event in &cases {
        for viewer in clients(&state) {
            assert!(
                WerewolfRules::view_event(&state, event, viewer).is_some(),
                "public event {event:?} remains visible to {viewer:?}"
            );
        }
    }
}

#[test]
fn enumerated_affordances_apply_at_the_same_public_checkpoint_without_oracle_leaks() {
    for count in [6, 12, 20] {
        let (mut state, _) = initial(count);
        for phase in [Phase::Night, Phase::Vote] {
            if phase == Phase::Vote {
                for index in 1..=3 {
                    expire(&mut state, index);
                }
            }
            assert_eq!(state.phase(), phase);
            let now = LogicalTime(state.phase_ends_at().0 - 1);
            for &seat in state.roster() {
                let commands = match WerewolfRules::legal_commands(&state, seat) {
                    LegalCommands::Enumerated(commands) => commands,
                    LegalCommands::None
                        if phase == Phase::Night && state.roles()[&seat] == Role::Villager =>
                    {
                        continue
                    }
                    other => panic!(
                        "expected meaningful enumeration for {seat:?}/{phase:?}, got {other:?}"
                    ),
                };
                assert!(!commands.is_empty());
                let LegalCommands::Enumerated(again) = WerewolfRules::legal_commands(&state, seat)
                else {
                    panic!("affordance ordering is stable");
                };
                assert_eq!(
                    canonical_encode(&commands).unwrap(),
                    canonical_encode(&again).unwrap()
                );
                let unique: std::collections::BTreeSet<_> = commands
                    .iter()
                    .map(|command| canonical_encode(command).unwrap())
                    .collect();
                assert_eq!(unique.len(), commands.len());
                for command in commands {
                    apply(
                        &mut state.clone(),
                        Input::Player { seat, command },
                        100,
                        now,
                    );
                }
            }
        }
    }
}

#[test]
fn hunter_owner_keeps_its_precommitted_mark_through_dawn_and_vote() {
    let (mut state, _) = initial(12);
    let hunter = seat_with(&state, Role::Hunter);
    let target = seat_with(&state, Role::Werewolf);
    submit(&mut state, hunter, NightChoice::HunterMark(Some(target)), 1);
    for phase in [Phase::Night, Phase::Dawn, Phase::Day, Phase::Vote] {
        assert_eq!(state.phase(), phase);
        let view = WerewolfRules::project(&state, Viewer::Seat(hunter));
        let PrivateKnowledge::Living { hunter_mark, .. } = view.knowledge else {
            panic!("Hunter remains living");
        };
        assert_eq!(
            hunter_mark,
            Some(target),
            "owner retains active mark throughout the round"
        );
        for &seat in state.roster().iter().filter(|&&seat| seat != hunter) {
            let view = WerewolfRules::project(&state, Viewer::Seat(seat));
            let PrivateKnowledge::Living { hunter_mark, .. } = view.knowledge else {
                panic!("all fixture seats remain living");
            };
            assert_eq!(
                hunter_mark, None,
                "another living seat cannot see Hunter's mark"
            );
        }
        if phase != Phase::Vote {
            expire(&mut state, 2);
        }
    }
}

#[test]
fn changing_only_retained_private_history_changes_no_ended_outside_observation() {
    for count in [6, 12, 20] {
        let config = Config {
            max_rounds: MaxRounds::new(1).unwrap(),
            ..Config::default()
        };
        let (before, _) = initial_with_config(count, config);
        let seer = seat_with(&before, Role::Seer);
        let wolf = seat_with(&before, Role::Werewolf);
        let mut investigated = before.clone();
        submit(&mut investigated, seer, NightChoice::Investigate(wolf), 1);
        let mut passed = before.clone();
        submit(&mut passed, seer, NightChoice::Pass, 1);
        let mut observed_a = Vec::new();
        let mut observed_b = Vec::new();
        for index in 2..=6 {
            observed_a.extend(expire(&mut investigated, index).events);
            observed_b.extend(expire(&mut passed, index).events);
        }
        assert_eq!(investigated.phase(), Phase::Ended);
        assert_eq!(passed.phase(), Phase::Ended);
        assert_eq!(investigated.roles(), passed.roles());
        assert_eq!(investigated.alive(), passed.alive());
        assert_ne!(investigated.history(), passed.history());
        for viewer in outside() {
            assert_projection_noninterference::<WerewolfRules>(
                "ended private-log differences",
                &investigated,
                &passed,
                viewer,
            );
            assert_eq!(
                visible_stream(&investigated, &observed_a, viewer),
                visible_stream(&passed, &observed_b, viewer)
            );
            assert_affordances_equal(&investigated, &passed, viewer);
        }
        for viewer in [Viewer::Seat(seer), Viewer::Audit] {
            assert_projection_differs::<WerewolfRules>(
                "authorized retained report visible after end",
                &investigated,
                &passed,
                viewer,
            );
        }
    }
}

#[test]
fn every_private_night_choice_variant_has_explicit_event_nonexistence_coverage() {
    let mut variants = std::collections::BTreeSet::new();
    let mut cases = 0;
    for count in [6, 12, 20] {
        let (before, _) = initial(count);
        for (&actor, &role) in before.roles() {
            if role == Role::Villager {
                continue;
            }
            let other = target_other_than(&before, actor);
            let mut choices = vec![NightChoice::Pass];
            match role {
                Role::Villager => unreachable!(),
                Role::Werewolf => choices.extend([
                    NightChoice::WolfTarget(None),
                    NightChoice::WolfTarget(Some(seat_with(&before, Role::Villager))),
                ]),
                Role::Seer => choices.push(NightChoice::Investigate(other)),
                Role::Doctor => choices.extend([
                    NightChoice::Protect(None),
                    NightChoice::Protect(Some(actor)),
                ]),
                Role::Witch => choices.extend([
                    NightChoice::WitchHeal(None),
                    NightChoice::WitchHeal(Some(actor)),
                    NightChoice::WitchPoison(None),
                    NightChoice::WitchPoison(Some(other)),
                ]),
                Role::Hunter => choices.extend([
                    NightChoice::HunterMark(None),
                    NightChoice::HunterMark(Some(other)),
                ]),
            }
            for choice in choices {
                variants.insert(match choice {
                    NightChoice::WolfTarget(_) => 0,
                    NightChoice::Investigate(_) => 1,
                    NightChoice::Protect(_) => 2,
                    NightChoice::WitchHeal(_) => 3,
                    NightChoice::WitchPoison(_) => 4,
                    NightChoice::HunterMark(_) => 5,
                    NightChoice::Pass => 6,
                });
                let mut after = before.clone();
                let outcome = submit(&mut after, actor, choice, 1);
                let event = submission(&outcome.events);
                for viewer in clients(&before) {
                    if viewer == Viewer::Seat(actor) {
                        assert!(WerewolfRules::view_event(&after, event, viewer).is_some());
                    } else {
                        assert!(WerewolfRules::view_event(&after, event, viewer).is_none());
                        assert_projection_noninterference::<WerewolfRules>(
                            "all choice variants preserve private event existence",
                            &before,
                            &after,
                            viewer,
                        );
                        assert_eq!(
                            visible_stream(&before, &[], viewer),
                            visible_stream(&after, &outcome.events, viewer)
                        );
                    }
                }
                assert!(WerewolfRules::view_event(&after, event, Viewer::Audit).is_some());
                cases += 1;
            }
        }
    }
    assert_eq!(
        variants.len(),
        7,
        "all seven NightChoice variants are exercised"
    );
    assert_eq!(
        cases, 58,
        "both targeted and optional pass shapes are exercised"
    );
}
