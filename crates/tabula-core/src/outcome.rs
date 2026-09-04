//! How a match ended. (doc 02 §2)
//!
//! ## The one rule
//!
//! **Games never compute ratings, currency, or rewards** (ADR-024, doc 00 §6.3).
//! A game emits a trustworthy, structured [`MatchOutcome`]; the platform's rating
//! service consumes it. That is what keeps ladder integrity uniform across games
//! that have nothing else in common.
//!
//! `standings` must cover every expected seat exactly once with contiguous ranks
//! starting at 0. An outcome's serialized form contains only game-authored
//! facts; the authoritative [`SeatRoster`] must be supplied again at each
//! platform boundary that consumes standings.

use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::{ids::SeatId, seat::SeatRoster};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchOutcome")]
pub struct MatchOutcome {
    kind: OutcomeKind,

    /// Full ordering, needed by the rating system. Rank 0 = winner; ties share a
    /// rank. Placement-rated games use the whole ordering, not just the winner.
    standings: SmallVec<[Standing; 8]>,

    /// Free-form, game-defined summary for UI: "checkmate", "3 wolves remain".
    ///
    /// Human-facing text, so it must be an i18n key or public-safe — never a
    /// carrier for hidden information.
    summary: CompactString,
}

#[derive(Serialize, Deserialize)]
struct RawMatchOutcome {
    kind: OutcomeKind,
    standings: SmallVec<[Standing; 8]>,
    summary: CompactString,
}

/// Why a proposed outcome cannot safely enter the rating boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MatchOutcomeError {
    #[error("aborted outcomes must not contain standings")]
    AbortedHasStandings,
    #[error("seat {seat:?} occurs more than once in standings")]
    DuplicateSeat { seat: SeatId },
    #[error("standings ranks must begin at 0 and contain no gaps")]
    NonContiguousRanks,
    #[error("standings do not cover expected seat {seat:?}")]
    MissingSeat { seat: SeatId },
    #[error("standings contain seat {seat:?}, which is not in the match roster")]
    UnexpectedSeat { seat: SeatId },
    #[error("expected roster contains seat {seat:?} more than once")]
    DuplicateExpectedSeat { seat: SeatId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    pub seat: SeatId,
    pub rank: u8,
    pub score: i64,
}

/// A [`MatchOutcome`] checked against the platform-owned roster that gave it
/// meaning. Consumers such as ratings should require this scoped witness rather
/// than trusting standings restored from a game snapshot alone.
#[derive(Clone, Copy, Debug)]
pub struct RosterValidatedOutcome<'outcome, 'roster> {
    outcome: &'outcome MatchOutcome,
    roster: &'roster SeatRoster,
}

impl<'outcome, 'roster> RosterValidatedOutcome<'outcome, 'roster> {
    #[must_use]
    pub const fn outcome(&self) -> &'outcome MatchOutcome {
        self.outcome
    }

    #[must_use]
    pub const fn roster(&self) -> &'roster SeatRoster {
        self.roster
    }
}

impl MatchOutcome {
    /// Validates the intrinsic form of a game-authored outcome.
    ///
    /// `Aborted` is intentionally the sole exception: it carries no standings
    /// because it does not count for ratings. The architecture permits tied
    /// placements for both `Draw` and multi-seat `Decisive` outcomes; scores are
    /// game-defined and are therefore unrestricted.
    ///
    /// The outcome must still be checked with [`Self::validate_against`] before
    /// any platform consumer relies on its standings after deserialization.
    /// [`Self::new_for_seats`] lets rules reject malformed outcomes early when
    /// they hold the current match's seat identities.
    ///
    /// @ai.role proof-boundary
    /// @ai.domain match.outcome
    /// @ai.invariant roster-complete-standings
    /// @ai.invariant contiguous-outcome-ranks
    /// @ai.evidence crate::outcome::tests::outcome_constructor_partitions
    /// @ai.evidence crate::outcome::tests::outcome_deserialization_rejects_structural_forgeries
    #[allow(clippy::doc_markdown)]
    pub fn new(
        kind: OutcomeKind,
        standings: SmallVec<[Standing; 8]>,
        summary: CompactString,
    ) -> Result<Self, MatchOutcomeError> {
        Self::validate_structural(kind, &standings)?;
        Ok(Self {
            kind,
            standings,
            summary,
        })
    }

