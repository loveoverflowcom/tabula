use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tabula_core::MatchId;
use tabula_protocol::{ClientEnvelope, ServerEnvelope};
/// Isolated admission HTTP version, independent of wire 0.1 (ADR-0041).
pub const MATCH_HTTP_VERSION: u16 = 1;
/// Complete request JSON bound.
pub const MAX_REQUEST_BYTES: usize = 65_536;
/// Per-attachment retained frame bound.
pub const MAX_BUFFERED_FRAMES: usize = 16;
/// Complete private JSON body bound.
pub const MAX_RESPONSE_BYTES: usize = 2_097_152;
/// Public-safe malformed HTTP carrier error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidMatchHttp;
impl std::fmt::Display for InvalidMatchHttp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid match HTTP carrier")
    }
}
impl std::error::Error for InvalidMatchHttp {}
fn identifier(v: &str) -> Result<(), InvalidMatchHttp> {
    if v.len() == 32
        && v.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        && v != "00000000000000000000000000000000"
    {
        Ok(())
    } else {
        Err(InvalidMatchHttp)
    }
}
fn binding(v: &str) -> Result<(), InvalidMatchHttp> {
    if (64..=2048).contains(&v.len())
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_ .".contains(&c))
        && !v.contains(' ')
    {
        Ok(())
    } else {
        Err(InvalidMatchHttp)
    }
}
/// A public routing hint, never authorization.
pub fn parse_match_id(v: &str) -> Result<MatchId, InvalidMatchHttp> {
    identifier(v)?;
    u128::from_str_radix(v, 16)
        .map(MatchId)
        .map_err(|_| InvalidMatchHttp)
}

/// Bounded raw setup draft; the registry owns game meaning.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchCreateRequest")]
pub struct MatchCreateRequest {
    version: u16,
    game_id: String,
    seats: u8,
    config: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchCreateRequest {
    version: u16,
    #[serde(deserialize_with = "crate::bounds::game")]
    game_id: String,
    seats: u8,
    #[serde(deserialize_with = "crate::bounds::config")]
    config: BTreeMap<String, String>,
}
impl MatchCreateRequest {
    pub fn new(
        game_id: String,
        seats: u8,
        config: BTreeMap<String, String>,
    ) -> Result<Self, InvalidMatchHttp> {
        if game_id.is_empty()
            || game_id.len() > 128
            || !(2..=8).contains(&seats)
            || config.len() > 16
            || config
                .iter()
                .any(|(k, v)| k.is_empty() || k.len() > 64 || v.len() > 128)
        {
            return Err(InvalidMatchHttp);
        }
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            game_id,
            seats,
            config,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn game_id(&self) -> &str {
        &self.game_id
    }
    pub fn seats(&self) -> u8 {
        self.seats
    }
    pub fn config(&self) -> &BTreeMap<String, String> {
        &self.config
    }
}
impl TryFrom<RawMatchCreateRequest> for MatchCreateRequest {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchCreateRequest) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(raw.game_id, raw.seats, raw.config)
    }
}
impl std::fmt::Debug for MatchCreateRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchCreateRequest([REDACTED])")
    }
}

/// A bounded code attempt; never a client-selected seat.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchJoinRequest")]
pub struct MatchJoinRequest {
    version: u16,
    code: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchJoinRequest {
    version: u16,
    #[serde(deserialize_with = "crate::bounds::short")]
    code: String,
}
impl MatchJoinRequest {
    pub fn new(code: String) -> Result<Self, InvalidMatchHttp> {
        if code.len() > 64 {
            return Err(InvalidMatchHttp);
        }
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            code,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn code(&self) -> &str {
        &self.code
    }
}
impl TryFrom<RawMatchJoinRequest> for MatchJoinRequest {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchJoinRequest) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(raw.code)
    }
}
impl std::fmt::Debug for MatchJoinRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchJoinRequest([REDACTED])")
    }
}

/// Request a fresh permission-checked in-memory grant.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchGrantRequest")]
pub struct MatchGrantRequest {
    version: u16,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchGrantRequest {
    version: u16,
}
impl MatchGrantRequest {
    pub fn new() -> Result<Self, InvalidMatchHttp> {
        Ok(Self {
            version: MATCH_HTTP_VERSION,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
}
impl TryFrom<RawMatchGrantRequest> for MatchGrantRequest {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchGrantRequest) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new()
    }
}
impl std::fmt::Debug for MatchGrantRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchGrantRequest([REDACTED])")
    }
}

/// A signed account/session/match/viewer-scoped grant.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchAttachRequest")]
pub struct MatchAttachRequest {
    version: u16,
    binding_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchAttachRequest {
    version: u16,
    #[serde(deserialize_with = "crate::bounds::token")]
    binding_id: String,
}
impl MatchAttachRequest {
    pub fn new(binding_id: String) -> Result<Self, InvalidMatchHttp> {
        binding(&binding_id)?;
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            binding_id,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }
}
impl TryFrom<RawMatchAttachRequest> for MatchAttachRequest {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchAttachRequest) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(raw.binding_id)
    }
}
impl std::fmt::Debug for MatchAttachRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchAttachRequest([REDACTED])")
    }
}

