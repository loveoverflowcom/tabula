use std::fmt;

use tabula_core::UserId;

use crate::{
    AccountEpoch, AuthSessionId, CredentialDigest, CredentialGeneration, SessionChannel,
    SessionContextId, SessionError, UnixMillis,
};

/// Initial ADR-0031 §5 idle policy, measured in server milliseconds.
pub const IDLE_LIFETIME_MS: u64 = 1_800_000;
/// Initial ADR-0031 §5 absolute policy, measured in server milliseconds.
pub const ABSOLUTE_LIFETIME_MS: u64 = 86_400_000;

/// Untrusted durable account facts. Convert through [`AccountRecord::try_from`].
/// These facts and construction do not establish verified provider login.
#[derive(Clone, PartialEq, Eq)]
pub struct RawAccountRecord {
    pub user_id: u128,
    pub authorization_epoch: u64,
    pub enabled: bool,
    pub last_observed_at_ms: u64,
}

/// Checked durable account facts, independent of provider authentication proof.
/// (ADR-0034/0036)
#[derive(Clone, PartialEq, Eq)]
pub struct AccountRecord {
    user_id: UserId,
    authorization_epoch: AccountEpoch,
    enabled: bool,
    last_observed_at: UnixMillis,
}

impl AccountRecord {
    /// Creates bounded internal/fixture facts; never verifies provider credentials.
    pub fn new(
        user_id: UserId,
        epoch: AccountEpoch,
        enabled: bool,
        now: UnixMillis,
    ) -> Result<Self, SessionError> {
        if user_id.0 == 0 {
            return Err(SessionError::InvalidInput);
        }
        Ok(Self {
            user_id,
            authorization_epoch: epoch,
            enabled,
            last_observed_at: now,
        })
    }

    pub const fn user_id(&self) -> UserId {
        self.user_id
    }
    pub const fn authorization_epoch(&self) -> AccountEpoch {
        self.authorization_epoch
    }
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
    pub const fn last_observed_at(&self) -> UnixMillis {
        self.last_observed_at
    }

    /// Advances the durable clock high-water without granting authority/activity.
    /// The adapter samples `now` only after acquiring its account ordering lock.
    pub fn observe(&mut self, now: UnixMillis) -> Result<(), SessionError> {
        if now < self.last_observed_at {
            return Err(SessionError::Unavailable);
        }
        self.last_observed_at = now;
        Ok(())
    }

    /// Epoch CAS for invalidation and stale in-flight authentication fencing.
    /// Exhaustion/regression is unavailable; a stale expected epoch is conflict.
    pub fn invalidate(
        &mut self,
        expected_epoch: AccountEpoch,
        now: UnixMillis,
    ) -> Result<(), SessionError> {
        self.observe(now)?;
        if self.authorization_epoch != expected_epoch {
            return Err(SessionError::Conflict);
        }
        let next = self.authorization_epoch.checked_next()?;
        self.authorization_epoch = next;
        self.last_observed_at = now;
        Ok(())
    }

    pub fn into_raw(self) -> RawAccountRecord {
        RawAccountRecord {
            user_id: self.user_id.0,
            authorization_epoch: self.authorization_epoch.get(),
            enabled: self.enabled,
            last_observed_at_ms: self.last_observed_at.get(),
        }
    }
}

impl TryFrom<RawAccountRecord> for AccountRecord {
    type Error = SessionError;

    fn try_from(raw: RawAccountRecord) -> Result<Self, Self::Error> {
        fn convert(raw: &RawAccountRecord) -> Result<AccountRecord, SessionError> {
            AccountRecord::new(
                UserId(raw.user_id),
                AccountEpoch::new(raw.authorization_epoch)?,
                raw.enabled,
                UnixMillis::new(raw.last_observed_at_ms)?,
            )
        }
        convert(&raw).map_err(|_| SessionError::Unavailable)
    }
}

