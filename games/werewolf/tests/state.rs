//! Canonical State structural validation, reconstruction barrier, and rules identity tests for Werewolf W2.

use std::{
    fs,
    path::{Path, PathBuf},
};

use smallvec::smallvec;
use tabula_core::{
    canonical_decode, canonical_encode, LogicalTime, MatchSeed, Millis, Occupant, RulesVersion,
    SeatEntry, SeatId, SeatRoster, TimerId, UserId,
};
use tabula_game_werewolf::{
    checked_deadline, create_initial_state_from_seed, Alignment, Ballot, Config, NightChoice,
    Phase, PhaseDuration, RawState, Role, State, StateError, WitchPotions, RULES_HASH,
    RULES_VERSION,
};

fn make_test_roster(seat_count: u8) -> SeatRoster {
    let mut seats = smallvec![];
    for i in 0..seat_count {
        seats.push(SeatEntry {
            seat: SeatId(i),
            occupant: Occupant::Human(UserId(1_000 + u128::from(i))),
            team: None,
        });
    }
    SeatRoster::new(seats).unwrap()
}

fn valid_initial_state(seat_count: u8) -> State {
    let roster = make_test_roster(seat_count);
    let config = Config::default();
    let seed = MatchSeed::from_bytes([123u8; 32]);
    let (state, _) =
        create_initial_state_from_seed(&config, &roster, LogicalTime::ZERO, &seed).unwrap();
    state
}

// ---------------------------------------------------------------------------
// 1. Canonical Serialization Round-Trip
// ---------------------------------------------------------------------------

#[test]
fn state_canonical_round_trip() {
    for seats in [6, 8, 10, 12, 16, 20] {
        let original = valid_initial_state(seats);
        let encoded = canonical_encode(&original).expect("encoding state succeeds");
        let decoded: State = canonical_decode(&encoded).expect("decoding state succeeds");

        assert_eq!(
            original, decoded,
            "round-trip state must be equal for {seats} seats"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Structural Validation Rejection of Corrupt RawState
// ---------------------------------------------------------------------------

#[test]
fn state_reconstruction_rejects_missing_role() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Remove role for seat 0
    raw.roles.remove(&SeatId(0));

    let encoded = canonical_encode(&raw).expect("corrupt RawState remains encodable");
    assert!(
        canonical_decode::<State>(&encoded).is_err(),
        "State deserialization must use the same validation barrier"
    );

    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::MissingRole { seat: SeatId(0) });
}

#[test]
fn state_reconstruction_rejects_extra_role() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Add role for an unknown seat 99
    raw.roles.insert(SeatId(99), Role::Villager);

    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::UnknownSeatInRoles { seat: SeatId(99) });
}

#[test]
fn state_reconstruction_rejects_unknown_seat_in_roles() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Replace seat 0's role key with unknown seat 42
    let role = raw.roles.remove(&SeatId(0)).unwrap();
    raw.roles.insert(SeatId(42), role);

    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::UnknownSeatInRoles { seat: SeatId(42) });
}

#[test]
fn state_reconstruction_rejects_wrong_preset_counts() {
    let original = valid_initial_state(6); // 6 seats: 1W, 1S, 1D, 0H, 0T, 3V
    let mut raw = RawState::from(original);

    // Mutate roles to have 2 Werewolves instead of 1
    let villager_seat = raw
        .roles
        .iter()
        .find(|(_, &r)| r == Role::Villager)
        .map(|(&s, _)| s)
        .unwrap();
    raw.roles.insert(villager_seat, Role::Werewolf);

    let err = State::try_from(raw).unwrap_err();
    assert!(
        matches!(err, StateError::RoleCountMismatch { .. }),
        "expected RoleCountMismatch, got {err:?}"
    );
}

#[test]
fn state_reconstruction_rejects_invalid_alive_set() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Add unknown seat to alive set
    raw.alive.insert(SeatId(88));

    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::UnknownSeatInAlive { seat: SeatId(88) });

    // Empty alive set in playing match
    let mut raw_empty_alive = RawState::from(valid_initial_state(6));
    raw_empty_alive.alive.clear();
    let err_empty = State::try_from(raw_empty_alive).unwrap_err();
    assert_eq!(err_empty, StateError::EmptyAliveSetInPlayingMatch);
}

