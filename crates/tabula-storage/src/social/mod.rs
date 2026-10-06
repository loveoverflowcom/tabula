//! Isolated durable friends authority (ADR-0043; doc 03 §14).
//! SQL belongs only here. Caller and disclosed-peer exclusions order graph/profile
//! changes with bounded publication; snapshots never authorize a later mutation.

use std::collections::{BTreeMap, BTreeSet};

use sqlx::{Postgres, Row, Transaction};
use tabula_core::UserId;
use tabula_lobby::social::{
    FriendRequestRecord, FriendRequestStatus, FriendRequestView, FriendView, GuardedSocial,
    PresenceObservation, RequestAction, SocialAction, SocialAuthority, SocialError, SocialIdentity,
    SocialMutation, SocialMutationResponse, SocialPresenceCandidate, SocialRelationship,
    SocialSearchResponse, SocialSearchRow, SocialSession, SocialSnapshot, MAX_SOCIAL_ROWS,
    SOCIAL_CONTRACT_VERSION,
};
use tabula_session::{
    AccountDisplayName, AccountHandle, AccountOperationId, ActivityKind, BoundedSocketFrame,
    CredentialOperation, HttpSessionAuthority, SessionAuthority, SessionError, SessionPublication,
    SessionSnapshot, SocketFramePublication,
};
use uuid::Uuid;

use crate::{
    accounts::{AccountsPrincipal, AccountsPublication},
    session::{LockedCredential, PgSessionStore},
};

/// Social output retains every actual Online observation's monotonic deadline,
/// alongside the viewer/peer SQL exclusions, until frame transfer (ADR-0043).
pub struct SocialPublication {
    inner: AccountsPublication,
    online_until: Option<std::time::Instant>,
}
impl std::fmt::Debug for SocialPublication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SocialPublication([REDACTED])")
    }
}
impl SocialPublication {
    fn current(&self) -> Result<(), SessionError> {
        if self
            .online_until
            .is_some_and(|until| std::time::Instant::now() >= until)
        {
            Err(SessionError::Unavailable)
        } else {
            Ok(())
        }
    }
}
impl SessionPublication for SocialPublication {
    fn snapshot(&self) -> &SessionSnapshot {
        self.inner.snapshot()
    }
    fn publish<R>(
        &mut self,
        action: impl FnOnce(&SessionSnapshot) -> R,
    ) -> Result<R, SessionError> {
        self.current()?;
        let result = self.inner.publish(action)?;
        self.current()?;
        Ok(result)
    }
}
impl SocketFramePublication for SocialPublication {
    fn handoff<R>(
        &mut self,
        frame: BoundedSocketFrame,
        transport: impl FnOnce(BoundedSocketFrame) -> R,
    ) -> Result<R, SessionError> {
        self.current()?;
        let until = self.online_until;
        self.inner.handoff(frame, |frame| {
            if until.is_some_and(|until| std::time::Instant::now() >= until) {
                Err(SessionError::Unavailable)
            } else {
                Ok(transport(frame))
            }
        })?
    }
}
impl From<AccountsPublication> for SocialPublication {
    fn from(inner: AccountsPublication) -> Self {
        Self {
            inner,
            online_until: None,
        }
    }
}

fn fail(_: impl std::fmt::Debug) -> SocialError {
    SocialError::Unavailable
}
fn auth(error: SessionError) -> SocialError {
    match error {
        SessionError::Unauthenticated => SocialError::Unauthenticated,
        SessionError::InvalidInput => SocialError::InvalidInput,
        SessionError::Conflict => SocialError::Conflict,
        SessionError::Unavailable => SocialError::Unavailable,
    }
}
fn signed(value: u64) -> Result<i64, SocialError> {
    i64::try_from(value).map_err(fail)
}
fn unsigned(value: i64) -> Result<u64, SocialError> {
    u64::try_from(value).map_err(fail)
}
fn id(value: &str) -> Result<Uuid, SocialError> {
    AccountOperationId::parse(value)
        .map(|id| Uuid::from_u128(id.get()))
        .map_err(|_| SocialError::InvalidInput)
}
fn wire_id(value: Uuid) -> String {
    value.simple().to_string()
}
fn principal(session: SocialSession) -> AccountsPrincipal {
    match session {
        SocialSession::Credential(op) => AccountsPrincipal::Credential(op),
        SocialSession::Connection(binding) => AccountsPrincipal::Binding(binding),
    }
}