/// Untrusted session row, containing only a digest, never the bearer credential.
/// Every adapter must validate it via [`SessionRecord::try_from`] (ADR-0036).
#[derive(Clone, PartialEq, Eq)]
pub struct RawSessionRecord {
    pub id: u128,
    pub user_id: u128,
    pub authorization_epoch: u64,
    pub channel: String,
    pub credential_generation: u64,
    pub credential_digest: Vec<u8>,
    pub context_id: u128,
    pub created_at_ms: u64,
    pub last_activity_at_ms: u64,
    pub last_observed_at_ms: u64,
    pub idle_deadline_ms: u64,
    pub absolute_deadline_ms: u64,
    pub revoked_at_ms: Option<u64>,
    pub expired_at_ms: Option<u64>,
}

/// Checked lifecycle facts. Pure methods take already sampled trusted server time;
/// no framework, SQL, clock read, provider validation or wire format is here.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionRecord {
    id: AuthSessionId,
    user_id: UserId,
    authorization_epoch: AccountEpoch,
    channel: SessionChannel,
    credential_generation: CredentialGeneration,
    credential_digest: CredentialDigest,
    context_id: SessionContextId,
    created_at: UnixMillis,
    last_activity_at: UnixMillis,
    last_observed_at: UnixMillis,
    idle_deadline: UnixMillis,
    absolute_deadline: UnixMillis,
    revoked_at: Option<UnixMillis>,
    expired_at: Option<UnixMillis>,
}

/// An established connection's immutable record/account/channel binding.
///
/// Credential generation is deliberately absent: rotation keeps this binding.
/// It is a historical snapshot, not a grant for subsequent effects (ADR-0031 §5).
/// Its structural fields can be recreated from checked internal storage facts.
/// Consumers must retain trusted established-connection provenance; never accept
/// a binding reconstructed from HTTP/wire input or public record identifiers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SessionBinding {
    id: AuthSessionId,
    user_id: UserId,
    authorization_epoch: AccountEpoch,
    channel: SessionChannel,
}

impl SessionBinding {
    pub const fn id(self) -> AuthSessionId {
        self.id
    }
    pub const fn user_id(self) -> UserId {
        self.user_id
    }
    pub const fn authorization_epoch(self) -> AccountEpoch {
        self.authorization_epoch
    }
    pub const fn channel(self) -> SessionChannel {
        self.channel
    }
}

/// Facts observed at one ordering point, not authorization for a later operation.
/// Neither this nor its binding may cross an unrelated await as an effect permit.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    record: SessionRecord,
}

impl SessionSnapshot {
    pub const fn id(&self) -> AuthSessionId {
        self.record.id
    }
    pub const fn user_id(&self) -> UserId {
        self.record.user_id
    }
    pub const fn authorization_epoch(&self) -> AccountEpoch {
        self.record.authorization_epoch
    }
    pub const fn channel(&self) -> SessionChannel {
        self.record.channel
    }
    pub const fn credential_generation(&self) -> CredentialGeneration {
        self.record.credential_generation
    }
    pub const fn context_id(&self) -> SessionContextId {
        self.record.context_id
    }
    pub const fn created_at(&self) -> UnixMillis {
        self.record.created_at
    }
    pub const fn last_activity_at(&self) -> UnixMillis {
        self.record.last_activity_at
    }
    pub const fn last_observed_at(&self) -> UnixMillis {
        self.record.last_observed_at
    }
    pub const fn idle_deadline(&self) -> UnixMillis {
        self.record.idle_deadline
    }
    pub const fn absolute_deadline(&self) -> UnixMillis {
        self.record.absolute_deadline
    }
    pub const fn revoked_at(&self) -> Option<UnixMillis> {
        self.record.revoked_at
    }
    pub const fn expired_at(&self) -> Option<UnixMillis> {
        self.record.expired_at
    }
    pub const fn binding(&self) -> SessionBinding {
        self.record.binding()
    }
}

