//! Canonical `.tbr` replay format and typed verifier. (doc 05 §8)
//!
//! A replay is evidence, not merely a file that decoded successfully. The
//! validated container below keeps format concerns (framing, lengths, order,
//! integrity, and canonical byte payloads) separate from the game's typed
//! `Config`, `Input`, and `State`. `ReplayRunner<R>` then reconstructs the
//! ordinary `GameRules::create`/`apply` path with the recorded logical times and
//! input indices. Its diagnostic API reports only the location strength justified
//! by the stored checkpoint evidence (doc 05 §7.3).
//!
//! @ai.role trust-boundary
//! @ai.domain replay.container
//! @ai.invariant replay-container-integrity
//! @ai.invariant projected-replay-never-carries-seed
//! @ai.invariant canonical-replay-roundtrips
//! @ai.evidence crate::replay::tests::corruption_is_rejected
//! @ai.evidence crate::replay::tests::projected_seed_is_rejected_by_writer_and_reader
//! @ai.evidence crate::replay::tests::round_trip_preserves_validated_structure
//!
//! # Logical v1 payload
//!
//! The complete logical payload is compressed as one zstd frame:
//!
//! ```text
//! TBR1 magic
//! format version: u16 little-endian
//! header length: u32 little-endian
//! header: Postcard(ReplayHeader)
//! repeated frames:
//!     frame length: u32 little-endian
//!     frame: Postcard(ReplayFrame)
//! trailer:
//!     input count: u64 little-endian
//!     final state hash: [u8; 32]
//!     CRC32 (IEEE) of every preceding logical byte
//! ```
//!
//! Canonical config and input payloads retain the kernel's
//! `ENCODING_VERSION.to_le_bytes() || postcard(value)` representation. The
//! outer framing is deliberately fixed-width and length-prefixed so the reader
//! can reject truncation and oversized declarations before allocating for a
//! frame. Replays contain only inputs that were accepted into the canonical log;
//! a replay-time rejection is therefore a divergence/corruption error.

#![allow(clippy::doc_markdown)]

use std::{
    fmt, fs,
    io::{Cursor, Read},
    path::Path,
};

use crc32fast::Hasher as Crc32Hasher;
use serde::{Deserialize, Serialize};
use tabula_core::{
    canonical_decode, GameId, GameVersion, InputIndex, LogicalTime, MatchId, MatchOutcome,
    MatchSeed, RulesVersion, SeatRoster, StateHash, StateVersion, ENCODING_VERSION,
};
use tabula_game_api::{Budget, Ctx, Effect, GameModule, GameRules, Input};

mod diagnostics;

pub use diagnostics::{
    DivergenceLocation, DivergenceWindow, ExactDivergence, FinalEvidenceOnly, ReplayDiagnosis,
    ReplayDiagnosisKind, ReproducerAvailability, ReproducerReason,
};

/// The only currently readable replay format version.
pub const REPLAY_FORMAT_VERSION: u16 = 1;
const MAGIC: &[u8; 4] = b"TBR1";
const TRAILER_LEN: usize = 8 + 32 + 4;

/// Maximum logical bytes held after decompression.
pub const MAX_DECOMPRESSED_REPLAY_BYTES: usize = 4 * 1024 * 1024;
/// Maximum compressed bytes accepted by the shell-facing reader.
pub const MAX_COMPRESSED_REPLAY_BYTES: usize = 4 * 1024 * 1024;
/// Maximum encoded header. This includes all header fields, not the framing prefix.
pub const MAX_HEADER_BYTES: usize = 128 * 1024;
/// Maximum canonical payload in one frame.
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
/// Maximum number of input frames in one offline replay.
pub const MAX_FRAME_COUNT: usize = 100_000;
/// Maximum canonical configuration payload.
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;
/// Maximum zstd back-reference window accepted by the decoder (4 MiB).
const MAX_ZSTD_WINDOW_SIZE: u64 = 1 << 22;

/// Whether a replay carries canonical authority or a viewer's redacted stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayKind {
    /// Seed plus accepted canonical inputs. Never distribute to players.
    Canonical,
    /// A viewer's projected initial data and opaque view-event frames.
    Projected { viewer: tabula_core::Viewer },
}

/// The metadata portion of the locked v1 replay model.
///
/// This is a raw draft value. Callers must pass it through [`ReplayDraft::validate`]
/// before it becomes a [`ValidatedReplay`]. Public fields are intentional here:
/// this is the construction-side DTO, not a proof-bearing domain value.
#[derive(Clone, Serialize, Deserialize)]
pub struct ReplayHeader {
    pub match_id: MatchId,
    pub game_id: GameId,
    pub game_version: GameVersion,
    pub rules_version: RulesVersion,
    pub rules_hash: [u8; 32],
    pub config: Vec<u8>,
    pub roster: SeatRoster,
    pub seed: Option<MatchSeed>,
    pub initial_snapshot: Option<Vec<u8>>,
    pub started_at: u64,
    pub duration_ms: u64,
    pub outcome: Option<MatchOutcome>,
    pub kind: ReplayKind,
}

impl fmt::Debug for ReplayHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplayHeader")
            .field("match_id", &self.match_id)
            .field("game_id", &self.game_id)
            .field("game_version", &self.game_version)
            .field("rules_version", &self.rules_version)
            .field("rules_hash", &self.rules_hash)
            .field("config_bytes", &self.config.len())
            .field("roster_seats", &self.roster.len())
            .field("seed", &self.seed.as_ref().map(|_| "<present>"))
            .field(
                "initial_snapshot_bytes",
                &self.initial_snapshot.as_ref().map(Vec::len),
            )
            .field("started_at", &self.started_at)
            .field("duration_ms", &self.duration_ms)
            .field("outcome", &self.outcome)
            .field("kind", &self.kind)
            .finish()
    }
}

/// One raw v1 frame. In a canonical replay, `input` is canonical
/// `Input<Command>` bytes. In a projected replay it is opaque projected event
/// bytes; projected playback is intentionally deferred to the client phase.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayFrame {
    pub input_index: InputIndex,
    pub logical_time: LogicalTime,
    pub input: Vec<u8>,
    pub checkpoint: Option<StateHash>,
}

/// Unvalidated replay construction DTO.
#[derive(Clone, Debug)]
pub struct ReplayDraft {
    pub header: ReplayHeader,
    pub frames: Vec<ReplayFrame>,
    pub final_state_hash: StateHash,
}

/// A replay whose framing, limits, ordering, canonical payload markers, and
/// canonical/projected invariants have all been checked.
pub struct ValidatedReplay {
    header: ReplayHeader,
    frames: Vec<ReplayFrame>,
    final_state_hash: StateHash,
}

/// An internal fingerprint that scopes diagnostic evidence to one validated
/// replay. It is intentionally not exposed: its only purpose is to prevent a
/// diagnosis from authorizing work on a different replay with the same rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ReplayScope([u8; 32]);

impl Clone for ValidatedReplay {
    fn clone(&self) -> Self {
        Self {
            header: self.header.clone(),
            frames: self.frames.clone(),
            final_state_hash: self.final_state_hash,
        }
    }
}

impl fmt::Debug for ValidatedReplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedReplay")
            .field("header", &self.header)
            .field("frames", &self.frames.len())
            .field("final_state_hash", &self.final_state_hash)
            .finish()
    }
}

impl ReplayDraft {
    /// Validate a raw construction value without serializing it.
    ///
    /// @ai.role proof-boundary
    /// @ai.domain replay.validate
    /// @ai.invariant projected-replay-never-carries-seed
    /// @ai.invariant canonical-replay-has-reconstruction-data
    /// @ai.evidence crate::replay::tests::projected_seed_is_rejected_by_writer_and_reader
    /// @ai.evidence crate::replay::tests::canonical_replay_requires_seed
    pub fn validate(self) -> Result<ValidatedReplay, ReplayError> {
        validate_parts(self.header, self.frames, self.final_state_hash)
    }

    /// Encode a validated v1 replay deterministically.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ReplayError> {
        self.clone().validate()?.to_bytes()
    }
}

impl ValidatedReplay {
    /// Read and validate a hostile `.tbr` byte stream.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain replay.decode
    /// @ai.invariant replay-container-integrity
    /// @ai.law canonical-replay-roundtrips
    /// @ai.evidence crate::replay::tests::corruption_is_rejected
    /// @ai.evidence crate::replay::tests::round_trip_preserves_validated_structure
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ReplayError> {
        if bytes.len() > MAX_COMPRESSED_REPLAY_BYTES {
            return Err(ReplayError::Corrupt(
                "compressed replay exceeds the configured limit".to_owned(),
            ));
        }

        let mut decoder = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(
            Cursor::new(bytes),
            MAX_ZSTD_WINDOW_SIZE,
        )
        .map_err(|_| ReplayError::Corrupt("invalid zstd frame".to_owned()))?;
        let mut limited = (&mut decoder).take((MAX_DECOMPRESSED_REPLAY_BYTES as u64) + 1);
        let mut payload = Vec::new();
        limited
            .read_to_end(&mut payload)
            .map_err(|_| ReplayError::Corrupt("truncated compressed stream".to_owned()))?;
        let cursor = decoder.into_inner();
        if cursor.position() != bytes.len() as u64 {
            return Err(ReplayError::Corrupt(
                "replay must contain exactly one zstd frame".to_owned(),
            ));
        }
        if payload.len() > MAX_DECOMPRESSED_REPLAY_BYTES {
            return Err(ReplayError::Corrupt(
                "decompressed replay exceeds the configured limit".to_owned(),
            ));
        }

        parse_payload(&payload)
    }

