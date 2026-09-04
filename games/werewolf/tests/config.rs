//! Focused constructor, boundary, deserialization barrier, and roster tests for Werewolf W1.

use smallvec::smallvec;
use tabula_core::{
    canonical_decode, canonical_encode, Millis, Occupant, SeatEntry, SeatId, SeatRoster, UserId,
};
use tabula_game_api::ConfigError;
use tabula_game_werewolf::{
    Alignment, Config, DurationError, MaxRounds, MaxRoundsError, PhaseDuration, PhaseDurations,
    Preset, RawConfig, RawPhaseDurations, Role, RoleCounts, SeatCount, SeatCountError, VoteMode,
    MAX_MAX_ROUNDS, MAX_PHASE_DURATION_MS, MAX_SEATS, MIN_MAX_ROUNDS, MIN_PHASE_DURATION_MS,
    MIN_SEATS,
};

// ---------------------------------------------------------------------------
// 1. Pinned ClassicV1 Table (6..=20 seats)
// ---------------------------------------------------------------------------

/// Independent reference table of expected `ClassicV1` role distributions.
/// Format: `(seats, werewolves, seers, doctors, hunters, witches, villagers)`
const EXPECTED_CLASSIC_V1: [(u8, u8, u8, u8, u8, u8, u8); 15] = [
    (6, 1, 1, 1, 0, 0, 3),
    (7, 1, 1, 1, 0, 0, 4),
    (8, 2, 1, 1, 1, 0, 3),
    (9, 2, 1, 1, 1, 0, 4),
    (10, 2, 1, 1, 1, 1, 4),
    (11, 2, 1, 1, 1, 1, 5),
    (12, 3, 1, 1, 1, 1, 5),
    (13, 3, 1, 1, 1, 1, 6),
    (14, 3, 1, 1, 1, 1, 7),
    (15, 3, 1, 1, 1, 1, 8),
    (16, 4, 1, 1, 1, 1, 8),
    (17, 4, 1, 1, 1, 1, 9),
    (18, 4, 1, 1, 1, 1, 10),
    (19, 4, 1, 1, 1, 1, 11),
    (20, 5, 1, 1, 1, 1, 11),
];

#[test]
fn classic_v1_table_matches_pinned_specification_for_all_seat_counts() {
    for &(seats, exp_w, exp_s, exp_d, exp_h, exp_t, exp_v) in &EXPECTED_CLASSIC_V1 {
        let sc = SeatCount::new(seats).expect("valid seat count in 6..=20");
        let counts = Preset::ClassicV1.role_counts(sc);

        assert_eq!(
            counts.werewolves, exp_w,
            "werewolf count mismatch for {seats} seats"
        );
        assert_eq!(counts.seers, exp_s, "seer count mismatch for {seats} seats");
        assert_eq!(
            counts.doctors, exp_d,
            "doctor count mismatch for {seats} seats"
        );
        assert_eq!(
            counts.hunters, exp_h,
            "hunter count mismatch for {seats} seats"
        );
        assert_eq!(
            counts.witches, exp_t,
            "witch count mismatch for {seats} seats"
        );
        assert_eq!(
            counts.villagers, exp_v,
            "villager count mismatch for {seats} seats"
        );

        // Invariant: sum of roles must equal seat count exactly.
        assert_eq!(
            counts.total(),
            seats,
            "sum of roles does not equal {seats} seats"
        );

        // Invariant: multiset contains exactly the expected count of each role.
        let multiset = counts.multiset();
        assert_eq!(multiset.len(), seats as usize);
        assert_eq!(
            multiset.iter().filter(|&&r| r == Role::Werewolf).count(),
            exp_w as usize
        );
        assert_eq!(
            multiset.iter().filter(|&&r| r == Role::Seer).count(),
            exp_s as usize
        );
        assert_eq!(
            multiset.iter().filter(|&&r| r == Role::Doctor).count(),
            exp_d as usize
        );
        assert_eq!(
            multiset.iter().filter(|&&r| r == Role::Hunter).count(),
            exp_h as usize
        );
        assert_eq!(
            multiset.iter().filter(|&&r| r == Role::Witch).count(),
            exp_t as usize
        );
        assert_eq!(
            multiset.iter().filter(|&&r| r == Role::Villager).count(),
            exp_v as usize
        );
    }
}

