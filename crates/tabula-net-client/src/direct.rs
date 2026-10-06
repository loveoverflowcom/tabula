//! Bounded direct-play sequencing and current-scope recovery (doc 04 §4).
//! No credentials or game state are stored here; pending intent is never truth.
use tabula_core::{GameId, GameVersion, MatchId};
use tabula_protocol::{ClientEnvelope, GameCommandFrame, ServerEnvelope, ServerMessage};

/// Whether the projection-only document can submit a new command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectState {
    /// An initial authorized projection has not arrived.
    Connecting,
    /// One command can be submitted.
    Ready,
    /// One command awaits its server receipt.
    Sending,
    /// Authority is being reacquired; private projections must be concealed.
    Recovering,
    /// A sent operation has no definitive receipt yet. Timeout does not mean failure.
    UnknownResult,
    /// A fresh full authorized projection is replacing the retired stream.
    Resyncing,
    /// A fresh projection is available, but an old operation cannot be resolved safely.
    ReadOnly,
    /// Authority/protocol failed or sequences exhausted; no commands can replay.
    Disconnected,
}

/// One authorized opaque stream with a single pending command.
#[derive(Debug)]
pub struct DirectClient {
    match_id: MatchId,
    game: GameId,
    game_version: GameVersion,
    next_seq: Option<u64>,
    pending: Option<ClientEnvelope>,
    operation_scope: Option<String>,
    generation: u64,
    unresolved: bool,
    frame: u64,
    revision: Option<u64>,
    state: DirectState,
}
impl DirectClient {
    /// Build from a server binding and durable next-sequence hint, not authority.
    pub fn new(
        match_id: MatchId,
        game: GameId,
        game_version: GameVersion,
        next_seq: u64,
    ) -> Result<Self, DirectError> {
        if next_seq == 0 {
            return Err(DirectError::Sequence);
        }
        Ok(Self {
            match_id,
            game,
            game_version,
            next_seq: Some(next_seq),
            pending: None,
            operation_scope: None,
            generation: 0,
            unresolved: false,
            frame: 0,
            revision: None,
            state: DirectState::Connecting,
        })
    }
    /// Current command gate.
    pub const fn state(&self) -> DirectState {
        self.state
    }
    /// Per-attachment visible revision, never a canonical version.
    pub const fn revision(&self) -> Option<u64> {
        self.revision
    }
    /// Allocate exactly one opaque intent without changing the projection.
    pub fn command(&mut self, payload: Vec<u8>) -> Result<ClientEnvelope, DirectError> {
        if self.state != DirectState::Ready {
            return Err(DirectError::Blocked);
        }
        let seq = self.next_seq.ok_or(DirectError::Sequence)?;
        let frame = GameCommandFrame::new(
            self.match_id,
            self.game.clone(),
            self.game_version.clone(),
            payload,
        )
        .map_err(|_| DirectError::Protocol)?;
        let command = ClientEnvelope::new(seq, seq, frame).map_err(|_| DirectError::Protocol)?;
        self.pending = Some(command.clone());
        self.next_seq = seq.checked_add(1);
        self.state = DirectState::Sending;
        Ok(command)
    }
    /// Validate the complete ordered batch before exposing its output.
    pub fn receive(&mut self, frames: &[ServerEnvelope]) -> Result<(), DirectError> {
        if matches!(
            self.state,
            DirectState::Disconnected | DirectState::Recovering | DirectState::UnknownResult
        ) {
            return Err(DirectError::Blocked);
        }
        let mut next_frame = self.frame;
        let mut revision = self.revision;
        let mut pending = self.pending.clone();
        let mut unresolved = self.unresolved;
        for frame in frames {
            if next_frame.checked_add(1) != Some(frame.frame()) {
                self.disconnect();
                return Err(DirectError::Gap);
            }
            next_frame = frame.frame();
            if revision.is_none()
                && !matches!(frame.body(), ServerMessage::MatchUpdate { revision: 0, .. })
            {
                self.disconnect();
                return Err(DirectError::Gap);
            }
            match frame.body() {
                ServerMessage::MatchUpdate { revision: next, .. } => {
                    if revision.map_or(*next != 0, |old| old.checked_add(1) != Some(*next)) {
                        self.disconnect();
                        return Err(DirectError::Gap);
                    }
                    revision = Some(*next);
                }
                ServerMessage::Ack { seq } | ServerMessage::Reject { seq, .. } => {
                    if pending.as_ref().map(ClientEnvelope::seq) != Some(*seq) {
                        self.disconnect();
                        return Err(DirectError::Sequence);
                    }
                    if matches!(
                        frame.body(),
                        ServerMessage::Reject {
                            error: tabula_protocol::ErrorCode::StaleSeq
                                | tabula_protocol::ErrorCode::Unavailable
                                | tabula_protocol::ErrorCode::OperationConflict,
                            ..
                        }
                    ) {
                        unresolved = true;
                    }
                    pending = None;
                }
            }
        }
        self.frame = next_frame;
        self.revision = revision;
        self.pending = pending;
        self.unresolved = unresolved;
        self.state = if unresolved {
            DirectState::ReadOnly
        } else if self.pending.is_some() {
            DirectState::Sending
        } else if self.next_seq.is_none() {
            DirectState::Disconnected
        } else if revision.is_some() {
            DirectState::Ready
        } else {
            DirectState::Connecting
        };
        Ok(())
    }
    /// Bind a server-derived non-authorizing operation-scope hint. Never a grant.
    pub fn bind_scope(&mut self, scope: &str) -> Result<(), DirectError> {
        if !valid_scope(scope) || self.operation_scope.is_some() {
            return Err(DirectError::Protocol);
        }
        self.operation_scope = Some(scope.to_owned());
        Ok(())
    }
    /// The local attachment generation rejects completions from retired requests.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Retain the exact command across uncertainty, while withdrawing authority.
    pub fn recover(&mut self) {
        self.generation = self.generation.saturating_add(1);
        self.revision = None;
        self.frame = 0;
        self.state = if self.pending.is_some() {
            DirectState::UnknownResult
        } else {
            DirectState::Recovering
        };
    }
    /// Accept frames only from this attachment's local request generation.
    pub fn receive_generation(
        &mut self,
        generation: u64,
        frames: &[ServerEnvelope],
    ) -> Result<bool, DirectError> {
        if generation != self.generation {
            return Ok(false);
        }
        self.receive(frames)?;
        Ok(true)
    }
    /// Restore one bounded, already decoded intent only after fresh authority and snapshot.
    /// A changed auth record/epoch/seat generation must never replay an old operation.
    pub fn restore_pending(
        &mut self,
        scope: &str,
        command: ClientEnvelope,
    ) -> Result<Option<ClientEnvelope>, DirectError> {
        if self.revision.is_none() || self.state != DirectState::Ready {
            return Err(DirectError::Blocked);
        }
        if self.operation_scope.as_deref() != Some(scope)
            || command.command().match_id() != self.match_id
            || command.command().game() != &self.game
            || command.command().game_version() != &self.game_version
            || self.next_seq.is_none_or(|next| command.seq() > next)
        {
            self.mark_unknown();
            return Ok(None);
        }
        self.next_seq = self
            .next_seq
            .and_then(|next| command.seq().checked_add(1).map(|after| next.max(after)));
        self.pending = Some(command.clone());
        self.state = DirectState::Sending;
        Ok(Some(command))
    }
    /// Full resync deliberately resets visible counters and presentation, never operation identity.
    pub fn resync(
        &mut self,
        scope: &str,
        next_seq: u64,
        frames: &[ServerEnvelope],
    ) -> Result<Option<ClientEnvelope>, DirectError> {
        if !valid_scope(scope)
            || next_seq == 0
            || frames.len() != 1
            || !matches!(frames[0].body(), ServerMessage::MatchUpdate { revision: 0, events, .. } if events.is_empty())
        {
            return Err(DirectError::Protocol);
        }
        let pending = self.pending.take();
        let old_scope = self.operation_scope.clone();
        self.operation_scope = Some(scope.to_owned());
        self.next_seq = Some(next_seq);
        self.frame = 0;
        self.revision = None;
        self.state = DirectState::Resyncing;
        self.receive(frames)?;
        match (old_scope, pending) {
            (Some(old), Some(command)) => self.restore_pending(&old, command),
            _ => Ok(None),
        }
    }
    /// A missing/expired receipt is unknown, so the new board stays read-only.
    pub fn mark_unknown(&mut self) {
        self.pending = None;
        self.unresolved = true;
        self.state = if self.revision.is_some() {
            DirectState::ReadOnly
        } else {
            DirectState::UnknownResult
        };
    }
    /// Whether an exact original command still awaits its definitive receipt.
    pub const fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    /// Failure closes the command gate; pending operations are never replayed.
    pub fn disconnect(&mut self) {
        self.state = DirectState::Disconnected;
    }
}
fn valid_scope(scope: &str) -> bool {
    scope.len() == 64
        && scope
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
/// Fixed public-safe boundary failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DirectError {
    /// Sending is not currently permitted.
    #[error("the online board is not ready to send")]
    Blocked,
    /// Output frames or visible revisions are not contiguous.
    #[error("the online stream is incomplete")]
    Gap,
    /// An initial sequence is invalid or a receipt is unexpected.
    #[error("the online receipt does not match its command")]
    Sequence,
    /// The opaque command exceeds protocol limits.
    #[error("the online command is malformed or too large")]
    Protocol,
}
#[cfg(test)]
mod tests {
    use super::*;
    use tabula_protocol::ErrorCode;
    fn client() -> DirectClient {
        DirectClient::new(
            MatchId(7),
            GameId::new("org.example.game").unwrap(),
            GameVersion::new("1.0.0").unwrap(),
            4,
        )
        .unwrap()
    }
    fn update(frame: u64, revision: u64) -> ServerEnvelope {
        ServerEnvelope::new(
            None,
            frame,
            ServerMessage::MatchUpdate {
                revision,
                view: vec![1],
                events: Vec::new(),
            },
        )
        .unwrap()
    }
    const SCOPE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[test]
    fn uncertainty_retains_exact_original_identity_until_fresh_same_scope_snapshot() {
        for next_seq in [4, 5] {
            let mut c = client();
            c.bind_scope(SCOPE).unwrap();
            c.receive(&[update(1, 0)]).unwrap();
            let original = c.command(vec![42]).unwrap();
            let old_generation = c.generation();
            c.recover();
            assert_eq!(c.state(), DirectState::UnknownResult);
            assert_eq!(c.revision(), None);
            assert_eq!(c.receive(&[update(1, 0)]), Err(DirectError::Blocked));
            assert_eq!(c.command(vec![99]), Err(DirectError::Blocked));
            assert!(!c
                .receive_generation(old_generation, &[update(2, 1)])
                .unwrap());
            let retry = c.resync(SCOPE, next_seq, &[update(1, 0)]).unwrap().unwrap();
            assert_eq!(retry, original);
            assert_eq!(c.state(), DirectState::Sending);
            c.receive(&[ServerEnvelope::new(Some(4), 2, ServerMessage::Ack { seq: 4 }).unwrap()])
                .unwrap();
            assert_eq!(c.command(vec![43]).unwrap().seq(), 5);
        }
    }
    #[test]
    fn changed_auth_epoch_or_seat_scope_never_replays_and_is_read_only() {
        let mut c = client();
        c.bind_scope(SCOPE).unwrap();
        c.receive(&[update(1, 0)]).unwrap();
        c.command(vec![42]).unwrap();
        c.recover();
        assert!(c
            .resync(&"b".repeat(64), 1, &[update(1, 0)])
            .unwrap()
            .is_none());
        assert_eq!(c.state(), DirectState::ReadOnly);
        assert_eq!(c.command(vec![42]), Err(DirectError::Blocked));
        assert!(!c.has_pending());
    }
    #[test]
    fn refresh_pending_is_a_hint_and_checks_match_game_version_and_sequence() {
        let mut source = client();
        source.receive(&[update(1, 0)]).unwrap();
        let original = source.command(vec![42]).unwrap();
        let mut restored = client();
        restored.bind_scope(SCOPE).unwrap();
        restored.receive(&[update(1, 0)]).unwrap();
        assert_eq!(
            restored.restore_pending(SCOPE, original.clone()).unwrap(),
            Some(original.clone())
        );
        for changed in [
            ClientEnvelope::new(5, 4, original.command().clone()).unwrap(),
            ClientEnvelope::new(
                4,
                4,
                GameCommandFrame::new(
                    MatchId(9),
                    original.command().game().clone(),
                    original.command().game_version().clone(),
                    vec![42],
                )
                .unwrap(),
            )
            .unwrap(),
            ClientEnvelope::new(
                4,
                4,
                GameCommandFrame::new(
                    MatchId(7),
                    GameId::new("org.other.game").unwrap(),
                    original.command().game_version().clone(),
                    vec![42],
                )
                .unwrap(),
            )
            .unwrap(),
            ClientEnvelope::new(
                4,
                4,
                GameCommandFrame::new(
                    MatchId(7),
                    original.command().game().clone(),
                    GameVersion::new("2.0.0").unwrap(),
                    vec![42],
                )
                .unwrap(),
            )
            .unwrap(),
        ] {
            let mut restored = client();
            restored.bind_scope(SCOPE).unwrap();
            restored.receive(&[update(1, 0)]).unwrap();
            assert!(restored.restore_pending(SCOPE, changed).unwrap().is_none());
            assert_eq!(restored.state(), DirectState::ReadOnly);
        }
    }
    #[test]
    fn unavailable_or_retired_receipt_is_unknown_and_never_claims_failed_commit() {
        for error in [
            ErrorCode::StaleSeq,
            ErrorCode::Unavailable,
            ErrorCode::OperationConflict,
        ] {
            let mut c = client();
            c.receive(&[update(1, 0)]).unwrap();
            c.command(vec![42]).unwrap();
            c.receive(&[
                ServerEnvelope::new(Some(4), 2, ServerMessage::Reject { seq: 4, error }).unwrap(),
            ])
            .unwrap();
            assert_eq!(c.state(), DirectState::ReadOnly);
            assert_eq!(c.command(vec![42]), Err(DirectError::Blocked));
        }
    }
    #[test]
    fn one_pending_command_until_matching_receipt() {
        let mut c = client();
        assert_eq!(c.command(vec![1]), Err(DirectError::Blocked));
        c.receive(&[update(1, 0)]).unwrap();
        assert_eq!(c.command(vec![2]).unwrap().seq(), 4);
        assert_eq!(c.command(vec![3]), Err(DirectError::Blocked));
        c.receive(&[ServerEnvelope::new(
            Some(4),
            2,
            ServerMessage::Reject {
                seq: 4,
                error: ErrorCode::RuleRejected,
            },
        )
        .unwrap()])
            .unwrap();
        assert_eq!(c.revision(), Some(0));
        assert_eq!(c.command(vec![3]).unwrap().seq(), 5);
    }
    #[test]
    fn disconnect_never_replays_or_sends() {
        let mut c = client();
        c.receive(&[update(1, 0), update(2, 1)]).unwrap();
        c.command(vec![2]).unwrap();
        c.disconnect();
        assert_eq!(c.revision(), Some(1));
        assert_eq!(c.command(vec![2]), Err(DirectError::Blocked));
        assert_eq!(c.receive(&[]), Err(DirectError::Blocked));
    }
    #[test]
    fn gaps_and_unmatched_receipts_are_atomic() {
        let mut c = client();
        c.receive(&[update(1, 0)]).unwrap();
        assert_eq!(c.receive(&[update(3, 1)]), Err(DirectError::Gap));
        assert_eq!(c.revision(), Some(0));
        let mut c = client();
        c.receive(&[update(1, 0)]).unwrap();
        c.command(vec![2]).unwrap();
        assert_eq!(
            c.receive(&[
                update(2, 1),
                ServerEnvelope::new(None, 3, ServerMessage::Ack { seq: 99 }).unwrap()
            ]),
            Err(DirectError::Sequence)
        );
        assert_eq!(c.revision(), Some(0));
    }
    #[test]
    fn first_snapshot_and_visible_revisions_are_exact() {
        for revision in [1, 9] {
            let mut c = client();
            assert_eq!(c.receive(&[update(1, revision)]), Err(DirectError::Gap));
        }
        for next in [0, 2, u64::MAX] {
            let mut c = client();
            c.receive(&[update(1, 0)]).unwrap();
            assert_eq!(c.receive(&[update(2, next)]), Err(DirectError::Gap));
            assert_eq!(c.revision(), Some(0));
        }
    }
    #[test]
    fn exhausted_sequence_closes_gate_after_final_receipt() {
        let mut c = DirectClient::new(
            MatchId(7),
            GameId::new("org.example.game").unwrap(),
            GameVersion::new("1.0.0").unwrap(),
            u64::MAX,
        )
        .unwrap();
        c.receive(&[update(1, 0)]).unwrap();
        assert_eq!(c.command(vec![1]).unwrap().seq(), u64::MAX);
        assert_eq!(c.state(), DirectState::Sending);
        c.receive(&[
            ServerEnvelope::new(Some(u64::MAX), 2, ServerMessage::Ack { seq: u64::MAX }).unwrap(),
        ])
        .unwrap();
        assert_eq!(c.state(), DirectState::Disconnected);
        assert_eq!(c.command(vec![1]), Err(DirectError::Blocked));
    }
    #[test]
    fn rejected_payload_does_not_consume_sequence_or_change_gate() {
        let mut c = client();
        c.receive(&[update(1, 0)]).unwrap();
        assert_eq!(
            c.command(vec![0; tabula_protocol::MAX_GAME_PAYLOAD_BYTES + 1]),
            Err(DirectError::Protocol)
        );
        assert_eq!(c.state(), DirectState::Ready);
        assert_eq!(c.revision(), Some(0));
        assert_eq!(
            c.command(vec![0; tabula_protocol::MAX_GAME_PAYLOAD_BYTES])
                .unwrap()
                .seq(),
            4
        );
    }

    #[test]
    fn projection_updates_do_not_release_pending_command() {
        let mut c = client();
        c.receive(&[update(1, 0)]).unwrap();
        let command = c.command(vec![2]).unwrap();
        assert_eq!(command.command().match_id(), MatchId(7));
        assert_eq!(command.command().game().as_str(), "org.example.game");
        assert_eq!(command.command().game_version().as_str(), "1.0.0");
        assert_eq!(command.command().payload(), &[2]);
        c.receive(&[update(2, 1)]).unwrap();
        assert_eq!(c.state(), DirectState::Sending);
        assert_eq!(c.command(vec![3]), Err(DirectError::Blocked));
        c.receive(&[ServerEnvelope::new(Some(4), 3, ServerMessage::Ack { seq: 4 }).unwrap()])
            .unwrap();
        assert_eq!(c.state(), DirectState::Ready);
        assert_eq!(c.command(vec![3]).unwrap().seq(), 5);
    }
}
