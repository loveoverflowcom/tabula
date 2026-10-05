use serde::{Deserialize, Deserializer, Serialize};

/// A wire compatibility version (doc 05 §3); this slice supports only 0.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Positional-encoding compatibility generation.
    pub major: u16,
    /// Version within that generation.
    pub minor: u16,
}

/// First executable, isolated pre-release contract under ADR-0039, not v1.
pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 0, minor: 1 };

pub(crate) fn supported<'de, D>(deserializer: D) -> Result<ProtocolVersion, D::Error>
where
    D: Deserializer<'de>,
{
    let version = ProtocolVersion::deserialize(deserializer)?;
    if version != PROTOCOL_VERSION {
        return Err(serde::de::Error::custom("unsupported protocol version"));
    }
    Ok(version)
}
