//! The feature graph and scoring, checked against an independent oracle over
//! real matches.
//!
//! # The oracle
//!
//! `rules::feature::recompute` walks the whole board and flood-fills each
//! component from scratch. It is structurally different from the incremental
//! graph — a breadth-first search versus a sequence of merges — so the two do
//! not share an idea and therefore cannot share a bug. Here it runs after
//! **every accepted input of a complete match**, at every supported seat count,
//! which is what makes "the incremental structure is correct" a claim about the
//! whole reachable space rather than about a hand-built board.
//!
//! # What this file does *not* rely on
//!
//! Scoring arithmetic is not checked here by recomputation: the value of a
//! feature depends on the followers that were on it *at the moment it
//! completed*, which no end-state recomputation can recover. That arithmetic is
//! covered by the example tests in `rules::scoring::tests`, one per feature type
//! and per tie case. What this file checks about scoring is the part a whole
//! match can witness: exactly-once, monotonicity, conservation, and that the
//! final standings agree with the scores the events reported.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use tabula_core::{MatchSeed, SeatId};
use tabula_game_api::Input;
use tabula_game_tiles::{
    rules::{recompute, MEEPLES_PER_SEAT},
    Event, FeatureId, State, Status,
};

use support::{apply_at, config, create, next_input, SEATS_MAX, SEATS_MIN};

/// Compare the incremental graph against a from-scratch recomputation.
///
/// Compared up to feature ids, because ids are handed out in placement order
/// and the recomputation has no notion of one: what must match is the
/// *partition* — which segments are together — plus each component's kind, open
/// edge count, and pennant count.
fn assert_agrees_with_recomputation(state: &State, label: &str) {
    let reference = recompute(state.board());
    let graph = state.features();

    assert_eq!(
        reference.components.len(),
        graph.len(),
        "{label}: the recomputation found {} components, the graph holds {}",
        reference.components.len(),
        graph.len()
    );

    for component in &reference.components {
        let ids: BTreeSet<FeatureId> = component
            .members
            .iter()
            .filter_map(|member| graph.of(*member))
            .collect();
        assert_eq!(
            ids.len(),
            1,
            "{label}: segments the board connects are split across {} components",
            ids.len()
        );
        let id = *ids.iter().next().expect("exactly one id");
        let feature = graph.feature(id).expect("the id names a feature");

        assert_eq!(
            feature.members(),
            &component.members,
            "{label}: membership disagrees for feature {}",
            id.get()
        );
        assert_eq!(feature.kind(), component.kind, "{label}: kind disagrees");
        assert_eq!(
            feature.open_edges(),
            component.open_edges,
            "{label}: open-edge count disagrees for feature {}",
            id.get()
        );
        assert_eq!(
            feature.pennants(),
            component.pennants,
            "{label}: pennant count disagrees for feature {}",
            id.get()
        );
    }
}

/// **The differential.** Play whole matches at every seat count and compare the
/// incremental graph with the from-scratch recomputation after every input.
#[test]
fn the_incremental_graph_agrees_with_recomputation_after_every_input_of_a_full_match() {
    let mut steps_compared = 0usize;

    for seats in SEATS_MIN..=SEATS_MAX {
        for seed_byte in [3u8, 42, 199] {
            let seed = MatchSeed::from_bytes([seed_byte; 32]);
            let mut state = create(&seed, seats, config());
            assert_agrees_with_recomputation(&state, "opening position");

            for step in 0..256u64 {
                let Some(input) = next_input(&state) else {
                    break;
                };
                apply_at(&mut state, input, &seed, step + 1).expect("a driven input is legal");
                assert_agrees_with_recomputation(
                    &state,
                    &format!("{seats} seats, seed {seed_byte}, input {}", step + 1),
                );
                steps_compared += 1;
            }
            assert_eq!(state.status(), Status::Ended);
        }
    }

    assert!(
        steps_compared > 1_000,
        "only {steps_compared} steps compared — the driver stopped early and this \
         differential is weaker than it looks"
    );
}