#[test]
fn state_reconstruction_rejects_incoherent_revealed_set() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Seat 0 is both alive and revealed
    raw.revealed.insert(SeatId(0), raw.roles[&SeatId(0)]);

    let err = State::try_from(raw).unwrap_err();
    assert_eq!(
        err,
        StateError::AliveAndRevealedConflict { seat: SeatId(0) }
    );

    // Seat 0 is dead (not in alive), but revealed role does not match assigned role
    let mut raw_mismatch = RawState::from(valid_initial_state(6));
    raw_mismatch.alive.remove(&SeatId(0));
    let true_role = raw_mismatch.roles[&SeatId(0)];
    let forged_role = if true_role == Role::Werewolf {
        Role::Villager
    } else {
        Role::Werewolf
    };
    raw_mismatch.revealed.insert(SeatId(0), forged_role);

    let err_mismatch = State::try_from(raw_mismatch).unwrap_err();
    assert!(
        matches!(err_mismatch, StateError::RevealedRoleMismatch { .. }),
        "expected RevealedRoleMismatch, got {err_mismatch:?}"
    );
}

#[test]
fn state_reconstruction_rejects_invalid_round() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Round 0 is invalid
    raw.round = 0;
    let err_zero = State::try_from(raw.clone()).unwrap_err();
    assert!(matches!(
        err_zero,
        StateError::InvalidRound { round: 0, .. }
    ));

    // Round > max_rounds (101 > 100)
    raw.round = 101;
    let err_over = State::try_from(raw).unwrap_err();
    assert!(matches!(
        err_over,
        StateError::InvalidRound { round: 101, .. }
    ));
}

#[test]
fn state_reconstruction_rejects_invalid_timer() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // TimerId(0) is invalid
    raw.current_timer = TimerId(0);
    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::InvalidTimerId);
}

#[test]
fn state_reconstruction_rejects_unsorted_or_duplicate_roster() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Swap seats to make roster unsorted
    raw.roster.swap(0, 1);
    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::RosterNotSortedOrUnique);

    // Duplicate seat in roster
    let mut raw_dup = RawState::from(valid_initial_state(6));
    raw_dup.roster[1] = raw_dup.roster[0];
    let err_dup = State::try_from(raw_dup).unwrap_err();
    assert_eq!(err_dup, StateError::RosterNotSortedOrUnique);
}

#[test]
fn state_reconstruction_rejects_votes_outside_vote_phase() {
    let original = valid_initial_state(6); // Phase::Night
    let mut raw = RawState::from(original);

    // Inserting a vote during Night
    raw.votes.insert(SeatId(0), Ballot::Target(SeatId(1)));

    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::VotesOutsideVotePhase);
}

#[test]
fn state_reconstruction_rejects_self_vote_in_vote_phase() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);
    raw.phase = Phase::Vote;

    // Self-vote
    raw.votes.insert(SeatId(0), Ballot::Target(SeatId(0)));

    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::SelfVoteNotAllowed { voter: SeatId(0) });
}

#[test]
fn state_reconstruction_rejects_night_choices_outside_night_phase() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);
    raw.phase = Phase::Day;

    raw.night_choices.insert(SeatId(0), NightChoice::Pass);

    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::NightChoicesOutsideNightPhase);
}

#[test]
fn state_reconstruction_rejects_unauthorized_night_choice() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Find a Villager seat and give them a Werewolf attack choice
    let villager = raw
        .roles
        .iter()
        .find(|(_, &r)| r == Role::Villager)
        .map(|(&s, _)| s)
        .unwrap();
    raw.night_choices
        .insert(villager, NightChoice::WolfTarget(Some(SeatId(1))));

    let err = State::try_from(raw).unwrap_err();
    assert!(
        matches!(
            err,
            StateError::UnauthorizedNightChoice {
                role: Role::Villager,
                ..
            }
        ),
        "expected UnauthorizedNightChoice, got {err:?}"
    );
}