/// Server-classified activity facts, never a client-reported UI gesture.
/// Call the activity policy only inside the actual effect's ordering boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityKind {
    ProtectedMutationCommitted,
    GameCommandAccepted,
    Read,
    Refresh,
    Rejected,
    Duplicate,
    Control,
}

impl ActivityKind {
    pub const fn extends_idle(self) -> bool {
        matches!(
            self,
            Self::ProtectedMutationCommitted | Self::GameCommandAccepted
        )
    }
}

impl SessionRecord {
    /// Builds a new record after the adapter atomically rechecks account epoch.
    /// Arguments are internal facts, not evidence of a verified provider login.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        id: AuthSessionId,
        user_id: UserId,
        epoch: AccountEpoch,
        channel: SessionChannel,
        credential_digest: CredentialDigest,
        context_id: SessionContextId,
        now: UnixMillis,
    ) -> Result<Self, SessionError> {
        if user_id.0 == 0 {
            return Err(SessionError::InvalidInput);
        }
        let absolute_deadline =
            deadline(now, ABSOLUTE_LIFETIME_MS).ok_or(SessionError::InvalidInput)?;
        let idle_deadline = deadline(now, IDLE_LIFETIME_MS).ok_or(SessionError::InvalidInput)?;
        Ok(Self {
            id,
            user_id,
            authorization_epoch: epoch,
            channel,
            credential_generation: CredentialGeneration::new(0)?,
            credential_digest,
            context_id,
            created_at: now,
            last_activity_at: now,
            last_observed_at: now,
            idle_deadline,
            absolute_deadline,
            revoked_at: None,
            expired_at: None,
        })
    }

    pub const fn id(&self) -> AuthSessionId {
        self.id
    }
    pub const fn user_id(&self) -> UserId {
        self.user_id
    }
    pub const fn authorization_epoch(&self) -> AccountEpoch {
        self.authorization_epoch
    }
    pub const fn channel(&self) -> SessionChannel {
        self.channel
    }
    pub const fn credential_generation(&self) -> CredentialGeneration {
        self.credential_generation
    }
    pub const fn credential_digest(&self) -> CredentialDigest {
        self.credential_digest
    }
    pub const fn context_id(&self) -> SessionContextId {
        self.context_id
    }
    pub const fn created_at(&self) -> UnixMillis {
        self.created_at
    }
    pub const fn last_activity_at(&self) -> UnixMillis {
        self.last_activity_at
    }
    pub const fn last_observed_at(&self) -> UnixMillis {
        self.last_observed_at
    }
    pub const fn idle_deadline(&self) -> UnixMillis {
        self.idle_deadline
    }
    pub const fn absolute_deadline(&self) -> UnixMillis {
        self.absolute_deadline
    }
    pub const fn revoked_at(&self) -> Option<UnixMillis> {
        self.revoked_at
    }
    pub const fn expired_at(&self) -> Option<UnixMillis> {
        self.expired_at
    }

    pub const fn binding(&self) -> SessionBinding {
        SessionBinding {
            id: self.id,
            user_id: self.user_id,
            authorization_epoch: self.authorization_epoch,
            channel: self.channel,
        }
    }

    /// Copies facts only; does not assert current validity or create an effect permit.
    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            record: self.clone(),
        }
    }

    /// Rechecks an established immutable binding, independent of verifier rotation.
    pub fn matches_binding(&self, binding: SessionBinding) -> bool {
        self.binding() == binding
    }

    /// Observes current record/account facts. Equality with either deadline expires.
    ///
    /// On ordinary denial this may advance `last_observed_at` or terminal expiry.
    /// An impossible future session epoch is corrupt: it is durably revoked and
    /// returns unavailable, so account epoch catch-up cannot revive it. The
    /// adapter must persist changed facts even for this unavailable outcome.
    /// Clock regression returns unavailable without changing the record. Reads
    /// never update `last_activity_at` or either deadline (ADR-0031 §5).
    pub fn observe(
        &mut self,
        account: &AccountRecord,
        now: UnixMillis,
    ) -> Result<SessionSnapshot, SessionError> {
        self.check_clock(account, now)?;
        if self.user_id != account.user_id {
            return Err(SessionError::Unavailable);
        }
        // Issuance CAS and monotonic account invalidation can only create a
        // session epoch <= the current account epoch. Retire an impossible
        // future epoch durably so a later account advance cannot revive it.
        if self.authorization_epoch > account.authorization_epoch {
            self.revoke(now)?;
            return Err(SessionError::Unavailable);
        }
        self.last_observed_at = now;
        if self.expired_at.is_none() && (now >= self.idle_deadline || now >= self.absolute_deadline)
        {
            self.expired_at = Some(now);
        }
        if !account.enabled
            || account.authorization_epoch != self.authorization_epoch
            || self.revoked_at.is_some()
            || self.expired_at.is_some()
        {
            return Err(SessionError::Unauthenticated);
        }
        Ok(self.snapshot())
    }

    fn check_clock(&self, account: &AccountRecord, now: UnixMillis) -> Result<(), SessionError> {
        if now < self.last_observed_at || now < account.last_observed_at {
            return Err(SessionError::Unavailable);
        }
        Ok(())
    }

    /// Observes a presented credential at the current generation and channel.
    pub fn observe_credential(
        &mut self,
        account: &AccountRecord,
        digest: CredentialDigest,
        channel: SessionChannel,
        now: UnixMillis,
    ) -> Result<SessionSnapshot, SessionError> {
        let snapshot = self.observe(account, now)?;
        if self.credential_digest != digest || self.channel != channel {
            return Err(SessionError::Unauthenticated);
        }
        Ok(snapshot)
    }

    /// Atomic generation/digest CAS policy. The adapter owns the transaction.
    /// No idle extension, new session, new account, new context or grace verifier.
    pub fn rotate(
        &mut self,
        account: &AccountRecord,
        current_digest: CredentialDigest,
        channel: SessionChannel,
        expected_generation: CredentialGeneration,
        replacement_digest: CredentialDigest,
        now: UnixMillis,
    ) -> Result<SessionSnapshot, SessionError> {
        self.observe_credential(account, current_digest, channel, now)?;
        if self.credential_generation != expected_generation {
            return Err(SessionError::Conflict);
        }
        if replacement_digest == self.credential_digest {
            return Err(SessionError::InvalidInput);
        }
        let next = self.credential_generation.checked_next()?;
        self.credential_generation = next;
        self.credential_digest = replacement_digest;
        Ok(self.snapshot())
    }

    /// Idempotent current-session revocation. It never grants authority/activity.
    /// Account/binding lookup and lock ordering remain the adapter's responsibility.
    pub fn revoke(&mut self, now: UnixMillis) -> Result<(), SessionError> {
        if now < self.last_observed_at {
            return Err(SessionError::Unavailable);
        }
        self.last_observed_at = now;
        if self.expired_at.is_none() && (now >= self.idle_deadline || now >= self.absolute_deadline)
        {
            self.expired_at = Some(now);
        }
        if self.revoked_at.is_none() {
            self.revoked_at = Some(now);
        }
        Ok(())
    }

    /// Pure activity policy for a successfully ordered effect. A prior observation
    /// alone is insufficient: storage must recheck and commit the protected effect
    /// and this change under its one authority lock/transaction (ADR-0036).
    pub fn record_activity(
        &mut self,
        account: &AccountRecord,
        activity: ActivityKind,
        now: UnixMillis,
    ) -> Result<SessionSnapshot, SessionError> {
        self.observe(account, now)?;
        if activity.extends_idle() {
            let idle_deadline = deadline(now, IDLE_LIFETIME_MS)
                .unwrap_or(self.absolute_deadline)
                .min(self.absolute_deadline);
            self.last_activity_at = now;
            self.idle_deadline = idle_deadline;
        }
        Ok(self.snapshot())
    }

    pub fn into_raw(self) -> RawSessionRecord {
        RawSessionRecord {
            id: self.id.get(),
            user_id: self.user_id.0,
            authorization_epoch: self.authorization_epoch.get(),
            channel: self.channel.as_str().to_owned(),
            credential_generation: self.credential_generation.get(),
            credential_digest: self.credential_digest.as_bytes().to_vec(),
            context_id: self.context_id.get(),
            created_at_ms: self.created_at.get(),
            last_activity_at_ms: self.last_activity_at.get(),
            last_observed_at_ms: self.last_observed_at.get(),
            idle_deadline_ms: self.idle_deadline.get(),
            absolute_deadline_ms: self.absolute_deadline.get(),
            revoked_at_ms: self.revoked_at.map(UnixMillis::get),
            expired_at_ms: self.expired_at.map(UnixMillis::get),
        }
    }
}

