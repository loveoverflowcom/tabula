use serde::{Deserialize, Serialize};
use tabula_core::{GameId, GameVersion, MatchId};

use crate::{limits, version, ErrorCode, ProtocolVersion, WireError, PROTOCOL_VERSION};

/// The only inbound carrier of game-specific bytes (ADR-008; doc 05 §2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawGameCommandFrame")]
pub struct GameCommandFrame {
    match_id: MatchId,
    game: GameId,
    game_version: GameVersion,
    payload: Vec<u8>,
}

#[derive(Deserialize)]
struct RawGameCommandFrame {
    match_id: MatchId,
    #[serde(deserialize_with = "limits::game_id")]
    game: GameId,
    #[serde(deserialize_with = "limits::game_version")]
    game_version: GameVersion,
    #[serde(deserialize_with = "limits::payload")]
    payload: Vec<u8>,
}

impl GameCommandFrame {
    /// Constructs a bounded command without interpreting opaque game bytes.
    pub fn new(
        match_id: MatchId,
        game: GameId,
        game_version: GameVersion,
        payload: Vec<u8>,
    ) -> Result<Self, WireError> {
        if game.as_str().len() > limits::MAX_GAME_ID_BYTES
            || game_version.as_str().len() > limits::MAX_GAME_VERSION_BYTES
            || payload.len() > limits::MAX_GAME_PAYLOAD_BYTES
        {
            return Err(WireError::LimitExceeded);
        }
        Ok(Self {
            match_id,
            game,
            game_version,
            payload,
        })
    }

    /// Match routing target, checked against trusted actor ownership.
    pub const fn match_id(&self) -> MatchId {
        self.match_id
    }

    /// Registered identity; the platform must never branch on a literal (I-9).
    pub const fn game(&self) -> &GameId {
        &self.game
    }

    /// Module version expected by this command.
    pub const fn game_version(&self) -> &GameVersion {
        &self.game_version
    }

    /// Opaque command bytes, decoded only through the erased module.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl TryFrom<RawGameCommandFrame> for GameCommandFrame {
    type Error = WireError;

    fn try_from(raw: RawGameCommandFrame) -> Result<Self, Self::Error> {
        Self::new(raw.match_id, raw.game, raw.game_version, raw.payload)
    }
}

/// Validated isolated client operation (doc 05 §2; ADR-0039).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawClientEnvelope")]
pub struct ClientEnvelope {
    v: ProtocolVersion,
    seq: u64,
    corr: u64,
    command: GameCommandFrame,
}

#[derive(Deserialize)]
struct RawClientEnvelope {
    #[serde(deserialize_with = "version::supported")]
    v: ProtocolVersion,
    #[serde(deserialize_with = "limits::nonzero")]
    seq: u64,
    corr: u64,
    command: GameCommandFrame,
}

impl ClientEnvelope {
    /// Sequences start at one; correlations are opaque and may be zero.
    pub fn new(seq: u64, corr: u64, command: GameCommandFrame) -> Result<Self, WireError> {
        if seq == 0 {
            return Err(WireError::ZeroCounter);
        }
        Ok(Self {
            v: PROTOCOL_VERSION,
            seq,
            corr,
            command,
        })
    }

    /// Exact pre-release version validated at the boundary.
    pub const fn version(&self) -> ProtocolVersion {
        self.v
    }
    /// Client-assigned operation sequence.
    pub const fn seq(&self) -> u64 {
        self.seq
    }
    /// Response correlation, never authentication or authority.
    pub const fn corr(&self) -> u64 {
        self.corr
    }
    /// The validated game command frame.
    pub const fn command(&self) -> &GameCommandFrame {
        &self.command
    }
}

impl TryFrom<RawClientEnvelope> for ClientEnvelope {
    type Error = WireError;

    fn try_from(raw: RawClientEnvelope) -> Result<Self, Self::Error> {
        if raw.v != PROTOCOL_VERSION {
            return Err(WireError::UnsupportedVersion);
        }
        Self::new(raw.seq, raw.corr, raw.command)
    }
}

/// Public-safe actor output (I-5/I-6; doc 05 §9.3; ADR-0039).
/// No variant carries canonical state/version/index, seeds, or authority IDs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawServerMessage")]
pub enum ServerMessage {
    /// Acknowledges only this client's operation, with no canonical counter.
    Ack {
        /// Accepted nonzero client sequence.
        seq: u64,
    },
    /// Rejects an operation using a fixed public-safe class.
    Reject {
        /// Rejected nonzero client sequence.
        seq: u64,
        /// No game diagnostics or private reason are included.
        error: ErrorCode,
    },
    /// Snapshot and events projected for exactly one authorized attachment.
    MatchUpdate {
        /// Per-attachment observable revision, never the canonical version.
        revision: u64,
        /// Opaque projected view, not canonical state.
        view: Vec<u8>,
        /// Opaque redacted events; invisible events are absent altogether.
        events: Vec<Vec<u8>>,
    },
}