/// Accepted relationships confer only this profile/presence disclosure, never game authority.
pub(crate) async fn are_friends(
    tx: &mut Transaction<'_, Postgres>,
    viewer: UserId,
    target: UserId,
) -> Result<bool, SessionError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tabula_friend_requests WHERE pair_low=LEAST($1::uuid,$2::uuid) AND pair_high=GREATEST($1::uuid,$2::uuid) AND status='accepted')")
        .bind(Uuid::from_u128(viewer.0)).bind(Uuid::from_u128(target.0)).fetch_one(&mut **tx).await.map_err(|_| SessionError::Unavailable)
}

fn record(row: &sqlx::postgres::PgRow) -> Result<FriendRequestRecord, SocialError> {
    let result = FriendRequestRecord {
        id: row
            .try_get::<Uuid, _>("request_id")
            .map_err(fail)?
            .as_u128(),
        sender: UserId(row.try_get::<Uuid, _>("sender_id").map_err(fail)?.as_u128()),
        recipient: UserId(
            row.try_get::<Uuid, _>("recipient_id")
                .map_err(fail)?
                .as_u128(),
        ),
        status: match row.try_get::<&str, _>("status").map_err(fail)? {
            "pending" => FriendRequestStatus::Pending,
            "accepted" => FriendRequestStatus::Accepted,
            "declined" => FriendRequestStatus::Declined,
            "expired" => FriendRequestStatus::Expired,
            "cancelled" => FriendRequestStatus::Cancelled,
            _ => return Err(SocialError::Unavailable),
        },
        revision: unsigned(row.try_get("revision").map_err(fail)?)?,
        created_at_ms: unsigned(row.try_get("created_at_ms").map_err(fail)?)?,
        expires_at_ms: unsigned(row.try_get("expires_at_ms").map_err(fail)?)?,
        updated_at_ms: unsigned(row.try_get("updated_at_ms").map_err(fail)?)?,
    };
    if result.id == 0
        || result.sender.0 == 0
        || result.recipient.0 == 0
        || result.sender == result.recipient
        || result.revision == 0
        || result.expires_at_ms <= result.created_at_ms
        || result.updated_at_ms < result.created_at_ms
    {
        return Err(SocialError::Unavailable);
    }
    Ok(result)
}

fn observed_record(
    mut record: FriendRequestRecord,
    now: u64,
) -> Result<FriendRequestRecord, SocialError> {
    if now < record.updated_at_ms {
        return Err(SocialError::Unavailable);
    }
    if record.status == FriendRequestStatus::Pending && now >= record.expires_at_ms {
        record.status = FriendRequestStatus::Expired;
        record.revision = record
            .revision
            .checked_add(1)
            .ok_or(SocialError::Unavailable)?;
        record.updated_at_ms = record.expires_at_ms;
    }
    Ok(record)
}

struct Person {
    identity: SocialIdentity,
    presence_permitted: bool,
    last_seen: Option<u64>,
}

async fn people(
    tx: &mut Transaction<'_, Postgres>,
    viewer: UserId,
    peers: &[UserId],
) -> Result<BTreeMap<UserId, Person>, SocialError> {
    let mut ids: Vec<_> = peers.iter().map(|peer| Uuid::from_u128(peer.0)).collect();
    ids.push(Uuid::from_u128(viewer.0));
    let rows = sqlx::query("SELECT p.user_id,p.handle,p.display_name,p.visibility,a.enabled,l.last_seen_ms FROM account_profiles p JOIN session_accounts a USING(user_id) LEFT JOIN tabula_social_last_seen l USING(user_id) WHERE p.user_id=ANY($1)")
        .bind(ids).fetch_all(&mut **tx).await.map_err(fail)?;
    let mut result = BTreeMap::new();
    for row in rows {
        let user = UserId(row.try_get::<Uuid, _>("user_id").map_err(fail)?.as_u128());
        let enabled: bool = row.try_get("enabled").map_err(fail)?;
        let visibility: &str = row.try_get("visibility").map_err(fail)?;
        let accepted = user == viewer || are_friends(tx, viewer, user).await.map_err(auth)?;
        let allowed = user == viewer
            || enabled && (visibility == "public" || visibility == "friends" && accepted);
        if !["public", "friends", "private"].contains(&visibility) {
            return Err(SocialError::Unavailable);
        }
        let handle = AccountHandle::new(row.try_get("handle").map_err(fail)?).map_err(fail)?;
        let display_name = if allowed {
            Some(
                AccountDisplayName::new(row.try_get("display_name").map_err(fail)?)
                    .map_err(fail)?
                    .as_str()
                    .to_owned(),
            )
        } else {
            None
        };
        let last_seen = if allowed {
            row.try_get::<Option<i64>, _>("last_seen_ms")
                .map_err(fail)?
                .map(unsigned)
                .transpose()?
        } else {
            None
        };
        result.insert(
            user,
            Person {
                identity: SocialIdentity {
                    user_id: wire_id(Uuid::from_u128(user.0)),
                    handle: handle.as_str().to_owned(),
                    display_name,
                },
                presence_permitted: allowed && accepted && user != viewer,
                last_seen,
            },
        );
    }
    if result.len()
        != peers
            .iter()
            .copied()
            .chain([viewer])
            .collect::<BTreeSet<_>>()
            .len()
    {
        return Err(SocialError::Unavailable);
    }
    Ok(result)
}

