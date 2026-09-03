//! The hidden-information security suite — a small, **opt-in** layer on top
//! of [`GameTestFixture`], separate from [`crate::conformance!`]. (doc 00
//! §4.2, doc 02 §7.3, ADR-005)
//!
//! # Why this is not part of `conformance!`
//!
//! [`GameTestFixture`] is deliberately generic over every game, hidden
//! information or not: Chess and `TicTacToe` must not need a fake
//! [`SecretModel`] just to satisfy a macro (this PR's items 6 and 19). A
//! game's [`SecretModel`] only exists to answer one question —
//! *what is secret, and to whom* — and only hidden-information games can
//! answer it honestly. [`HiddenInformationFixture`] adds exactly the one
//! fact a security scan needs beyond what [`GameTestFixture`] already
//! supplies (the exact spectator viewers for a
//! `SpectatorPolicy::GameControlled` game — see [`client_viewer_universe`]),
//! and [`check`] (exposed to game authors as [`crate::projection_security!`])
//! drives the two containment oracles and the module-declared-capability
//! consistency check in [`crate::projection`] over a real reachable trace.
//!
//! # What "the viewer universe" means here, and why it cannot be guessed
//!
//! A [`SecretModel`] only names who is *authorized* for a given secret. It
//! says nothing about who *else exists* to be unauthorized — a `SecretModel`
//! that only ever mentions seat 0 and seat 1 gives a generic scanner no way
//! to learn that seat 2 is also seated at the table, so a scanner that
//! derived its viewer list from `Secret::authorized` alone would never
//! construct `Viewer::Seat(2)` and would happily pass a leak straight to it.
//! [`client_viewer_universe`] instead derives the complete list from trusted
//! match/game facts that do not depend on any one `Secret`'s author
//! remembering every seat: [`GameTestFixture::roster`] (every seat, always)
//! and `GameModule::capabilities().spectators()` (the declared spectator
//! policy). See `tests/projection_noninterference.rs` for a fixture that
//! leaks to an unlisted seat and a test proving this derivation catches it
//! anyway.

use tabula_core::{SpectatorTier, Viewer};
use tabula_game_api::{GameModule, SpectatorPolicy};

use crate::conformance::{scenario, GameTestFixture, RulesOf};
use crate::determinism::run_typed_trace;
use crate::projection::{assert_no_event_bypasses_redaction, assert_no_leaks, SecretModel};

/// Additional facts a [`GameTestFixture`] must supply to receive the
/// hidden-information security suite, on top of what [`GameTestFixture`]
/// already provides.
///
/// Bound on `RulesOf<Self>: SecretModel` rather than a separate associated
/// type: the fixture already names `Self::Module`, and its rules either
/// implement [`SecretModel`] or they do not — there is no second way for a
/// fixture to say "this game has hidden information" that could drift from
/// the first.
pub trait HiddenInformationFixture: GameTestFixture
where
    RulesOf<Self>: SecretModel,
{
    /// The exact spectator [`Viewer`]s to scan when
    /// `GameModule::capabilities().spectators()` is
    /// [`SpectatorPolicy::GameControlled`] — the one spectator policy whose
    /// concrete viewer set the platform, not the game, decides case by case
    /// (doc 02 §4.2: "the game's `project(Spectator)` decides; platform
    /// allows attach"), so it cannot be derived generically the way `Live`
    /// and `Delayed { by }` can.
    ///
    /// The default `None` is a deliberate "not supplied" signal, not an
    /// empty viewer set: [`client_viewer_universe`] panics rather than
    /// silently scanning zero game-controlled spectators when the policy is
    /// `GameControlled` and this returns `None` (this PR's item 4 — "do not
    /// silently skip `GameControlled`").
    fn game_controlled_spectators() -> Option<Vec<Viewer>> {
        None
    }
}

