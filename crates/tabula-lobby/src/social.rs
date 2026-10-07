//! Isolated participant-authorized friends and presence contracts (doc 03 §14).
//! This module has no SQL, transport, registry or game authority.

#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
use std::future::Future;

use serde::{Deserialize, Serialize};
use tabula_core::UserId;
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
use tabula_session::{
    CredentialOperation, HttpSessionAuthority, SessionBinding, SocketFramePublication,
};

/// Separate account/social compatibility generation; match wire is unchanged.
pub const SOCIAL_CONTRACT_VERSION: u16 = 2;
/// Maximum friends or visible request rows in one bounded read.
pub const MAX_SOCIAL_ROWS: usize = 200;
/// Maximum results of one account-handle search.
pub const MAX_SEARCH_ROWS: usize = 20;
/// Pending requests expire after one day of trusted server time.
pub const REQUEST_LIFETIME_MS: u64 = 86_400_000;
/// No observation older than five seconds may be displayed as live presence.
pub const PRESENCE_FRESHNESS_MS: u64 = 5_000;

/// Purpose-limited account identity; private profile names are absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialIdentity {
    pub user_id: String,
    pub handle: String,
    pub display_name: Option<String>,
}

/// A real friend request's durable lifecycle, not a room or match invitation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FriendRequestStatus {
    Pending,
    Accepted,
    Declined,
    Expired,
    Cancelled,
}

/// A participant-visible request projection with a resource-scoped CAS revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendRequestView {
    pub request_id: String,
    pub sender: SocialIdentity,
    pub recipient: SocialIdentity,
    pub status: FriendRequestStatus,
    pub revision: u64,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub updated_at_ms: u64,
}

/// Viewer-authorized observation; unknown never carries a private timestamp.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PresenceObservation {
    Unknown,
    Online {
        as_of_ms: u64,
    },
    Offline {
        as_of_ms: u64,
        last_seen_ms: Option<u64>,
    },
    Stale {
        as_of_ms: Option<u64>,
        last_seen_ms: Option<u64>,
    },
}

/// One accepted peer, whose presence still needs a fresh live-owner observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendView {
    pub identity: SocialIdentity,
    pub presence: PresenceObservation,
    /// Internal projection fact, absent from JSON; no reason for withholding leaks.
    #[serde(skip)]
    pub presence_permitted: bool,
}

/// Full bounded viewer projection; scope/revision belong only to its stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialSnapshot {
    pub version: u16,
    pub viewer_id: String,
    pub scope_id: String,
    pub revision: u64,
    pub generated_at_ms: u64,
    pub friends: Vec<FriendView>,
    pub requests: Vec<FriendRequestView>,
}

/// Relationship facts distinguish outgoing and incoming pending authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocialRelationship {
    None,
    OutgoingPending,
    IncomingPending,
    Accepted,
    Declined,
    Expired,
    Cancelled,
}

/// One directory match, with its exact request if the viewer is a participant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialSearchRow {
    pub identity: SocialIdentity,
    pub relationship: SocialRelationship,
    pub request: Option<FriendRequestView>,
}

/// Bounded authenticated handle search; an empty query returns no directory rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialSearchResponse {
    pub version: u16,
    pub viewer_id: String,
    pub query: String,
    pub results: Vec<SocialSearchRow>,
}

/// The exact intended resource mutation, never a caller-provided actor identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum SocialAction {
    Send {
        target_user_id: String,
    },
    Accept {
        request_id: String,
        expected_revision: u64,
    },
    Decline {
        request_id: String,
        expected_revision: u64,
    },
    Cancel {
        request_id: String,
        expected_revision: u64,
    },
}

/// One durable idempotency key, scoped by storage to the authenticated actor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialMutation {
    pub operation_id: String,
    #[serde(flatten)]
    pub action: SocialAction,
}

