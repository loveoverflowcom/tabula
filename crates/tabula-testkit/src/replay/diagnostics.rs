//! Evidence-strength classification for canonical replay verification.

use tabula_core::InputIndex;
use tabula_game_api::GameRules;

use super::{
    CheckpointEvidence, Divergence, DivergenceKind, ReplayDraft, ReplayRunner, ReplayScope,
    VerifyReport,
};

/// The strongest localization claim supported by replay evidence.
///
/// The exact and window variants are deliberately backed by private
/// constructors. Callers can inspect an established location, but cannot
/// assemble an `Exact` claim without the adjacent verified checkpoint that
/// establishes it.
///
/// (doc 05 §7.3)
///
/// @ai.role verifier
/// @ai.domain replay.diagnosis
/// @ai.invariant divergence-location-never-overclaims-evidence
/// @ai.evidence crate::replay::tests::exact_checkpoint_diagnosis_is_adjacent
/// @ai.evidence crate::replay::tests::sparse_checkpoint_diagnosis_is_a_window
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DivergenceLocation {
    /// The failing checkpoint immediately follows a matching checkpoint.
    Exact(ExactDivergence),
    /// The first failing claim is separated from the nearest verified boundary
    /// by one or more inputs whose state was not stored.
    Window(DivergenceWindow),
    /// Only a final replay claim is known to disagree; no transition is
    /// identified as the first divergent transition.
    FinalEvidenceOnly(FinalEvidenceOnly),
}

/// An exact divergence location established by adjacent checkpoint evidence
/// (doc 05 §7.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactDivergence {
    input_index: InputIndex,
    previous_verified: InputIndex,
}

impl ExactDivergence {
    /// The input whose transition is established as divergent.
    #[must_use]
    pub const fn input_index(&self) -> InputIndex {
        self.input_index
    }

    /// The immediately preceding checkpoint that matched.
    #[must_use]
    pub const fn previous_verified(&self) -> InputIndex {
        self.previous_verified
    }
}

/// A bounded location based on a verified lower checkpoint and a later failing
/// stored claim. The first divergent transition may be anywhere in this
/// interval (doc 05 §7.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DivergenceWindow {
    after_verified: Option<InputIndex>,
    at_or_before: InputIndex,
    first_failing_evidence: InputIndex,
}

impl DivergenceWindow {
    /// The nearest earlier checkpoint whose recomputed hash matched, if one
    /// exists.
    #[must_use]
    pub const fn after_verified(&self) -> Option<InputIndex> {
        self.after_verified
    }

    /// The upper bound established by the first failing stored checkpoint.
    #[must_use]
    pub const fn at_or_before(&self) -> InputIndex {
        self.at_or_before
    }

    /// The input index of the first failing stored checkpoint claim.
    #[must_use]
    pub const fn first_failing_evidence(&self) -> InputIndex {
        self.first_failing_evidence
    }
}

/// A final hash or terminal-outcome claim without an exact transition
/// localization (doc 05 §7.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalEvidenceOnly {
    after_verified: Option<InputIndex>,
    final_input: Option<InputIndex>,
}

impl FinalEvidenceOnly {
    /// The nearest earlier matching checkpoint, if one exists.
    #[must_use]
    pub const fn after_verified(&self) -> Option<InputIndex> {
        self.after_verified
    }

    /// The last input covered by the final claim, or `None` for a create-only
    /// replay.
    #[must_use]
    pub const fn final_input(&self) -> Option<InputIndex> {
        self.final_input
    }
}

/// The evidence category of one replay diagnosis (doc 05 §7.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayDiagnosisKind {
    /// A stored state checkpoint disagrees with the recomputed state hash.
    CheckpointState,
    /// The final trailer hash disagrees, while all stored checkpoints match.
    FinalStateHashOnly,
    /// The derived terminal outcome disagrees with the stored outcome claim.
    TerminalOutcome,
}

impl core::fmt::Display for ReplayDiagnosisKind {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match self {
            Self::CheckpointState => "CHECKPOINT_STATE",
            Self::FinalStateHashOnly => "FINAL_STATE_HASH_ONLY",
            Self::TerminalOutcome => "TERMINAL_OUTCOME",
        };
        formatter.write_str(name)
    }
}

/// A divergence together with the strongest location claim justified by all
/// stored checkpoint evidence (doc 05 §7.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayDiagnosis {
    replay_scope: ReplayScope,
    kind: ReplayDiagnosisKind,
    location: DivergenceLocation,
    evidence: Divergence,
}

impl ReplayDiagnosis {
    /// The category of evidence that failed.
    #[must_use]
    pub const fn kind(&self) -> ReplayDiagnosisKind {
        self.kind
    }

    /// The evidence-backed location claim.
    #[must_use]
    pub const fn location(&self) -> &DivergenceLocation {
        &self.location
    }