#[test]
fn role_counts_by_role_query_matches_fields() {
    let counts = RoleCounts {
        werewolves: 3,
        seers: 1,
        doctors: 1,
        hunters: 1,
        witches: 1,
        villagers: 5,
    };
    assert_eq!(counts.count(Role::Werewolf), 3);
    assert_eq!(counts.count(Role::Seer), 1);
    assert_eq!(counts.count(Role::Doctor), 1);
    assert_eq!(counts.count(Role::Hunter), 1);
    assert_eq!(counts.count(Role::Witch), 1);
    assert_eq!(counts.count(Role::Villager), 5);
}

// ---------------------------------------------------------------------------
// 2. Role and Alignment Semantics
// ---------------------------------------------------------------------------

#[test]
fn role_alignment_mapping_is_correct() {
    assert_eq!(Role::Villager.alignment(), Alignment::Village);
    assert_eq!(Role::Seer.alignment(), Alignment::Village);
    assert_eq!(Role::Doctor.alignment(), Alignment::Village);
    assert_eq!(Role::Hunter.alignment(), Alignment::Village);
    assert_eq!(Role::Witch.alignment(), Alignment::Village);
    assert_eq!(Role::Werewolf.alignment(), Alignment::Wolf);

    assert!(Role::Werewolf.is_wolf());
    assert!(!Role::Werewolf.is_village());
    assert!(Role::Villager.is_village());
    assert!(!Role::Villager.is_wolf());
}

#[test]
fn role_all_contains_all_six_roles_uniquely() {
    assert_eq!(Role::ALL.len(), 6);
    for role in Role::ALL {
        assert_eq!(Role::ALL.iter().filter(|&&r| r == role).count(), 1);
    }
}

// ---------------------------------------------------------------------------
// 3. SeatCount Boundaries
// ---------------------------------------------------------------------------

#[test]
fn seat_count_boundaries() {
    // Below minimum
    assert_eq!(
        SeatCount::new(0),
        Err(SeatCountError::OutOfRange { count: 0 })
    );
    assert_eq!(
        SeatCount::new(1),
        Err(SeatCountError::OutOfRange { count: 1 })
    );
    assert_eq!(
        SeatCount::new(5),
        Err(SeatCountError::OutOfRange { count: 5 })
    );

    // Minimum boundary
    let min = SeatCount::new(MIN_SEATS).expect("min seat count is valid");
    assert_eq!(min.get(), 6);

    // Interior value
    let mid = SeatCount::new(12).expect("interior seat count is valid");
    assert_eq!(mid.get(), 12);

    // Maximum boundary
    let max = SeatCount::new(MAX_SEATS).expect("max seat count is valid");
    assert_eq!(max.get(), 20);

    // Above maximum
    assert_eq!(
        SeatCount::new(21),
        Err(SeatCountError::OutOfRange { count: 21 })
    );
    assert_eq!(
        SeatCount::new(255),
        Err(SeatCountError::OutOfRange { count: 255 })
    );
}

// ---------------------------------------------------------------------------
// 4. PhaseDuration Boundaries
// ---------------------------------------------------------------------------

#[test]
fn phase_duration_boundaries() {
    // Below minimum: 0, 999 ms
    assert_eq!(
        PhaseDuration::from_millis(Millis(0)),
        Err(DurationError::OutOfRange { millis: 0 })
    );
    assert_eq!(
        PhaseDuration::from_millis(Millis(999)),
        Err(DurationError::OutOfRange { millis: 999 })
    );

    // Minimum boundary: 1_000 ms
    let min = PhaseDuration::from_millis(Millis(MIN_PHASE_DURATION_MS)).expect("1_000 ms is valid");
    assert_eq!(min.millis(), 1_000);
    assert_eq!(min.get(), Millis(1_000));

    // Maximum boundary: 600_000 ms (10 minutes)
    let max =
        PhaseDuration::from_millis(Millis(MAX_PHASE_DURATION_MS)).expect("600_000 ms is valid");
    assert_eq!(max.millis(), 600_000);

    // Above maximum: 600_001 ms, u64::MAX
    assert_eq!(
        PhaseDuration::from_millis(Millis(600_001)),
        Err(DurationError::OutOfRange { millis: 600_001 })
    );
    assert_eq!(
        PhaseDuration::from_millis(Millis(u64::MAX)),
        Err(DurationError::OutOfRange { millis: u64::MAX })
    );

    // Seconds conversion boundaries
    assert_eq!(
        PhaseDuration::from_secs(0),
        Err(DurationError::OutOfRange { millis: 0 })
    );
    assert_eq!(PhaseDuration::from_secs(1).unwrap().millis(), 1_000);
    assert_eq!(PhaseDuration::from_secs(600).unwrap().millis(), 600_000);
    assert_eq!(
        PhaseDuration::from_secs(601),
        Err(DurationError::OutOfRange { millis: 601_000 })
    );
}