    /// Read a replay through the filesystem shell.
    pub fn read(path: &Path) -> Result<Self, ReplayError> {
        let file = fs::File::open(path).map_err(ReplayError::Io)?;
        let mut bytes = Vec::new();
        file.take((MAX_COMPRESSED_REPLAY_BYTES as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(ReplayError::Io)?;
        if bytes.len() > MAX_COMPRESSED_REPLAY_BYTES {
            return Err(ReplayError::Corrupt(
                "compressed replay exceeds the configured limit".to_owned(),
            ));
        }
        Self::from_bytes(&bytes)
    }

    /// Re-encode the validated value. The output is deterministic for the same
    /// validated structure and pinned compression settings.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ReplayError> {
        let header = postcard::to_allocvec(&self.header)
            .map_err(|_| ReplayError::Corrupt("header is not encodable".to_owned()))?;
        if header.len() > MAX_HEADER_BYTES {
            return Err(ReplayError::Corrupt(
                "header exceeds the configured limit".to_owned(),
            ));
        }

        let mut payload = Vec::with_capacity(16 + header.len() + self.frames.len() * 16);
        payload.extend_from_slice(MAGIC);
        payload.extend_from_slice(&REPLAY_FORMAT_VERSION.to_le_bytes());
        let header_len = u32::try_from(header.len())
            .map_err(|_| ReplayError::Corrupt("header length does not fit framing".to_owned()))?;
        payload.extend_from_slice(&header_len.to_le_bytes());
        payload.extend_from_slice(&header);

        for frame in &self.frames {
            let encoded = postcard::to_allocvec(frame)
                .map_err(|_| ReplayError::Corrupt("frame is not encodable".to_owned()))?;
            if encoded.len() > MAX_FRAME_BYTES {
                return Err(ReplayError::Corrupt(
                    "frame exceeds the configured limit".to_owned(),
                ));
            }
            let frame_len = u32::try_from(encoded.len()).map_err(|_| {
                ReplayError::Corrupt("frame length does not fit framing".to_owned())
            })?;
            payload.extend_from_slice(&frame_len.to_le_bytes());
            payload.extend_from_slice(&encoded);
        }

        payload.extend_from_slice(&(self.frames.len() as u64).to_le_bytes());
        payload.extend_from_slice(&self.final_state_hash.0);
        let checksum = crc32(&payload);
        payload.extend_from_slice(&checksum.to_le_bytes());

        if payload.len() > MAX_DECOMPRESSED_REPLAY_BYTES {
            return Err(ReplayError::Corrupt(
                "replay exceeds the configured decompressed limit".to_owned(),
            ));
        }
        Ok(ruzstd::encoding::compress_to_vec(
            payload.as_slice(),
            ruzstd::encoding::CompressionLevel::Uncompressed,
        ))
    }

    fn diagnosis_scope(&self) -> Result<ReplayScope, ReplayError> {
        let bytes = self.to_bytes()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"tabula.replay.diagnosis.scope.v1");
        hasher.update(&bytes);
        Ok(ReplayScope(*hasher.finalize().as_bytes()))
    }

    #[must_use]
    pub fn header(&self) -> &ReplayHeader {
        &self.header
    }

    #[must_use]
    pub fn frames(&self) -> &[ReplayFrame] {
        &self.frames
    }

    #[must_use]
    pub fn final_state_hash(&self) -> StateHash {
        self.final_state_hash
    }
}

/// The rules identity supplied by a game-specific boundary.
#[derive(Clone, PartialEq, Eq)]
pub struct ReplayIdentity {
    pub game_id: GameId,
    pub rules_version: RulesVersion,
    pub rules_hash: [u8; 32],
}

impl fmt::Debug for ReplayIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplayIdentity")
            .field("game_id", &self.game_id)
            .field("rules_version", &self.rules_version)
            .field("rules_hash", &self.rules_hash)
            .finish()
    }
}

impl ReplayIdentity {
    /// Build the identity from a real game module, including its build-derived
    /// rules-half hash. A zero hash is left visible to [`ReplayRunner::check`]
    /// as unreplayable rather than being promoted to an exact verdict.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain replay.identity
    #[must_use]
    pub fn from_module<M: GameModule>() -> Self {
        Self {
            game_id: M::metadata().id().clone(),
            rules_version: M::Rules::RULES_VERSION,
            rules_hash: M::rules_hash(),
        }
    }
}

/// Replays a canonical `.tbr` through an ordinary `GameRules` implementation.
///
/// The type is generic over rules rather than over the file format. The format
/// stores opaque canonical bytes; only this typed boundary decodes `Config` and
/// `Input<R::Command>`. This keeps game-specific types out of the container.
pub struct ReplayRunner<R: GameRules> {
    replay: ValidatedReplay,
    identity: ReplayIdentity,
    config: R::Config,
    inputs: Vec<Input<R::Command>>,
    state: R::State,
    state_version: StateVersion,
    next_frame: usize,
    derived_outcome: Option<MatchOutcome>,
    terminal_input_index: Option<u64>,
}

impl<R: GameRules> fmt::Debug for ReplayRunner<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplayRunner")
            .field("game_id", &self.replay.header.game_id)
            .field("rules_version", &self.replay.header.rules_version)
            .field("frames", &self.replay.frames.len())
            .field("next_frame", &self.next_frame)
            .field("state_version", &self.state_version)
            .finish_non_exhaustive()
    }
}

impl<R: GameRules> ReplayRunner<R> {
    /// Open a canonical replay and create its initial state through
    /// `GameRules::create`.
    pub fn open(path: &Path, identity: ReplayIdentity) -> Result<Self, ReplayError> {
        Self::from_validated(ValidatedReplay::read(path)?, identity)
    }

    /// Open a canonical replay from bytes without filesystem access.
    pub fn from_bytes(bytes: &[u8], identity: ReplayIdentity) -> Result<Self, ReplayError> {
        Self::from_validated(ValidatedReplay::from_bytes(bytes)?, identity)
    }

    #[must_use]
    pub fn header(&self) -> &ReplayHeader {
        self.replay.header()
    }