/// Attachment hint and the existing checked opaque wire command.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchCommandRequest")]
pub struct MatchCommandRequest {
    version: u16,
    attachment_id: String,
    command: ClientEnvelope,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchCommandRequest {
    version: u16,
    #[serde(deserialize_with = "crate::bounds::id")]
    attachment_id: String,
    command: ClientEnvelope,
}
impl MatchCommandRequest {
    pub fn new(attachment_id: String, command: ClientEnvelope) -> Result<Self, InvalidMatchHttp> {
        identifier(&attachment_id)?;
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            attachment_id,
            command,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn attachment_id(&self) -> &str {
        &self.attachment_id
    }
    pub fn command(&self) -> &ClientEnvelope {
        &self.command
    }
}
impl TryFrom<RawMatchCommandRequest> for MatchCommandRequest {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchCommandRequest) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(raw.attachment_id, raw.command)
    }
}
impl std::fmt::Debug for MatchCommandRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchCommandRequest([REDACTED])")
    }
}

/// Read a bounded attachment queue without renewing idle authority.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchPollRequest")]
pub struct MatchPollRequest {
    version: u16,
    attachment_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchPollRequest {
    version: u16,
    #[serde(deserialize_with = "crate::bounds::id")]
    attachment_id: String,
}
impl MatchPollRequest {
    pub fn new(attachment_id: String) -> Result<Self, InvalidMatchHttp> {
        identifier(&attachment_id)?;
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            attachment_id,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn attachment_id(&self) -> &str {
        &self.attachment_id
    }
}
impl TryFrom<RawMatchPollRequest> for MatchPollRequest {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchPollRequest) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(raw.attachment_id)
    }
}
impl std::fmt::Debug for MatchPollRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchPollRequest([REDACTED])")
    }
}

/// Server-assigned display seat and non-secret routing hints.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchAdmission")]
pub struct MatchAdmission {
    version: u16,
    match_id: String,
    game_id: String,
    game_version: String,
    seat: u8,
    join_code: Option<String>,
    ready: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchAdmission {
    version: u16,
    #[serde(deserialize_with = "crate::bounds::id")]
    match_id: String,
    #[serde(deserialize_with = "crate::bounds::game")]
    game_id: String,
    #[serde(deserialize_with = "crate::bounds::short")]
    game_version: String,
    seat: u8,
    #[serde(deserialize_with = "crate::bounds::optional_code")]
    join_code: Option<String>,
    ready: bool,
}
impl MatchAdmission {
    pub fn new(
        match_id: String,
        game_id: String,
        game_version: String,
        seat: u8,
        join_code: Option<String>,
        ready: bool,
    ) -> Result<Self, InvalidMatchHttp> {
        identifier(&match_id)?;
        if game_id.is_empty()
            || game_id.len() > 128
            || game_version.is_empty()
            || game_version.len() > 64
            || seat > 7
            || join_code.as_ref().is_some_and(|c| {
                c.len() != 12
                    || !c
                        .bytes()
                        .all(|b| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&b))
            })
        {
            return Err(InvalidMatchHttp);
        }
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            match_id,
            game_id,
            game_version,
            seat,
            join_code,
            ready,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn match_id(&self) -> &str {
        &self.match_id
    }
    pub fn game_id(&self) -> &str {
        &self.game_id
    }
    pub fn game_version(&self) -> &str {
        &self.game_version
    }
    pub fn seat(&self) -> u8 {
        self.seat
    }
    pub fn join_code(&self) -> Option<&str> {
        self.join_code.as_deref()
    }
    pub fn ready(&self) -> bool {
        self.ready
    }
}
impl TryFrom<RawMatchAdmission> for MatchAdmission {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchAdmission) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(
            raw.match_id,
            raw.game_id,
            raw.game_version,
            raw.seat,
            raw.join_code,
            raw.ready,
        )
    }
}
impl std::fmt::Debug for MatchAdmission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchAdmission([REDACTED])")
    }
}