// ---------------------------------------------------------------------------
// 5. MaxRounds Boundaries
// ---------------------------------------------------------------------------

#[test]
fn max_rounds_boundaries() {
    // Below minimum: 0
    assert_eq!(
        MaxRounds::new(0),
        Err(MaxRoundsError::OutOfRange { rounds: 0 })
    );

    // Minimum boundary: 1
    let min = MaxRounds::new(MIN_MAX_ROUNDS).expect("1 round is valid");
    assert_eq!(min.get(), 1);

    // Maximum boundary: 100
    let max = MaxRounds::new(MAX_MAX_ROUNDS).expect("100 rounds is valid");
    assert_eq!(max.get(), 100);

    // Above maximum: 101, u32::MAX
    assert_eq!(
        MaxRounds::new(101),
        Err(MaxRoundsError::OutOfRange { rounds: 101 })
    );
    assert_eq!(
        MaxRounds::new(u32::MAX),
        Err(MaxRoundsError::OutOfRange { rounds: u32::MAX })
    );

    // Default is 100
    assert_eq!(MaxRounds::default().get(), 100);
}

// ---------------------------------------------------------------------------
// 6. Config and PhaseDurations Defaults
// ---------------------------------------------------------------------------

#[test]
fn config_default_is_semantically_valid() {
    let cfg = Config::default();
    assert_eq!(cfg.preset, Preset::ClassicV1);
    assert_eq!(cfg.vote_mode, VoteMode::Plurality);
    assert_eq!(cfg.max_rounds.get(), 100);

    assert_eq!(cfg.phase_durations.night.millis(), 30_000);
    assert_eq!(cfg.phase_durations.dawn.millis(), 2_000);
    assert_eq!(cfg.phase_durations.day.millis(), 120_000);
    assert_eq!(cfg.phase_durations.vote.millis(), 30_000);
    assert_eq!(cfg.phase_durations.dusk.millis(), 2_000);
}

// ---------------------------------------------------------------------------
// 7. Serialization Barrier: Invalid DTOs Rejected
// ---------------------------------------------------------------------------

#[test]
fn deserialization_rejects_out_of_range_seat_count() {
    let raw_under: u8 = 5;
    let encoded_under = canonical_encode(&raw_under).unwrap();
    assert!(canonical_decode::<SeatCount>(&encoded_under).is_err());

    let raw_over: u8 = 21;
    let encoded_over = canonical_encode(&raw_over).unwrap();
    assert!(canonical_decode::<SeatCount>(&encoded_over).is_err());

    let raw_valid: u8 = 10;
    let encoded_valid = canonical_encode(&raw_valid).unwrap();
    let decoded: SeatCount = canonical_decode(&encoded_valid).unwrap();
    assert_eq!(decoded.get(), 10);
}

#[test]
fn deserialization_rejects_out_of_range_phase_duration() {
    let raw_under: u64 = 999;
    let encoded_under = canonical_encode(&raw_under).unwrap();
    assert!(canonical_decode::<PhaseDuration>(&encoded_under).is_err());

    let raw_over: u64 = 600_001;
    let encoded_over = canonical_encode(&raw_over).unwrap();
    assert!(canonical_decode::<PhaseDuration>(&encoded_over).is_err());

    let raw_valid: u64 = 30_000;
    let encoded_valid = canonical_encode(&raw_valid).unwrap();
    let decoded: PhaseDuration = canonical_decode(&encoded_valid).unwrap();
    assert_eq!(decoded.millis(), 30_000);
}

