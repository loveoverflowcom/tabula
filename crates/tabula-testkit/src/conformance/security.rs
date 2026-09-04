//! The hidden-information security suite — a small, **opt-in** layer on top
//! of [`GameTestFixture`], separate from [`crate::conformance!`]. (doc 00
//! §4.2, doc 02 §7.3, ADR-005)
//!
//! # Why this is not part of `conformance!`
//!
//! [`GameTestFixture`] is deliberately generic over every game, hidden
//! information or not: Chess and other perfect-information games must not need a fake
//! [`SecretModel`] just to satisfy a macro. A
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
use crate::projection::{
    assert_no_event_bypasses_redaction, assert_no_leaks, EventScanCoverage, LeakScanCoverage,
    SecretModel,
};

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
    /// The exact spectator tiers to scan when
    /// `GameModule::capabilities().spectators()` is
    /// [`SpectatorPolicy::GameControlled`] — the one spectator policy whose
    /// concrete viewer set the platform, not the game, decides case by case
    /// (doc 02 §4.2: "the game's `project(Spectator)` decides; platform
    /// allows attach"), so it cannot be derived generically the way `Live`
    /// and `Delayed { by }` can.
    ///
    /// Typed as `SpectatorTier`, not `Viewer` — this hook exists to name
    /// *which spectators*, so the type should make "which seat" or "audit
    /// tooling" unrepresentable here rather than merely undesired. Every
    /// tier this returns is wrapped in `Viewer::Spectator` by
    /// [`client_viewer_universe`], which is also where a caller who really
    /// does mean to add a `Viewer::Seat`/`Viewer::Audit` beyond the roster
    /// would have to say so explicitly and separately — not silently, by
    /// slipping it through a spectator hook.
    ///
    /// The default `None` is a deliberate "not supplied" signal, not an
    /// empty tier set: [`client_viewer_universe`] panics rather than
    /// silently scanning zero game-controlled spectators when the policy is
    /// `GameControlled` and this returns `None` (this PR's item 4 — "do not
    /// silently skip `GameControlled`").
    fn game_controlled_spectators() -> Option<Vec<SpectatorTier>> {
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
            viewers.extend(extra.into_iter().map(Viewer::Spectator));
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
/// - If the reachable trace never actually exercised the properties this
///   suite claims to check — see [`SecurityCoverage`] below. A fixture whose
///   `deterministic_script` never deals a hand, or whose roster leaves no
///   unauthorized viewer to check against, must not be allowed to look like
///   a passing security suite; a green tick that means nothing is exactly
///   the failure mode `tabula-testkit`'s own conformance docs warn about.
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

    let mut coverage = SecurityCoverage::default();

    for (step_index, step) in trace.iter().enumerate() {
        let case = format!("{} step {step_index}", core::any::type_name::<F>());
        let leak_coverage = assert_no_leaks::<RulesOf<F>>(&case, &step.state, &viewers);
        let event_coverage = assert_no_event_bypasses_redaction::<RulesOf<F>>(
            &case,
            &step.state,
            &step.events,
            &viewers,
        );
        coverage.accumulate(leak_coverage, event_coverage);
    }

    coverage.assert_not_vacuous::<F>();
}

/// Evidence that a [`check`] run actually exercised the properties it
/// claims to, not just replayed an empty script against an empty
/// `SecretModel` and reported green.
///
/// This exists because passing a fixture-driven suite is not itself
/// evidence: `tabula-testkit`'s own conformance docs call out "a green tick
/// that means nothing" as a bug class in its own right, and the naive
/// version of this suite — `deterministic_script() -> vec![]`, an initial
/// state with no dealt secrets — would pass every assertion in
/// [`crate::projection`] over zero real comparisons.
#[derive(Debug, Default)]
struct SecurityCoverage {
    /// Reachable-trace steps at which `SecretModel::secrets` declared at
    /// least one secret.
    states_with_secrets: usize,
    /// (secret, viewer) pairs [`assert_no_leaks`] actually compared, across
    /// every step, because the viewer was unauthorized for that secret.
    unauthorized_projection_checks: usize,
    /// Canonical events for which `view_event` returned `Some(..)` to at
    /// least one viewer, across every step.
    canonical_events_scanned: usize,
    /// (event, viewer, secret) triples [`assert_no_event_bypasses_redaction`]
    /// actually compared, across every step, because the viewer was
    /// unauthorized for that secret.
    unauthorized_view_event_checks: usize,
}

impl SecurityCoverage {
    fn accumulate(&mut self, leaks: LeakScanCoverage, events: EventScanCoverage) {
        if leaks.secrets_declared > 0 {
            self.states_with_secrets += 1;
        }
        self.unauthorized_projection_checks += leaks.unauthorized_checks;
        self.canonical_events_scanned += events.events_with_output;
        self.unauthorized_view_event_checks += events.unauthorized_checks;
    }

    /// # Panics
    /// If any coverage counter that must be nonzero for this run to mean
    /// anything is zero. Each assertion names exactly which claim would
    /// have been vacuous, so a game author sees precisely what to add to
    /// `deterministic_script`/`SecretModel`/the roster rather than a single
    /// "coverage too low".
    fn assert_not_vacuous<F: HiddenInformationFixture>(&self)
    where
        RulesOf<F>: SecretModel,
    {
        let name = core::any::type_name::<F>();
        assert!(
            self.states_with_secrets > 0,
            "{name}'s reachable trace never reached a state where SecretModel::secrets \
             declared anything — this suite would pass on a fixture that never actually deals \
             or otherwise creates hidden information. Extend deterministic_script() to reach a \
             state with at least one real secret.",
        );
        assert!(
            self.unauthorized_projection_checks > 0,
            "{name}'s reachable trace never compared `project` against an (unauthorized viewer, \
             secret) pair — every viewer in the derived universe was authorized for every \
             declared secret, so the View containment scan ran over zero real comparisons. \
             Either the roster/spectator policy leaves no unauthorized viewer, or every secret's \
             `authorized` list is too permissive.",
        );
        assert!(
            self.canonical_events_scanned > 0,
            "{name}'s reachable trace never had `view_event` return Some(..) to any viewer for \
             any canonical event — either deterministic_script() produced no events, or every \
             `view_event` call returned None. A ViewEvent containment scan that never actually \
             saw a redacted event proves nothing about redaction correctness.",
        );
        assert!(
            self.unauthorized_view_event_checks > 0,
            "{name}'s reachable trace scanned canonical events but never compared one against \
             an (unauthorized viewer, secret) pair — every event's relevant secrets (state- or \
             event-local) were either empty or fully authorized for every viewer that received \
             output. Extend the script/SecretModel/SecretModel::event_secrets so at least one \
             event actually carries something an unauthorized viewer's redaction is checked \
             against.",
        );
    }
}

/// Expand the mandatory hidden-information security suite for a
/// [`HiddenInformationFixture`].
///
/// ```rust,ignore
/// tabula_testkit::projection_security!(WerewolfFixture);
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