/// Short-lived grant held only in document memory.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchGrant")]
pub struct MatchGrant {
    version: u16,
    ready: bool,
    binding_id: Option<String>,
    seat: u8,
    game_id: String,
    game_version: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchGrant {
    version: u16,
    ready: bool,
    #[serde(deserialize_with = "crate::bounds::optional_token")]
    binding_id: Option<String>,
    seat: u8,
    #[serde(deserialize_with = "crate::bounds::game")]
    game_id: String,
    #[serde(deserialize_with = "crate::bounds::short")]
    game_version: String,
}
impl MatchGrant {
    pub fn new(
        ready: bool,
        binding_id: Option<String>,
        seat: u8,
        game_id: String,
        game_version: String,
    ) -> Result<Self, InvalidMatchHttp> {
        if ready != binding_id.is_some()
            || seat > 7
            || game_id.is_empty()
            || game_id.len() > 128
            || game_version.is_empty()
            || game_version.len() > 64
        {
            return Err(InvalidMatchHttp);
        }
        if let Some(v) = &binding_id {
            binding(v)?;
        }
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            ready,
            binding_id,
            seat,
            game_id,
            game_version,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn ready(&self) -> bool {
        self.ready
    }
    pub fn binding_id(&self) -> Option<&str> {
        self.binding_id.as_deref()
    }
    pub fn seat(&self) -> u8 {
        self.seat
    }
    pub fn game_id(&self) -> &str {
        &self.game_id
    }
    pub fn game_version(&self) -> &str {
        &self.game_version
    }
}
impl TryFrom<RawMatchGrant> for MatchGrant {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchGrant) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(
            raw.ready,
            raw.binding_id,
            raw.seat,
            raw.game_id,
            raw.game_version,
        )
    }
}
impl std::fmt::Debug for MatchGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchGrant([REDACTED])")
    }
}

/// Fresh visible stream and durable next-sequence hint.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchAttachment")]
pub struct MatchAttachment {
    version: u16,
    attachment_id: String,
    seat: u8,
    next_seq: u64,
    frames: Vec<ServerEnvelope>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchAttachment {
    version: u16,
    #[serde(deserialize_with = "crate::bounds::id")]
    attachment_id: String,
    seat: u8,
    next_seq: u64,
    #[serde(deserialize_with = "crate::bounds::frames")]
    frames: Vec<ServerEnvelope>,
}
impl MatchAttachment {
    pub fn new(
        attachment_id: String,
        seat: u8,
        next_seq: u64,
        frames: Vec<ServerEnvelope>,
    ) -> Result<Self, InvalidMatchHttp> {
        identifier(&attachment_id)?;
        if seat > 7 || next_seq == 0 || frames.len() > MAX_BUFFERED_FRAMES {
            return Err(InvalidMatchHttp);
        }
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            attachment_id,
            seat,
            next_seq,
            frames,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn attachment_id(&self) -> &str {
        &self.attachment_id
    }
    pub fn seat(&self) -> u8 {
        self.seat
    }
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }
    pub fn frames(&self) -> &[ServerEnvelope] {
        &self.frames
    }
}
impl TryFrom<RawMatchAttachment> for MatchAttachment {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchAttachment) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(raw.attachment_id, raw.seat, raw.next_seq, raw.frames)
    }
}
impl std::fmt::Debug for MatchAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchAttachment([REDACTED])")
    }
}

/// Existing projection/redacted-event/receipt envelopes only.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawMatchFrames")]
pub struct MatchFrames {
    version: u16,
    frames: Vec<ServerEnvelope>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMatchFrames {
    version: u16,
    #[serde(deserialize_with = "crate::bounds::frames")]
    frames: Vec<ServerEnvelope>,
}
impl MatchFrames {
    pub fn new(frames: Vec<ServerEnvelope>) -> Result<Self, InvalidMatchHttp> {
        if frames.len() > MAX_BUFFERED_FRAMES {
            return Err(InvalidMatchHttp);
        }
        Ok(Self {
            version: MATCH_HTTP_VERSION,
            frames,
        })
    }
    pub const fn version(&self) -> u16 {
        self.version
    }
    pub fn frames(&self) -> &[ServerEnvelope] {
        &self.frames
    }
}
impl TryFrom<RawMatchFrames> for MatchFrames {
    type Error = InvalidMatchHttp;
    fn try_from(raw: RawMatchFrames) -> Result<Self, Self::Error> {
        if raw.version != MATCH_HTTP_VERSION {
            return Err(InvalidMatchHttp);
        }
        Self::new(raw.frames)
    }
}
impl std::fmt::Debug for MatchFrames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MatchFrames([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_serde_is_checked() {
        for body in [r#"{"version":2}"#, r#"{"version":1,"seat":0}"#] {
            assert!(serde_json::from_str::<MatchGrantRequest>(body).is_err());
        }
        assert!(MatchCreateRequest::new("x".into(), 1, BTreeMap::new()).is_err());
        assert!(parse_match_id("00000000000000000000000000000000").is_err());
        assert!(MatchJoinRequest::new("x".repeat(65)).is_err());
    }
    #[test]
    fn stable_minimal_vectors() {
        assert_eq!(
            serde_json::to_string(&MatchGrantRequest::new().unwrap()).unwrap(),
            r#"{"version":1}"#
        );
        let v = MatchJoinRequest::new("ABCD2345EFGH".into()).unwrap();
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            r#"{"version":1,"code":"ABCD2345EFGH"}"#
        );
    }
}