    fn from_validated(
        replay: ValidatedReplay,
        identity: ReplayIdentity,
    ) -> Result<Self, ReplayError> {
        if replay.header.kind != ReplayKind::Canonical {
            return Err(ReplayError::UnsupportedReplayKind);
        }
        if replay.header.game_id != identity.game_id {
            return Err(ReplayError::Unreplayable(
                "replay game identity does not match the selected typed runner".to_owned(),
            ));
        }
        if replay.header.rules_version != identity.rules_version {
            return Err(ReplayError::Unreplayable(format!(
                "replay rules version {} is not linked to the selected implementation {}",
                replay.header.rules_version.0, identity.rules_version.0
            )));
        }

        let config = canonical_decode::<R::Config>(&replay.header.config)
            .map_err(|_| ReplayError::Corrupt("canonical config cannot be decoded".to_owned()))?;
        let inputs = replay
            .frames
            .iter()
            .map(|frame| {
                canonical_decode::<Input<R::Command>>(&frame.input).map_err(|_| {
                    ReplayError::Corrupt("canonical input cannot be decoded".to_owned())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let created =
            create_state::<R>(&config, &replay.header.roster, replay.header.seed.as_ref())?;
        let terminal_input_index = created.terminal_outcome.as_ref().map(|_| 0);
        Ok(Self {
            replay,
            identity,
            config,
            inputs,
            state: created.state,
            state_version: StateVersion(0),
            next_frame: 0,
            derived_outcome: created.terminal_outcome,
            terminal_input_index,
        })
    }

    /// Decide whether the selected implementation is an exact linked rules
    /// build before spending time executing the replay.
    ///
    /// @ai.role compatibility-check
    /// @ai.domain replay.identity
    /// @ai.invariant differing-rules-hash-is-compatible-version
    /// @ai.evidence tests::same_rules_version_with_different_hash_is_compatible_version
    #[must_use]
    pub fn check(&self) -> ReplayVerdict {
        if self.replay.header.rules_hash == [0; 32] || self.identity.rules_hash == [0; 32] {
            return ReplayVerdict::Unreplayable {
                reason: "no authoritative rules hash is available".to_owned(),
            };
        }
        if self.replay.header.rules_hash == self.identity.rules_hash {
            ReplayVerdict::Exact
        } else {
            ReplayVerdict::CompatibleVersion
        }
    }

    /// Apply the next accepted input and compare its optional checkpoint.
    ///
    /// An unexpected rejection is an error, never a successful step: canonical
    /// replay frames represent inputs that the live log accepted.
    pub fn step(&mut self) -> Result<Option<StepResult>, ReplayError> {
        let Some(frame) = self.replay.frames.get(self.next_frame) else {
            return Ok(None);
        };
        if let Some(terminal_input_index) = self.terminal_input_index {
            return Err(ReplayError::InputAfterEndMatch {
                input_index: frame.input_index.0,
                terminal_input_index,
            });
        }
        let input = self
            .inputs
            .get(self.next_frame)
            .ok_or_else(|| ReplayError::Corrupt("frame/input count mismatch".to_owned()))?;
        let seed = self
            .replay
            .header
            .seed
            .as_ref()
            .ok_or(ReplayError::CanonicalSeedMissing)?;
        let mut rng = tabula_core::DetRng::for_input(seed, frame.input_index);
        let mut ctx = Ctx {
            now: frame.logical_time,
            index: frame.input_index,
            rng: &mut rng,
            budget: Budget {
                max_apply_micros: u32::MAX,
                max_events_per_input: u16::MAX,
            },
        };
        let outcome = R::apply(&mut self.state, input.clone(), &mut ctx).map_err(|error| {
            ReplayError::UnexpectedRejection {
                input_index: frame.input_index.0,
                code: error.code,
            }
        })?;
        let terminal = terminal_outcome(&outcome.effects, frame.input_index.0)?;

        self.state_version = StateVersion(
            self.state_version
                .0
                .checked_add(1)
                .ok_or_else(|| ReplayError::Corrupt("state version overflow".to_owned()))?,
        );
        self.next_frame += 1;
        if let Some(outcome) = terminal {
            self.terminal_input_index = Some(frame.input_index.0);
            self.derived_outcome = Some(outcome);
        }
        let actual = R::state_hash(&self.state);
        let checkpoint_matched = frame.checkpoint.map(|expected| expected == actual);
        Ok(Some(StepResult {
            input_index: frame.input_index.0,
            logical_time: frame.logical_time,
            state_version: self.state_version,
            state_hash: actual,
            checkpoint: frame.checkpoint,
            checkpoint_matched,
        }))
    }

    /// Re-run from the initial state through the requested accepted-input
    /// state version. State versions count replay frames, because rejected
    /// inputs are not part of a canonical replay.
    ///
    /// @ai.role verifier
    /// @ai.domain replay.seek
    /// @ai.invariant verified-seek-never-hides-divergence
    /// @ai.evidence crate::replay::tests::seek_never_hides_checkpoint_divergence
    pub fn seek(&mut self, to: StateVersion) -> Result<PrefixPosition, ReplayError> {
        if to.0 > self.replay.frames.len() as u64 {
            return Err(ReplayError::SeekOutOfRange(to));
        }
        self.reset()?;
        while self.state_version < to {
            let result = self.step()?.ok_or_else(|| {
                ReplayError::Corrupt("replay ended before the requested state version".to_owned())
            })?;
            if result.checkpoint_matched == Some(false) {
                return Err(ReplayError::PrefixDivergence {
                    divergence: Box::new(self.checkpoint_divergence(&result)),
                });
            }
        }
        let replay_end = StateVersion(self.replay.frames.len() as u64);
        let final_hash_checked = self.state_version == replay_end;
        let state_hash = R::state_hash(&self.state);
        if final_hash_checked && state_hash != self.replay.final_state_hash {
            return Err(ReplayError::PrefixDivergence {
                divergence: Box::new(self.final_hash_divergence(state_hash)),
            });
        }
        if self.state_version == replay_end && self.derived_outcome != self.replay.header.outcome {
            return Err(ReplayError::PrefixDivergence {
                divergence: Box::new(self.terminal_divergence()),
            });
        }
        let checkpoints_checked = self.replay.frames[..self.next_frame]
            .iter()
            .filter(|frame| frame.checkpoint.is_some())
            .count() as u64;
        let position = PositionEvidence {
            state_version: self.state_version,
            state_hash,
            checkpoints_checked,
            terminal_outcome: self.derived_outcome.clone(),
            outcome_checked: self.state_version == replay_end,
            final_hash_checked,
        };
        if checkpoints_checked == 0 {
            Ok(PrefixPosition::Reconstructed(position))
        } else {
            Ok(PrefixPosition::Verified(position))
        }
    }

    /// Re-run every frame, compare every claimed checkpoint and compare the
    /// trailer's final hash. A divergence is data, not an early `Err`, so the
    /// caller receives the full report. Decode, framing, and unexpected
    /// rejection failures remain errors.
    ///
    /// @ai.role verifier
    /// @ai.domain replay.verify
    /// @ai.pure false
    /// @ai.invariant replay-reproduces-checkpoint-hashes
    /// @ai.invariant replay-terminal-outcome-matches
    /// @ai.invariant first-divergence-is-diagnosable
    /// @ai.evidence crate::replay::tests::checkpoint_mismatch_reports_context
    /// @ai.evidence crate::replay::tests::final_hash_mismatch_fails_verification
    /// @ai.evidence crate::replay::tests::replay_terminal_outcome_matches_effect
    /// @ai.evidence crate::replay::tests::end_match_from_create_is_verified
    pub fn verify(&mut self) -> Result<VerifyReport, ReplayError> {
        self.reset()?;
        let verdict = self.check();
        if matches!(verdict, ReplayVerdict::Unreplayable { .. }) {
            return Err(ReplayError::Unreplayable(format!("{verdict:?}")));
        }

        let mut report = VerifyReport {
            replay_scope: self.replay.diagnosis_scope()?,
            verdict,
            expected_final_state_hash: self.replay.final_state_hash,
            actual_final_state_hash: StateHash([0; 32]),
            final_hash_checked: false,
            expected_outcome: self.replay.header.outcome.clone(),
            actual_outcome: None,
            outcome_checked: false,
            inputs_replayed: 0,
            checkpoints_checked: 0,
            checkpoint_evidence: Vec::new(),
            divergences: Vec::new(),
        };

        while let Some(result) = self.step()? {
            report.inputs_replayed += 1;
            if let Some(expected) = result.checkpoint {
                report.checkpoints_checked += 1;
                report.checkpoint_evidence.push(CheckpointEvidence {
                    input_index: InputIndex(result.input_index),
                    expected,
                    actual: result.state_hash,
                });
                if result.checkpoint_matched != Some(true) {
                    report.divergences.push(Divergence {
                        kind: DivergenceKind::Checkpoint,
                        input_index: result.input_index,
                        logical_time: Some(result.logical_time),
                        expected: expected.0,
                        actual: result.state_hash.0,
                        rules_version: self.replay.header.rules_version,
                        rules_hash: self.replay.header.rules_hash,
                        previous_checkpoint: previous_checkpoint(
                            &self.replay.frames,
                            self.next_frame.saturating_sub(1),
                        ),
                        next_checkpoint: next_checkpoint(&self.replay.frames, self.next_frame),
                        expected_outcome: None,
                        actual_outcome: None,
                    });
                }
            }
        }

        report.actual_final_state_hash = R::state_hash(&self.state);
        report.final_hash_checked = true;
        if report.actual_final_state_hash != report.expected_final_state_hash {
            report
                .divergences
                .push(self.final_hash_divergence(report.actual_final_state_hash));
        }

        report.actual_outcome.clone_from(&self.derived_outcome);
        report.outcome_checked = true;
        if report.actual_outcome != report.expected_outcome {
            report.divergences.push(self.terminal_divergence());
        }

        Ok(report)
    }

    fn reset(&mut self) -> Result<(), ReplayError> {
        let created = create_state::<R>(
            &self.config,
            &self.replay.header.roster,
            self.replay.header.seed.as_ref(),
        )?;
        self.state = created.state;
        self.state_version = StateVersion(0);
        self.next_frame = 0;
        self.derived_outcome = created.terminal_outcome;
        self.terminal_input_index = self.derived_outcome.as_ref().map(|_| 0);
        Ok(())
    }

    fn checkpoint_divergence(&self, result: &StepResult) -> Divergence {
        let expected = result.checkpoint.unwrap_or(StateHash([0; 32]));
        Divergence {
            kind: DivergenceKind::Checkpoint,
            input_index: result.input_index,
            logical_time: Some(result.logical_time),
            expected: expected.0,
            actual: result.state_hash.0,
            rules_version: self.replay.header.rules_version,
            rules_hash: self.replay.header.rules_hash,
            previous_checkpoint: previous_checkpoint(
                &self.replay.frames,
                self.next_frame.saturating_sub(1),
            ),
            next_checkpoint: next_checkpoint(&self.replay.frames, self.next_frame),
            expected_outcome: None,
            actual_outcome: None,
        }
    }

    fn terminal_divergence(&self) -> Divergence {
        let final_frame = self.replay.frames.last();
        Divergence {
            kind: DivergenceKind::TerminalOutcome,
            input_index: final_frame.map_or(0, |frame| frame.input_index.0),
            logical_time: final_frame.map(|frame| frame.logical_time),
            expected: outcome_fingerprint(self.replay.header.outcome.as_ref()),
            actual: outcome_fingerprint(self.derived_outcome.as_ref()),
            rules_version: self.replay.header.rules_version,
            rules_hash: self.replay.header.rules_hash,
            previous_checkpoint: self
                .replay
                .frames
                .iter()
                .rev()
                .find_map(|frame| frame.checkpoint.map(|_| frame.input_index.0)),
            next_checkpoint: None,
            expected_outcome: self.replay.header.outcome.clone(),
            actual_outcome: self.derived_outcome.clone(),
        }
    }

    fn final_hash_divergence(&self, actual: StateHash) -> Divergence {
        let final_frame = self.replay.frames.last();
        Divergence {
            kind: DivergenceKind::FinalStateHash,
            input_index: final_frame.map_or(0, |frame| frame.input_index.0),
            logical_time: final_frame.map(|frame| frame.logical_time),
            expected: self.replay.final_state_hash.0,
            actual: actual.0,
            rules_version: self.replay.header.rules_version,
            rules_hash: self.replay.header.rules_hash,
            previous_checkpoint: self
                .replay
                .frames
                .iter()
                .rev()
                .find_map(|frame| frame.checkpoint.map(|_| frame.input_index.0)),
            next_checkpoint: None,
            expected_outcome: None,
            actual_outcome: None,
        }
    }
}

/// Evidence returned by a prefix replay. Checkpoint-bearing prefixes are
/// verified against the claims they contain; prefixes without such claims are
/// explicitly reconstructed rather than promoted to verified positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrefixPosition {
    Verified(PositionEvidence),
    Reconstructed(PositionEvidence),
}

/// State and evidence at a requested replay prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionEvidence {
    pub state_version: StateVersion,
    pub state_hash: StateHash,
    pub checkpoints_checked: u64,
    pub terminal_outcome: Option<MatchOutcome>,
    pub outcome_checked: bool,
    pub final_hash_checked: bool,
}

fn terminal_outcome(
    effects: &[Effect],
    input_index: u64,
) -> Result<Option<MatchOutcome>, ReplayError> {
    let mut terminal = None;
    for effect in effects {
        if let Effect::EndMatch { outcome } = effect {
            if terminal.is_some() {
                return Err(ReplayError::MultipleEndMatch { input_index });
            }
            terminal = Some(outcome.clone());
        }
    }
    Ok(terminal)
}

fn outcome_fingerprint(outcome: Option<&MatchOutcome>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.replay.outcome.v1");
    if let Some(outcome) = outcome {
        if let Ok(bytes) = tabula_core::canonical_encode(outcome) {
            hasher.update(&bytes);
        }
    }
    *hasher.finalize().as_bytes()
}

struct CreatedState<R: GameRules> {
    state: R::State,
    terminal_outcome: Option<MatchOutcome>,
}

fn create_state<R: GameRules>(
    config: &R::Config,
    roster: &SeatRoster,
    seed: Option<&MatchSeed>,
) -> Result<CreatedState<R>, ReplayError> {
    let seed = seed.ok_or(ReplayError::CanonicalSeedMissing)?;
    let mut rng = tabula_core::DetRng::for_input(seed, InputIndex(0));
    let mut ctx = Ctx {
        now: LogicalTime::ZERO,
        index: InputIndex(0),
        rng: &mut rng,
        budget: Budget {
            max_apply_micros: u32::MAX,
            max_events_per_input: u16::MAX,
        },
    };
    let init = R::create(config, roster, &mut ctx)
        .map_err(|error| ReplayError::CreateRejected(format!("{error:?}")))?;
    let terminal_outcome = terminal_outcome(&init.effects, 0)?;
    Ok(CreatedState {
        state: init.state,
        terminal_outcome,
    })
}

fn validate_parts(
    header: ReplayHeader,
    frames: Vec<ReplayFrame>,
    final_state_hash: StateHash,
) -> Result<ValidatedReplay, ReplayError> {
    if header.config.is_empty() || header.config.len() > MAX_CONFIG_BYTES {
        return Err(ReplayError::Corrupt(
            "config is empty or exceeds the configured limit".to_owned(),
        ));
    }
    validate_canonical_marker(&header.config, "config")?;
    if header.rules_hash == [0; 32] {
        return Err(ReplayError::Corrupt(
            "rules hash must be an authoritative non-zero identity".to_owned(),
        ));
    }

    match &header.kind {
        ReplayKind::Canonical => {
            if header.seed.is_none() {
                return Err(ReplayError::CanonicalSeedMissing);
            }
            if header.initial_snapshot.is_some() {
                return Err(ReplayError::Corrupt(
                    "canonical replay must not carry an initial snapshot".to_owned(),
                ));
            }
        }
        ReplayKind::Projected { .. } => {
            if header.seed.is_some() {
                return Err(ReplayError::ProjectedContainsSeed);
            }
            if header.initial_snapshot.as_ref().is_none_or(Vec::is_empty) {
                return Err(ReplayError::Corrupt(
                    "projected replay requires initial projected data".to_owned(),
                ));
            }
        }
    }
    if header
        .initial_snapshot
        .as_ref()
        .is_some_and(|bytes| bytes.len() > MAX_FRAME_BYTES)
    {
        return Err(ReplayError::Corrupt(
            "initial snapshot exceeds the configured limit".to_owned(),
        ));
    }
    if frames.len() > MAX_FRAME_COUNT {
        return Err(ReplayError::Corrupt(
            "frame count exceeds the configured limit".to_owned(),
        ));
    }

    let mut previous_time = LogicalTime::ZERO;
    for (position, frame) in frames.iter().enumerate() {
        let expected_index = InputIndex(
            u64::try_from(position + 1)
                .map_err(|_| ReplayError::Corrupt("frame index overflow".to_owned()))?,
        );
        if frame.input_index != expected_index {
            return Err(ReplayError::Corrupt(
                "frame input indices must be contiguous starting at one".to_owned(),
            ));
        }
        if frame.logical_time < previous_time {
            return Err(ReplayError::Corrupt(
                "frame logical times must be monotonic".to_owned(),
            ));
        }
        if frame.input.is_empty() || frame.input.len() > MAX_FRAME_BYTES {
            return Err(ReplayError::Corrupt(
                "frame payload is empty or exceeds the configured limit".to_owned(),
            ));
        }
        if matches!(header.kind, ReplayKind::Canonical) {
            validate_canonical_marker(&frame.input, "input")?;
        }
        previous_time = frame.logical_time;
    }

    Ok(ValidatedReplay {
        header,
        frames,
        final_state_hash,
    })
}

fn validate_canonical_marker(bytes: &[u8], name: &'static str) -> Result<(), ReplayError> {
    if bytes.len() < 2 || bytes[..2] != ENCODING_VERSION.to_le_bytes() {
        return Err(ReplayError::Corrupt(format!(
            "{name} is not encoded with canonical encoding version {ENCODING_VERSION}"
        )));
    }
    Ok(())
}

fn parse_payload(payload: &[u8]) -> Result<ValidatedReplay, ReplayError> {
    if payload.len() < MAGIC.len() || &payload[..MAGIC.len()] != MAGIC {
        return Err(ReplayError::BadMagic);
    }
    if payload.len() < MAGIC.len() + 2 + 4 + TRAILER_LEN {
        return Err(ReplayError::Corrupt("truncated replay payload".to_owned()));
    }

    let version = u16::from_le_bytes([payload[4], payload[5]]);
    if version > REPLAY_FORMAT_VERSION {
        return Err(ReplayError::FormatTooNew(version));
    }
    if version < REPLAY_FORMAT_VERSION {
        return Err(ReplayError::FormatTooOld(version));
    }

    let header_len = u32::from_le_bytes([payload[6], payload[7], payload[8], payload[9]]) as usize;
    if header_len > MAX_HEADER_BYTES {
        return Err(ReplayError::Corrupt(
            "declared header exceeds the configured limit".to_owned(),
        ));
    }
    let header_start: usize = 10;
    let header_end = header_start
        .checked_add(header_len)
        .ok_or_else(|| ReplayError::Corrupt("header length overflows".to_owned()))?;
    let body_end = payload
        .len()
        .checked_sub(TRAILER_LEN)
        .ok_or_else(|| ReplayError::Corrupt("truncated replay trailer".to_owned()))?;
    if header_end > body_end {
        return Err(ReplayError::Corrupt("truncated replay header".to_owned()));
    }
    // Scan only fixed-width framing before the CRC. Header/frame Postcard bytes
    // and all trailer metadata remain untrusted until the complete logical
    // payload has passed the integrity check.
    let frame_ranges = scan_frame_ranges(payload, header_end, body_end)?;
    let checksum_start = payload.len() - 4;
    let expected_checksum = u32::from_le_bytes(
        payload[checksum_start..checksum_start + 4]
            .try_into()
            .map_err(|_| ReplayError::Corrupt("truncated checksum".to_owned()))?,
    );
    let actual_checksum = crc32(&payload[..checksum_start]);
    if actual_checksum != expected_checksum {
        return Err(ReplayError::ChecksumMismatch);
    }

    let header = postcard::from_bytes::<ReplayHeader>(&payload[header_start..header_end])
        .map_err(|_| ReplayError::Corrupt("header is not valid Postcard".to_owned()))?;
    let frames = frame_ranges
        .into_iter()
        .map(|range| {
            postcard::from_bytes::<ReplayFrame>(&payload[range])
                .map_err(|_| ReplayError::Corrupt("frame is not valid Postcard".to_owned()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let input_count_start = body_end;
    let input_count = u64::from_le_bytes(
        payload[input_count_start..input_count_start + 8]
            .try_into()
            .map_err(|_| ReplayError::Corrupt("truncated input count".to_owned()))?,
    );
    let final_hash_start = input_count_start + 8;
    let final_hash = StateHash(
        payload[final_hash_start..final_hash_start + 32]
            .try_into()
            .map_err(|_| ReplayError::Corrupt("truncated final hash".to_owned()))?,
    );
    if input_count != frames.len() as u64 {
        return Err(ReplayError::InputCountMismatch {
            declared: input_count,
            actual: frames.len(),
        });
    }
    validate_parts(header, frames, final_hash)
}

fn scan_frame_ranges(
    payload: &[u8],
    header_end: usize,
    body_end: usize,
) -> Result<Vec<std::ops::Range<usize>>, ReplayError> {
    let mut ranges = Vec::new();
    let mut cursor = header_end;
    while cursor < body_end {
        if body_end - cursor < 4 {
            return Err(ReplayError::Corrupt("truncated frame length".to_owned()));
        }
        let frame_len = u32::from_le_bytes([
            payload[cursor],
            payload[cursor + 1],
            payload[cursor + 2],
            payload[cursor + 3],
        ]) as usize;
        cursor += 4;
        if frame_len > MAX_FRAME_BYTES {
            return Err(ReplayError::Corrupt(
                "declared frame exceeds the configured limit".to_owned(),
            ));
        }
        let frame_end = cursor
            .checked_add(frame_len)
            .ok_or_else(|| ReplayError::Corrupt("frame length overflows".to_owned()))?;
        if frame_end > body_end {
            return Err(ReplayError::Corrupt("truncated frame".to_owned()));
        }
        ranges.push(cursor..frame_end);
        if ranges.len() > MAX_FRAME_COUNT {
            return Err(ReplayError::Corrupt(
                "frame count exceeds the configured limit".to_owned(),
            ));
        }
        cursor = frame_end;
    }
    Ok(ranges)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut hasher = Crc32Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

fn previous_checkpoint(frames: &[ReplayFrame], before: usize) -> Option<u64> {
    frames[..before]
        .iter()
        .rev()
        .find_map(|frame| frame.checkpoint.map(|_| frame.input_index.0))
}

fn next_checkpoint(frames: &[ReplayFrame], after: usize) -> Option<u64> {
    frames[after..]
        .iter()
        .find_map(|frame| frame.checkpoint.map(|_| frame.input_index.0))
}

/// Can this replay be trusted against the selected rules implementation?
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayVerdict {
    /// The authoritative game rules hash is linked and equal.
    Exact,
    /// The rules version is linked but the identity differs; hashes must still
    /// be compared, alongside state checkpoints and terminal outcome; a
    /// mismatch is a divergence.
    CompatibleVersion,
    /// Reserved for future snapshot migration support. Phase 1 does not fake it.
    NeedsMigration { from: RulesVersion },
    /// No honest typed reconstruction is available.
    Unreplayable { reason: String },
}

/// Result of one accepted replay input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepResult {
    pub input_index: u64,
    pub logical_time: LogicalTime,
    pub state_version: StateVersion,
    pub state_hash: StateHash,
    pub checkpoint: Option<StateHash>,
    pub checkpoint_matched: Option<bool>,
}

/// What kind of evidence diverged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DivergenceKind {
    Checkpoint,
    FinalStateHash,
    TerminalOutcome,
}

/// A localized verification discrepancy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub kind: DivergenceKind,
    pub input_index: u64,
    pub logical_time: Option<LogicalTime>,
    pub expected: [u8; 32],
    pub actual: [u8; 32],
    pub rules_version: RulesVersion,
    pub rules_hash: [u8; 32],
    /// The nearest earlier claimed checkpoint, if any.
    pub previous_checkpoint: Option<u64>,
    /// The nearest later claimed checkpoint, if any.
    pub next_checkpoint: Option<u64>,
    /// The expected terminal effect, when this is a terminal-outcome divergence.
    pub expected_outcome: Option<MatchOutcome>,
    /// The terminal effect derived by replay, when this is a terminal-outcome divergence.
    pub actual_outcome: Option<MatchOutcome>,
}

/// One stored checkpoint claim and the state hash produced at that input.
/// This is raw verification evidence; callers should use [`VerifyReport::diagnoses`]
/// for a proof-strength classification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointEvidence {
    input_index: InputIndex,
    expected: StateHash,
    actual: StateHash,
}

impl CheckpointEvidence {
    /// The stored checkpoint's input index.
    #[must_use]
    pub const fn input_index(&self) -> InputIndex {
        self.input_index
    }

    /// The state hash stored in the replay at this checkpoint.
    #[must_use]
    pub const fn expected(&self) -> StateHash {
        self.expected
    }

    /// The state hash recomputed by verification at this checkpoint.
    #[must_use]
    pub const fn actual(&self) -> StateHash {
        self.actual
    }

    /// Whether the stored and recomputed hashes agree.
    #[must_use]
    pub fn matched(&self) -> bool {
        self.expected == self.actual
    }
}

/// Complete verification evidence. `is_verified()` is intentionally stricter
/// than "the file parsed": every checkpoint, final trailer hash, and terminal
/// outcome must have been compared, with no divergence. Checkpoint claims are
/// retained in execution order so diagnostics can distinguish a first failing
/// claim from a later reconvergence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyReport {
    replay_scope: ReplayScope,
    verdict: ReplayVerdict,
    expected_final_state_hash: StateHash,
    actual_final_state_hash: StateHash,
    final_hash_checked: bool,
    expected_outcome: Option<MatchOutcome>,
    actual_outcome: Option<MatchOutcome>,
    outcome_checked: bool,
    inputs_replayed: u64,
    checkpoints_checked: u64,
    checkpoint_evidence: Vec<CheckpointEvidence>,
    divergences: Vec<Divergence>,
}

impl VerifyReport {
    /// The rules compatibility verdict established before replay execution.
    #[must_use]
    pub const fn verdict(&self) -> &ReplayVerdict {
        &self.verdict
    }

    /// The final hash stored in the replay trailer.
    #[must_use]
    pub const fn expected_final_state_hash(&self) -> StateHash {
        self.expected_final_state_hash
    }

    /// The final hash recomputed during verification.
    #[must_use]
    pub const fn actual_final_state_hash(&self) -> StateHash {
        self.actual_final_state_hash
    }

    /// Whether the replay's final hash was compared.
    #[must_use]
    pub const fn final_hash_checked(&self) -> bool {
        self.final_hash_checked
    }

    /// The terminal outcome claimed by the replay.
    #[must_use]
    pub fn expected_outcome(&self) -> Option<&MatchOutcome> {
        self.expected_outcome.as_ref()
    }

    /// The terminal outcome derived during verification.
    #[must_use]
    pub fn actual_outcome(&self) -> Option<&MatchOutcome> {
        self.actual_outcome.as_ref()
    }

    /// Whether the terminal outcome was compared.
    #[must_use]
    pub const fn outcome_checked(&self) -> bool {
        self.outcome_checked
    }

    /// Number of canonical inputs executed during verification.
    #[must_use]
    pub const fn inputs_replayed(&self) -> u64 {
        self.inputs_replayed
    }

    /// Number of stored checkpoints compared during verification.
    #[must_use]
    pub const fn checkpoints_checked(&self) -> u64 {
        self.checkpoints_checked
    }

    /// Read-only stored checkpoint evidence in execution order.
    #[must_use]
    pub fn checkpoint_evidence(&self) -> &[CheckpointEvidence] {
        &self.checkpoint_evidence
    }

    /// Read-only discrepancies found during verification.
    #[must_use]
    pub fn divergences(&self) -> &[Divergence] {
        &self.divergences
    }

    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.final_hash_checked && self.outcome_checked && self.divergences.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("not a .tbr file (bad magic)")]
    BadMagic,
    #[error("format version {0} is newer than this reader supports")]
    FormatTooNew(u16),
    #[error("format version {0} is older than this reader supports")]
    FormatTooOld(u16),
    #[error("corrupt replay: {0}")]
    Corrupt(String),
    #[error("canonical replay is missing its MatchSeed")]
    CanonicalSeedMissing,
    #[error("projected replay must not contain a MatchSeed")]
    ProjectedContainsSeed,
    #[error("projected replay execution is not supported by the canonical runner")]
    UnsupportedReplayKind,
    #[error("replay is unreplayable: {0}")]
    Unreplayable(String),
    #[error("replay input {input_index} was rejected during verification: {code:?}")]
    UnexpectedRejection {
        input_index: u64,
        code: tabula_core::RuleErrorCode,
    },
    #[error("game create rejected the replay header: {0}")]
    CreateRejected(String),
    #[error("replay input count mismatch: declared {declared}, decoded {actual}")]
    InputCountMismatch { declared: u64, actual: usize },
    #[error("replay checksum mismatch")]
    ChecksumMismatch,
    #[error("replay prefix diverged: {divergence:?}")]
    PrefixDivergence { divergence: Box<Divergence> },
    #[error("replay input {input_index} emitted EndMatch more than once")]
    MultipleEndMatch { input_index: u64 },
    #[error("replay input {input_index} follows terminal input {terminal_input_index}")]
    InputAfterEndMatch {
        input_index: u64,
        terminal_input_index: u64,
    },
    #[error("replay seek target {0:?} is outside the replay")]
    SeekOutOfRange(StateVersion),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;
    use tabula_core::{
        canonical_encode, Occupant, OutcomeKind, SeatEntry, SeatId, Standing, UserId, Viewer,
    };
    use tabula_game_api::{Init, InitError, Outcome};

    // Generated independently with `zstd --long=27` from the test payload in
    // ruzstd's own large-window fixture. The frame declares a 128 MiB window,
    // while its decompressed content remains small enough for this test.
    const ZSTD_128_MIB_WINDOW_FRAME: &[u8] = &[
        0x28, 0xb5, 0x2f, 0xfd, 0x04, 0x88, 0xbc, 0x01, 0x00, 0xd4, 0x02, 0x54, 0x68, 0x65, 0x20,
        0x71, 0x75, 0x69, 0x63, 0x6b, 0x20, 0x62, 0x72, 0x6f, 0x77, 0x6e, 0x20, 0x66, 0x6f, 0x78,
        0x20, 0x6a, 0x75, 0x6d, 0x70, 0x73, 0x20, 0x6f, 0x76, 0x65, 0x72, 0x20, 0x74, 0x68, 0x65,
        0x20, 0x6c, 0x61, 0x7a, 0x79, 0x20, 0x64, 0x6f, 0x67, 0x2e, 0x0a, 0x01, 0x00, 0x85, 0xfe,
        0x87, 0xb9, 0x2a, 0x03, 0x4d, 0x00, 0x00, 0x08, 0x68, 0x01, 0x00, 0xfc, 0x4f, 0x1d, 0x08,
        0x01, 0xba, 0xb8, 0xd5, 0xc8,
    ];

    fn zstd_128_mib_window_with_dictionary_id() -> Vec<u8> {
        let mut frame = Vec::with_capacity(ZSTD_128_MIB_WINDOW_FRAME.len() + 4);
        // FHD 0x07: non-single-segment, content checksum present, 4-byte DID.
        frame.extend_from_slice(&[0x28, 0xb5, 0x2f, 0xfd, 0x07, 0x88, 1, 0, 0, 0]);
        frame.extend_from_slice(&ZSTD_128_MIB_WINDOW_FRAME[6..]);
        frame
    }

    fn header(kind: ReplayKind, seed: Option<MatchSeed>) -> ReplayHeader {
        ReplayHeader {
            match_id: MatchId(7),
            game_id: GameId::new("com.example.test").unwrap(),
            game_version: GameVersion::new("1.2.3").unwrap(),
            rules_version: RulesVersion(1),
            rules_hash: [9; 32],
            config: canonical_encode(&7u8).unwrap(),
            roster: SeatRoster::new(smallvec![SeatEntry {
                seat: SeatId(0),
                occupant: Occupant::Human(UserId(1)),
                team: None,
            }])
            .unwrap(),
            seed,
            initial_snapshot: match kind {
                ReplayKind::Canonical => None,
                ReplayKind::Projected { .. } => Some(vec![1, 2, 3]),
            },
            started_at: 0,
            duration_ms: 0,
            outcome: None,
            kind,
        }
    }

    fn draft(kind: ReplayKind, seed: Option<MatchSeed>) -> ReplayDraft {
        ReplayDraft {
            header: header(kind, seed),
            frames: vec![ReplayFrame {
                input_index: InputIndex(1),
                logical_time: LogicalTime(1_000),
                input: canonical_encode(&7u8).unwrap(),
                checkpoint: Some(StateHash([4; 32])),
            }],
            final_state_hash: StateHash([5; 32]),
        }
    }

    #[test]
    fn projected_seed_is_rejected_by_writer_and_reader() {
        let seed = MatchSeed::from_bytes([3; 32]);
        let invalid = draft(
            ReplayKind::Projected {
                viewer: Viewer::Spectator(tabula_core::SpectatorTier::Live),
            },
            Some(seed),
        );
        assert!(matches!(
            invalid.validate(),
            Err(ReplayError::ProjectedContainsSeed)
        ));

        let valid = draft(
            ReplayKind::Projected {
                viewer: Viewer::Spectator(tabula_core::SpectatorTier::Live),
            },
            None,
        );
        let bytes = valid.to_bytes().unwrap();
        let mut logical = decode_for_test(&bytes);
        // The reader must enforce the same invariant even when the bytes were
        // assembled by a malicious producer rather than this writer.
        let header_start = 10;
        let header_len = u32::from_le_bytes(logical[6..10].try_into().unwrap()) as usize;
        let mut parsed: ReplayHeader =
            postcard::from_bytes(&logical[header_start..header_start + header_len]).unwrap();
        parsed.seed = Some(MatchSeed::from_bytes([8; 32]));
        let encoded = postcard::to_allocvec(&parsed).unwrap();
        logical.splice(
            header_start..header_start + header_len,
            encoded.iter().copied(),
        );
        logical[6..10].copy_from_slice(&u32::try_from(encoded.len()).unwrap().to_le_bytes());
        rewrite_checksum(&mut logical);
        let malicious = encode_for_test(&logical);
        assert!(matches!(
            ValidatedReplay::from_bytes(&malicious),
            Err(ReplayError::ProjectedContainsSeed)
        ));
    }

    #[test]
    fn canonical_replay_requires_seed() {
        let error = draft(ReplayKind::Canonical, None).validate().unwrap_err();
        assert!(matches!(error, ReplayError::CanonicalSeedMissing));

        let valid = draft(ReplayKind::Canonical, Some(MatchSeed::from_bytes([3; 32])))
            .to_bytes()
            .unwrap();
        let mut logical = decode_for_test(&valid);
        let header_len = u32::from_le_bytes(logical[6..10].try_into().unwrap()) as usize;
        let mut parsed: ReplayHeader = postcard::from_bytes(&logical[10..10 + header_len]).unwrap();
        parsed.seed = None;
        let encoded = postcard::to_allocvec(&parsed).unwrap();
        logical.splice(10..10 + header_len, encoded.iter().copied());
        logical[6..10].copy_from_slice(&u32::try_from(encoded.len()).unwrap().to_le_bytes());
        rewrite_checksum(&mut logical);
        let malicious = encode_for_test(&logical);
        assert!(matches!(
            ValidatedReplay::from_bytes(&malicious),
            Err(ReplayError::CanonicalSeedMissing)
        ));
    }

    #[test]
    fn round_trip_preserves_validated_structure() {
        let replay = draft(ReplayKind::Canonical, Some(MatchSeed::from_bytes([3; 32])))
            .validate()
            .unwrap();
        let bytes = replay.to_bytes().unwrap();
        let decoded = ValidatedReplay::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.header().match_id, replay.header().match_id);
        assert_eq!(decoded.frames().len(), replay.frames().len());
        assert_eq!(decoded.final_state_hash(), replay.final_state_hash());
        assert_eq!(decoded.to_bytes().unwrap(), bytes);
    }

