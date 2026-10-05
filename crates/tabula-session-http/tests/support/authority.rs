//! Deterministic authority double for transport partitions, not durable DB evidence.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tabula_core::UserId;
use tabula_session::{
    AccountEpoch, AccountRecord, AuthSessionId, CredentialDigest, CredentialOperation,
    HttpSessionAuthority, IssueSession, ProviderIdentityKey, RotateCredential, RotateSession,
    SelfProfileSnapshot, SessionAuthority, SessionBinding, SessionChannel, SessionContextId,
    SessionCredential, SessionError, SessionPublication, SessionRecord, SessionSnapshot,
    UnixMillis,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug)]
pub struct OperationGate {
    pub entered: Semaphore,
    pub release: Semaphore,
}

impl OperationGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entered: Semaphore::new(0),
            release: Semaphore::new(0),
        })
    }

    pub async fn wait_entered(&self) {
        tokio::time::timeout(Duration::from_secs(5), self.entered.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
    }
}

#[derive(Debug)]
struct State {
    account: AccountRecord,
    sessions: BTreeMap<AuthSessionId, SessionRecord>,
    now: UnixMillis,
    failure: Option<SessionError>,
    mutation_failure: Option<SessionError>,
    fail_after_commit: bool,
    mutations: usize,
    issuance_attempts: usize,
    gate: Option<Arc<OperationGate>>,
}

#[derive(Clone, Debug)]
pub struct TestAuthority {
    state: Arc<Mutex<State>>,
    ordering: Arc<Semaphore>,
}

impl TestAuthority {
    pub fn new() -> Self {
        let now = UnixMillis::new(1_000).unwrap();
        Self {
            state: Arc::new(Mutex::new(State {
                account: AccountRecord::new(
                    UserId(0x000a_11ce),
                    AccountEpoch::new(0).unwrap(),
                    true,
                    now,
                )
                .unwrap(),
                sessions: BTreeMap::new(),
                now,
                failure: None,
                mutation_failure: None,
                fail_after_commit: false,
                mutations: 0,
                issuance_attempts: 0,
                gate: None,
            })),
            ordering: Arc::new(Semaphore::new(1)),
        }
    }

    pub fn fixture(&self, channel: SessionChannel) -> (String, SessionSnapshot) {
        self.fixture_with_context(channel, None)
    }

    pub fn fixture_with_context(
        &self,
        channel: SessionChannel,
        context: Option<SessionContextId>,
    ) -> (String, SessionSnapshot) {
        let credential = SessionCredential::generate().unwrap();
        let mut state = self.state.lock().unwrap();
        let id = AuthSessionId::new(state.sessions.len() as u128 + 0x500).unwrap();
        let record = SessionRecord::issue(
            id,
            state.account.user_id(),
            state.account.authorization_epoch(),
            channel,
            credential.digest(),
            context.unwrap_or_else(|| {
                SessionContextId::new(state.sessions.len() as u128 + 0xc000).unwrap()
            }),
            state.now,
        )
        .unwrap();
        let snapshot = record.snapshot();
        state.sessions.insert(id, record);
        (credential.expose_encoded(), snapshot)
    }

    pub fn account_id(&self) -> String {
        format!("{:032x}", self.state.lock().unwrap().account.user_id().0)
    }

    pub fn set_failure(&self, failure: Option<SessionError>) {
        self.state.lock().unwrap().failure = failure;
    }

    pub fn set_mutation_fault(&self, error: SessionError, after_commit: bool) {
        let mut state = self.state.lock().unwrap();
        state.mutation_failure = Some(error);
        state.fail_after_commit = after_commit;
    }

    pub fn set_time(&self, now: u64) {
        self.state.lock().unwrap().now = UnixMillis::new(now).unwrap();
    }

    pub fn pause_next_operation(&self) -> Arc<OperationGate> {
        let gate = OperationGate::new();
        self.state.lock().unwrap().gate = Some(gate.clone());
        gate
    }

    pub fn mutations(&self) -> usize {
        self.state.lock().unwrap().mutations
    }

    pub fn issuance_attempts(&self) -> usize {
        self.state.lock().unwrap().issuance_attempts
    }

    pub fn snapshot(&self, id: AuthSessionId) -> SessionSnapshot {
        self.state.lock().unwrap().sessions[&id].snapshot()
    }

    async fn wait_gate(&self) {
        let gate = self.state.lock().unwrap().gate.take();
        if let Some(gate) = gate {
            gate.entered.add_permits(1);
            gate.release.acquire().await.unwrap().forget();
        }
    }

    async fn lock_ordering(&self) -> OwnedSemaphorePermit {
        self.ordering.clone().acquire_owned().await.unwrap()
    }

    fn operation(
        &self,
        request: CredentialOperation,
        terminal_logout: bool,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut state = self.state.lock().unwrap();
        if let Some(error) = state.failure {
            return Err(error);
        }
        let now = state.now;
        state.account.observe(now)?;
        let account = state.account.clone();
        let session = state
            .sessions
            .values_mut()
            .find(|session| session.credential_digest() == request.digest)
            .ok_or(SessionError::Unauthenticated)?;
        if terminal_logout {
            session.observe_logout_context(&account, request, now)
        } else {
            session.observe_operation(&account, request, now)
        }
    }
}

impl SessionAuthority for TestAuthority {
    async fn account_snapshot(
        &self,
        _: ProviderIdentityKey,
    ) -> Result<AccountRecord, SessionError> {
        Err(SessionError::Unavailable)
    }