fn request_view(
    record: &FriendRequestRecord,
    identities: &BTreeMap<UserId, Person>,
) -> Result<FriendRequestView, SocialError> {
    Ok(FriendRequestView {
        request_id: wire_id(Uuid::from_u128(record.id)),
        sender: identities
            .get(&record.sender)
            .ok_or(SocialError::Unavailable)?
            .identity
            .clone(),
        recipient: identities
            .get(&record.recipient)
            .ok_or(SocialError::Unavailable)?
            .identity
            .clone(),
        status: record.status,
        revision: record.revision,
        created_at_ms: record.created_at_ms,
        expires_at_ms: record.expires_at_ms,
        updated_at_ms: record.updated_at_ms,
    })
}

const PARTICIPANT_ROWS: &str = "SELECT * FROM tabula_friend_requests WHERE sender_id=$1 OR recipient_id=$1 ORDER BY CASE status WHEN 'accepted' THEN 0 WHEN 'pending' THEN 1 ELSE 2 END,updated_at_ms DESC,request_id LIMIT 200";

impl PgSessionStore {
    async fn social_reject(
        &self,
        mut tx: Transaction<'_, Postgres>,
        mut credential: LockedCredential,
        error: SocialError,
        has_savepoint: bool,
        discard_resource: bool,
    ) -> Result<GuardedSocial<SocialMutationResponse, SocialPublication>, SocialError> {
        if discard_resource && has_savepoint {
            sqlx::query("ROLLBACK TO SAVEPOINT social_mutation")
                .execute(&mut *tx)
                .await
                .map_err(fail)?;
        }
        let observation = credential.observe(&mut tx, ActivityKind::Rejected).await;
        if observation.is_err() && has_savepoint && !discard_resource {
            sqlx::query("ROLLBACK TO SAVEPOINT social_mutation")
                .execute(&mut *tx)
                .await
                .map_err(fail)?;
            credential.save_observation(&mut tx).await.map_err(auth)?;
        }
        self.commit(tx).await.map_err(auth)?;
        Err(observation.err().map_or(error, auth))
    }
    /// Explicit isolated schema setup; never called by production startup.
    pub async fn migrate_social(&self) -> Result<(), SessionError> {
        let mut migrations = sqlx::migrate!("./session_migrations")
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        migrations.extend(sqlx::migrate!("./accounts_migrations").iter().cloned());
        migrations.extend(sqlx::migrate!("./social_migrations").iter().cloned());
        migrations.sort_by_key(|migration| migration.version);
        if migrations
            .windows(2)
            .any(|pair| pair[0].version == pair[1].version)
        {
            return Err(SessionError::Unavailable);
        }
        let mut connection = self
            .pool()
            .acquire()
            .await
            .map_err(|_| SessionError::Unavailable)?;
        connection.close_on_drop();
        let result = sqlx::migrate::Migrator::with_migrations(migrations)
            .run(&mut *connection)
            .await
            .map_err(|_| SessionError::Unavailable);
        let _ = connection.close().await;
        result
    }

    async fn social_viewer(&self, session: SocialSession) -> Result<UserId, SocialError> {
        match session {
            SocialSession::Credential(op) => self.read_session(op).await,
            SocialSession::Connection(binding) => self.observe_binding(binding).await,
        }
        .map(|snapshot| snapshot.user_id())
        .map_err(auth)
    }