/// Durable operation disposition; duplicate retries do not renew session idle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialMutationResponse {
    pub version: u16,
    pub viewer_id: String,
    pub operation_id: String,
    pub duplicate: bool,
    pub request: FriendRequestView,
}

/// Compatibility and resync only; cookies authenticate the upgrade, never Hello.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SocialClientMessage {
    Hello { version: u16 },
    Resync,
}

/// Snapshot-only isolated stream. Resync starts a new scope, never replays history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SocialServerMessage {
    Snapshot { snapshot: SocialSnapshot },
}

/// Public-safe social failure class; target existence and private policy stay hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SocialError {
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("invalid_input")]
    InvalidInput,
    #[error("denied")]
    Denied,
    #[error("conflict")]
    Conflict,
    #[error("rate_limited")]
    RateLimited,
    #[error("unavailable")]
    Unavailable,
}

/// Provenance of one current read; connection bindings originate only at upgrade.
#[derive(Clone, Copy)]
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
pub enum SocialSession {
    Credential(CredentialOperation),
    Connection(SessionBinding),
}
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
impl std::fmt::Debug for SocialSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SocialSession([REDACTED])")
    }
}

/// Trusted gateway transport observation; storage still rechecks this binding
/// and current peer disclosure under the shared publication guard (ADR-0044).
#[derive(Clone, Copy)]
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
pub struct SocialPresenceCandidate {
    pub binding: SessionBinding,
    /// None means a gateway-attested fresh transport; Some retains the last
    /// positive timestamp while an attachment is stale but not yet torn down.
    pub stale_as_of_ms: Option<u64>,
    /// Fresh observations retain their monotonic deadline through publication;
    /// stale candidates have no live deadline and cannot establish Online.
    pub fresh_until: Option<std::time::Instant>,
}
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
impl From<SessionBinding> for SocialPresenceCandidate {
    fn from(binding: SessionBinding) -> Self {
        Self {
            binding,
            stale_as_of_ms: None,
            fresh_until: Some(std::time::Instant::now() + std::time::Duration::from_secs(5)),
        }
    }
}
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
impl std::fmt::Debug for SocialPresenceCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SocialPresenceCandidate([REDACTED])")
    }
}

/// Private candidate plus its one-frame authority/target-policy exclusion.
/// The adapter must retain the guard through actual bounded frame handoff.
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
pub struct GuardedSocial<T, P> {
    pub value: T,
    pub publication: P,
}
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
impl<T, P> std::fmt::Debug for GuardedSocial<T, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GuardedSocial([REDACTED])")
    }
}

/// Durable participant authorization and publication port, implemented only by storage.
#[cfg(all(feature = "authority", not(target_arch = "wasm32")))]
pub trait SocialAuthority: HttpSessionAuthority {
    type SocialPublication: SocketFramePublication;

    fn social_snapshot(
        &self,
        session: SocialSession,
        live_bindings: Vec<SocialPresenceCandidate>,
    ) -> impl Future<
        Output = Result<GuardedSocial<SocialSnapshot, Self::SocialPublication>, SocialError>,
    > + Send;

    fn social_search(
        &self,
        session: SocialSession,
        query: String,
    ) -> impl Future<
        Output = Result<GuardedSocial<SocialSearchResponse, Self::SocialPublication>, SocialError>,
    > + Send;

    fn social_mutate(
        &self,
        session: CredentialOperation,
        mutation: SocialMutation,
    ) -> impl Future<
        Output = Result<
            GuardedSocial<SocialMutationResponse, Self::SocialPublication>,
            SocialError,
        >,
    > + Send;

    /// Debounced offline transition metadata; never used as live online truth.
    fn social_record_offline(
        &self,
        user: UserId,
        at_ms: u64,
    ) -> impl Future<Output = Result<(), SocialError>> + Send;
}

/// SQL-free durable request facts for the total transition decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriendRequestRecord {
    pub id: u128,
    pub sender: UserId,
    pub recipient: UserId,
    pub status: FriendRequestStatus,
    pub revision: u64,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub updated_at_ms: u64,
}