    #[test]
    fn corruption_is_rejected() {
        let replay = draft(ReplayKind::Canonical, Some(MatchSeed::from_bytes([3; 32])))
            .to_bytes()
            .unwrap();
        let mut logical = decode_for_test(&replay);
        logical[0] = b'X';
        let corrupted_magic = encode_for_test(&logical);
        assert!(matches!(
            ValidatedReplay::from_bytes(&corrupted_magic),
            Err(ReplayError::BadMagic)
        ));

        let mut truncated = replay.clone();
        truncated.truncate(truncated.len() - 1);
        assert!(ValidatedReplay::from_bytes(&truncated).is_err());

        let mut body_corrupt = decode_for_test(&replay);
        body_corrupt[10] ^= 1;
        let bytes = encode_for_test(&body_corrupt);
        assert!(ValidatedReplay::from_bytes(&bytes).is_err());
    }

    #[test]
    fn trailer_metadata_is_crc_protected() {
        let replay = draft(ReplayKind::Canonical, Some(MatchSeed::from_bytes([3; 32])))
            .to_bytes()
            .unwrap();
        let logical = decode_for_test(&replay);
        let header_len = u32::from_le_bytes(logical[6..10].try_into().unwrap()) as usize;
        let frame_payload = 10 + header_len + 4;
        let body_end = logical.len() - TRAILER_LEN;
        for offset in [10, frame_payload, body_end, body_end + 8] {
            let mut mutated = logical.clone();
            mutated[offset] ^= 1;
            let error = ValidatedReplay::from_bytes(&encode_for_test(&mutated)).unwrap_err();
            assert!(
                matches!(error, ReplayError::ChecksumMismatch),
                "offset {offset}"
            );
        }
    }