/// Derive the complete set of real client [`Viewer`]s this game's match can
/// be attached by: every roster seat, plus whatever
/// `GameModule::capabilities().spectators()` allows. Never includes
/// [`Viewer::Audit`] — see [`crate::projection`]'s module docs on why Audit
/// must never be scanned as an unauthorized client.
///
/// # Panics
/// If the module declares `SpectatorPolicy::GameControlled` and
/// `F::game_controlled_spectators()` returns `None` — the concrete
/// spectator viewers cannot be inferred, so this fails loudly rather than
/// silently scanning none.
pub fn client_viewer_universe<F>() -> Vec<Viewer>
where
    F: HiddenInformationFixture,
    RulesOf<F>: SecretModel,
{
    let mut viewers: Vec<Viewer> = F::roster()
        .iter()
        .map(|entry| Viewer::Seat(entry.seat))
        .collect();

    match <F::Module as GameModule>::capabilities().spectators() {
        SpectatorPolicy::Forbidden => {}
        SpectatorPolicy::Live => viewers.push(Viewer::Spectator(SpectatorTier::Live)),
        SpectatorPolicy::Delayed { by } => {
            viewers.push(Viewer::Spectator(SpectatorTier::Delayed { by }));
        }
        SpectatorPolicy::GameControlled => {
            let extra = F::game_controlled_spectators().unwrap_or_else(|| {
                panic!(
                    "{} declares SpectatorPolicy::GameControlled but does not override \
                     HiddenInformationFixture::game_controlled_spectators() — the concrete \
                     spectator viewers cannot be inferred generically (doc 02 §4.2 leaves that \
                     decision to the game); name them explicitly.",
                    core::any::type_name::<F>()
                )
            });
            viewers.extend(extra);
        }
    }

    viewers
}

/// Run the mandatory hidden-information security suite for `F`. Exposed to
/// game authors as [`crate::projection_security!`].
///
/// For every step of the reachable trace produced by replaying
/// [`GameTestFixture::deterministic_script`] through `create`/`apply` (see
/// `crate::determinism::run_typed_trace` — `create`'s own step, then one per
/// *accepted* input; a rejected input contributes nothing, by contracts R2
/// and R8, which is the existing determinism harness's job to enforce, not
/// this suite's), scans that step's resulting state and emitted events with
/// both containment oracles from [`crate::projection`], against
/// [`client_viewer_universe`].
///
/// # Panics
/// - If `F::Module`'s declared capabilities say `hidden_information ==
///   false` — a fixture claiming this suite for a module that is not
///   declared to have hidden information is a fixture/module mismatch, not a
///   passing result (this PR's item 19).
/// - If `F::deterministic_script` is not legal for `F`'s config/roster/seed.
/// - If any reachable state's `project` or `view_event` leaks a [`Secret`](crate::projection::Secret)'s
///   tokens to an unauthorized [`Viewer`] — see [`assert_no_leaks`] and
///   [`assert_no_event_bypasses_redaction`] for the exact oracle and
///   diagnostic shape.
pub fn check<F>()
where
    F: HiddenInformationFixture,
    RulesOf<F>: SecretModel,
{
    assert!(
        <F::Module as GameModule>::capabilities().hidden_information(),
        "{} implements HiddenInformationFixture over {}, but {}'s declared capabilities say \
         hidden_information = false. Either the module under-declares its capabilities, or this \
         fixture does not belong on it — a hidden-information security suite over a module that \
         disclaims hidden information proves nothing.",
        core::any::type_name::<F>(),
        core::any::type_name::<F::Module>(),
        core::any::type_name::<F::Module>(),
    );

    let viewers = client_viewer_universe::<F>();
    let script: Vec<_> = F::deterministic_script();
    let trace = run_typed_trace::<RulesOf<F>>(&scenario::<F>(script))
        .expect("HiddenInformationFixture::deterministic_script must be legal for this fixture");

    for (step_index, step) in trace.iter().enumerate() {
        let case = format!("{} step {step_index}", core::any::type_name::<F>());
        assert_no_leaks::<RulesOf<F>>(&case, &step.state, &viewers);
        assert_no_event_bypasses_redaction::<RulesOf<F>>(
            &case,
            &step.state,
            &step.events,
            &viewers,
        );
    }
}

/// Expand the mandatory hidden-information security suite for a
/// [`HiddenInformationFixture`].
///
/// ```rust,ignore
/// tabula_testkit::projection_security!(CardsFixture);
/// ```
///
/// Deliberately **not** part of [`crate::conformance!`] — see this module's
/// docs for why a perfect-information game must never be asked to satisfy
/// it.
#[macro_export]
macro_rules! projection_security {
    ($fixture:ty) => {
        #[test]
        fn tabula_hidden_information_security_suite() {
            $crate::conformance::security::check::<$fixture>();
        }
    };
}