/// Actor-intended action resolved to a trusted request before transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestAction {
    Accept,
    Decline,
    Cancel,
}

/// Expiry persists even when the intended action is rejected at its commit boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestDecision {
    pub record: FriendRequestRecord,
    pub result: Result<(), SocialError>,
}

/// Canonical opaque account/operation/resource identifier shared by v2 DTOs.
pub fn canonical_social_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

impl SocialIdentity {
    /// Checks identity bounds independently of the transport's encoded-frame cap.
    pub fn validate(&self) -> Result<(), SocialError> {
        let valid_name = self.display_name.as_ref().is_none_or(|name| {
            !name.is_empty()
                && name.len() <= 256
                && name.chars().count() <= 64
                && name.trim() == name
                && !name
                    .chars()
                    .any(|ch| ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}'))
        });
        if !canonical_social_id(&self.user_id)
            || !(3..=32).contains(&self.handle.len())
            || !self
                .handle
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || !valid_name
        {
            return Err(SocialError::InvalidInput);
        }
        Ok(())
    }
}

impl FriendRequestView {
    /// Checks participant identity and resource revision/timestamp consistency.
    pub fn validate(&self) -> Result<(), SocialError> {
        self.sender.validate()?;
        self.recipient.validate()?;
        if !canonical_social_id(&self.request_id)
            || self.sender.user_id == self.recipient.user_id
            || self.revision == 0
            || self.expires_at_ms <= self.created_at_ms
            || self.updated_at_ms < self.created_at_ms
        {
            return Err(SocialError::InvalidInput);
        }
        Ok(())
    }
}

impl SocialSnapshot {
    /// Fails closed on malformed scopes, duplicate rows and incoherent observations.
    pub fn validate(&self) -> Result<(), SocialError> {
        if self.version != SOCIAL_CONTRACT_VERSION
            || !canonical_social_id(&self.viewer_id)
            || !canonical_social_id(&self.scope_id)
            || self.revision == 0
            || self.friends.len() > MAX_SOCIAL_ROWS
            || self.requests.len() > MAX_SOCIAL_ROWS
        {
            return Err(SocialError::InvalidInput);
        }
        // These collections are already bounded. A prefix scan keeps duplicate
        // rejection allocation-free without retaining a browser tree index.
        for (index, friend) in self.friends.iter().enumerate() {
            friend.identity.validate()?;
            if friend.identity.user_id == self.viewer_id
                || self.friends[..index]
                    .iter()
                    .any(|previous| previous.identity.user_id == friend.identity.user_id)
            {
                return Err(SocialError::InvalidInput);
            }
            match friend.presence {
                PresenceObservation::Online { as_of_ms } if as_of_ms > self.generated_at_ms => {
                    return Err(SocialError::InvalidInput)
                }
                PresenceObservation::Offline {
                    as_of_ms,
                    last_seen_ms,
                } if as_of_ms > self.generated_at_ms
                    || last_seen_ms.is_some_and(|last| last > as_of_ms) =>
                {
                    return Err(SocialError::InvalidInput)
                }
                PresenceObservation::Stale {
                    as_of_ms,
                    last_seen_ms,
                } if as_of_ms.is_some_and(|at| at > self.generated_at_ms)
                    || last_seen_ms.is_some_and(|last| as_of_ms.is_none_or(|at| last > at)) =>
                {
                    return Err(SocialError::InvalidInput);
                }
                _ => {}
            }
        }
        for (index, request) in self.requests.iter().enumerate() {
            request.validate()?;
            if self.requests[..index]
                .iter()
                .any(|previous| previous.request_id == request.request_id)
                || (request.sender.user_id != self.viewer_id
                    && request.recipient.user_id != self.viewer_id)
                || request.updated_at_ms > self.generated_at_ms
            {
                return Err(SocialError::InvalidInput);
            }
        }
        Ok(())
    }
}