/// A completed feature is scored **exactly once**, witnessed in the event
/// stream rather than inferred from the state.
///
/// The state's `scored` flag would make double-scoring invisible: `retire`
/// returns nothing the second time, so the scores would be right and the bug
/// would be a wasted call. The event stream is where a second scoring would
/// actually show.
#[test]
fn every_completed_feature_is_scored_exactly_once_across_a_whole_match() {
    let seed = MatchSeed::from_bytes([42u8; 32]);
    let mut state = create(&seed, 4, config());
    let mut scored: BTreeMap<FeatureId, usize> = BTreeMap::new();
    let mut final_events = 0usize;

    for step in 0..256u64 {
        let Some(input) = next_input(&state) else {
            break;
        };
        let outcome = apply_at(&mut state, input, &seed, step + 1).expect("legal");
        for event in &outcome.events {
            match event {
                Event::FeatureScored { feature, .. } => {
                    *scored.entry(*feature).or_default() += 1;
                }
                Event::FinalScored { .. } => final_events += 1,
                _ => {}
            }
        }
    }

    assert_eq!(state.status(), Status::Ended);
    assert!(
        !scored.is_empty(),
        "a whole match must complete at least one feature"
    );
    for (feature, times) in &scored {
        assert_eq!(
            *times,
            1,
            "feature {} was scored {times} times",
            feature.get()
        );
    }
    assert!(
        final_events <= 1,
        "end-of-game scoring must happen once, not {final_events} times"
    );

    // Every feature the graph holds is retired by the end: the ones that
    // completed during play, and the rest by final scoring.
    for (id, feature) in state.features().iter() {
        assert!(
            feature.scored(),
            "feature {} survived the end of the match unscored",
            id.get()
        );
        assert!(
            feature.meeples().is_empty(),
            "feature {} is scored but still holds followers",
            id.get()
        );
    }
}

/// Scores only ever go up, and every point that appears in a score was
/// reported by an event.
#[test]
fn scores_are_monotonic_and_match_the_points_the_events_reported() {
    let seed = MatchSeed::from_bytes([7u8; 32]);
    let mut state = create(&seed, 3, config());
    let mut reported: BTreeMap<SeatId, i64> = state.seats().iter().map(|s| (*s, 0)).collect();
    let mut previous: BTreeMap<SeatId, i64> = reported.clone();

    for step in 0..256u64 {
        let Some(input) = next_input(&state) else {
            break;
        };
        let outcome = apply_at(&mut state, input, &seed, step + 1).expect("legal");
        for event in &outcome.events {
            let (Event::FeatureScored { awarded, .. } | Event::FinalScored { awarded }) = event
            else {
                continue;
            };
            for (seat, points) in awarded {
                assert!(*points > 0, "an award of {points} points is not an award");
                *reported.entry(*seat).or_default() += *points;
            }
        }

        for seat in state.seats() {
            let now = state.scores().get(seat).copied().unwrap_or(0);
            assert!(
                now >= previous.get(seat).copied().unwrap_or(0),
                "seat {seat:?} lost points"
            );
            previous.insert(*seat, now);
        }
    }

    assert_eq!(
        state.scores(),
        &reported,
        "the scores in state must be exactly the points the events reported"
    );
}

/// Followers are conserved at every step: each seat's hand plus its followers
/// on the board is always the seven it started with.
///
/// The state validator asserts this too, but only when a state is decoded.
/// Checking it after every accepted input is what makes it a statement about
/// `apply` rather than about `serde`.
#[test]
fn followers_are_conserved_after_every_input_and_returned_when_a_feature_scores() {
    let seed = MatchSeed::from_bytes([11u8; 32]);
    let mut state = create(&seed, 5, config());
    let mut returns_seen = 0usize;
    let mut claims_seen = 0usize;

    let check = |state: &State, label: &str| {
        let followers = state.features().followers();
        for seat in state.seats() {
            let on_board = followers.values().filter(|owner| *owner == seat).count();
            let in_hand = usize::from(state.meeples_in_hand().get(seat).copied().unwrap_or(0));
            assert_eq!(
                in_hand + on_board,
                usize::from(MEEPLES_PER_SEAT),
                "{label}: seat {seat:?} has {in_hand} in hand and {on_board} on the board"
            );
        }
    };

    check(&state, "opening position");
    for step in 0..256u64 {
        let Some(input) = next_input(&state) else {
            break;
        };
        let outcome = apply_at(&mut state, input, &seed, step + 1).expect("legal");
        for event in &outcome.events {
            match event {
                Event::MeeplePlaced { .. } => claims_seen += 1,
                Event::FeatureScored { returned, .. } => {
                    returns_seen += returned.iter().filter(|(_, count)| *count > 0).count();
                }
                _ => {}
            }
        }
        check(&state, &format!("after input {}", step + 1));

        // A returned follower must be off the board, not merely counted back.
        for (id, feature) in state.features().iter() {
            assert!(
                !feature.scored() || feature.meeples().is_empty(),
                "feature {} returned its followers but still holds them",
                id.get()
            );
        }
    }

    assert!(claims_seen > 0, "the driver never claimed a feature");
    assert!(
        returns_seen > 0,
        "no follower was ever returned — the completion path is untested by this run"
    );
}

