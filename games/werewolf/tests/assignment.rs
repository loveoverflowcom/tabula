#![allow(clippy::doc_markdown)] // `SeatId` is a machine-readable domain type.

//! Deterministic role assignment and initial match creation tests for Werewolf W2.

use std::collections::BTreeSet;

use smallvec::smallvec;
use tabula_core::{
    canonical_encode, LogicalTime, MatchSeed, Occupant, SeatEntry, SeatId, SeatRoster, UserId,
};
use tabula_game_werewolf::{
    create_initial_state_from_seed, Config, Event, Phase, PlayerStatus, Role, RoleCounts,
    SeatCount, MAX_SEATS, MIN_SEATS,
};

/// Helper to build a valid `SeatRoster` with arbitrary `SeatId`s.
fn make_roster(seat_ids: &[u8]) -> SeatRoster {
    let mut seats = smallvec![];
    for &id in seat_ids {
        seats.push(SeatEntry {
            seat: SeatId(id),
            occupant: Occupant::Human(UserId(1_000 + u128::from(id))),
            team: None,
        });
    }
    SeatRoster::new(seats).expect("unique seats in test helper")
}

/// Helper to build a standard dense `SeatRoster` for `n` seats: `0..n`.
fn make_dense_roster(n: u8) -> SeatRoster {
    let ids: Vec<u8> = (0..n).collect();
    make_roster(&ids)
}

// ---------------------------------------------------------------------------
// 1. Role Assignment Exhaustiveness (6..=20 seats)
// ---------------------------------------------------------------------------

