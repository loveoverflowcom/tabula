//! **Tests the enforcement, not just the rule.** (doc 07 Phase 0, doc 09 §7 step 4)
//!
//! A conformance harness that cannot fail is worse than no harness: it produces
//! a green tick that means nothing, and everyone downstream believes it.
//!
//! Each test here builds a game that is *deliberately* broken in one specific
//! way and asserts the harness rejects it. Phase 0's exit criteria name two of
//! these explicitly — a seeded `HashMap` iteration bug and a rejection that
//! mutates state.
//!
//! These fixtures are the only place in the workspace where a `HashMap` is
//! allowed in canonical state, and it is allowed precisely because the point is
//! to watch it get caught.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use tabula_core::{
    MatchSeed, Occupant, RuleError, RuleErrorCode, RulesVersion, SeatEntry, SeatId, SeatRoster,
    UserId, Viewer,
};
use tabula_game_api::{Ctx, GameRules, Init, InitError, Input, Outcome};
use tabula_testkit::determinism::{assert_deterministic, assert_transactional_on_error, Scenario};

/// Stands in for the associated types these fixtures do not exercise.
///
/// A named unit struct rather than `()`: it keeps the fixtures shaped like a real
/// game, where `View` must be a distinct type from `State` (doc 02 §7.1).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Nothing;

fn roster() -> SeatRoster {
    SeatRoster::new(smallvec![SeatEntry {
        seat: SeatId(0),
        occupant: Occupant::Human(UserId(1)),
        team: None,
    }])
    .expect("fixture seats are unique")
}

fn inputs<C: Clone>(command: C, n: usize) -> Vec<Input<C>> {
    (0..n)
        .map(|_| Input::Player {
            seat: SeatId(0),
            command: command.clone(),
        })
        .collect()
}

/// Asserts `f` panics, and that the panic message names the invariant.
fn assert_harness_rejects(invariant: &str, f: impl FnOnce()) {
    let result = catch_unwind(AssertUnwindSafe(f));
    let Err(payload) = result else {
        panic!("the harness ACCEPTED a game that violates {invariant}. The conformance suite is not enforcing anything.");
    };

    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("<non-string panic>");

    assert!(
        message.contains(invariant),
        "the harness rejected the game but did not name {invariant}. A failure that \
         does not say which invariant broke is a failure someone will paper over.\n\
         message was: {message}"
    );
}

// ---------------------------------------------------------------------------
// R2: a rejection that mutates state
// ---------------------------------------------------------------------------

/// Mutates *before* validating — the exact anti-pattern doc 02 §3.2 exists to
/// prevent, and the one that corrupts a match invisibly until a replay diverges.
struct MutatesOnRejection;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Counter {
    touched: u32,
}

impl GameRules for MutatesOnRejection {
    type State = Counter;
    type Command = Nothing;
    type Event = Nothing;
    type View = Nothing;
    type ViewEvent = Nothing;
    type Config = Nothing;

    const RULES_VERSION: RulesVersion = RulesVersion(1);

    fn create(_: &Nothing, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        Ok(Init {
            state: Counter::default(),
            events: SmallVec::new(),
            effects: SmallVec::new(),
        })
    }

    fn apply(
        state: &mut Counter,
        _: Input<Nothing>,
        _: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        state.touched += 1; // <-- the bug: mutate, then reject
        Err(RuleError::code(RuleErrorCode::IllegalMove))
    }

    fn project(_: &Counter, _: Viewer) -> Nothing {
        Nothing
    }
    fn view_event(_: &Counter, _: &Nothing, _: Viewer) -> Option<Nothing> {
        None
    }
}

#[test]
fn harness_catches_a_rejection_that_mutates_state() {
    let scenario = Scenario::<MutatesOnRejection> {
        config: Nothing,
        roster: roster(),
        seed: MatchSeed::from_bytes([1u8; 32]),
        inputs: inputs(Nothing, 4),
    };

    assert_harness_rejects("R2", || {
        assert_transactional_on_error::<MutatesOnRejection>(&scenario);
    });
}

