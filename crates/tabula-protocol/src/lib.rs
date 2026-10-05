//! # `tabula-protocol` — bounded, opaque offline wire contracts
//!
//! ADR-0039 opens only this isolated pre-release actor slice. Full Phase 4,
//! production listeners, negotiation, resume and platform messages remain
//! gated (doc 07). Version 0.1 is not the future production v1 protocol.
//!
//! Game bytes are opaque (ADR-008, I-9). Outputs carry projections and redacted
//! events only, never canonical state, seed, input index or authority identities
//! (I-5/I-6; doc 05 §9). Observable revisions belong to one attachment, so a
//! private action cannot disclose its existence through canonical version gaps.
//!
//! Private envelope/frame fields and checked serde conversions preserve limits
//! even when callers use ordinary serde instead of the frame codecs. Frame
//! codecs additionally bound the actual encoded input/output, including JSON
//! whitespace and unknown fields (doc 05 §9.1). JSON ignores unknown fields;
//! Postcard rejects trailing bytes. Golden vectors pin the positional encoding
//! for every wire type (I-13; doc 05 §3.3).

#![forbid(unsafe_code)]

mod codec;
mod envelope;
mod error;
mod limits;
mod version;

pub use codec::{decode_client, decode_server, encode_client, encode_server, Codec};
pub use envelope::{ClientEnvelope, GameCommandFrame, ServerEnvelope, ServerMessage};
pub use error::{ErrorCode, WireError};
pub use limits::{
    MAX_EVENTS, MAX_EVENT_BYTES, MAX_GAME_ID_BYTES, MAX_GAME_PAYLOAD_BYTES, MAX_GAME_VERSION_BYTES,
    MAX_INBOUND_FRAME_BYTES, MAX_OUTBOUND_FRAME_BYTES, MAX_VIEW_BYTES,
};
pub use version::{ProtocolVersion, PROTOCOL_VERSION};
