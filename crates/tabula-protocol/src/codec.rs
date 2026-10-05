use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    ClientEnvelope, ServerEnvelope, WireError, MAX_INBOUND_FRAME_BYTES, MAX_OUTBOUND_FRAME_BYTES,
};

/// Shared binary and debugging codecs without any runtime (doc 05 §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    /// Positional binary format; every byte must belong to this frame.
    Postcard,
    /// Human-readable debugging format; unknown fields are forward-compatible.
    Json,
}

/// Encode one client operation and enforce the actual 64 KiB frame cap.
pub fn encode_client(codec: Codec, envelope: &ClientEnvelope) -> Result<Vec<u8>, WireError> {
    encode(codec, envelope, MAX_INBOUND_FRAME_BYTES)
}

/// Decode one bounded client frame before invoking any game or actor code.
pub fn decode_client(codec: Codec, bytes: &[u8]) -> Result<ClientEnvelope, WireError> {
    decode(codec, bytes, MAX_INBOUND_FRAME_BYTES)
}

/// Encode one projected server output and enforce the actual 1 MiB frame cap.
pub fn encode_server(codec: Codec, envelope: &ServerEnvelope) -> Result<Vec<u8>, WireError> {
    encode(codec, envelope, MAX_OUTBOUND_FRAME_BYTES)
}

/// Decode one bounded projected server frame with the same serde invariants.
pub fn decode_server(codec: Codec, bytes: &[u8]) -> Result<ServerEnvelope, WireError> {
    decode(codec, bytes, MAX_OUTBOUND_FRAME_BYTES)
}

fn encode<T: Serialize>(codec: Codec, value: &T, limit: usize) -> Result<Vec<u8>, WireError> {
    let bytes = match codec {
        Codec::Postcard => postcard::to_allocvec(value).map_err(|_| WireError::Malformed)?,
        Codec::Json => serde_json::to_vec(value).map_err(|_| WireError::Malformed)?,
    };
    if bytes.len() > limit {
        return Err(WireError::LimitExceeded);
    }
    Ok(bytes)
}

fn decode<T: DeserializeOwned>(codec: Codec, bytes: &[u8], limit: usize) -> Result<T, WireError> {
    if bytes.len() > limit {
        return Err(WireError::LimitExceeded);
    }
    match codec {
        Codec::Postcard => {
            let (value, remaining) =
                postcard::take_from_bytes(bytes).map_err(|_| WireError::Malformed)?;
            if !remaining.is_empty() {
                return Err(WireError::TrailingBytes);
            }
            Ok(value)
        }
        Codec::Json => serde_json::from_slice(bytes).map_err(|_| WireError::Malformed),
    }
}