#[test]
fn deserialization_rejects_out_of_range_max_rounds() {
    let raw_zero: u32 = 0;
    let encoded_zero = canonical_encode(&raw_zero).unwrap();
    assert!(canonical_decode::<MaxRounds>(&encoded_zero).is_err());

    let raw_over: u32 = 101;
    let encoded_over = canonical_encode(&raw_over).unwrap();
    assert!(canonical_decode::<MaxRounds>(&encoded_over).is_err());

    let raw_valid: u32 = 50;
    let encoded_valid = canonical_encode(&raw_valid).unwrap();
    let decoded: MaxRounds = canonical_decode(&encoded_valid).unwrap();
    assert_eq!(decoded.get(), 50);
}

#[test]
fn deserialization_rejects_config_with_invalid_nested_durations() {
    let raw = RawConfig {
        phase_durations: RawPhaseDurations {
            night_ms: 500, // invalid (< 1_000)
            dawn_ms: 2_000,
            day_ms: 120_000,
            vote_ms: 30_000,
            dusk_ms: 2_000,
        },
        ..Default::default()
    };

    let encoded = canonical_encode(&raw).unwrap();
    let result = canonical_decode::<Config>(&encoded);
    assert!(
        result.is_err(),
        "must reject RawConfig with night_ms < 1_000"
    );

    let raw_over = RawConfig {
        phase_durations: RawPhaseDurations {
            night_ms: 30_000,
            dawn_ms: 2_000,
            day_ms: 700_000, // invalid (> 600_000)
            vote_ms: 30_000,
            dusk_ms: 2_000,
        },
        ..Default::default()
    };
    let encoded_over = canonical_encode(&raw_over).unwrap();
    assert!(
        canonical_decode::<Config>(&encoded_over).is_err(),
        "must reject RawConfig with day_ms > 600_000"
    );
}

#[test]
fn deserialization_rejects_config_with_invalid_max_rounds() {
    let raw_zero = RawConfig {
        max_rounds: 0,
        ..Default::default()
    };
    let encoded_zero = canonical_encode(&raw_zero).unwrap();
    assert!(
        canonical_decode::<Config>(&encoded_zero).is_err(),
        "must reject RawConfig with max_rounds = 0"
    );

    let raw_over = RawConfig {
        max_rounds: 101,
        ..Default::default()
    };
    let encoded_over = canonical_encode(&raw_over).unwrap();
    assert!(
        canonical_decode::<Config>(&encoded_over).is_err(),
        "must reject RawConfig with max_rounds = 101"
    );
}

// ---------------------------------------------------------------------------
// 8. Canonical Round-Trip Testing
// ---------------------------------------------------------------------------

#[test]
fn config_canonical_round_trip() {
    let original = Config::default();
    let bytes = canonical_encode(&original).expect("encoding succeeds");
    let restored: Config = canonical_decode(&bytes).expect("decoding succeeds");
    assert_eq!(original, restored);

    let custom = Config::new(
        Preset::ClassicV1,
        VoteMode::AbsoluteMajority,
        PhaseDurations {
            night: PhaseDuration::from_secs(45).unwrap(),
            dawn: PhaseDuration::from_secs(5).unwrap(),
            day: PhaseDuration::from_secs(180).unwrap(),
            vote: PhaseDuration::from_secs(60).unwrap(),
            dusk: PhaseDuration::from_secs(3).unwrap(),
        },
        MaxRounds::new(42).unwrap(),
    );
    let custom_bytes = canonical_encode(&custom).expect("encoding succeeds");
    let custom_restored: Config = canonical_decode(&custom_bytes).expect("decoding succeeds");
    assert_eq!(custom, custom_restored);
}