impl TryFrom<RawSessionRecord> for SessionRecord {
    type Error = SessionError;

    fn try_from(raw: RawSessionRecord) -> Result<Self, Self::Error> {
        fn convert(raw: &RawSessionRecord) -> Result<SessionRecord, SessionError> {
            let record = SessionRecord {
                id: AuthSessionId::new(raw.id)?,
                user_id: UserId(raw.user_id),
                authorization_epoch: AccountEpoch::new(raw.authorization_epoch)?,
                channel: SessionChannel::parse(&raw.channel)?,
                credential_generation: CredentialGeneration::new(raw.credential_generation)?,
                credential_digest: CredentialDigest::from_slice(&raw.credential_digest)?,
                context_id: SessionContextId::new(raw.context_id)?,
                created_at: UnixMillis::new(raw.created_at_ms)?,
                last_activity_at: UnixMillis::new(raw.last_activity_at_ms)?,
                last_observed_at: UnixMillis::new(raw.last_observed_at_ms)?,
                idle_deadline: UnixMillis::new(raw.idle_deadline_ms)?,
                absolute_deadline: UnixMillis::new(raw.absolute_deadline_ms)?,
                revoked_at: raw.revoked_at_ms.map(UnixMillis::new).transpose()?,
                expired_at: raw.expired_at_ms.map(UnixMillis::new).transpose()?,
            };
            if record.user_id.0 == 0
                || record.last_activity_at < record.created_at
                || record.last_observed_at < record.last_activity_at
                || record.last_activity_at >= record.absolute_deadline
                || Some(record.absolute_deadline)
                    != deadline(record.created_at, ABSOLUTE_LIFETIME_MS)
                || record.idle_deadline
                    != deadline(record.last_activity_at, IDLE_LIFETIME_MS)
                        .unwrap_or(record.absolute_deadline)
                        .min(record.absolute_deadline)
                || record
                    .revoked_at
                    .is_some_and(|at| at < record.created_at || at > record.last_observed_at)
                || record.expired_at.is_some_and(|at| {
                    at < record.idle_deadline.min(record.absolute_deadline)
                        || at > record.last_observed_at
                })
            {
                return Err(SessionError::Unavailable);
            }
            Ok(record)
        }
        convert(&raw).map_err(|_| SessionError::Unavailable)
    }
}

fn deadline(now: UnixMillis, duration: u64) -> Option<UnixMillis> {
    now.get()
        .checked_add(duration)
        .and_then(|value| UnixMillis::new(value).ok())
}

macro_rules! redacted_debug {
    ($($name:ident),+ $(,)?) => { $(impl fmt::Debug for $name {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { formatter.write_str(concat!(stringify!($name), "([REDACTED])")) }
    })+ };
}

redacted_debug!(
    RawAccountRecord,
    AccountRecord,
    RawSessionRecord,
    SessionRecord,
    SessionBinding,
    SessionSnapshot
);

#[cfg(test)]
mod tests;