// ---------------------------------------------------------------------------
// I-2: a HashMap in canonical state
// ---------------------------------------------------------------------------

/// Canonical state containing a `HashMap`. Postcard serializes a map in
/// *iteration order*, and two independently constructed `HashMap`s in one thread
/// get different `RandomState` seeds — so the two runs the harness performs
/// encode the same logical state to different bytes.
///
/// This is why `assert_deterministic` builds state from scratch twice rather than
/// hashing one state twice: hashing the same instance twice would agree, and the
/// bug would ship.
struct HashMapInState;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Bag {
    // The hazard. Banned everywhere else by clippy.toml's `disallowed-types`.
    items: HashMap<u32, u32>,
}

impl GameRules for HashMapInState {
    type State = Bag;
    type Command = Nothing;
    type Event = Nothing;
    type View = Nothing;
    type ViewEvent = Nothing;
    type Config = Nothing;

    const RULES_VERSION: RulesVersion = RulesVersion(1);

    fn create(_: &Nothing, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        // Enough keys that a coincidental order match is negligible.
        let items = (0..32u32).map(|i| (i, i * 7)).collect();
        Ok(Init {
            state: Bag { items },
            events: SmallVec::new(),
            effects: SmallVec::new(),
        })
    }

    fn apply(_: &mut Bag, _: Input<Nothing>, _: &mut Ctx<'_>) -> Result<Outcome<Self>, RuleError> {
        Ok(Outcome::empty())
    }

    fn project(_: &Bag, _: Viewer) -> Nothing {
        Nothing
    }
    fn view_event(_: &Bag, _: &Nothing, _: Viewer) -> Option<Nothing> {
        None
    }
}

#[test]
fn harness_catches_a_hashmap_in_canonical_state() {
    let scenario = Scenario::<HashMapInState> {
        config: Nothing,
        roster: roster(),
        seed: MatchSeed::from_bytes([2u8; 32]),
        inputs: inputs(Nothing, 2),
    };

    assert_harness_rejects("I-2", || {
        assert_deterministic::<HashMapInState>(&scenario);
    });
}

// ---------------------------------------------------------------------------
// I-2: nondeterministic event ordering
// ---------------------------------------------------------------------------

/// Emits events by iterating a `HashMap`. The state is perfectly deterministic —
/// only the *event order* varies, which a state-hash-only check would miss
/// entirely. This is why `assert_deterministic` compares the encoded event
/// stream separately (R7).
struct UnorderedEvents;

impl GameRules for UnorderedEvents {
    type State = Nothing;
    type Command = Nothing;
    type Event = u32;
    type View = Nothing;
    type ViewEvent = Nothing;
    type Config = Nothing;

    const RULES_VERSION: RulesVersion = RulesVersion(1);

    fn create(_: &Nothing, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
        Ok(Init {
            state: Nothing,
            events: SmallVec::new(),
            effects: SmallVec::new(),
        })
    }

    fn apply(
        _: &mut Nothing,
        _: Input<Nothing>,
        _: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError> {
        let bag: HashMap<u32, u32> = (0..32u32).map(|i| (i, i)).collect();
        Ok(Outcome {
            events: bag.values().copied().collect(), // <-- the bug: unordered iteration
            effects: SmallVec::new(),
        })
    }

    fn project(_: &Nothing, _: Viewer) -> Nothing {
        Nothing
    }
    fn view_event(_: &Nothing, _: &u32, _: Viewer) -> Option<Nothing> {
        None
    }
}

#[test]
fn harness_catches_unordered_event_emission() {
    let scenario = Scenario::<UnorderedEvents> {
        config: Nothing,
        roster: roster(),
        seed: MatchSeed::from_bytes([3u8; 32]),
        inputs: inputs(Nothing, 2),
    };

    assert_harness_rejects("I-2", || {
        assert_deterministic::<UnorderedEvents>(&scenario);
    });
}