    #[test]
    fn recomputed_trailer_metadata_reaches_semantic_validation() {
        let replay = draft(ReplayKind::Canonical, Some(MatchSeed::from_bytes([3; 32])))
            .to_bytes()
            .unwrap();
        let mut count = decode_for_test(&replay);
        let body_end = count.len() - TRAILER_LEN;
        count[body_end..body_end + 8].copy_from_slice(&99u64.to_le_bytes());
        rewrite_checksum(&mut count);
        assert!(matches!(
            ValidatedReplay::from_bytes(&encode_for_test(&count)),
            Err(ReplayError::InputCountMismatch {
                declared: 99,
                actual: 1
            })
        ));

        let mut final_hash = decode_for_test(&replay);
        final_hash[body_end + 8] ^= 1;
        rewrite_checksum(&mut final_hash);
        let decoded = ValidatedReplay::from_bytes(&encode_for_test(&final_hash)).unwrap();
        assert_ne!(decoded.final_state_hash(), StateHash([5; 32]));
    }

    #[test]
    fn zstd_limits_and_exact_frame_boundary_are_enforced() {
        let (draft, _) = counter_draft(&[CounterCommand::Add(1)]);
        let valid = draft.to_bytes().unwrap();

        let oversized_window = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(
            Cursor::new(ZSTD_128_MIB_WINDOW_FRAME),
            MAX_ZSTD_WINDOW_SIZE,
        );
        assert!(matches!(
            oversized_window,
            Err(
                ruzstd::decoding::errors::FrameDecoderError::WindowSizeTooBig {
                    requested,
                    max,
                }
            ) if requested == 128 * 1024 * 1024 && max == MAX_ZSTD_WINDOW_SIZE
        ));

        let large_logical = vec![0u8; MAX_DECOMPRESSED_REPLAY_BYTES + 1];
        let large_output = encode_for_test(&large_logical);
        assert!(ValidatedReplay::from_bytes(&large_output).is_err());

        let mut second_frame = valid.clone();
        second_frame.extend_from_slice(&encode_for_test(b"trailing"));
        assert!(matches!(
            ValidatedReplay::from_bytes(&second_frame),
            Err(ReplayError::Corrupt(message)) if message.contains("exactly one")
        ));

        let mut random_trailing = valid;
        random_trailing.extend_from_slice(&[1, 2, 3, 4]);
        assert!(matches!(
            ValidatedReplay::from_bytes(&random_trailing),
            Err(ReplayError::Corrupt(message)) if message.contains("exactly one")
        ));
    }