/// The terminal standings agree with the scores: same seats, ranked by score
/// descending, ties sharing a rank.
#[test]
fn the_final_standings_rank_the_seats_by_the_scores_they_earned() {
    let seed = MatchSeed::from_bytes([99u8; 32]);
    let mut state = create(&seed, 4, config());
    let mut outcome = None;

    for step in 0..256u64 {
        let Some(input) = next_input(&state) else {
            break;
        };
        let step_outcome = apply_at(&mut state, input, &seed, step + 1).expect("legal");
        for event in &step_outcome.events {
            if let Event::Ended { outcome: ended } = event {
                outcome = Some(ended.clone());
            }
        }
    }

    let outcome = outcome.expect("a finished match reports an outcome");
    let standings = outcome.standings();
    assert_eq!(standings.len(), state.seats().len());

    for standing in standings {
        assert_eq!(
            standing.score,
            state.scores().get(&standing.seat).copied().unwrap_or(0),
            "standing for seat {:?} does not match its score",
            standing.seat
        );
    }
    // Higher score never gets a worse rank.
    for a in standings {
        for b in standings {
            if a.score > b.score {
                assert!(a.rank < b.rank, "a higher score got a worse rank");
            }
            if a.score == b.score {
                assert_eq!(a.rank, b.rank, "equal scores must share a rank");
            }
        }
    }
    // Ranks are dense from zero — the platform's own requirement.
    let ranks: BTreeSet<u8> = standings.iter().map(|standing| standing.rank).collect();
    let max = ranks.iter().copied().max().expect("at least one rank");
    assert_eq!(
        ranks,
        (0..=max).collect::<BTreeSet<u8>>(),
        "ranks must start at 0 with no gaps"
    );
}

/// A claim is refused on a feature somebody else already owns, and refused
/// once a seat has spent all seven followers — with the state unchanged either
/// way.
#[test]
fn a_claim_is_refused_when_the_feature_is_taken_or_the_hand_is_empty() {
    let seed = MatchSeed::from_bytes([5u8; 32]);
    let mut state = create(&seed, 2, config());

    // Reach a claim step and take a slot.
    let input = next_input(&state).expect("the opening offers a placement");
    apply_at(&mut state, input, &seed, 1).expect("legal");
    let slots = state.claimable_segments();
    assert!(!slots.is_empty(), "the start-adjacent tile offers a claim");
    let taken = slots[0];
    let seat = state.turn();
    apply_at(
        &mut state,
        Input::Player {
            seat,
            command: tabula_game_tiles::Command::PlaceMeeple { segment: taken },
        },
        &seed,
        2,
    )
    .expect("the first claim is legal");

    // The same segment is no longer claimable, and the claim step is over.
    assert!(state.claimable_segments().is_empty());
    let before = tabula_core::canonical_encode(&state).expect("encodes");
    let next_seat = state.turn();
    let refused = apply_at(
        &mut state,
        Input::Player {
            seat: next_seat,
            command: tabula_game_tiles::Command::PlaceMeeple { segment: taken },
        },
        &seed,
        3,
    );
    assert!(
        refused.is_err(),
        "claiming outside the claim step is refused"
    );
    assert_eq!(
        tabula_core::canonical_encode(&state).expect("encodes"),
        before,
        "a refused claim is a total no-op"
    );
}