fn seat_with_role(raw: &RawState, role: Role) -> SeatId {
    raw.roles
        .iter()
        .find(|(_, &assigned)| assigned == role)
        .map(|(&seat, _)| seat)
        .expect("test state contains the requested role")
}

fn mark_dead(raw: &mut RawState, seat: SeatId) {
    raw.alive.remove(&seat);
    raw.revealed.insert(seat, raw.roles[&seat]);
}

fn night_choice_error(raw: &mut RawState, actor: SeatId, choice: NightChoice) -> StateError {
    raw.night_choices.insert(actor, choice);
    let encoded = canonical_encode(raw).expect("corrupt RawState remains encodable");
    assert!(
        canonical_decode::<State>(&encoded).is_err(),
        "State deserialization must reject invalid night choices"
    );
    State::try_from(raw.clone()).unwrap_err()
}

#[test]
fn state_reconstruction_rejects_invalid_wolf_targets() {
    let mut raw_dead = RawState::from(valid_initial_state(8));
    let wolf = seat_with_role(&raw_dead, Role::Werewolf);
    let dead_target = raw_dead
        .roles
        .iter()
        .find(|(&seat, &role)| seat != wolf && !role.is_wolf())
        .map(|(&seat, _)| seat)
        .expect("8-seat state contains a non-wolf target");
    mark_dead(&mut raw_dead, dead_target);
    assert_eq!(
        night_choice_error(
            &mut raw_dead,
            wolf,
            NightChoice::WolfTarget(Some(dead_target)),
        ),
        StateError::DeadNightTarget {
            target: dead_target
        }
    );

    let raw_other_wolf = RawState::from(valid_initial_state(8));
    let wolves: Vec<_> = raw_other_wolf
        .roles
        .iter()
        .filter_map(|(&seat, &role)| role.is_wolf().then_some(seat))
        .collect();
    assert_eq!(wolves.len(), 2);
    let mut raw_other_wolf = raw_other_wolf;
    assert_eq!(
        night_choice_error(
            &mut raw_other_wolf,
            wolves[0],
            NightChoice::WolfTarget(Some(wolves[1])),
        ),
        StateError::WerewolfTargetNotAllowed { target: wolves[1] }
    );
}

#[test]
fn state_reconstruction_rejects_dead_doctor_and_witch_targets() {
    let mut raw_doctor = RawState::from(valid_initial_state(10));
    let doctor = seat_with_role(&raw_doctor, Role::Doctor);
    let doctor_target = seat_with_role(&raw_doctor, Role::Villager);
    mark_dead(&mut raw_doctor, doctor_target);
    assert_eq!(
        night_choice_error(
            &mut raw_doctor,
            doctor,
            NightChoice::Protect(Some(doctor_target)),
        ),
        StateError::DeadNightTarget {
            target: doctor_target
        }
    );

    let mut raw_witch_heal = RawState::from(valid_initial_state(10));
    let witch = seat_with_role(&raw_witch_heal, Role::Witch);
    let witch_target = seat_with_role(&raw_witch_heal, Role::Villager);
    mark_dead(&mut raw_witch_heal, witch_target);
    assert_eq!(
        night_choice_error(
            &mut raw_witch_heal,
            witch,
            NightChoice::WitchHeal(Some(witch_target)),
        ),
        StateError::DeadNightTarget {
            target: witch_target
        }
    );

    let mut raw_witch_poison = RawState::from(valid_initial_state(10));
    let witch = seat_with_role(&raw_witch_poison, Role::Witch);
    let witch_target = seat_with_role(&raw_witch_poison, Role::Villager);
    mark_dead(&mut raw_witch_poison, witch_target);
    assert_eq!(
        night_choice_error(
            &mut raw_witch_poison,
            witch,
            NightChoice::WitchPoison(Some(witch_target)),
        ),
        StateError::DeadNightTarget {
            target: witch_target
        }
    );
}