#[test]
fn role_assignment_exhaustive_across_all_supported_seat_counts() {
    let config = Config::default();
    let seed = MatchSeed::from_bytes([42u8; 32]);

    for seats in MIN_SEATS..=MAX_SEATS {
        let roster = make_dense_roster(seats);
        let (state, events) =
            create_initial_state_from_seed(&config, &roster, LogicalTime::ZERO, &seed)
                .expect("initial state creation succeeds");

        // Roster matches exactly
        assert_eq!(state.roster().len(), usize::from(seats));
        assert_eq!(state.roles().len(), usize::from(seats));

        // Every seat has exactly one role assignment
        for entry in &roster {
            assert!(
                state.roles().contains_key(&entry.seat),
                "seat {:?} missing role in {seats}-player match",
                entry.seat
            );
        }

        // Role counts match the pinned ClassicV1 preset
        let expected_counts = config
            .preset
            .role_counts(SeatCount::new(seats).expect("valid seat count"));
        let mut actual_counts = RoleCounts {
            werewolves: 0,
            seers: 0,
            doctors: 0,
            hunters: 0,
            witches: 0,
            villagers: 0,
        };
        for role in state.roles().values() {
            match role {
                Role::Werewolf => actual_counts.werewolves += 1,
                Role::Seer => actual_counts.seers += 1,
                Role::Doctor => actual_counts.doctors += 1,
                Role::Hunter => actual_counts.hunters += 1,
                Role::Witch => actual_counts.witches += 1,
                Role::Villager => actual_counts.villagers += 1,
            }
        }
        assert_eq!(
            actual_counts, expected_counts,
            "role counts mismatch for {seats}-seat match"
        );

        // Events emitted match creation facts
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            Event::RolesAssigned {
                roles: state.roles().clone()
            }
        );
        match events[1] {
            Event::PhaseChanged {
                phase,
                round,
                timer_id,
                ends_at,
            } => {
                assert_eq!(phase, Phase::Night);
                assert_eq!(round, 1);
                assert_eq!(timer_id.0, 1);
                assert_eq!(ends_at, state.phase_ends_at());
            }
            Event::RolesAssigned { .. } => panic!("expected PhaseChanged event as second event"),
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Non-Contiguous SeatIds
// ---------------------------------------------------------------------------

#[test]
fn role_assignment_works_with_non_contiguous_and_irregular_seat_ids() {
    let irregular_ids = [200, 12, 42, 5, 177, 100, 3, 250];
    let roster = make_roster(&irregular_ids);
    let config = Config::default();
    let seed = MatchSeed::from_bytes([7u8; 32]);

    let (state, _) = create_initial_state_from_seed(&config, &roster, LogicalTime::ZERO, &seed)
        .expect("creation succeeds with non-contiguous seats");

    let expected_sorted_seats: Vec<SeatId> = {
        let mut s: Vec<SeatId> = irregular_ids.iter().map(|&id| SeatId(id)).collect();
        s.sort_unstable();
        s
    };

    assert_eq!(
        state.roster(),
        expected_sorted_seats,
        "state roster must be strictly canonically sorted"
    );

    for &seat in &expected_sorted_seats {
        assert!(
            state.roles().contains_key(&seat),
            "seat {seat:?} must have an assigned role"
        );
        assert!(
            state.alive().contains(&seat),
            "seat {seat:?} must be in alive set"
        );
        assert_eq!(
            state.player_status().get(&seat),
            Some(&PlayerStatus::Active),
            "seat {seat:?} must be Active"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Roster-Order Invariance
// ---------------------------------------------------------------------------

#[test]
fn roster_order_invariance_produces_identical_assignment() {
    let order_a = [10, 50, 5, 20, 100, 30];
    let order_b = [30, 10, 100, 5, 50, 20];
    let order_c = [5, 10, 20, 30, 50, 100];

    let roster_a = make_roster(&order_a);
    let roster_b = make_roster(&order_b);
    let roster_c = make_roster(&order_c);

    let config = Config::default();
    let seed = MatchSeed::from_bytes([123u8; 32]);
    let now = LogicalTime(5_000);

    let (state_a, _) = create_initial_state_from_seed(&config, &roster_a, now, &seed).unwrap();
    let (state_b, _) = create_initial_state_from_seed(&config, &roster_b, now, &seed).unwrap();
    let (state_c, _) = create_initial_state_from_seed(&config, &roster_c, now, &seed).unwrap();

    assert_eq!(
        state_a.roles(),
        state_b.roles(),
        "different roster order must produce identical role assignment"
    );
    assert_eq!(
        state_a.roles(),
        state_c.roles(),
        "different roster order must produce identical role assignment"
    );

    let bytes_a = canonical_encode(&state_a).unwrap();
    let bytes_b = canonical_encode(&state_b).unwrap();
    let bytes_c = canonical_encode(&state_c).unwrap();

    assert_eq!(bytes_a, bytes_b, "states must be byte-identical");
    assert_eq!(bytes_a, bytes_c, "states must be byte-identical");
}

// ---------------------------------------------------------------------------
// 4. Seed Determinism
// ---------------------------------------------------------------------------

#[test]
fn seed_determinism_produces_byte_identical_state() {
    let roster = make_dense_roster(12);
    let config = Config::default();
    let seed = MatchSeed::from_bytes([99u8; 32]);
    let now = LogicalTime(1_000);

    let (state_1, events_1) = create_initial_state_from_seed(&config, &roster, now, &seed).unwrap();
    let (state_2, events_2) = create_initial_state_from_seed(&config, &roster, now, &seed).unwrap();

    assert_eq!(state_1, state_2);
    assert_eq!(events_1, events_2);

    let bytes_1 = canonical_encode(&state_1).unwrap();
    let bytes_2 = canonical_encode(&state_2).unwrap();
    assert_eq!(
        bytes_1, bytes_2,
        "identical seed must produce byte-identical canonical state"
    );
}

// ---------------------------------------------------------------------------
// 5. Seed Sensitivity Positive Control
// ---------------------------------------------------------------------------

#[test]
fn seed_sensitivity_positive_control_produces_different_assignments() {
    let roster = make_dense_roster(12);
    let config = Config::default();

    let mut distinct_assignments = BTreeSet::new();
    // Test across a few distinct seeds to show the shuffle actually varies the assignment.
    for i in 0..10u8 {
        let mut seed_bytes = [0u8; 32];
        seed_bytes[0] = i;
        seed_bytes[31] = i.wrapping_mul(7);
        let seed = MatchSeed::from_bytes(seed_bytes);

        let (state, _) =
            create_initial_state_from_seed(&config, &roster, LogicalTime::ZERO, &seed).unwrap();
        distinct_assignments.insert(state.roles().clone());
    }

    assert!(
        distinct_assignments.len() > 1,
        "distinct match seeds must produce different role assignments across trials (positive control)"
    );
}

// ---------------------------------------------------------------------------
// 6. Initial State Structure on Creation
// ---------------------------------------------------------------------------

#[test]
fn initial_state_has_valid_night_1_structure() {
    let roster = make_dense_roster(10); // 10 seats: 2W, 1S, 1D, 1H, 1T, 4V
    let config = Config::default();
    let seed = MatchSeed::from_bytes([15u8; 32]);
    let now = LogicalTime(2_500);

    let (state, _) = create_initial_state_from_seed(&config, &roster, now, &seed).unwrap();

    assert_eq!(state.phase(), Phase::Night);
    assert_eq!(state.round(), 1);
    assert_eq!(state.current_timer().0, 1);
    assert_eq!(
        state.phase_ends_at(),
        LogicalTime(2_500 + config.phase_durations.night.millis())
    );
    assert_eq!(state.alive().len(), 10);
    assert!(state.revealed().is_empty());
    assert!(state.night_choices().is_empty());
    assert!(state.votes().is_empty());
    assert!(state.outcome().is_none());
    assert_eq!(state.last_doctor_target(), None);
    assert!(state.seer_history().is_empty());

    // 10 seats has 1 Witch, so witch_potions must be Some with both charges
    assert_eq!(
        state.witch_potions(),
        Some(tabula_game_werewolf::WitchPotions {
            heal: true,
            poison: true,
        })
    );

    // 6 seats has 0 Witches, so witch_potions must be None
    let roster_6 = make_dense_roster(6);
    let (state_6, _) = create_initial_state_from_seed(&config, &roster_6, now, &seed).unwrap();
    assert_eq!(state_6.witch_potions(), None);
}