    async fn guarded_request(
        &self,
        op: CredentialOperation,
        request_id: Uuid,
    ) -> Result<GuardedSocial<FriendRequestView, AccountsPublication>, SocialError> {
        let viewer = self.social_viewer(SocialSession::Credential(op)).await?;
        let row = sqlx::query("SELECT * FROM tabula_friend_requests WHERE request_id=$1 AND (sender_id=$2 OR recipient_id=$2)")
            .bind(request_id).bind(Uuid::from_u128(viewer.0)).fetch_optional(self.pool()).await.map_err(fail)?.ok_or(SocialError::Denied)?;
        let candidate = record(&row)?;
        let peer = if candidate.sender == viewer {
            candidate.recipient
        } else {
            candidate.sender
        };
        let publication = self
            .begin_accounts_publication(AccountsPrincipal::Credential(op), vec![peer])
            .await
            .map_err(auth)?;
        let mut tx = self.begin().await.map_err(auth)?;
        let now = Self::database_clock(&mut tx).await.map_err(auth)?.get();
        let row = sqlx::query("SELECT * FROM tabula_friend_requests WHERE request_id=$1 AND (sender_id=$2 OR recipient_id=$2)")
            .bind(request_id).bind(Uuid::from_u128(viewer.0)).fetch_optional(&mut *tx).await.map_err(fail)?.ok_or(SocialError::Denied)?;
        let identities = people(&mut tx, viewer, &[peer]).await?;
        let value = request_view(&observed_record(record(&row)?, now)?, &identities)?;
        self.commit(tx).await.map_err(auth)?;
        publication.with_current(|_| ()).map_err(auth)?;
        Ok(GuardedSocial { value, publication })
    }
}

impl SocialAuthority for PgSessionStore {
    type SocialPublication = SocialPublication;

