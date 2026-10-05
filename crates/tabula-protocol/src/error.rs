use serde::{Deserialize, Serialize};

/// Public-safe rejection classes, never game diagnostics (I-5; doc 05 §9.3).
/// Variant order is part of the 0.1 Postcard contract (I-13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// Invalid bounded frame or game-command encoding.
    Malformed,
    /// The trusted attachment does not authorize this operation.
    Unauthorized,
    /// Command targets a different match.
    WrongMatch,
    /// Command game identity/version differs from the actor's module.
    WrongGame,
    /// Bounded actor mailbox has no capacity.
    Busy,
    /// Sequence has fallen outside the retained retry window.
    StaleSeq,
    /// Sequence is beyond the actor's accepted window.
    SeqTooFar,
    /// Reused operation identity has different command bytes.
    OperationConflict,
    /// Rules rejected the command; no private reason is exposed.
    RuleRejected,
    /// Authority or durable append is unavailable.
    Unavailable,
    /// Match authority is terminal.
    Terminal,
}

/// Local boundary failure; details are intentionally fixed and non-sensitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    /// The isolated slice supports only its exact advertised version.
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    /// Client sequences and isolated attachment frame counters start at one.
    #[error("sequence or frame counter must be nonzero")]
    ZeroCounter,
    /// An opaque field, identity, collection, or frame exceeded a fixed cap.
    #[error("wire limit exceeded")]
    LimitExceeded,
    /// The codec could not encode or decode the given representation.
    #[error("malformed wire representation")]
    Malformed,
    /// A positional frame must be consumed completely.
    #[error("trailing bytes in postcard frame")]
    TrailingBytes,
}
