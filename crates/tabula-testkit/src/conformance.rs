//! The mandatory suite. (doc 02 §11.1)
//!
//! # The fifteen tests
//!
//! | Test | Asserts | Invariant |
//! |---|---|---|
//! | `determinism_same_inputs` | Two runs of a random input sequence produce identical state hash, events, effects | I-2 |
//! | `determinism_across_snapshot` | Snapshot mid-run, restore, continue → same final hash as an uninterrupted run | I-8 |
//! | `error_is_transactional` | Every rejected input leaves the state hash unchanged | R2 |
//! | `no_panic_on_hostile_input` | Fuzzed commands, out-of-range seats, wrong phase, timers that do not exist | R3 |
//! | `state_roundtrip` | `snapshot` → `restore` → identical hash, for random states | I-8 |
//! | `version_monotonic` | `state_version` +1 per accepted input, unchanged on rejection | I-7 |
//! | `projection_hides_secrets` | `SecretModel` scan for all unauthorised viewers, spectators included | I-5 |
//! | `view_event_never_bypasses` | Every canonical event maps through `view_event` for every viewer | I-6 |
//! | `view_event_consistency` | Folding `ViewEvent`s onto a `View` equals `project` at the new version (opt-in) | — |
//! | `bot_self_play_terminates` | 1000 bot-vs-bot matches reach a terminal state within `max_match_duration` | — |
//! | `outcome_wellformed` | Standings cover all seats exactly once; ranks contiguous from 0 | — |
//! | `manifest_matches_code` | `game.toml` == compiled metadata/capabilities | — |
//! | `golden_replays` | Committed `tests/replays/<game>/*.tbr` still reproduce their recorded hashes | I-8, I-16 |
//! | `no_forbidden_deps` | The rules feature set builds with no banned crate in the tree | I-1 |
//! | `apply_within_budget` | p99 `apply` under `capabilities.apply_budget` on the CI machine class | — |
//!
//! # Why a macro rather than a generic test function
//!
//! `#[test]` cannot be generic. The macro emits one real `#[test]` per row so a
//! failure names the invariant it broke, instead of one opaque
//! `conformance_suite` failure that could mean any of fifteen things.

/// Expand the mandatory conformance suite for a `GameModule`.
///
/// ```rust,ignore
/// tabula_testkit::conformance!(my_game::MyModule);
///
/// // Opt into the stricter client-side view folding check:
/// tabula_testkit::conformance!(my_game::MyModule, view_event_consistency);
/// ```
///
/// TODO(phase 0): implement each arm against the harnesses in
/// [`crate::determinism`], [`crate::projection`], [`crate::selfplay`], and
/// [`crate::replay`]. Build them in that order — `determinism_same_inputs` first,
/// because everything else assumes it holds.
#[macro_export]
macro_rules! conformance {
    ($module:path) => {
        $crate::conformance!($module,);
    };
    ($module:path, $($opt:ident),* $(,)?) => {
        // TODO(phase 0): emit one #[test] fn per row of the table in
        // `tabula_testkit::conformance`. Each must name the invariant in its
        // failure message — "I-5 violated: seat 1's hand appeared in the
        // spectator projection at input 37" beats "assertion failed".
        //
        // Recommended emission order (cheapest signal first):
        //   1. no_forbidden_deps        (compile-time; fails fastest)
        //   2. manifest_matches_code
        //   3. outcome_wellformed
        //   4. version_monotonic
        //   5. error_is_transactional
        //   6. determinism_same_inputs
        //   7. state_roundtrip
        //   8. determinism_across_snapshot
        //   9. no_panic_on_hostile_input (proptest)
        //  10. projection_hides_secrets  (proptest, needs SecretModel)
        //  11. view_event_never_bypasses
        //  12. bot_self_play_terminates
        //  13. golden_replays
        //  14. apply_within_budget
        //  15. view_event_consistency   (opt-in via $opt)
        const _: () = {
            let _ = stringify!($module);
            $( let _ = stringify!($opt); )*
        };
    };
}