impl SocialSearchResponse {
    /// Checks bounded directory results before a client exposes their projection.
    pub fn validate(&self) -> Result<(), SocialError> {
        if self.version != SOCIAL_CONTRACT_VERSION
            || !canonical_social_id(&self.viewer_id)
            || self.query.len() > 64
            || self.results.len() > MAX_SEARCH_ROWS
            || self.query.chars().any(char::is_control)
        {
            return Err(SocialError::InvalidInput);
        }
        for (index, row) in self.results.iter().enumerate() {
            row.identity.validate()?;
            if self.results[..index]
                .iter()
                .any(|previous| previous.identity.user_id == row.identity.user_id)
                || row.identity.user_id == self.viewer_id
            {
                return Err(SocialError::InvalidInput);
            }
            if let Some(request) = &row.request {
                request.validate()?;
                if !((request.sender.user_id == self.viewer_id
                    && request.recipient.user_id == row.identity.user_id)
                    || (request.recipient.user_id == self.viewer_id
                        && request.sender.user_id == row.identity.user_id))
                {
                    return Err(SocialError::InvalidInput);
                }
                let expected = match request.status {
                    FriendRequestStatus::Pending if request.sender.user_id == self.viewer_id => {
                        SocialRelationship::OutgoingPending
                    }
                    FriendRequestStatus::Pending => SocialRelationship::IncomingPending,
                    FriendRequestStatus::Accepted => SocialRelationship::Accepted,
                    FriendRequestStatus::Declined => SocialRelationship::Declined,
                    FriendRequestStatus::Expired => SocialRelationship::Expired,
                    FriendRequestStatus::Cancelled => SocialRelationship::Cancelled,
                };
                if row.relationship != expected {
                    return Err(SocialError::InvalidInput);
                }
            } else if row.relationship != SocialRelationship::None {
                return Err(SocialError::InvalidInput);
            }
        }
        Ok(())
    }
}

impl SocialMutation {
    /// Checks operation/resource syntax; storage separately authorizes the actor.
    pub fn validate(&self) -> Result<(), SocialError> {
        if !canonical_social_id(&self.operation_id) {
            return Err(SocialError::InvalidInput);
        }
        match &self.action {
            SocialAction::Send { target_user_id } if canonical_social_id(target_user_id) => Ok(()),
            SocialAction::Accept {
                request_id,
                expected_revision,
            }
            | SocialAction::Decline {
                request_id,
                expected_revision,
            }
            | SocialAction::Cancel {
                request_id,
                expected_revision,
            } if canonical_social_id(request_id)
                && *expected_revision > 0
                && *expected_revision < i64::MAX as u64 =>
            {
                Ok(())
            }
            _ => Err(SocialError::InvalidInput),
        }
    }
}

impl SocialMutationResponse {
    /// Checks operation and participant coherence without trusting a response as authority.
    pub fn validate(&self) -> Result<(), SocialError> {
        self.request.validate()?;
        if self.version != SOCIAL_CONTRACT_VERSION
            || !canonical_social_id(&self.viewer_id)
            || !canonical_social_id(&self.operation_id)
            || (self.request.sender.user_id != self.viewer_id
                && self.request.recipient.user_id != self.viewer_id)
        {
            return Err(SocialError::InvalidInput);
        }
        Ok(())
    }
}

impl FriendRequestRecord {
    pub fn new(
        id: u128,
        sender: UserId,
        recipient: UserId,
        now_ms: u64,
    ) -> Result<Self, SocialError> {
        if id == 0 || sender.0 == 0 || recipient.0 == 0 || sender == recipient {
            return Err(SocialError::InvalidInput);
        }
        let expires_at_ms = now_ms
            .checked_add(REQUEST_LIFETIME_MS)
            .ok_or(SocialError::InvalidInput)?;
        Ok(Self {
            id,
            sender,
            recipient,
            status: FriendRequestStatus::Pending,
            revision: 1,
            created_at_ms: now_ms,
            expires_at_ms,
            updated_at_ms: now_ms,
        })
    }