    #[test]
    fn truncated_zstd_stream_is_rejected() {
        let (draft, _) = counter_draft(&[CounterCommand::Add(1)]);
        let mut truncated = draft.to_bytes().unwrap();
        truncated.truncate(truncated.len().saturating_sub(1));
        assert!(ValidatedReplay::from_bytes(&truncated).is_err());
    }

    #[test]
    fn dictionary_id_frame_cannot_bypass_window_guard() {
        let dictionary = zstd_128_mib_window_with_dictionary_id();
        let decoder = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(
            Cursor::new(&dictionary),
            MAX_ZSTD_WINDOW_SIZE,
        );
        assert!(matches!(
            decoder,
            Err(
                ruzstd::decoding::errors::FrameDecoderError::WindowSizeTooBig {
                    requested,
                    max,
                }
            ) if requested == 128 * 1024 * 1024 && max == MAX_ZSTD_WINDOW_SIZE
        ));
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct CounterState {
        value: u32,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    enum CounterCommand {
        Add(u8),
        Reject,
        End,
        DoubleEnd,
    }

    #[derive(Clone, Debug, Serialize)]
    struct CounterView {
        value: u32,
    }

    struct CounterRules;

    impl GameRules for CounterRules {
        type State = CounterState;
        type Command = CounterCommand;
        type Event = u32;
        type View = CounterView;
        type ViewEvent = u32;
        type Config = u8;

        const RULES_VERSION: RulesVersion = RulesVersion(1);
        const RULES_HASH: [u8; 32] = [9; 32];

        fn create(config: &u8, _: &SeatRoster, _: &mut Ctx<'_>) -> Result<Init<Self>, InitError> {
            let effects = match config {
                0 => smallvec![Effect::EndMatch {
                    outcome: counter_outcome("counter created terminal"),
                }],
                1 => {
                    let outcome = counter_outcome("counter created terminal");
                    smallvec![
                        Effect::EndMatch {
                            outcome: outcome.clone(),
                        },
                        Effect::EndMatch { outcome },
                    ]
                }
                _ => smallvec::SmallVec::new(),
            };
            Ok(Init {
                state: CounterState { value: 0 },
                events: smallvec::SmallVec::new(),
                effects,
            })
        }

        fn apply(
            state: &mut CounterState,
            input: Input<CounterCommand>,
            _: &mut Ctx<'_>,
        ) -> Result<Outcome<Self>, tabula_core::RuleError> {
            match input {
                Input::Player {
                    command: CounterCommand::Add(amount),
                    ..
                } => {
                    state.value += u32::from(amount);
                    Ok(Outcome {
                        events: smallvec::smallvec![u32::from(amount)],
                        effects: smallvec::SmallVec::new(),
                    })
                }
                Input::Player {
                    command: CounterCommand::Reject,
                    ..
                } => Err(tabula_core::RuleError::code(
                    tabula_core::RuleErrorCode::IllegalMove,
                )),
                Input::Player {
                    command: CounterCommand::End,
                    ..
                } => Ok(Outcome {
                    events: smallvec::SmallVec::new(),
                    effects: smallvec![Effect::EndMatch {
                        outcome: counter_outcome("counter ended"),
                    }],
                }),
                Input::Player {
                    command: CounterCommand::DoubleEnd,
                    ..
                } => {
                    let outcome = counter_outcome("counter ended");
                    Ok(Outcome {
                        events: smallvec::SmallVec::new(),
                        effects: smallvec![
                            Effect::EndMatch {
                                outcome: outcome.clone(),
                            },
                            Effect::EndMatch { outcome },
                        ],
                    })
                }
                Input::Timer { .. } | Input::Seat { .. } | Input::Admin(_) => Ok(Outcome::empty()),
            }
        }

        fn project(state: &CounterState, _: Viewer) -> CounterView {
            CounterView { value: state.value }
        }

        fn view_event(_: &CounterState, event: &u32, _: Viewer) -> Option<u32> {
            Some(*event)
        }
    }

    fn counter_roster() -> SeatRoster {
        SeatRoster::new(smallvec![SeatEntry {
            seat: SeatId(0),
            occupant: Occupant::Human(UserId(11)),
            team: None,
        }])
        .unwrap()
    }

    fn counter_outcome(summary: &str) -> MatchOutcome {
        MatchOutcome::new_for_seats(
            OutcomeKind::Decisive,
            smallvec![Standing {
                seat: SeatId(0),
                rank: 0,
                score: 1,
            }],
            summary.into(),
            &[SeatId(0)],
        )
        .unwrap()
    }

    // This fixture helper intentionally keeps the first EndMatch when building
    // malformed artifacts; the runner's scanner must reject the duplicate.
    fn first_terminal_outcome(effects: &[Effect]) -> Option<MatchOutcome> {
        effects.iter().find_map(|effect| {
            if let Effect::EndMatch { outcome } = effect {
                Some(outcome.clone())
            } else {
                None
            }
        })
    }

    fn counter_draft(commands: &[CounterCommand]) -> (ReplayDraft, StateHash) {
        let seed = MatchSeed::from_bytes([42; 32]);
        let config = 7u8;
        let roster = counter_roster();
        let mut create_rng = tabula_core::DetRng::for_input(&seed, InputIndex(0));
        let mut create_ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut create_rng,
            budget: Budget {
                max_apply_micros: u32::MAX,
                max_events_per_input: u16::MAX,
            },
        };
        let init = CounterRules::create(&config, &roster, &mut create_ctx).unwrap();
        let mut derived_outcome = first_terminal_outcome(&init.effects);
        let mut state = init.state;
        let mut frames = Vec::new();
        for (position, command) in commands.iter().cloned().enumerate() {
            let input = Input::Player {
                seat: SeatId(0),
                command,
            };
            let index = InputIndex(position as u64 + 1);
            let logical_time = LogicalTime((position as u64 + 1) * 100);
            let mut rng = tabula_core::DetRng::for_input(&seed, index);
            let mut ctx = Ctx {
                now: logical_time,
                index,
                rng: &mut rng,
                budget: Budget {
                    max_apply_micros: u32::MAX,
                    max_events_per_input: u16::MAX,
                },
            };
            let outcome = CounterRules::apply(&mut state, input.clone(), &mut ctx).unwrap();
            if derived_outcome.is_none() {
                derived_outcome = first_terminal_outcome(&outcome.effects);
            }
            frames.push(ReplayFrame {
                input_index: index,
                logical_time,
                input: canonical_encode(&input).unwrap(),
                checkpoint: Some(CounterRules::state_hash(&state)),
            });
        }
        let final_hash = CounterRules::state_hash(&state);
        (
            ReplayDraft {
                header: ReplayHeader {
                    match_id: MatchId(99),
                    game_id: GameId::new("com.example.counter").unwrap(),
                    game_version: GameVersion::new("1.0.0").unwrap(),
                    rules_version: RulesVersion(1),
                    rules_hash: [9; 32],
                    config: canonical_encode(&config).unwrap(),
                    roster,
                    seed: Some(seed),
                    initial_snapshot: None,
                    started_at: 0,
                    duration_ms: commands.len() as u64 * 100,
                    outcome: derived_outcome,
                    kind: ReplayKind::Canonical,
                },
                frames,
                final_state_hash: final_hash,
            },
            final_hash,
        )
    }

    fn counter_initial_draft(
        config: u8,
        outcome: Option<MatchOutcome>,
    ) -> (ReplayDraft, StateHash) {
        let seed = MatchSeed::from_bytes([42; 32]);
        let roster = counter_roster();
        let mut create_rng = tabula_core::DetRng::for_input(&seed, InputIndex(0));
        let mut create_ctx = Ctx {
            now: LogicalTime::ZERO,
            index: InputIndex(0),
            rng: &mut create_rng,
            budget: Budget {
                max_apply_micros: u32::MAX,
                max_events_per_input: u16::MAX,
            },
        };
        let init = CounterRules::create(&config, &roster, &mut create_ctx).unwrap();
        let final_hash = CounterRules::state_hash(&init.state);
        (
            ReplayDraft {
                header: ReplayHeader {
                    match_id: MatchId(100),
                    game_id: GameId::new("com.example.counter").unwrap(),
                    game_version: GameVersion::new("1.0.0").unwrap(),
                    rules_version: RulesVersion(1),
                    rules_hash: [9; 32],
                    config: canonical_encode(&config).unwrap(),
                    roster,
                    seed: Some(seed),
                    initial_snapshot: None,
                    started_at: 0,
                    duration_ms: 0,
                    outcome,
                    kind: ReplayKind::Canonical,
                },
                frames: Vec::new(),
                final_state_hash: final_hash,
            },
            final_hash,
        )
    }

    fn counter_identity() -> ReplayIdentity {
        ReplayIdentity {
            game_id: GameId::new("com.example.counter").unwrap(),
            rules_version: RulesVersion(1),
            rules_hash: [9; 32],
        }
    }

    #[test]
    fn runner_replays_independent_live_execution_and_supports_seek() {
        let (draft, live_final_hash) = counter_draft(&[
            CounterCommand::Add(2),
            CounterCommand::Add(3),
            CounterCommand::Add(5),
        ]);
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        assert_eq!(runner.check(), ReplayVerdict::Exact);
        let report = runner.verify().unwrap();
        assert!(report.is_verified());
        assert_eq!(report.checkpoints_checked(), 3);
        assert_eq!(report.actual_final_state_hash(), live_final_hash);
        assert!(report.outcome_checked());
        assert_eq!(report.expected_outcome(), None);
        assert_eq!(report.actual_outcome(), None);

        let position = runner.seek(StateVersion(1)).unwrap();
        assert!(matches!(position, PrefixPosition::Verified(_)));
        let next = runner.step().unwrap().unwrap();
        assert_eq!(next.input_index, 2);
        assert_eq!(next.state_version, StateVersion(2));

        let first = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .verify()
            .unwrap();
        let second = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .verify()
            .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn same_rules_version_with_different_hash_is_compatible_version() {
        let (draft, _) = counter_draft(&[]);
        let bytes = draft.to_bytes().unwrap();
        let mut identity = counter_identity();
        identity.rules_hash[0] ^= 1;

        let runner = ReplayRunner::<CounterRules>::from_bytes(&bytes, identity).unwrap();

        assert_eq!(runner.check(), ReplayVerdict::CompatibleVersion);
    }

    #[test]
    fn checkpoint_mismatch_reports_context() {
        let (mut draft, _) = counter_draft(&[
            CounterCommand::Add(2),
            CounterCommand::Add(3),
            CounterCommand::Add(5),
        ]);
        draft.frames[1].checkpoint = Some(StateHash([77; 32]));
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let divergence = report.divergences().first().unwrap();
        assert_eq!(divergence.kind, DivergenceKind::Checkpoint);
        assert_eq!(divergence.input_index, 2);
        assert_eq!(divergence.logical_time, Some(LogicalTime(200)));
        assert_eq!(divergence.previous_checkpoint, Some(1));
        assert_eq!(divergence.next_checkpoint, Some(3));
        assert!(!report.is_verified());
    }

    #[test]
    fn exact_checkpoint_diagnosis_is_adjacent() {
        let (mut draft, _) = counter_draft(&[
            CounterCommand::Add(2),
            CounterCommand::Add(3),
            CounterCommand::Add(5),
        ]);
        draft.frames[1].checkpoint = Some(StateHash([77; 32]));
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let diagnosis = report.diagnosis().unwrap();

        assert_eq!(diagnosis.kind(), ReplayDiagnosisKind::CheckpointState);
        assert_eq!(diagnosis.evidence().input_index, 2);
        assert!(matches!(
            diagnosis.location(),
            DivergenceLocation::Exact(exact)
                if exact.input_index() == InputIndex(2)
                    && exact.previous_verified() == InputIndex(1)
        ));
    }

    #[test]
    fn sparse_checkpoint_diagnosis_is_a_window() {
        let (mut draft, _) = counter_draft(&[
            CounterCommand::Add(1),
            CounterCommand::Add(1),
            CounterCommand::Add(1),
            CounterCommand::Add(1),
        ]);
        draft.frames[1].checkpoint = None;
        draft.frames[2].checkpoint = None;
        draft.frames[3].checkpoint = Some(StateHash([77; 32]));
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let diagnosis = report.diagnosis().unwrap();

        assert_eq!(diagnosis.kind(), ReplayDiagnosisKind::CheckpointState);
        assert!(matches!(
            diagnosis.location(),
            DivergenceLocation::Window(window)
                if window.after_verified() == Some(InputIndex(1))
                    && window.at_or_before() == InputIndex(4)
                    && window.first_failing_evidence() == InputIndex(4)
        ));
    }

    #[test]
    fn first_checkpoint_without_previous_evidence_is_a_window() {
        let (mut draft, _) = counter_draft(&[CounterCommand::Add(1), CounterCommand::Add(1)]);
        draft.frames[0].checkpoint = Some(StateHash([77; 32]));
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let diagnosis = report.diagnosis().unwrap();

        assert!(matches!(
            diagnosis.location(),
            DivergenceLocation::Window(window)
                if window.after_verified().is_none()
                    && window.at_or_before() == InputIndex(1)
        ));
    }

    #[test]
    fn checkpoint_reconvergence_does_not_restore_monotonicity() {
        let (mut draft, _) = counter_draft(&[CounterCommand::Add(1), CounterCommand::Add(1)]);
        draft.frames[0].checkpoint = Some(StateHash([77; 32]));
        let expected_second = draft.frames[1].checkpoint;
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();

        assert_eq!(
            report
                .checkpoint_evidence()
                .iter()
                .map(CheckpointEvidence::matched)
                .collect::<Vec<_>>(),
            vec![false, true]
        );
        assert_eq!(
            report.checkpoint_evidence()[1].expected(),
            expected_second.unwrap()
        );
        let diagnosis = report.diagnosis().unwrap();
        assert!(matches!(
            diagnosis.location(),
            DivergenceLocation::Window(window)
                if window.after_verified().is_none()
                    && window.at_or_before() == InputIndex(1)
        ));
    }

    #[test]
    fn final_hash_only_diagnosis_is_final_evidence() {
        let (mut draft, _) = counter_draft(&[CounterCommand::Add(1), CounterCommand::Add(1)]);
        draft.final_state_hash = StateHash([88; 32]);
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let diagnoses = report.diagnoses();

        assert_eq!(diagnoses.len(), 1);
        assert_eq!(diagnoses[0].kind(), ReplayDiagnosisKind::FinalStateHashOnly);
        assert!(matches!(
            diagnoses[0].location(),
            DivergenceLocation::FinalEvidenceOnly(final_only)
                if final_only.after_verified() == Some(InputIndex(2))
                    && final_only.final_input() == Some(InputIndex(2))
        ));
    }

    #[test]
    fn terminal_outcome_diagnosis_is_structured() {
        let (mut draft, _) = counter_draft(&[CounterCommand::End]);
        draft.header.outcome = Some(counter_outcome("wrong outcome"));
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let diagnosis = report.diagnoses().pop().unwrap();

        assert_eq!(diagnosis.kind(), ReplayDiagnosisKind::TerminalOutcome);
        assert!(diagnosis.evidence().expected_outcome.is_some());
        assert!(diagnosis.evidence().actual_outcome.is_some());
        assert!(matches!(
            diagnosis.location(),
            DivergenceLocation::FinalEvidenceOnly(final_only)
                if final_only.after_verified() == Some(InputIndex(1))
                    && final_only.final_input() == Some(InputIndex(1))
        ));
    }

    #[test]
    fn diagnosis_is_deterministic_for_identical_evidence() {
        let (mut draft, _) = counter_draft(&[
            CounterCommand::Add(2),
            CounterCommand::Add(3),
            CounterCommand::Add(5),
        ]);
        draft.frames[1].checkpoint = Some(StateHash([77; 32]));
        let bytes = draft.to_bytes().unwrap();
        let first = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .verify()
            .unwrap()
            .diagnoses();
        let second = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .verify()
            .unwrap()
            .diagnoses();

        assert_eq!(first, second);
    }

    #[test]
    fn checkpoint_reproducer_validates_and_reproduces() {
        let (mut draft, _) = counter_draft(&[
            CounterCommand::Add(2),
            CounterCommand::Add(3),
            CounterCommand::Add(5),
        ]);
        draft.frames[1].checkpoint = Some(StateHash([77; 32]));
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let diagnosis = report.diagnosis().unwrap();
        let ReproducerAvailability::Available(reproducer) = runner.reproducer(&diagnosis) else {
            panic!("checkpoint evidence should derive a prefix");
        };

        assert_eq!(reproducer.frames().len(), 2);
        assert_eq!(reproducer.final_state_hash(), StateHash([77; 32]));
        assert_eq!(reproducer.header().match_id, draft.header.match_id);
        assert_eq!(reproducer.header().game_id, draft.header.game_id);
        assert_eq!(reproducer.header().config, draft.header.config);
        assert_eq!(
            reproducer.header().rules_version,
            draft.header.rules_version
        );
        assert_eq!(reproducer.header().rules_hash, draft.header.rules_hash);
        assert_eq!(reproducer.header().seed, draft.header.seed);
        assert_eq!(reproducer.header().outcome, None);

        let mut reproducer_runner = ReplayRunner::<CounterRules>::from_bytes(
            &reproducer.to_bytes().unwrap(),
            counter_identity(),
        )
        .unwrap();
        let reproducer_report = reproducer_runner.verify().unwrap();
        assert!(reproducer_report.divergences().iter().any(|divergence| {
            divergence.kind == DivergenceKind::Checkpoint && divergence.input_index == 2
        }));
    }

    #[test]
    fn non_checkpoint_reproducer_is_not_derived() {
        let (mut draft, _) = counter_draft(&[CounterCommand::Add(1)]);
        draft.final_state_hash = StateHash([88; 32]);
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let diagnosis = report.diagnosis().unwrap();

        assert!(matches!(
            runner.reproducer(&diagnosis),
            ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointEvidenceRequired
            }
        ));
    }

    #[test]
    fn final_checkpoint_reproducer_reports_original_as_minimal() {
        let (mut draft, _) = counter_draft(&[CounterCommand::Add(1)]);
        draft.frames[0].checkpoint = Some(StateHash([77; 32]));
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        let diagnosis = report.diagnosis().unwrap();

        assert!(matches!(
            runner.reproducer(&diagnosis),
            ReproducerAvailability::OriginalReplayIsMinimal
        ));
    }

    #[test]
    fn diagnosis_cannot_authorize_another_replay_even_on_final_frame() {
        let (mut first_draft, _) = counter_draft(&[CounterCommand::Add(1)]);
        first_draft.frames[0].checkpoint = Some(StateHash([77; 32]));
        let first_bytes = first_draft.to_bytes().unwrap();
        let mut first_runner =
            ReplayRunner::<CounterRules>::from_bytes(&first_bytes, counter_identity()).unwrap();
        let diagnosis = first_runner.verify().unwrap().diagnosis().unwrap();

        let (mut second_draft, _) = counter_draft(&[CounterCommand::Add(2)]);
        second_draft.frames[0].checkpoint = Some(StateHash([77; 32]));
        let second_bytes = second_draft.to_bytes().unwrap();
        let mut second_runner =
            ReplayRunner::<CounterRules>::from_bytes(&second_bytes, counter_identity()).unwrap();
        let second_diagnosis = second_runner.verify().unwrap().diagnosis().unwrap();
        assert_ne!(
            diagnosis.evidence().actual,
            second_diagnosis.evidence().actual
        );

        assert!(matches!(
            second_runner.reproducer(&diagnosis),
            ReproducerAvailability::InsufficientEvidence {
                reason: ReproducerReason::CheckpointClaimUnavailable
            }
        ));
    }

    #[test]
    fn seek_never_hides_checkpoint_divergence() {
        let (mut draft, _) = counter_draft(&[
            CounterCommand::Add(2),
            CounterCommand::Add(3),
            CounterCommand::Add(5),
        ]);
        draft.frames[1].checkpoint = Some(StateHash([77; 32]));
        let bytes = draft.to_bytes().unwrap();
        let error = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .seek(StateVersion(2))
            .unwrap_err();
        assert!(matches!(
            error,
            ReplayError::PrefixDivergence { divergence }
                if divergence.kind == DivergenceKind::Checkpoint
                    && divergence.input_index == 2
        ));
    }

    #[test]
    fn seek_never_hides_final_hash_divergence() {
        let (mut draft, _) = counter_draft(&[CounterCommand::Add(1)]);
        draft.final_state_hash = StateHash([88; 32]);
        let bytes = draft.to_bytes().unwrap();
        let error = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .seek(StateVersion(1))
            .unwrap_err();
        assert!(matches!(
            error,
            ReplayError::PrefixDivergence { divergence }
                if divergence.kind == DivergenceKind::FinalStateHash
        ));
    }

    #[test]
    fn final_hash_mismatch_fails_verification() {
        let (mut draft, _) = counter_draft(&[CounterCommand::Add(2)]);
        draft.final_state_hash = StateHash([88; 32]);
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        assert_eq!(
            report.divergences().last().unwrap().kind,
            DivergenceKind::FinalStateHash
        );
        assert!(!report.is_verified());
    }

    #[test]
    fn replay_terminal_outcome_matches_effect() {
        let (draft, _) = counter_draft(&[CounterCommand::End]);
        let expected = draft.header.outcome.clone();
        assert!(expected.is_some());
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        assert!(report.outcome_checked());
        assert_eq!(report.expected_outcome(), expected.as_ref());
        assert_eq!(report.actual_outcome(), report.expected_outcome());
        assert!(report.is_verified());
    }

    #[test]
    fn end_match_from_create_is_verified() {
        let expected = counter_outcome("counter created terminal");
        let (draft, initial_hash) = counter_initial_draft(0, Some(expected.clone()));
        assert_eq!(draft.final_state_hash, initial_hash);
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        assert_eq!(report.inputs_replayed(), 0);
        assert_eq!(report.expected_outcome(), Some(&expected));
        assert_eq!(report.actual_outcome(), Some(&expected));
        assert!(report.outcome_checked());
        assert!(report.is_verified());
    }

    #[test]
    fn multiple_end_match_from_create_is_rejected() {
        let (draft, _) =
            counter_initial_draft(1, Some(counter_outcome("counter created terminal")));
        let bytes = draft.to_bytes().unwrap();
        let error =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap_err();
        assert!(matches!(
            error,
            ReplayError::MultipleEndMatch { input_index: 0 }
        ));
    }

    #[test]
    fn canonical_input_cannot_follow_create_end_match() {
        let (mut draft, _) =
            counter_initial_draft(0, Some(counter_outcome("counter created terminal")));
        let input = Input::Player {
            seat: SeatId(0),
            command: CounterCommand::Add(1),
        };
        draft.frames.push(ReplayFrame {
            input_index: InputIndex(1),
            logical_time: LogicalTime(100),
            input: canonical_encode(&input).unwrap(),
            checkpoint: None,
        });
        let bytes = draft.to_bytes().unwrap();
        let error = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .verify()
            .unwrap_err();
        assert!(matches!(
            error,
            ReplayError::InputAfterEndMatch {
                input_index: 1,
                terminal_input_index: 0
            }
        ));
    }

    #[test]
    fn missing_terminal_outcome_is_divergence() {
        let (mut draft, _) = counter_draft(&[CounterCommand::End]);
        draft.header.outcome = None;
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        assert_eq!(
            report.divergences().last().unwrap().kind,
            DivergenceKind::TerminalOutcome
        );
        assert!(report.actual_outcome().is_some());
        assert!(!report.is_verified());
    }

    #[test]
    fn wrong_terminal_outcome_is_divergence() {
        let (mut draft, _) = counter_draft(&[CounterCommand::End]);
        draft.header.outcome = Some(counter_outcome("wrong outcome"));
        let bytes = draft.to_bytes().unwrap();
        let mut runner =
            ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity()).unwrap();
        let report = runner.verify().unwrap();
        assert_eq!(
            report.divergences().last().unwrap().kind,
            DivergenceKind::TerminalOutcome
        );
        assert_ne!(report.expected_outcome(), report.actual_outcome());
        assert!(!report.is_verified());
    }