#[test]
fn state_reconstruction_rejects_invalid_hunter_targets() {
    let mut raw_self = RawState::from(valid_initial_state(10));
    let hunter = seat_with_role(&raw_self, Role::Hunter);
    assert_eq!(
        night_choice_error(&mut raw_self, hunter, NightChoice::HunterMark(Some(hunter)),),
        StateError::SelfNightTarget { actor: hunter }
    );

    let mut raw_dead = RawState::from(valid_initial_state(10));
    let hunter = seat_with_role(&raw_dead, Role::Hunter);
    let dead_target = seat_with_role(&raw_dead, Role::Villager);
    mark_dead(&mut raw_dead, dead_target);
    assert_eq!(
        night_choice_error(
            &mut raw_dead,
            hunter,
            NightChoice::HunterMark(Some(dead_target)),
        ),
        StateError::DeadNightTarget {
            target: dead_target
        }
    );
}

#[test]
fn state_reconstruction_accepts_role_specific_living_targets() {
    let mut raw = RawState::from(valid_initial_state(10));
    let wolf = seat_with_role(&raw, Role::Werewolf);
    let seer = seat_with_role(&raw, Role::Seer);
    let doctor = seat_with_role(&raw, Role::Doctor);
    let witch = seat_with_role(&raw, Role::Witch);
    let hunter = seat_with_role(&raw, Role::Hunter);
    let villager = seat_with_role(&raw, Role::Villager);

    raw.night_choices
        .insert(wolf, NightChoice::WolfTarget(Some(villager)));
    raw.night_choices
        .insert(seer, NightChoice::Investigate(villager));
    raw.night_choices
        .insert(doctor, NightChoice::Protect(Some(doctor)));
    raw.night_choices
        .insert(witch, NightChoice::WitchHeal(Some(witch)));
    raw.night_choices
        .insert(hunter, NightChoice::HunterMark(Some(villager)));

    assert!(State::try_from(raw).is_ok());
}

#[test]
fn state_reconstruction_rejects_invalid_doctor_target() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Unknown seat as Doctor target
    raw.last_doctor_target = Some(SeatId(99));
    let err = State::try_from(raw).unwrap_err();
    assert_eq!(err, StateError::UnknownDoctorTarget { target: SeatId(99) });
}

#[test]
fn state_reconstruction_rejects_invalid_witch_potions() {
    // 6 seats has 0 witches
    let original_6 = valid_initial_state(6);
    let mut raw_6 = RawState::from(original_6);
    raw_6.witch_potions = Some(WitchPotions::default());

    let err_6 = State::try_from(raw_6).unwrap_err();
    assert_eq!(err_6, StateError::InvalidWitchPotionOwnership);

    // 10 seats has 1 witch
    let original_10 = valid_initial_state(10);
    let mut raw_10 = RawState::from(original_10);
    raw_10.witch_potions = None;

    let err_10 = State::try_from(raw_10).unwrap_err();
    assert_eq!(err_10, StateError::InvalidWitchPotionOwnership);
}

#[test]
fn state_reconstruction_rejects_seer_history_alignment_mismatch() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Find werewolf seat
    let wolf_seat = raw
        .roles
        .iter()
        .find(|(_, &r)| r == Role::Werewolf)
        .map(|(&s, _)| s)
        .unwrap();

    // Claim Seer investigated wolf and got Village alignment
    raw.seer_history.insert(wolf_seat, Alignment::Village);

    let err = State::try_from(raw).unwrap_err();
    assert!(
        matches!(err, StateError::SeerHistoryAlignmentMismatch { .. }),
        "expected SeerHistoryAlignmentMismatch, got {err:?}"
    );
}

#[test]
fn state_reconstruction_rejects_phase_and_outcome_mismatch() {
    let original = valid_initial_state(6);
    let mut raw = RawState::from(original);

    // Ended phase without outcome
    raw.phase = Phase::Ended;
    raw.outcome = None;
    let err_ended = State::try_from(raw).unwrap_err();
    assert_eq!(err_ended, StateError::EndedWithoutOutcome);
}