#[test]
fn primitive_types_canonical_round_trip() {
    // Preset
    let preset_bytes = canonical_encode(&Preset::ClassicV1).unwrap();
    assert_eq!(
        canonical_decode::<Preset>(&preset_bytes).unwrap(),
        Preset::ClassicV1
    );

    // VoteMode
    let vm_bytes = canonical_encode(&VoteMode::AbsoluteMajority).unwrap();
    assert_eq!(
        canonical_decode::<VoteMode>(&vm_bytes).unwrap(),
        VoteMode::AbsoluteMajority
    );

    // Role
    for role in Role::ALL {
        let r_bytes = canonical_encode(&role).unwrap();
        assert_eq!(canonical_decode::<Role>(&r_bytes).unwrap(), role);
    }

    // Alignment
    let a_bytes = canonical_encode(&Alignment::Wolf).unwrap();
    assert_eq!(
        canonical_decode::<Alignment>(&a_bytes).unwrap(),
        Alignment::Wolf
    );

    // SeatCount
    let sc = SeatCount::new(12).unwrap();
    let sc_bytes = canonical_encode(&sc).unwrap();
    assert_eq!(canonical_decode::<SeatCount>(&sc_bytes).unwrap(), sc);

    // PhaseDuration
    let pd = PhaseDuration::from_millis(Millis(45_000)).unwrap();
    let duration_bytes = canonical_encode(&pd).unwrap();
    assert_eq!(
        canonical_decode::<PhaseDuration>(&duration_bytes).unwrap(),
        pd
    );

    // MaxRounds
    let mr = MaxRounds::new(25).unwrap();
    let mr_bytes = canonical_encode(&mr).unwrap();
    assert_eq!(canonical_decode::<MaxRounds>(&mr_bytes).unwrap(), mr);

    // RoleCounts
    let rc = Preset::ClassicV1.role_counts(SeatCount::new(8).unwrap());
    let rc_bytes = canonical_encode(&rc).unwrap();
    assert_eq!(canonical_decode::<RoleCounts>(&rc_bytes).unwrap(), rc);
}

// ---------------------------------------------------------------------------
// 9. Seat Roster Validation
// ---------------------------------------------------------------------------

fn make_test_roster(seat_count: u8, has_empty: bool, has_team: bool) -> SeatRoster {
    let mut seats = smallvec![];
    for i in 0..seat_count {
        let occupant = if has_empty && i == 0 {
            Occupant::Empty
        } else {
            Occupant::Human(UserId(1_000 + u128::from(i)))
        };
        let team = if has_team && i == 0 { Some(1) } else { None };
        seats.push(SeatEntry {
            seat: SeatId(i),
            occupant,
            team,
        });
    }
    SeatRoster::new(seats).expect("unique seat ids in test helper")
}

#[test]
fn roster_validation_enforces_boundaries_and_rules() {
    let config = Config::default();

    // Below min seats (5 seats)
    let roster_5 = make_test_roster(5, false, false);
    assert!(matches!(
        config.validate_roster(&roster_5),
        Err(ConfigError::SeatCount)
    ));

    // Minimum seats (6 seats)
    let roster_6 = make_test_roster(6, false, false);
    assert_eq!(config.validate_roster(&roster_6).unwrap().get(), 6);

    // Maximum seats (20 seats)
    let roster_20 = make_test_roster(20, false, false);
    assert_eq!(config.validate_roster(&roster_20).unwrap().get(), 20);

    // Above max seats (21 seats)
    let roster_21 = make_test_roster(21, false, false);
    assert!(matches!(
        config.validate_roster(&roster_21),
        Err(ConfigError::SeatCount)
    ));

    // Empty occupant rejected
    let roster_empty = make_test_roster(8, true, false);
    assert!(matches!(
        config.validate_roster(&roster_empty),
        Err(ConfigError::Field(ref f)) if f == "occupant"
    ));

    // Assigned team rejected (teams = None required)
    let roster_team = make_test_roster(8, false, true);
    assert!(matches!(
        config.validate_roster(&roster_team),
        Err(ConfigError::Field(ref f)) if f == "team"
    ));
}

#[test]
fn validate_seat_count_matches_bounds() {
    assert!(matches!(
        Config::validate_seat_count(5),
        Err(ConfigError::SeatCount)
    ));
    assert_eq!(Config::validate_seat_count(6).unwrap().get(), 6);
    assert_eq!(Config::validate_seat_count(20).unwrap().get(), 20);
    assert!(matches!(
        Config::validate_seat_count(21),
        Err(ConfigError::SeatCount)
    ));
}