#[derive(Deserialize)]
enum RawServerMessage {
    Ack {
        #[serde(deserialize_with = "limits::nonzero")]
        seq: u64,
    },
    Reject {
        #[serde(deserialize_with = "limits::nonzero")]
        seq: u64,
        error: ErrorCode,
    },
    MatchUpdate {
        revision: u64,
        #[serde(deserialize_with = "limits::view")]
        view: Vec<u8>,
        #[serde(deserialize_with = "limits::events")]
        events: Vec<Vec<u8>>,
    },
}

impl ServerMessage {
    /// Validates manually built variants before they cross an envelope boundary.
    pub fn validate(&self) -> Result<(), WireError> {
        match self {
            Self::Ack { seq } | Self::Reject { seq, .. } if *seq == 0 => {
                return Err(WireError::ZeroCounter);
            }
            Self::MatchUpdate { view, events, .. }
                if view.len() > limits::MAX_VIEW_BYTES
                    || events.len() > limits::MAX_EVENTS
                    || events
                        .iter()
                        .any(|event| event.len() > limits::MAX_EVENT_BYTES) =>
            {
                return Err(WireError::LimitExceeded);
            }
            _ => {}
        }
        check_postcard_size(self, limits::MAX_OUTBOUND_FRAME_BYTES)
    }
}

impl TryFrom<RawServerMessage> for ServerMessage {
    type Error = WireError;

    fn try_from(raw: RawServerMessage) -> Result<Self, Self::Error> {
        let body = match raw {
            RawServerMessage::Ack { seq } => Self::Ack { seq },
            RawServerMessage::Reject { seq, error } => Self::Reject { seq, error },
            RawServerMessage::MatchUpdate {
                revision,
                view,
                events,
            } => Self::MatchUpdate {
                revision,
                view,
                events,
            },
        };
        body.validate()?;
        Ok(body)
    }
}

/// One bounded projected output with a per-attachment frame counter (ADR-0039).
/// A new explicit attachment snapshot starts a new isolated output stream;
/// full-connection multiplexing/reconnect counters remain a host/network gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawServerEnvelope")]
pub struct ServerEnvelope {
    v: ProtocolVersion,
    corr: Option<u64>,
    frame: u64,
    body: ServerMessage,
}

#[derive(Deserialize)]
struct RawServerEnvelope {
    #[serde(deserialize_with = "version::supported")]
    v: ProtocolVersion,
    corr: Option<u64>,
    #[serde(deserialize_with = "limits::nonzero")]
    frame: u64,
    body: ServerMessage,
}

impl ServerEnvelope {
    /// Validates counters, projected-field caps and complete Postcard frame size.
    /// The selected codec additionally checks its actual encoded frame size.
    pub fn new(corr: Option<u64>, frame: u64, body: ServerMessage) -> Result<Self, WireError> {
        if frame == 0 {
            return Err(WireError::ZeroCounter);
        }
        body.validate()?;
        let envelope = Self {
            v: PROTOCOL_VERSION,
            corr,
            frame,
            body,
        };
        check_postcard_size(&envelope, limits::MAX_OUTBOUND_FRAME_BYTES)?;
        Ok(envelope)
    }

    /// Exact pre-release version validated at the boundary.
    pub const fn version(&self) -> ProtocolVersion {
        self.v
    }
    /// Correlation only when caused by a client operation.
    pub const fn corr(&self) -> Option<u64> {
        self.corr
    }
    /// Nonzero isolated attachment-stream counter, not a canonical match counter.
    pub const fn frame(&self) -> u64 {
        self.frame
    }
    /// The public-safe acknowledgment, rejection or projected update.
    pub const fn body(&self) -> &ServerMessage {
        &self.body
    }
}

impl TryFrom<RawServerEnvelope> for ServerEnvelope {
    type Error = WireError;

    fn try_from(raw: RawServerEnvelope) -> Result<Self, Self::Error> {
        if raw.v != PROTOCOL_VERSION {
            return Err(WireError::UnsupportedVersion);
        }
        Self::new(raw.corr, raw.frame, raw.body)
    }
}

fn check_postcard_size<T: Serialize>(value: &T, limit: usize) -> Result<(), WireError> {
    let size = postcard::serialize_with_flavor::<T, postcard::ser_flavors::Size, usize>(
        value,
        postcard::ser_flavors::Size::default(),
    )
    .map_err(|_| WireError::Malformed)?;
    if size > limit {
        return Err(WireError::LimitExceeded);
    }
    Ok(())
}