#[test]
fn initial_state_creation_rejects_deadline_overflow() {
    let roster = make_test_roster(6);
    let config = Config::default();
    let seed = MatchSeed::from_bytes([17u8; 32]);

    let err =
        create_initial_state_from_seed(&config, &roster, LogicalTime(u64::MAX), &seed).unwrap_err();
    assert!(matches!(err, StateError::DeadlineOverflow { .. }));
}

#[test]
fn checked_deadline_is_exact_until_u64_overflow() {
    let duration = PhaseDuration::from_millis(Millis(1_000)).unwrap();
    assert_eq!(
        checked_deadline(LogicalTime(u64::MAX - 1_000), duration).unwrap(),
        LogicalTime(u64::MAX)
    );
    assert!(matches!(
        checked_deadline(LogicalTime(u64::MAX), duration),
        Err(StateError::DeadlineOverflow { .. })
    ));
}

// ---------------------------------------------------------------------------
// 3. Rules Identity Verification
// ---------------------------------------------------------------------------

#[test]
fn rules_version_matches_manifest() {
    let manifest_version = include_str!("../game.toml")
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "rules_version")
                .then(|| value.split('#').next()?.trim().parse::<u32>().ok())?
        })
        .expect("game.toml must declare a numeric rules_version");

    assert_eq!(RULES_VERSION, RulesVersion(manifest_version));
}

#[test]
fn rules_hash_is_non_zero() {
    assert_ne!(
        RULES_HASH, [0u8; 32],
        "build-derived RULES_HASH must not be all-zero"
    );
}

#[test]
fn rules_hash_matches_independent_rules_subtree_oracle() {
    let sources = independent_rules_sources();
    assert!(
        !sources.is_empty(),
        "the rules source tree must not be empty"
    );
    assert_eq!(RULES_HASH, independent_rules_hash(&sources));
}

#[test]
fn rules_subtree_is_discovered_recursively_in_canonical_order() {
    let root = rules_root();
    let sources = independent_rules_sources();
    let source_paths: Vec<_> = sources.iter().map(|(path, _)| path.clone()).collect();
    let mut filesystem_paths = Vec::new();
    collect_rust_paths(&root, &root, &mut filesystem_paths);
    filesystem_paths.sort();

    assert_eq!(filesystem_paths, source_paths);
    assert!(source_paths.windows(2).all(|paths| paths[0] < paths[1]));
    assert!(source_paths
        .iter()
        .all(|path| !path.is_absolute() && !path.starts_with("..")));
}

#[test]
fn changing_canonical_rules_source_changes_hash_oracle() {
    let sources = independent_rules_sources();
    let before = independent_rules_hash(&sources);
    let mut mutated = sources;
    mutated
        .first_mut()
        .expect("the rules source tree is not empty")
        .1
        .push(b'\n');

    assert_ne!(before, independent_rules_hash(&mutated));
}

fn rules_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules")
}

fn independent_rules_sources() -> Vec<(PathBuf, Vec<u8>)> {
    let root = rules_root();
    let mut paths = Vec::new();
    collect_rust_paths(&root, &root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|relative| {
            let bytes = fs::read(root.join(&relative)).expect("rules source must be readable");
            (relative, bytes)
        })
        .collect()
}

/// The build script preimage is `domain || rules_version || sorted(path || len || bytes)`.
fn independent_rules_hash(sources: &[(PathBuf, Vec<u8>)]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.rules.source.v2");
    hasher.update(&RULES_VERSION.0.to_le_bytes());
    for (relative, bytes) in sources {
        let relative = relative.to_string_lossy().replace('\\', "/");
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    *hasher.finalize().as_bytes()
}

fn collect_rust_paths(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("rules directory must be readable") {
        let path = entry
            .expect("rules directory entry must be readable")
            .path();
        if path.is_dir() {
            collect_rust_paths(root, &path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(
                path.strip_prefix(root)
                    .expect("rules source must remain under rules directory")
                    .to_owned(),
            );
        }
    }
}