    // Keep the complete disclosure fence and resource requery in one ordered boundary.
    #[allow(clippy::too_many_lines)]
    async fn social_snapshot(
        &self,
        session: SocialSession,
        live_bindings: Vec<SocialPresenceCandidate>,
    ) -> Result<GuardedSocial<SocialSnapshot, Self::SocialPublication>, SocialError> {
        if live_bindings.len() > 128 {
            return Err(SocialError::Unavailable);
        }
        let viewer = self.social_viewer(session).await?;
        for _ in 0..3 {
            let candidates = sqlx::query(PARTICIPANT_ROWS)
                .bind(Uuid::from_u128(viewer.0))
                .fetch_all(self.pool())
                .await
                .map_err(fail)?;
            let peers: BTreeSet<_> = candidates
                .iter()
                .map(record)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|r| {
                    if r.sender == viewer {
                        r.recipient
                    } else {
                        r.sender
                    }
                })
                .collect();
            let publication = self
                .begin_accounts_publication(principal(session), peers.iter().copied().collect())
                .await
                .map_err(auth)?;
            let mut tx = self.begin().await.map_err(auth)?;
            let now = Self::database_clock(&mut tx).await.map_err(auth)?.get();
            let rows = sqlx::query(PARTICIPANT_ROWS)
                .bind(Uuid::from_u128(viewer.0))
                .fetch_all(&mut *tx)
                .await
                .map_err(fail)?;
            let records = rows.iter().map(record).collect::<Result<Vec<_>, _>>()?;
            if records.iter().any(|r| {
                !publication.includes(if r.sender == viewer {
                    r.recipient
                } else {
                    r.sender
                })
            }) {
                drop(tx);
                drop(publication);
                continue;
            }
            let identities =
                people(&mut tx, viewer, &peers.iter().copied().collect::<Vec<_>>()).await?;
            let mut online = BTreeMap::new();
            let mut stale = BTreeMap::new();
            for candidate in &live_bindings {
                let binding = candidate.binding;
                if peers.contains(&binding.user_id())
                    && identities
                        .get(&binding.user_id())
                        .is_some_and(|person| person.presence_permitted)
                {
                    match self
                        .observe_binding_published(&mut tx, binding, &publication)
                        .await
                    {
                        Ok(_) => {
                            if let Some(at) = candidate.stale_as_of_ms {
                                if at > now {
                                    return Err(SocialError::Unavailable);
                                }
                                stale
                                    .entry(binding.user_id())
                                    .and_modify(|old: &mut u64| *old = (*old).max(at))
                                    .or_insert(at);
                            } else {
                                let until =
                                    candidate.fresh_until.ok_or(SocialError::Unavailable)?;
                                if until <= std::time::Instant::now() {
                                    return Err(SocialError::Unavailable);
                                }
                                online
                                    .entry(binding.user_id())
                                    .and_modify(|old: &mut std::time::Instant| {
                                        *old = (*old).max(until);
                                    })
                                    .or_insert(until);
                            }
                        }
                        Err(SessionError::Unauthenticated) => {}
                        Err(error) => return Err(auth(error)),
                    }
                }
            }
            let mut friends = Vec::new();
            let mut requests = Vec::new();
            let mut online_until = None;
            for record in records {
                let record = observed_record(record, now)?;
                if record.status == FriendRequestStatus::Accepted {
                    let peer = if record.sender == viewer {
                        record.recipient
                    } else {
                        record.sender
                    };
                    let person = identities.get(&peer).ok_or(SocialError::Unavailable)?;
                    let presence = if !person.presence_permitted {
                        PresenceObservation::Unknown
                    } else if let Some(until) = online.get(&peer) {
                        online_until = Some(
                            online_until.map_or(*until, |old: std::time::Instant| old.min(*until)),
                        );
                        PresenceObservation::Online { as_of_ms: now }
                    } else if let Some(at) = stale.get(&peer) {
                        PresenceObservation::Stale {
                            as_of_ms: Some(*at),
                            last_seen_ms: person.last_seen,
                        }
                    } else {
                        PresenceObservation::Offline {
                            as_of_ms: now,
                            last_seen_ms: person.last_seen,
                        }
                    };
                    friends.push(FriendView {
                        identity: person.identity.clone(),
                        presence,
                        presence_permitted: person.presence_permitted,
                    });
                }
                requests.push(request_view(&record, &identities)?);
            }
            friends.sort_by(|a, b| {
                a.identity
                    .handle
                    .cmp(&b.identity.handle)
                    .then(a.identity.user_id.cmp(&b.identity.user_id))
            });
            self.commit(tx).await.map_err(auth)?;
            publication.with_current(|_| ()).map_err(auth)?;
            return Ok(GuardedSocial {
                value: SocialSnapshot {
                    version: SOCIAL_CONTRACT_VERSION,
                    viewer_id: wire_id(Uuid::from_u128(viewer.0)),
                    scope_id: String::new(),
                    revision: 1,
                    generated_at_ms: now,
                    friends,
                    requests,
                },
                publication: SocialPublication {
                    inner: publication,
                    online_until,
                },
            });
        }
        Err(SocialError::Unavailable)
    }

    async fn social_search(
        &self,
        session: SocialSession,
        query: String,
    ) -> Result<GuardedSocial<SocialSearchResponse, Self::SocialPublication>, SocialError> {
        if query.len() > 64 || query.chars().any(char::is_control) {
            return Err(SocialError::InvalidInput);
        }
        let query = query.trim().to_owned();
        let viewer = self.social_viewer(session).await?;
        for _ in 0..3 {
            // Literal prefix, not caller-controlled SQL LIKE metacharacters or email search.
            let candidates: Vec<Uuid> = if query.is_empty() {
                Vec::new()
            } else {
                sqlx::query_scalar("SELECT p.user_id FROM account_profiles p JOIN session_accounts a USING(user_id) WHERE left(p.handle,char_length($1))=$1 AND p.user_id<>$2 AND a.enabled AND (p.visibility='public' OR p.visibility='friends' AND EXISTS(SELECT 1 FROM tabula_friend_requests r WHERE r.status='accepted' AND r.pair_low=LEAST($2::uuid,p.user_id) AND r.pair_high=GREATEST($2::uuid,p.user_id))) ORDER BY p.handle LIMIT 20")
                .bind(&query).bind(Uuid::from_u128(viewer.0)).fetch_all(self.pool()).await.map_err(fail)?
            };
            let peers: Vec<_> = candidates.iter().map(|id| UserId(id.as_u128())).collect();
            let publication = self
                .begin_accounts_publication(principal(session), peers.clone())
                .await
                .map_err(auth)?;
            let mut tx = self.begin().await.map_err(auth)?;
            let now = Self::database_clock(&mut tx).await.map_err(auth)?.get();
            let current: Vec<Uuid> = if query.is_empty() {
                Vec::new()
            } else {
                sqlx::query_scalar("SELECT p.user_id FROM account_profiles p JOIN session_accounts a USING(user_id) WHERE left(p.handle,char_length($1))=$1 AND p.user_id<>$2 AND a.enabled AND (p.visibility='public' OR p.visibility='friends' AND EXISTS(SELECT 1 FROM tabula_friend_requests r WHERE r.status='accepted' AND r.pair_low=LEAST($2::uuid,p.user_id) AND r.pair_high=GREATEST($2::uuid,p.user_id))) ORDER BY p.handle LIMIT 20")
                .bind(&query).bind(Uuid::from_u128(viewer.0)).fetch_all(&mut *tx).await.map_err(fail)?
            };
            if current
                .iter()
                .any(|id| !publication.includes(UserId(id.as_u128())))
            {
                drop(tx);
                drop(publication);
                continue;
            }
            let identities = people(&mut tx, viewer, &peers).await?;
            let mut results = Vec::new();
            for target in current {
                let row = sqlx::query("SELECT * FROM tabula_friend_requests WHERE pair_low=LEAST($1::uuid,$2::uuid) AND pair_high=GREATEST($1::uuid,$2::uuid) ORDER BY updated_at_ms DESC,request_id DESC LIMIT 1")
                    .bind(Uuid::from_u128(viewer.0)).bind(target).fetch_optional(&mut *tx).await.map_err(fail)?;
                let request = row
                    .as_ref()
                    .map(record)
                    .transpose()?
                    .map(|record| observed_record(record, now))
                    .transpose()?;
                let relationship = match request.as_ref().map(|r| r.status) {
                    None => SocialRelationship::None,
                    Some(FriendRequestStatus::Pending)
                        if request.as_ref().is_some_and(|r| r.sender == viewer) =>
                    {
                        SocialRelationship::OutgoingPending
                    }
                    Some(FriendRequestStatus::Pending) => SocialRelationship::IncomingPending,
                    Some(FriendRequestStatus::Accepted) => SocialRelationship::Accepted,
                    Some(FriendRequestStatus::Declined) => SocialRelationship::Declined,
                    Some(FriendRequestStatus::Expired) => SocialRelationship::Expired,
                    Some(FriendRequestStatus::Cancelled) => SocialRelationship::Cancelled,
                };
                results.push(SocialSearchRow {
                    identity: identities
                        .get(&UserId(target.as_u128()))
                        .ok_or(SocialError::Unavailable)?
                        .identity
                        .clone(),
                    relationship,
                    request: request.map(|r| request_view(&r, &identities)).transpose()?,
                });
            }
            self.commit(tx).await.map_err(auth)?;
            publication.with_current(|_| ()).map_err(auth)?;
            return Ok(GuardedSocial {
                value: SocialSearchResponse {
                    version: SOCIAL_CONTRACT_VERSION,
                    viewer_id: wire_id(Uuid::from_u128(viewer.0)),
                    query,
                    results,
                },
                publication: publication.into(),
            });
        }
        Err(SocialError::Unavailable)
    }

    // Admission, idempotency, resource effects and final-time recheck share this transaction.
    #[allow(clippy::too_many_lines)]
    async fn social_mutate(
        &self,
        session: CredentialOperation,
        mutation: SocialMutation,
    ) -> Result<GuardedSocial<SocialMutationResponse, Self::SocialPublication>, SocialError> {
        mutation.validate()?;
        let operation = id(&mutation.operation_id)?;
        let viewer = self
            .social_viewer(SocialSession::Credential(session))
            .await?;
        let (peer, existing_id, intended) = match &mutation.action {
            SocialAction::Send { target_user_id } => {
                (UserId(id(target_user_id)?.as_u128()), None, None)
            }
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
            } => {
                if *expected_revision == 0 || *expected_revision >= i64::MAX as u64 {
                    return Err(SocialError::InvalidInput);
                }
                let request_id = id(request_id)?;
                let row = sqlx::query("SELECT * FROM tabula_friend_requests WHERE request_id=$1 AND (sender_id=$2 OR recipient_id=$2)")
                    .bind(request_id).bind(Uuid::from_u128(viewer.0)).fetch_optional(self.pool()).await.map_err(fail)?.ok_or(SocialError::Denied)?;
                let request = record(&row)?;
                let action = match mutation.action {
                    SocialAction::Accept { .. } => RequestAction::Accept,
                    SocialAction::Decline { .. } => RequestAction::Decline,
                    _ => RequestAction::Cancel,
                };
                (
                    if request.sender == viewer {
                        request.recipient
                    } else {
                        request.sender
                    },
                    Some(request_id),
                    Some((action, *expected_revision)),
                )
            }
        };
        if peer == viewer {
            return Err(SocialError::Denied);
        }
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_profiles WHERE user_id=$1)")
                .bind(Uuid::from_u128(peer.0))
                .fetch_one(self.pool())
                .await
                .map_err(fail)?;
        if !exists {
            return Err(SocialError::Denied);
        }
        let fingerprint = match &mutation.action {
            SocialAction::Send { target_user_id } => format!("send:{target_user_id}"),
            SocialAction::Accept {
                request_id,
                expected_revision,
            } => format!("accept:{request_id}:{expected_revision}"),
            SocialAction::Decline {
                request_id,
                expected_revision,
            } => format!("decline:{request_id}:{expected_revision}"),
            SocialAction::Cancel {
                request_id,
                expected_revision,
            } => format!("cancel:{request_id}:{expected_revision}"),
        };
        let mut tx = self.begin().await.map_err(auth)?;
        let mut credential = LockedCredential::lock_accounts(&mut tx, &[peer], session)
            .await
            .map_err(auth)?;
        let snapshot = match credential.observe(&mut tx, ActivityKind::Read).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.commit(tx).await.map_err(auth)?;
                return Err(auth(error));
            }
        };
        if snapshot.user_id() != viewer {
            return Err(SocialError::Unauthenticated);
        }
        let old = sqlx::query("SELECT fingerprint,request_id FROM tabula_social_operations WHERE actor_id=$1 AND operation_id=$2")
            .bind(Uuid::from_u128(viewer.0)).bind(operation).fetch_optional(&mut *tx).await.map_err(fail)?;
        let (request_id, duplicate) = if let Some(old) = old {
            if old.try_get::<&str, _>("fingerprint").map_err(fail)? != fingerprint {
                return self
                    .social_reject(tx, credential, SocialError::Conflict, false, false)
                    .await;
            }
            (old.try_get("request_id").map_err(fail)?, true)
        } else {
            sqlx::query("SAVEPOINT social_mutation")
                .execute(&mut *tx)
                .await
                .map_err(fail)?;
            let pending_deadline;
            let outcome = if let Some(request_id) = existing_id {
                let row = sqlx::query(
                    "SELECT * FROM tabula_friend_requests WHERE request_id=$1 FOR UPDATE",
                )
                .bind(request_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(fail)?
                .ok_or(SocialError::Denied)?;
                let request = record(&row)?;
                pending_deadline = Some((request_id, request.expires_at_ms));
                let now = Self::database_clock(&mut tx).await.map_err(auth)?.get();
                let (action, revision) = intended.ok_or(SocialError::InvalidInput)?;
                let decision = request.decide(viewer, action, revision, now);
                if action == RequestAction::Accept && decision.result.is_ok() {
                    let eligible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_profiles p JOIN session_accounts a USING(user_id) WHERE p.user_id=$1 AND a.enabled=TRUE)")
                        .bind(Uuid::from_u128(peer.0)).fetch_one(&mut *tx).await.map_err(fail)?;
                    if !eligible {
                        return self
                            .social_reject(tx, credential, SocialError::Denied, true, true)
                            .await;
                    }
                }
                if decision.record != request {
                    sqlx::query("UPDATE tabula_friend_requests SET status=$2,revision=$3,updated_at_ms=$4 WHERE request_id=$1")
                        .bind(request_id).bind(status(decision.record.status)).bind(signed(decision.record.revision)?).bind(signed(decision.record.updated_at_ms)?).execute(&mut *tx).await.map_err(fail)?;
                }
                if let Err(error) = decision.result {
                    return self.social_reject(tx, credential, error, true, false).await;
                }
                request_id
            } else {
                let eligible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_profiles p JOIN session_accounts a USING(user_id) WHERE p.user_id=$1 AND a.enabled=TRUE)")
                    .bind(Uuid::from_u128(peer.0)).fetch_one(&mut *tx).await.map_err(fail)?;
                if !eligible {
                    return self
                        .social_reject(tx, credential, SocialError::Denied, true, true)
                        .await;
                }
                let now = Self::database_clock(&mut tx).await.map_err(auth)?.get();
                // Lazy terminal expiry releases the active pair without reviving it.
                sqlx::query("UPDATE tabula_friend_requests SET status='expired',revision=revision+1,updated_at_ms=expires_at_ms WHERE pair_low=LEAST($1::uuid,$2::uuid) AND pair_high=GREATEST($1::uuid,$2::uuid) AND status='pending' AND expires_at_ms<=$3")
                    .bind(Uuid::from_u128(viewer.0)).bind(Uuid::from_u128(peer.0)).bind(signed(now)?).execute(&mut *tx).await.map_err(fail)?;
                let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tabula_friend_requests WHERE pair_low=LEAST($1::uuid,$2::uuid) AND pair_high=GREATEST($1::uuid,$2::uuid) AND status IN ('pending','accepted'))")
                    .bind(Uuid::from_u128(viewer.0)).bind(Uuid::from_u128(peer.0)).fetch_one(&mut *tx).await.map_err(fail)?;
                if active {
                    return self
                        .social_reject(tx, credential, SocialError::Conflict, true, true)
                        .await;
                }
                for user in [viewer, peer] {
                    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tabula_friend_requests WHERE (sender_id=$1 OR recipient_id=$1) AND status IN ('pending','accepted') AND (status='accepted' OR expires_at_ms>$2)")
                        .bind(Uuid::from_u128(user.0)).bind(signed(now)?).fetch_one(&mut *tx).await.map_err(fail)?;
                    if count >= i64::try_from(MAX_SOCIAL_ROWS).map_err(fail)? {
                        return self
                            .social_reject(tx, credential, SocialError::RateLimited, true, true)
                            .await;
                    }
                }
                let request_id =
                    Uuid::from_u128(AccountOperationId::generate().map_err(auth)?.get());
                let request = FriendRequestRecord::new(request_id.as_u128(), viewer, peer, now)?;
                pending_deadline = Some((request_id, request.expires_at_ms));
                sqlx::query("INSERT INTO tabula_friend_requests(request_id,sender_id,recipient_id,status,revision,created_at_ms,expires_at_ms,updated_at_ms) VALUES($1,$2,$3,'pending',1,$4,$5,$4)")
                    .bind(request_id).bind(Uuid::from_u128(viewer.0)).bind(Uuid::from_u128(peer.0)).bind(signed(now)?).bind(signed(request.expires_at_ms)?).execute(&mut *tx).await.map_err(fail)?;
                request_id
            };
            sqlx::query("INSERT INTO tabula_social_operations(actor_id,operation_id,fingerprint,request_id) VALUES($1,$2,$3,$4)")
                .bind(Uuid::from_u128(viewer.0)).bind(operation).bind(&fingerprint).bind(outcome).execute(&mut *tx).await.map_err(fail)?;
            // Resource/receipt waits may cross the request's deadline after its row
            // decision. Recheck trusted time at the actual commit boundary.
            let final_now = Self::database_clock(&mut tx).await.map_err(auth)?.get();
            if let Some((request_id, deadline)) = pending_deadline {
                if final_now >= deadline {
                    sqlx::query("ROLLBACK TO SAVEPOINT social_mutation")
                        .execute(&mut *tx)
                        .await
                        .map_err(fail)?;
                    sqlx::query("UPDATE tabula_friend_requests SET status='expired',revision=revision+1,updated_at_ms=expires_at_ms WHERE request_id=$1 AND status='pending'").bind(request_id).execute(&mut *tx).await.map_err(fail)?;
                    return self
                        .social_reject(tx, credential, SocialError::Conflict, true, false)
                        .await;
                }
            }
            if let Err(error) = credential
                .observe(&mut tx, ActivityKind::ProtectedMutationCommitted)
                .await
            {
                sqlx::query("ROLLBACK TO SAVEPOINT social_mutation")
                    .execute(&mut *tx)
                    .await
                    .map_err(fail)?;
                credential.save_observation(&mut tx).await.map_err(auth)?;
                self.commit(tx).await.map_err(auth)?;
                return Err(auth(error));
            }
            (outcome, false)
        };
        if duplicate {
            credential
                .observe(&mut tx, ActivityKind::Duplicate)
                .await
                .map_err(auth)?;
        }
        self.commit(tx).await.map_err(auth)?;
        let GuardedSocial {
            value: request,
            publication,
        } = self.guarded_request(session, request_id).await?;
        Ok(GuardedSocial {
            value: SocialMutationResponse {
                version: SOCIAL_CONTRACT_VERSION,
                viewer_id: wire_id(Uuid::from_u128(viewer.0)),
                operation_id: mutation.operation_id,
                duplicate,
                request,
            },
            publication: publication.into(),
        })
    }

    async fn social_record_offline(&self, user: UserId, at_ms: u64) -> Result<(), SocialError> {
        let mut tx = self.begin().await.map_err(auth)?;
        Self::lock_accounts(&mut tx, &[user]).await.map_err(auth)?;
        let now = Self::database_clock(&mut tx).await.map_err(auth)?.get();
        if at_ms > now {
            return Err(SocialError::InvalidInput);
        }
        sqlx::query("INSERT INTO tabula_social_last_seen(user_id,last_seen_ms) VALUES($1,$2) ON CONFLICT(user_id) DO UPDATE SET last_seen_ms=GREATEST(tabula_social_last_seen.last_seen_ms,EXCLUDED.last_seen_ms)")
            .bind(Uuid::from_u128(user.0)).bind(signed(at_ms)?).execute(&mut *tx).await.map_err(fail)?;
        self.commit(tx).await.map_err(auth)
    }
}

fn status(status: FriendRequestStatus) -> &'static str {
    match status {
        FriendRequestStatus::Pending => "pending",
        FriendRequestStatus::Accepted => "accepted",
        FriendRequestStatus::Declined => "declined",
        FriendRequestStatus::Expired => "expired",
        FriendRequestStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests;
