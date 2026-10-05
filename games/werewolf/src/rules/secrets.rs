//! Verification-only containment model for whole private structures (I-5/I-6).
//!
//! Canonical encoding has a two-byte version prefix; nested field tokens omit
//! that prefix. A complete role map is specific enough at every supported seat
//! count. Individual Role, SeatId, potion flags, and short action/report records
//! are too small for a trustworthy byte-substring oracle, so they deliberately
//! have zero token coverage. The independent visibility/noninterference tests
//! in `tests/security.rs` cover those scalar and event-existence gaps.
//!
//! Retained canonical history is modeled only when it contains a private fact.
//! This catches forwarding that whole structure, without asserting that a raw
//! canonical event record has the same encoding as its projected ViewEvent.

use tabula_core::{canonical_encode, Viewer};
use tabula_testkit::{Secret, SecretModel};

use super::{Event, Phase, State, WerewolfRules};

fn nested_token<T: serde::Serialize>(value: &T) -> Vec<u8> {
    canonical_encode(value)
        .expect("Werewolf verification data encodes")
        .into_iter()
        .skip(2)
        .collect()
}

fn full_viewers(state: &State) -> Vec<Viewer> {
    let mut authorized = vec![Viewer::Audit];
    authorized.extend(
        state
            .roster()
            .iter()
            .copied()
            .filter(|seat| !state.alive().contains(seat))
            .map(Viewer::Seat),
    );
    authorized
}

impl SecretModel for WerewolfRules {
    fn secrets(state: &State) -> Vec<Secret> {
        let mut secrets = Vec::new();
        if state.phase() != Phase::Ended {
            secrets.push(Secret::authorized(
                "whole secret role assignment",
                vec![nested_token(state.roles())],
                full_viewers(state),
            ));
        }
        if state.history.iter().any(|event| {
            matches!(
                event,
                Event::NightActionSubmitted { .. }
                    | Event::SeerReport { .. }
                    | Event::NightResolved { .. }
                    | Event::HunterTriggered { .. }
            )
        }) {
            secrets.push(Secret::authorized(
                "complete canonical history containing private actions",
                vec![nested_token(&state.history)],
                full_viewers(state),
            ));
        }
        secrets
    }

    fn event_secrets(_state_after: &State, event: &Event) -> Vec<Secret> {
        match event {
            // Assignment is audit-only forever, even after public role reveal.
            Event::RolesAssigned { roles } => vec![Secret::authorized(
                "audit-only initial role-assignment event",
                vec![nested_token(roles)],
                vec![Viewer::Audit],
            )],
            // These records have only short scalar tokens. Their exact None
            // policy is tested independently rather than giving false coverage.
            Event::NightActionSubmitted { .. }
            | Event::SeerReport { .. }
            | Event::NightResolved { .. }
            | Event::HunterTriggered { .. }
            | Event::PhaseChanged { .. }
            | Event::BallotChanged { .. }
            | Event::VoteResolved { .. }
            | Event::DeathRevealed { .. }
            | Event::SeatStatusChanged { .. }
            | Event::MatchEnded { .. } => Vec::new(),
        }
    }
}