    /// Builds an outcome and verifies that its standings cover these match seats.
    ///
    /// This is the rules-side fail-fast check. It deliberately does not persist
    /// `expected_seats`: a snapshot cannot prove its own roster. Platform code
    /// must use [`Self::validate_against`] with the separately persisted roster
    /// before it consumes a restored outcome.
    pub fn new_for_seats(
        kind: OutcomeKind,
        standings: SmallVec<[Standing; 8]>,
        summary: CompactString,
        expected_seats: &[SeatId],
    ) -> Result<Self, MatchOutcomeError> {
        let outcome = Self::new(kind, standings, summary)?;
        outcome.validate_against_seats(expected_seats)?;
        Ok(outcome)
    }

    /// Checks standings with the platform's authoritative roster and returns a
    /// scoped witness for consumers at that boundary.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain match.outcome
    /// @ai.invariant roster-complete-standings
    /// @ai.evidence crate::outcome::tests::restored_outcome_requires_authoritative_roster_validation
    #[allow(clippy::doc_markdown)]
    pub fn validate_against<'outcome, 'roster>(
        &'outcome self,
        roster: &'roster SeatRoster,
    ) -> Result<RosterValidatedOutcome<'outcome, 'roster>, MatchOutcomeError> {
        let seats: SmallVec<[SeatId; 8]> = roster.iter().map(|entry| entry.seat).collect();
        self.validate_against_seats(&seats)?;
        Ok(RosterValidatedOutcome {
            outcome: self,
            roster,
        })
    }

    fn from_serialized(raw: RawMatchOutcome) -> Result<Self, MatchOutcomeError> {
        Self::new(raw.kind, raw.standings, raw.summary)
    }

