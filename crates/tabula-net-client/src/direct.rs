//! Bounded PR2 direct-play sequencing (doc 04 §4, ADR-0041).
//! No credentials, game state, reconnect or replay is stored here.
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
    /// Transport/protocol failed or sequences exhausted; no commands can replay.
    Disconnected,
}

/// One authorized opaque stream with a single pending command.
#[derive(Debug)]
pub struct DirectClient {
    match_id: MatchId,
    game: GameId,
    game_version: GameVersion,
    next_seq: Option<u64>,
    pending: Option<u64>,
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
        self.pending = Some(seq);
        self.next_seq = seq.checked_add(1);
        self.state = DirectState::Sending;
        Ok(command)
    }
    /// Validate the complete ordered batch before exposing its output.
    pub fn receive(&mut self, frames: &[ServerEnvelope]) -> Result<(), DirectError> {
        if self.state == DirectState::Disconnected {
            return Err(DirectError::Blocked);
        }
        let mut next_frame = self.frame;
        let mut revision = self.revision;
        let mut pending = self.pending;
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
                    if pending != Some(*seq) {
                        self.disconnect();
                        return Err(DirectError::Sequence);
                    }
                    pending = None;
                }
            }
        }
        self.frame = next_frame;
        self.revision = revision;
        self.pending = pending;
        self.state = if pending.is_some() {
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
    /// Failure closes the command gate; pending operations are never replayed.
    pub fn disconnect(&mut self) {
        self.state = DirectState::Disconnected;
    }
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