    #[test]
    fn multiple_end_match_effects_are_rejected() {
        let (draft, _) = counter_draft(&[CounterCommand::DoubleEnd]);
        let bytes = draft.to_bytes().unwrap();
        let error = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .verify()
            .unwrap_err();
        assert!(matches!(
            error,
            ReplayError::MultipleEndMatch { input_index: 1 }
        ));
    }

    #[test]
    fn canonical_log_cannot_continue_after_end_match() {
        let (draft, _) = counter_draft(&[CounterCommand::End, CounterCommand::Add(1)]);
        let bytes = draft.to_bytes().unwrap();
        let error = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .verify()
            .unwrap_err();
        assert!(matches!(
            error,
            ReplayError::InputAfterEndMatch {
                input_index: 2,
                terminal_input_index: 1
            }
        ));
    }

    #[test]
    fn replay_rejection_is_not_reported_as_success() {
        let (mut draft, _) = counter_draft(&[]);
        let input = Input::Player {
            seat: SeatId(0),
            command: CounterCommand::Reject,
        };
        draft.frames.push(ReplayFrame {
            input_index: InputIndex(1),
            logical_time: LogicalTime(100),
            input: canonical_encode(&input).unwrap(),
            checkpoint: None,
        });
        let bytes = draft.to_bytes().unwrap();
        let error = ReplayRunner::<CounterRules>::from_bytes(&bytes, counter_identity())
            .unwrap()
            .verify()
            .unwrap_err();
        assert!(matches!(
            error,
            ReplayError::UnexpectedRejection { input_index: 1, .. }
        ));
    }