    fn validate_against_seats(&self, expected_seats: &[SeatId]) -> Result<(), MatchOutcomeError> {
        for (index, expected) in expected_seats.iter().enumerate() {
            if expected_seats[..index].contains(expected) {
                return Err(MatchOutcomeError::DuplicateExpectedSeat { seat: *expected });
            }
        }
        if !matches!(self.kind, OutcomeKind::Aborted { .. }) {
            for expected in expected_seats {
                if !self
                    .standings
                    .iter()
                    .any(|standing| standing.seat == *expected)
                {
                    return Err(MatchOutcomeError::MissingSeat { seat: *expected });
                }
            }
            for standing in &self.standings {
                if !expected_seats.contains(&standing.seat) {
                    return Err(MatchOutcomeError::UnexpectedSeat {
                        seat: standing.seat,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_structural(
        kind: OutcomeKind,
        standings: &[Standing],
    ) -> Result<(), MatchOutcomeError> {
        if matches!(kind, OutcomeKind::Aborted { .. }) {
            return standings
                .is_empty()
                .then_some(())
                .ok_or(MatchOutcomeError::AbortedHasStandings);
        }

        for (index, standing) in standings.iter().enumerate() {
            if standings[..index]
                .iter()
                .any(|previous| previous.seat == standing.seat)
            {
                return Err(MatchOutcomeError::DuplicateSeat {
                    seat: standing.seat,
                });
            }
        }

        let Some(max_rank) = standings.iter().map(|standing| standing.rank).max() else {
            return Err(MatchOutcomeError::NonContiguousRanks);
        };
        if (0..=max_rank).any(|rank| !standings.iter().any(|standing| standing.rank == rank)) {
            return Err(MatchOutcomeError::NonContiguousRanks);
        }
        Ok(())
    }

    #[must_use]
    pub const fn kind(&self) -> OutcomeKind {
        self.kind
    }

    #[must_use]
    pub fn standings(&self) -> &[Standing] {
        &self.standings
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

impl TryFrom<RawMatchOutcome> for MatchOutcome {
    type Error = MatchOutcomeError;

    fn try_from(raw: RawMatchOutcome) -> Result<Self, Self::Error> {
        Self::from_serialized(raw)
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use smallvec::smallvec;

    use super::*;
    use crate::{canonical_decode, canonical_encode, Occupant, SeatEntry};

    #[derive(Serialize)]
    struct LegacyMatchOutcomeV1 {
        kind: OutcomeKind,
        standings: SmallVec<[Standing; 8]>,
        summary: CompactString,
    }

    fn roster(seats: &[SeatId]) -> SeatRoster {
        SeatRoster::new(
            seats
                .iter()
                .map(|seat| SeatEntry {
                    seat: *seat,
                    occupant: Occupant::Empty,
                    team: None,
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn outcome_constructor_partitions() {
        let seats = [SeatId(7), SeatId(42), SeatId(99)];
        let valid = MatchOutcome::new_for_seats(
            OutcomeKind::Decisive,
            smallvec![
                Standing {
                    seat: SeatId(7),
                    rank: 0,
                    score: i64::MAX
                },
                Standing {
                    seat: SeatId(42),
                    rank: 1,
                    score: i64::MIN
                },
                Standing {
                    seat: SeatId(99),
                    rank: 1,
                    score: 0
                },
            ],
            "placement".into(),
            &seats,
        );
        assert!(valid.is_ok(), "ties and unrestricted scores are valid");
        assert!(matches!(
            MatchOutcome::new_for_seats(OutcomeKind::Draw, smallvec![], "".into(), &seats),
            Err(MatchOutcomeError::NonContiguousRanks)
        ));
        assert!(matches!(
            MatchOutcome::new_for_seats(
                OutcomeKind::Draw,
                smallvec![Standing {
                    seat: SeatId(7),
                    rank: 0,
                    score: 0
                }],
                "".into(),
                &seats,
            ),
            Err(MatchOutcomeError::MissingSeat { seat: SeatId(42) })
        ));
        assert!(matches!(
            MatchOutcome::new_for_seats(
                OutcomeKind::Aborted {
                    reason: AbortReason::OperatorCancelled
                },
                smallvec![Standing {
                    seat: SeatId(7),
                    rank: 0,
                    score: 0
                }],
                "".into(),
                &seats,
            ),
            Err(MatchOutcomeError::AbortedHasStandings)
        ));
    }

    #[test]
    fn outcome_deserialization_rejects_structural_forgeries() {
        let duplicate = RawMatchOutcome {
            kind: OutcomeKind::Draw,
            standings: smallvec![
                Standing {
                    seat: SeatId(7),
                    rank: 0,
                    score: 0
                },
                Standing {
                    seat: SeatId(7),
                    rank: 1,
                    score: 0
                },
            ],
            summary: "bad".into(),
        };
        let gapped = RawMatchOutcome {
            kind: OutcomeKind::Draw,
            standings: smallvec![Standing {
                seat: SeatId(7),
                rank: 1,
                score: 0
            }],
            summary: "bad".into(),
        };
        let incomplete = RawMatchOutcome {
            kind: OutcomeKind::Draw,
            standings: smallvec![Standing {
                seat: SeatId(7),
                rank: 0,
                score: 0
            }],
            summary: "bad".into(),
        };
        let aborted = RawMatchOutcome {
            kind: OutcomeKind::Aborted {
                reason: AbortReason::PlatformFailure,
            },
            standings: smallvec![Standing {
                seat: SeatId(7),
                rank: 0,
                score: 0
            }],
            summary: "bad".into(),
        };
        for raw in [duplicate, gapped, aborted] {
            let bytes = canonical_encode(&raw).unwrap();
            assert!(canonical_decode::<MatchOutcome>(&bytes).is_err());
        }

        let bytes = canonical_encode(&incomplete).unwrap();
        let outcome = canonical_decode::<MatchOutcome>(&bytes).unwrap();
        assert!(matches!(
            outcome.validate_against(&roster(&[SeatId(7), SeatId(42)])),
            Err(MatchOutcomeError::MissingSeat { seat: SeatId(42) })
        ));
    }

    #[test]
    fn restored_outcome_requires_authoritative_roster_validation() {
        let outcome = MatchOutcome::new(
            OutcomeKind::Draw,
            smallvec![Standing {
                seat: SeatId(7),
                rank: 0,
                score: 0,
            }],
            "incomplete snapshot".into(),
        )
        .unwrap();

        let restored =
            canonical_decode::<MatchOutcome>(&canonical_encode(&outcome).unwrap()).unwrap();
        let roster = roster(&[SeatId(7)]);
        let validated = restored.validate_against(&roster).unwrap();
        assert_eq!(validated.outcome().summary(), "incomplete snapshot");
        assert_eq!(validated.roster().len(), 1);
    }

    #[test]
    fn match_outcome_encoding_remains_compatible_with_v1() {
        let standings = smallvec![Standing {
            seat: SeatId(7),
            rank: 0,
            score: 1,
        }];
        let outcome =
            MatchOutcome::new(OutcomeKind::Decisive, standings.clone(), "win".into()).unwrap();
        let legacy = LegacyMatchOutcomeV1 {
            kind: OutcomeKind::Decisive,
            standings,
            summary: "win".into(),
        };

        assert_eq!(
            canonical_encode(&outcome).unwrap(),
            canonical_encode(&legacy).unwrap()
        );
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeKind {
    Decisive,
    Draw,

    /// Ended early. **Must not count for ratings** — the rating job checks this
    /// variant, not the reason.
    Aborted {
        reason: AbortReason,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbortReason {
    NotEnoughPlayers,
    OperatorCancelled,
    PlatformFailure,

    /// A game's `apply` panicked and was caught by the runtime's `catch_unwind`
    /// (doc 01 §5.2). Always a Sev-2 bug: it violates contract R3 (`apply` never
    /// panics on any input). The process survives; that match does not.
    RulesPanic,

    TimedOutIdle,
}