    async fn issue_session(&self, _: IssueSession) -> Result<SessionSnapshot, SessionError> {
        self.state.lock().unwrap().issuance_attempts += 1;
        Err(SessionError::Unavailable)
    }

    async fn observe_credential(
        &self,
        digest: CredentialDigest,
        channel: SessionChannel,
    ) -> Result<SessionSnapshot, SessionError> {
        self.read_session(CredentialOperation {
            digest,
            channel,
            context: None,
        })
        .await
    }

    async fn observe_binding(&self, _: SessionBinding) -> Result<SessionSnapshot, SessionError> {
        Err(SessionError::Unavailable)
    }

    async fn rotate_session(&self, _: RotateSession) -> Result<SessionSnapshot, SessionError> {
        Err(SessionError::Unavailable)
    }

    async fn revoke_session(&self, _: SessionBinding) -> Result<(), SessionError> {
        Err(SessionError::Unavailable)
    }

    async fn invalidate_account_epoch(
        &self,
        user_id: UserId,
        expected_epoch: AccountEpoch,
    ) -> Result<AccountRecord, SessionError> {
        let _ordering = self.lock_ordering().await;
        let mut state = self.state.lock().unwrap();
        if user_id != state.account.user_id() {
            return Err(SessionError::Unauthenticated);
        }
        let now = state.now;
        state.account.invalidate(expected_epoch, now)?;
        Ok(state.account.clone())
    }
}

impl HttpSessionAuthority for TestAuthority {
    type Publication = TestPublication;

    async fn read_session(
        &self,
        request: CredentialOperation,
    ) -> Result<SessionSnapshot, SessionError> {
        self.wait_gate().await;
        let _ordering = self.lock_ordering().await;
        self.operation(request, false)
    }

    async fn read_logout_context(
        &self,
        request: CredentialOperation,
    ) -> Result<SessionSnapshot, SessionError> {
        self.wait_gate().await;
        let _ordering = self.lock_ordering().await;
        self.operation(request, true)
    }

    async fn read_self_profile(
        &self,
        request: CredentialOperation,
    ) -> Result<SelfProfileSnapshot, SessionError> {
        Ok(SelfProfileSnapshot::from_session(
            &self.read_session(request).await?,
        ))
    }

    async fn rotate_credential(
        &self,
        request: RotateCredential,
    ) -> Result<SessionSnapshot, SessionError> {
        self.wait_gate().await;
        let _ordering = self.lock_ordering().await;
        let mut state = self.state.lock().unwrap();
        if let Some(error) = state.failure {
            return Err(error);
        }
        if let Some(error) = state.mutation_failure {
            if !state.fail_after_commit {
                return Err(error);
            }
        }
        let now = state.now;
        state.account.observe(now)?;
        let account = state.account.clone();
        let session = state
            .sessions
            .values_mut()
            .find(|session| session.credential_digest() == request.credential.digest)
            .ok_or(SessionError::Unauthenticated)?;
        let result = session.rotate_credential(&account, request, now);
        if result.is_ok() {
            state.mutations += 1;
        }
        if result.is_ok() && state.fail_after_commit {
            return Err(state.mutation_failure.unwrap());
        }
        result
    }

    async fn revoke_credential(&self, request: CredentialOperation) -> Result<(), SessionError> {
        self.wait_gate().await;
        let _ordering = self.lock_ordering().await;
        let mut state = self.state.lock().unwrap();
        if let Some(error) = state.failure {
            return Err(error);
        }
        if let Some(error) = state.mutation_failure {
            if !state.fail_after_commit {
                return Err(error);
            }
        }
        let now = state.now;
        state.account.observe(now)?;
        let account = state.account.clone();
        let session = state
            .sessions
            .values_mut()
            .find(|session| session.credential_digest() == request.digest)
            .ok_or(SessionError::Unauthenticated)?;
        let result = session.revoke_credential(&account, request, now);
        if result.is_ok() {
            state.mutations += 1;
        }
        if result.is_ok() && state.fail_after_commit {
            return Err(state.mutation_failure.unwrap());
        }
        result
    }

    async fn begin_publication(
        &self,
        request: CredentialOperation,
    ) -> Result<Self::Publication, SessionError> {
        self.wait_gate().await;
        let ordering = self.lock_ordering().await;
        let snapshot = self.operation(request, false)?;
        let permit = Arc::new(Mutex::new(Some(ordering)));
        let expiry = Instant::now() + Duration::from_secs(2);
        let expiry_permit = permit.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            expiry_permit.lock().unwrap().take();
        });
        Ok(TestPublication {
            snapshot,
            permit,
            expiry,
        })
    }
}

#[derive(Debug)]
pub struct TestPublication {
    snapshot: SessionSnapshot,
    permit: Arc<Mutex<Option<OwnedSemaphorePermit>>>,
    expiry: Instant,
}

impl SessionPublication for TestPublication {
    fn snapshot(&self) -> &SessionSnapshot {
        &self.snapshot
    }

    fn publish<R>(
        &mut self,
        publication: impl FnOnce(&SessionSnapshot) -> R,
    ) -> Result<R, SessionError> {
        let mut permit = self.permit.lock().unwrap();
        if Instant::now() >= self.expiry || permit.is_none() {
            permit.take();
            return Err(SessionError::Unavailable);
        }
        let result = publication(&self.snapshot);
        permit.take();
        Ok(result)
    }
}

impl Drop for TestPublication {
    fn drop(&mut self) {
        self.permit.lock().unwrap().take();
    }
}