    /// The first failing stored claim represented by this diagnosis.
    #[must_use]
    pub const fn evidence(&self) -> &Divergence {
        &self.evidence
    }
}

/// Why a smaller canonical replay cannot be derived from the available
/// evidence (doc 05 §7.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReproducerReason {
    /// A checkpoint mismatch is required to define a safe prefix boundary.
    CheckpointEvidenceRequired,
    /// The diagnosis refers to a checkpoint that is not present in this
    /// runner's validated replay.
    CheckpointClaimUnavailable,
    /// The candidate prefix failed the replay container validation.
    DerivedPrefixInvalid,
    /// Re-executing the candidate prefix did not yield a valid canonical run.
    PrefixExecutionFailed,
}

impl core::fmt::Display for ReproducerReason {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let message = match self {
            Self::CheckpointEvidenceRequired => {
                "a checkpoint mismatch is required to derive a replay prefix"
            }
            Self::CheckpointClaimUnavailable => {
                "the diagnosed checkpoint is not present in this replay"
            }
            Self::DerivedPrefixInvalid => "the derived prefix failed replay validation",
            Self::PrefixExecutionFailed => "the derived prefix could not be executed",
        };
        formatter.write_str(message)
    }
}

/// Result of attempting to derive a smaller canonical replay (doc 05 §7.3).
#[derive(Clone, Debug)]
pub enum ReproducerAvailability {
    /// A validated replay prefix that preserves the failing checkpoint claim.
    Available(Box<super::ValidatedReplay>),
    /// The failing evidence is on the final frame, so removing frames cannot
    /// produce a smaller replay that contains that claim.
    OriginalReplayIsMinimal,
    /// No honest smaller replay can be derived from this evidence.
    InsufficientEvidence { reason: ReproducerReason },
}

impl VerifyReport {
    /// Classify the first relevant failure in each evidence category.
    ///
    /// Checkpoint claims are scanned in execution order. No monotonicity is
    /// assumed: a mismatch may be followed by a later matching checkpoint.
    /// When the failing checkpoint is not adjacent to a matching checkpoint,
    /// the result is a [`DivergenceLocation::Window`], never an exact first
    /// transition claim.
    ///
    /// @ai.role verifier
    /// @ai.domain replay.diagnosis
    /// @ai.pure true
    /// @ai.invariant divergence-location-never-overclaims-evidence
    /// @ai.law diagnosis-is-deterministic
    /// @ai.evidence crate::replay::tests::exact_checkpoint_diagnosis_is_adjacent
    /// @ai.evidence crate::replay::tests::sparse_checkpoint_diagnosis_is_a_window
    /// @ai.evidence crate::replay::tests::checkpoint_reconvergence_does_not_restore_monotonicity
    /// @ai.evidence crate::replay::tests::final_hash_only_diagnosis_is_final_evidence
    pub fn diagnoses(&self) -> Vec<ReplayDiagnosis> {
        let first_checkpoint = self
            .checkpoint_evidence
            .iter()
            .find(|claim| !claim.matched());
        let mut diagnoses = Vec::new();

        if let Some(claim) = first_checkpoint {
            if let Some(evidence) = self.divergences.iter().find(|divergence| {
                divergence.kind == DivergenceKind::Checkpoint
                    && divergence.input_index == claim.input_index.0
            }) {
                let location = checkpoint_location(&self.checkpoint_evidence, claim);
                diagnoses.push(ReplayDiagnosis {
                    replay_scope: self.replay_scope.clone(),
                    kind: ReplayDiagnosisKind::CheckpointState,
                    location,
                    evidence: evidence.clone(),
                });
            }
        } else if let Some(evidence) = self
            .divergences
            .iter()
            .find(|divergence| divergence.kind == DivergenceKind::FinalStateHash)
        {
            diagnoses.push(ReplayDiagnosis {
                replay_scope: self.replay_scope.clone(),
                kind: ReplayDiagnosisKind::FinalStateHashOnly,
                location: final_evidence_location(self),
                evidence: evidence.clone(),
            });
        }

        if let Some(evidence) = self
            .divergences
            .iter()
            .find(|divergence| divergence.kind == DivergenceKind::TerminalOutcome)
        {
            diagnoses.push(ReplayDiagnosis {
                replay_scope: self.replay_scope.clone(),
                kind: ReplayDiagnosisKind::TerminalOutcome,
                location: final_evidence_location(self),
                evidence: evidence.clone(),
            });
        }

        diagnoses
    }

    /// Return the first diagnosis, if any. A checkpoint diagnosis takes
    /// precedence over a derived final-hash discrepancy because it is the
    /// stronger state evidence.
    #[must_use]
    pub fn diagnosis(&self) -> Option<ReplayDiagnosis> {
        self.diagnoses().into_iter().next()
    }
}