    pub fn decide(
        &self,
        actor: UserId,
        action: RequestAction,
        expected_revision: u64,
        now_ms: u64,
    ) -> RequestDecision {
        let mut record = self.clone();
        let result = if now_ms < self.updated_at_ms
            || self.revision == 0
            || self.expires_at_ms <= self.created_at_ms
        {
            Err(SocialError::Unavailable)
        } else if actor != self.sender && actor != self.recipient {
            Err(SocialError::Denied)
        } else {
            if record.status == FriendRequestStatus::Pending && now_ms >= record.expires_at_ms {
                match record.revision.checked_add(1) {
                    Some(revision) => {
                        record.status = FriendRequestStatus::Expired;
                        record.revision = revision;
                        record.updated_at_ms = now_ms;
                    }
                    None => {
                        return RequestDecision {
                            record,
                            result: Err(SocialError::Unavailable),
                        }
                    }
                }
            }
            if record.revision != expected_revision || record.status != FriendRequestStatus::Pending
            {
                Err(SocialError::Conflict)
            } else if (action == RequestAction::Cancel && actor != record.sender)
                || (action != RequestAction::Cancel && actor != record.recipient)
            {
                Err(SocialError::Denied)
            } else {
                match record.revision.checked_add(1) {
                    Some(revision) => {
                        record.status = match action {
                            RequestAction::Accept => FriendRequestStatus::Accepted,
                            RequestAction::Decline => FriendRequestStatus::Declined,
                            RequestAction::Cancel => FriendRequestStatus::Cancelled,
                        };
                        record.revision = revision;
                        record.updated_at_ms = now_ms;
                        Ok(())
                    }
                    None => Err(SocialError::Unavailable),
                }
            }
        };
        RequestDecision { record, result }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(id: u128) -> SocialIdentity {
        SocialIdentity {
            user_id: format!("{id:032x}"),
            handle: format!("peer_{id}"),
            display_name: None,
        }
    }

    #[test]
    fn bounded_snapshot_accepts_unique_ids_and_rejects_duplicates_at_collection_edges() {
        let viewer = identity(1);
        let snapshot = SocialSnapshot {
            version: SOCIAL_CONTRACT_VERSION,
            viewer_id: viewer.user_id.clone(),
            scope_id: "00000000000000000000000000000009".into(),
            revision: 1,
            generated_at_ms: 2,
            friends: (2..=201)
                .map(|id| FriendView {
                    identity: identity(id),
                    presence: PresenceObservation::Unknown,
                    presence_permitted: false,
                })
                .collect(),
            requests: (2..=201)
                .map(|id| FriendRequestView {
                    request_id: format!("{:032x}", id + 1000),
                    sender: viewer.clone(),
                    recipient: identity(id),
                    status: FriendRequestStatus::Pending,
                    revision: 1,
                    created_at_ms: 1,
                    expires_at_ms: 10,
                    updated_at_ms: 1,
                })
                .collect(),
        };
        assert_eq!(snapshot.friends.len(), MAX_SOCIAL_ROWS);
        assert_eq!(snapshot.requests.len(), MAX_SOCIAL_ROWS);
        assert_eq!(snapshot.validate(), Ok(()));
        for index in [0, 99, 199] {
            let other = (index + 1) % MAX_SOCIAL_ROWS;
            let mut duplicate = snapshot.clone();
            duplicate.friends[index].identity.user_id =
                duplicate.friends[other].identity.user_id.clone();
            assert_eq!(duplicate.validate(), Err(SocialError::InvalidInput));
            let mut duplicate = snapshot.clone();
            duplicate.requests[index].request_id = duplicate.requests[other].request_id.clone();
            assert_eq!(duplicate.validate(), Err(SocialError::InvalidInput));
        }
        let mut over_bound = snapshot.clone();
        over_bound.friends.push(FriendView {
            identity: identity(202),
            presence: PresenceObservation::Unknown,
            presence_permitted: false,
        });
        assert_eq!(over_bound.validate(), Err(SocialError::InvalidInput));
        let mut over_bound = snapshot;
        let mut extra = over_bound.requests[0].clone();
        extra.request_id = "0000000000000000000000000000ffffff".into();
        over_bound.requests.push(extra);
        assert_eq!(over_bound.validate(), Err(SocialError::InvalidInput));
    }

    #[test]
    fn bounded_search_accepts_unique_ids_and_rejects_duplicates_at_collection_edges() {
        let search = SocialSearchResponse {
            version: SOCIAL_CONTRACT_VERSION,
            viewer_id: "00000000000000000000000000000001".into(),
            query: "peer".into(),
            results: (2..=21)
                .map(|id| SocialSearchRow {
                    identity: identity(id),
                    relationship: SocialRelationship::None,
                    request: None,
                })
                .collect(),
        };
        assert_eq!(search.results.len(), MAX_SEARCH_ROWS);
        assert_eq!(search.validate(), Ok(()));
        for index in [0, 10, 19] {
            let other = (index + 1) % MAX_SEARCH_ROWS;
            let mut duplicate = search.clone();
            duplicate.results[index].identity.user_id =
                duplicate.results[other].identity.user_id.clone();
            assert_eq!(duplicate.validate(), Err(SocialError::InvalidInput));
        }
        let mut over_bound = search;
        over_bound.results.push(SocialSearchRow {
            identity: identity(22),
            relationship: SocialRelationship::None,
            request: None,
        });
        assert_eq!(over_bound.validate(), Err(SocialError::InvalidInput));
    }

    #[test]
    fn only_exact_actor_can_commit_each_pending_action() {
        let request = FriendRequestRecord::new(1, UserId(2), UserId(3), 100).unwrap();
        for (action, allowed) in [
            (RequestAction::Accept, UserId(3)),
            (RequestAction::Decline, UserId(3)),
            (RequestAction::Cancel, UserId(2)),
        ] {
            for actor in [UserId(2), UserId(3), UserId(4)] {
                let decision = request.decide(actor, action, 1, 101);
                assert_eq!(decision.result.is_ok(), actor == allowed);
                if actor != allowed {
                    assert_eq!(decision.record, request);
                }
            }
        }
    }

    #[test]
    fn expiry_at_commit_precedes_intended_action_and_persists() {
        let request = FriendRequestRecord::new(1, UserId(2), UserId(3), 100).unwrap();
        assert!(request
            .decide(
                UserId(3),
                RequestAction::Accept,
                1,
                request.expires_at_ms - 1
            )
            .result
            .is_ok());
        let decision = request.decide(UserId(3), RequestAction::Accept, 1, request.expires_at_ms);
        assert_eq!(decision.result, Err(SocialError::Conflict));
        assert_eq!(decision.record.status, FriendRequestStatus::Expired);
        assert_eq!(decision.record.revision, 2);
    }

    #[test]
    fn cas_and_terminal_states_cannot_be_overwritten() {
        let request = FriendRequestRecord::new(1, UserId(2), UserId(3), 100).unwrap();
        let accepted = request
            .decide(UserId(3), RequestAction::Accept, 1, 101)
            .record;
        for action in [
            RequestAction::Accept,
            RequestAction::Decline,
            RequestAction::Cancel,
        ] {
            let decision = accepted.decide(UserId(3), action, 2, 102);
            assert_eq!(decision.result, Err(SocialError::Conflict));
            assert_eq!(decision.record, accepted);
        }
        assert_eq!(
            request
                .decide(UserId(3), RequestAction::Accept, 2, 101)
                .record,
            request
        );
    }
}