    #[test]
    fn hostile_structural_metadata_is_rejected_before_frame_decode() {
        let (draft, _) = counter_draft(&[CounterCommand::Add(1)]);
        let bytes = draft.to_bytes().unwrap();

        let mut future = decode_for_test(&bytes);
        future[4..6].copy_from_slice(&(REPLAY_FORMAT_VERSION + 1).to_le_bytes());
        let future = encode_for_test(&future);
        assert!(matches!(
            ValidatedReplay::from_bytes(&future),
            Err(ReplayError::FormatTooNew(2))
        ));

        let mut count = decode_for_test(&bytes);
        let body_end = count.len() - TRAILER_LEN;
        count[body_end..body_end + 8].copy_from_slice(&99u64.to_le_bytes());
        rewrite_checksum(&mut count);
        let count = encode_for_test(&count);
        assert!(matches!(
            ValidatedReplay::from_bytes(&count),
            Err(ReplayError::InputCountMismatch {
                declared: 99,
                actual: 1
            })
        ));

        let mut oversized = decode_for_test(&bytes);
        let body_end = oversized.len() - TRAILER_LEN;
        oversized.splice(
            body_end..body_end,
            (u32::try_from(MAX_FRAME_BYTES + 1).unwrap()).to_le_bytes(),
        );
        rewrite_checksum(&mut oversized);
        let oversized = encode_for_test(&oversized);
        assert!(matches!(
            ValidatedReplay::from_bytes(&oversized),
            Err(ReplayError::Corrupt(message)) if message.contains("declared frame")
        ));
    }

    #[test]
    fn non_monotonic_reader_frames_are_rejected() {
        let (draft, _) = counter_draft(&[CounterCommand::Add(1), CounterCommand::Add(1)]);
        let bytes = draft.to_bytes().unwrap();
        let mut logical = decode_for_test(&bytes);
        let header_len = u32::from_le_bytes(logical[6..10].try_into().unwrap()) as usize;
        let mut cursor = 10 + header_len;
        let first_len =
            u32::from_le_bytes(logical[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4 + first_len;
        let second_len =
            u32::from_le_bytes(logical[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        let mut second: ReplayFrame =
            postcard::from_bytes(&logical[cursor..cursor + second_len]).unwrap();
        second.input_index = InputIndex(1);
        let replacement = postcard::to_allocvec(&second).unwrap();
        assert_eq!(replacement.len(), second_len);
        logical[cursor..cursor + second_len].copy_from_slice(&replacement);
        rewrite_checksum(&mut logical);
        let bytes = encode_for_test(&logical);
        assert!(matches!(
            ValidatedReplay::from_bytes(&bytes),
            Err(ReplayError::Corrupt(message)) if message.contains("contiguous")
        ));
    }

    fn decode_for_test(bytes: &[u8]) -> Vec<u8> {
        let mut stream = ruzstd::decoding::StreamingDecoder::new(Cursor::new(bytes)).unwrap();
        let mut output = Vec::new();
        stream.read_to_end(&mut output).unwrap();
        output
    }

    fn encode_for_test(logical: &[u8]) -> Vec<u8> {
        ruzstd::encoding::compress_to_vec(logical, ruzstd::encoding::CompressionLevel::Uncompressed)
    }

    fn rewrite_checksum(logical: &mut [u8]) {
        let checksum_start = logical.len() - 4;
        let checksum = crc32(&logical[..checksum_start]);
        logical[checksum_start..].copy_from_slice(&checksum.to_le_bytes());
    }
}