impl<R: GameRules> ReplayRunner<R> {
    /// Derive a validated prefix for a checkpoint diagnosis without touching
    /// the filesystem. The prefix keeps frames through the first failing
    /// checkpoint and uses that claim as its final-state evidence, so running
    /// it reproduces the relevant checkpoint failure at the same input.
    ///
    /// @ai.role verifier
    /// @ai.domain replay.reproducer
    /// @ai.pure true
    /// @ai.invariant derived-replay-preserves-container-invariants
    /// @ai.law reproducer-preserves-failing-checkpoint
    /// @ai.evidence crate::replay::tests::checkpoint_reproducer_validates_and_reproduces
    /// @ai.evidence crate::replay::tests::non_checkpoint_reproducer_is_not_derived
    pub fn reproducer(&self, diagnosis: &ReplayDiagnosis) -> ReproducerAvailability {
        if diagnosis.kind != ReplayDiagnosisKind::CheckpointState {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointEvidenceRequired,
            };
        }
        if diagnosis.evidence.rules_version != self.replay.header.rules_version
            || diagnosis.evidence.rules_hash != self.replay.header.rules_hash
        {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointClaimUnavailable,
            };
        }
        let Ok(replay_scope) = self.replay.diagnosis_scope() else {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointClaimUnavailable,
            };
        };
        if diagnosis.replay_scope != replay_scope {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointClaimUnavailable,
            };
        }

        let target = match &diagnosis.location {
            DivergenceLocation::Exact(exact) => exact.input_index,
            DivergenceLocation::Window(window) => window.at_or_before,
            DivergenceLocation::FinalEvidenceOnly(_) => {
                return ReproducerAvailability::InsufficientEvidence {
                    reason: ReproducerReason::CheckpointEvidenceRequired,
                };
            }
        };
        let Some((position, frame)) = self
            .replay
            .frames
            .iter()
            .enumerate()
            .find(|(_, frame)| frame.input_index == target)
        else {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointClaimUnavailable,
            };
        };
        let Some(expected) = frame.checkpoint else {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointClaimUnavailable,
            };
        };
        if expected.0 != diagnosis.evidence.expected {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointClaimUnavailable,
            };
        }
        let Ok(mut prefix_runner) =
            Self::from_validated(self.replay.clone(), self.identity.clone())
        else {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::PrefixExecutionFailed,
            };
        };
        let mut actual_at_target = None;
        for _ in 0..=position {
            match prefix_runner.step() {
                Ok(Some(result)) => actual_at_target = Some(result.state_hash),
                Ok(None) | Err(_) => {
                    return ReproducerAvailability::InsufficientEvidence {
                        reason: ReproducerReason::PrefixExecutionFailed,
                    };
                }
            }
        }
        if actual_at_target != Some(tabula_core::StateHash(diagnosis.evidence.actual)) {
            return ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointClaimUnavailable,
            };
        }
        if position + 1 == self.replay.frames.len() {
            return ReproducerAvailability::OriginalReplayIsMinimal;
        }

        let mut header = self.replay.header.clone();
        header.duration_ms = frame.logical_time.0;
        header.outcome = prefix_runner.derived_outcome;
        let draft = ReplayDraft {
            header,
            frames: self.replay.frames[..=position].to_vec(),
            final_state_hash: expected,
        };
        match draft.validate() {
            Ok(replay) => ReproducerAvailability::Available(Box::new(replay)),
            Err(_) => ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::DerivedPrefixInvalid,
            },
        }
    }
}

fn checkpoint_location(
    checkpoints: &[CheckpointEvidence],
    failing: &CheckpointEvidence,
) -> DivergenceLocation {
    let previous_verified = checkpoints
        .iter()
        .filter(|claim| claim.input_index() < failing.input_index())
        .rev()
        .find(|claim| claim.matched())
        .map(CheckpointEvidence::input_index);
    let adjacent = previous_verified.is_some_and(|previous| {
        previous
            .0
            .checked_add(1)
            .is_some_and(|next| next == failing.input_index().0)
    });

    if let Some(previous_verified) = previous_verified.filter(|_| adjacent) {
        DivergenceLocation::Exact(ExactDivergence {
            input_index: failing.input_index(),
            previous_verified,
        })
    } else {
        DivergenceLocation::Window(DivergenceWindow {
            after_verified: previous_verified,
            at_or_before: failing.input_index(),
            first_failing_evidence: failing.input_index(),
        })
    }
}

fn final_evidence_location(report: &VerifyReport) -> DivergenceLocation {
    let after_verified = report
        .checkpoint_evidence
        .iter()
        .rev()
        .find(|claim| claim.matched())
        .map(CheckpointEvidence::input_index);
    let final_input =
        (report.inputs_replayed() > 0).then_some(InputIndex(report.inputs_replayed()));
    DivergenceLocation::FinalEvidenceOnly(FinalEvidenceOnly {
        after_verified,
        final_input,
    })
}
