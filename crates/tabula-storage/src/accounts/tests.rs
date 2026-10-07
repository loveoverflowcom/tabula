//! Ignored real `PostgreSQL` acceptance. Missing setup fails; no fake authority.
use super::*;
use sqlx::{postgres::PgPoolOptions, AssertSqlSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use tabula_session::{
    AuthSessionId, IssueSession, SessionAuthority, SessionChannel, SessionContextBinding,
    SessionContextId, SessionCredential,
};
static NEXT: AtomicU64 = AtomicU64::new(1);
async fn legacy_grant(
    store: &PgSessionStore,
    identity: ProviderIdentityKey,
    epoch: Option<AccountEpoch>,
) -> Result<(SessionCredential, AccountOperationId), SessionError> {
    let grant = SessionCredential::generate()?;
    let id = AccountOperationId::generate()?;
    store
        .create_enrollment(
            VerifiedEnrollment {
                identity,
                expected_policy_epoch: store.enrollment_policy().await?.epoch,
                expected_account_epoch: epoch,
            },
            grant.digest(),
            id,
        )
        .await?;
    Ok((grant, id))
}
struct Fixture {
    admin: sqlx::PgPool,
    pool: sqlx::PgPool,
    other: sqlx::PgPool,
    schema: String,
}
#[tokio::test]
#[ignore = "requires real disposable PostgreSQL16; missing setup fails"]
#[allow(clippy::too_many_lines)] // One legacy identity/epoch preservation law with real storage.
async fn legacy_profile_completion_preserves_identity_and_rejects_uncaptured_or_stale_epoch() {
    let fixture = Fixture::new().await;
    let store = PgSessionStore::new(fixture.pool.clone());
    let identity = ProviderIdentityKey::new(
        "https://fixture.example/oauth2/openid/tabula",
        "legacy-subject",
    )
    .unwrap();
    let user_id = UserId(AccountOperationId::generate().unwrap().get());
    store
        .provision_fixture_identity(
            identity.clone(),
            tabula_session::AccountRecord::new(
                user_id,
                AccountEpoch::new(4).unwrap(),
                true,
                UnixMillis::new(0).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(!store.account_profile_ready(user_id).await.unwrap());
    let (grant, id) = legacy_grant(&store, identity.clone(), None).await.unwrap();
    assert_eq!(
        store
            .register_account(
                grant.digest(),
                id,
                AccountHandle::new("uncaptured".into()).unwrap(),
                AccountDisplayName::new("Uncaptured".into()).unwrap()
            )
            .await
            .unwrap(),
        EnrollmentStatus::Rejected
    );
    let (grant, id) = legacy_grant(
        &store,
        identity.clone(),
        Some(AccountEpoch::new(4).unwrap()),
    )
    .await
    .unwrap();
    store
        .invalidate_account_epoch(user_id, AccountEpoch::new(4).unwrap())
        .await
        .unwrap();
    assert_eq!(
        store
            .register_account(
                grant.digest(),
                id,
                AccountHandle::new("stale_handle".into()).unwrap(),
                AccountDisplayName::new("Stale".into()).unwrap()
            )
            .await
            .unwrap(),
        EnrollmentStatus::Rejected
    );
    assert!(matches!(
        legacy_grant(
            &store,
            identity.clone(),
            Some(AccountEpoch::new(4).unwrap())
        )
        .await,
        Err(SessionError::Unauthenticated)
    ));
    assert!(!store.account_profile_ready(user_id).await.unwrap());
    let (grant, id) = legacy_grant(
        &store,
        identity.clone(),
        Some(AccountEpoch::new(5).unwrap()),
    )
    .await
    .unwrap();
    let credential = SessionCredential::generate().unwrap();
    let authenticated = store
        .issue_session(IssueSession {
            identity: identity.clone(),
            expected_epoch: AccountEpoch::new(5).unwrap(),
            id: AuthSessionId::new(AccountOperationId::generate().unwrap().get()).unwrap(),
            channel: SessionChannel::BrowserCookie,
            credential_digest: credential.digest(),
            context_id: SessionContextId::new(AccountOperationId::generate().unwrap().get())
                .unwrap(),
        })
        .await
        .unwrap();
    let operation = CredentialOperation {
        digest: credential.digest(),
        channel: SessionChannel::BrowserCookie,
        context: Some(SessionContextBinding {
            context_id: authenticated.context_id(),
            authorization_epoch: authenticated.authorization_epoch(),
        }),
    };
    let (mut publication, absent) = store.self_account_profile(operation).await.unwrap();
    assert!(
        absent.is_none(),
        "current legacy authority proves absence without an invented profile"
    );
    publication.publish(|_| ()).unwrap();
    drop(publication);
    assert_eq!(
        store
            .register_account(
                grant.digest(),
                id,
                AccountHandle::new("legacy_handle".into()).unwrap(),
                AccountDisplayName::new("Ngọc Hà".into()).unwrap()
            )
            .await
            .unwrap(),
        EnrollmentStatus::AcceptedWithoutSession
    );
    assert!(store.account_profile_ready(user_id).await.unwrap());
    let (grant, id) = legacy_grant(
        &store,
        identity.clone(),
        Some(AccountEpoch::new(5).unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(
        store
            .register_account(
                grant.digest(),
                id,
                AccountHandle::new("replacement_handle".into()).unwrap(),
                AccountDisplayName::new("Must not replace".into()).unwrap()
            )
            .await
            .unwrap(),
        EnrollmentStatus::AcceptedWithoutSession
    );
    let row =
        sqlx::query("SELECT user_id,handle,display_name,visibility,revision FROM account_profiles")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(row.get::<Uuid, _>("user_id").as_u128(), user_id.0);
    assert_eq!(row.get::<String, _>("handle"), "legacy_handle");
    assert_eq!(row.get::<String, _>("display_name"), "Ngọc Hà");
    assert_eq!(row.get::<String, _>("visibility"), "public");
    assert_eq!(row.get::<i64, _>("revision"), 1);
    let completed_receipt = (grant.digest(), id);
    let account = store.account_snapshot(identity.clone()).await.unwrap();
    assert_eq!(account.user_id(), user_id);
    assert_eq!(account.authorization_epoch().get(), 5);
    assert!(account.enabled());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_auth_sessions")
            .fetch_one(&fixture.pool)
            .await
            .unwrap(),
        1
    );
    // A ready duplicate cannot borrow later account authority, even though it
    // would leave existing metadata unchanged. Completed receipts stay historical.
    let (grant, id) = legacy_grant(&store, identity, Some(AccountEpoch::new(5).unwrap()))
        .await
        .unwrap();
    store
        .invalidate_account_epoch(user_id, AccountEpoch::new(5).unwrap())
        .await
        .unwrap();
    assert_eq!(
        store
            .register_account(
                grant.digest(),
                id,
                AccountHandle::new("new_duplicate".into()).unwrap(),
                AccountDisplayName::new("Denied duplicate".into()).unwrap()
            )
            .await
            .unwrap(),
        EnrollmentStatus::Rejected
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT display_name FROM account_profiles WHERE user_id=$1"
        )
        .bind(Uuid::from_u128(user_id.0))
        .fetch_one(&fixture.pool)
        .await
        .unwrap(),
        "Ngọc Hà"
    );
    assert_eq!(
        store
            .register_account(
                completed_receipt.0,
                completed_receipt.1,
                AccountHandle::new("replacement_handle".into()).unwrap(),
                AccountDisplayName::new("Must not replace".into()).unwrap()
            )
            .await
            .unwrap(),
        EnrollmentStatus::AcceptedWithoutSession,
        "completed receipt remains historical and creates no fresh profile/session authority"
    );
    let disabled = ProviderIdentityKey::new(
        "https://fixture.example/oauth2/openid/tabula",
        "disabled-legacy",
    )
    .unwrap();
    store
        .provision_fixture_identity(
            disabled.clone(),
            tabula_session::AccountRecord::new(
                UserId(AccountOperationId::generate().unwrap().get()),
                AccountEpoch::new(0).unwrap(),
                false,
                UnixMillis::new(0).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        legacy_grant(&store, disabled, Some(AccountEpoch::new(0).unwrap())).await,
        Err(SessionError::Unauthenticated)
    ));
    fixture.close().await;
}
async fn open(url: &str, schema: &str, capacity: u32) -> sqlx::PgPool {
    let path = schema.to_owned();
    PgPoolOptions::new()
        .max_connections(capacity)
        .after_connect(move |conn, _| {
            let path = path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(path)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(url)
        .await
        .unwrap()
}
impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("DATABASE_URL")
            .expect("ignored PostgreSQL acceptance requires DATABASE_URL");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("PostgreSQL acceptance must connect");
        let version: i32 =
            sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
                .fetch_one(&admin)
                .await
                .unwrap();
        assert!(version >= 160_000);
        let schema = format!(
            "tabula_accounts_acceptance_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        );
        assert!(
            schema.len() <= 63
                && schema
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        );
        sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&admin)
            .await
            .unwrap();
        let pool = open(&url, &schema, 8).await;
        let other = open(&url, &schema, 1).await;
        let store = PgSessionStore::new(pool.clone());
        store.migrate_accounts().await.unwrap();
        store
            .set_enrollment_enabled(AccountEpoch::new(0).unwrap(), true)
            .await
            .unwrap();
        Self {
            admin,
            pool,
            other,
            schema,
        }
    }
    async fn close(self) {
        self.pool.close().await;
        self.other.close().await;
        sqlx::query(AssertSqlSafe(format!(
            "DROP SCHEMA {} CASCADE",
            self.schema
        )))
        .execute(&self.admin)
        .await
        .unwrap();
        self.admin.close().await;
    }
    async fn enroll(&self, subject: &str, handle: &str) -> (CredentialOperation, UserId) {
        let store = PgSessionStore::new(self.pool.clone());
        let identity =
            ProviderIdentityKey::new("https://fixture.example/oauth2/openid/tabula", subject)
                .unwrap();
        let grant = SessionCredential::generate().unwrap();
        let id = AccountOperationId::generate().unwrap();
        let policy = store.enrollment_policy().await.unwrap();
        store
            .create_enrollment(
                VerifiedEnrollment {
                    identity: identity.clone(),
                    expected_policy_epoch: policy.epoch,
                    expected_account_epoch: None,
                },
                grant.digest(),
                id,
            )
            .await
            .unwrap();
        assert_eq!(
            store
                .register_account(
                    grant.digest(),
                    id,
                    AccountHandle::new(handle.into()).unwrap(),
                    AccountDisplayName::new(handle.into()).unwrap()
                )
                .await
                .unwrap(),
            EnrollmentStatus::AcceptedWithoutSession
        );
        let credential = SessionCredential::generate().unwrap();
        let snapshot = store
            .issue_session(IssueSession {
                identity,
                expected_epoch: AccountEpoch::new(0).unwrap(),
                id: AuthSessionId::new(AccountOperationId::generate().unwrap().get()).unwrap(),
                channel: SessionChannel::BrowserCookie,
                credential_digest: credential.digest(),
                context_id: SessionContextId::new(AccountOperationId::generate().unwrap().get())
                    .unwrap(),
            })
            .await
            .unwrap();
        (
            CredentialOperation {
                digest: credential.digest(),
                channel: SessionChannel::BrowserCookie,
                context: Some(SessionContextBinding {
                    context_id: snapshot.context_id(),
                    authorization_epoch: snapshot.authorization_epoch(),
                }),
            },
            snapshot.user_id(),
        )
    }
}
#[tokio::test]
#[ignore = "requires real disposable PostgreSQL16; missing setup fails"]
#[allow(clippy::too_many_lines)] // Independent pools exercise one atomic enrollment/receipt law.
async fn enrollment_receipt_and_unique_provider_pair_are_atomic_across_pools() {
    let fixture = Fixture::new().await;
    let first = PgSessionStore::new(fixture.pool.clone());
    let second = PgSessionStore::new(fixture.other.clone());
    let policy = first.enrollment_policy().await.unwrap();
    let identity = ProviderIdentityKey::new(
        "https://fixture.example/oauth2/openid/tabula",
        "unmapped-subject",
    )
    .unwrap();
    let a = SessionCredential::generate().unwrap();
    let b = SessionCredential::generate().unwrap();
    let ia = AccountOperationId::generate().unwrap();
    let ib = AccountOperationId::generate().unwrap();
    for (digest, id) in [(a.digest(), ia), (b.digest(), ib)] {
        first
            .create_enrollment(
                VerifiedEnrollment {
                    identity: identity.clone(),
                    expected_policy_epoch: policy.epoch,
                    expected_account_epoch: None,
                },
                digest,
                id,
            )
            .await
            .unwrap();
    }
    let (a_result, b_result) = tokio::join!(
        first.register_account(
            a.digest(),
            ia,
            AccountHandle::new("first_handle".into()).unwrap(),
            AccountDisplayName::new("First".into()).unwrap()
        ),
        second.register_account(
            b.digest(),
            ib,
            AccountHandle::new("second_handle".into()).unwrap(),
            AccountDisplayName::new("Second".into()).unwrap()
        )
    );
    assert_eq!(a_result.unwrap(), EnrollmentStatus::AcceptedWithoutSession);
    assert_eq!(b_result.unwrap(), EnrollmentStatus::AcceptedWithoutSession);
    for table in [
        "session_accounts",
        "session_provider_identities",
        "account_profiles",
    ] {
        let count = match table {
            "session_accounts" => {
                sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_accounts")
            }
            "session_provider_identities" => {
                sqlx::query_scalar("SELECT count(*) FROM session_provider_identities")
            }
            _ => sqlx::query_scalar("SELECT count(*) FROM account_profiles"),
        }
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_auth_sessions")
            .fetch_one(&fixture.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        first
            .register_account(
                a.digest(),
                ia,
                AccountHandle::new("first_handle".into()).unwrap(),
                AccountDisplayName::new("First".into()).unwrap()
            )
            .await
            .unwrap(),
        EnrollmentStatus::AcceptedWithoutSession
    );
    assert_eq!(
        first
            .register_account(
                a.digest(),
                ia,
                AccountHandle::new("changed_handle".into()).unwrap(),
                AccountDisplayName::new("First".into()).unwrap()
            )
            .await,
        Err(SessionError::InvalidInput)
    );
    first
        .set_enrollment_enabled(policy.epoch, false)
        .await
        .unwrap();
    assert_eq!(
        second.read_enrollment(a.digest()).await,
        Err(SessionError::Unauthenticated)
    );
    fixture.close().await;
}
#[tokio::test]
#[ignore = "requires real disposable PostgreSQL16; missing setup fails"]
async fn target_profile_guard_orders_visibility_write_and_rechecks_current_authority() {
    let fixture = Fixture::new().await;
    let first = PgSessionStore::new(fixture.pool.clone());
    let second = PgSessionStore::new(fixture.other.clone());
    let (viewer, _) = fixture.enroll("viewer", "viewer_handle").await;
    let (target, target_id) = fixture.enroll("target", "target_handle").await;
    let (mut publication, visible) = first
        .other_account_profile(viewer, AccountHandle::new("target_handle".into()).unwrap())
        .await
        .unwrap();
    assert_eq!(visible.visibility, AccountProfileVisibility::Public);
    let backend: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&fixture.other)
        .await
        .unwrap();
    let write = tokio::spawn(async move {
        second
            .update_account_profile(
                target,
                AccountOperationId::generate().unwrap(),
                1,
                AccountDisplayName::new("Hidden".into()).unwrap(),
                AccountProfileVisibility::Private,
            )
            .await
    });
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND NOT granted)").bind(backend).fetch_one(&fixture.admin).await.unwrap();
        if waiting {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "target permission write must reach independent PostgreSQL advisory-wait oracle"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        !write.is_finished(),
        "target permission mutation must wait for other account first-frame fence"
    );
    assert_eq!(
        publication
            .publish(|_| visible.display_name.as_str().to_owned())
            .unwrap(),
        "target_handle"
    );
    drop(publication);
    write.await.unwrap().unwrap();
    assert!(matches!(
        first
            .other_account_profile(viewer, AccountHandle::new("target_handle".into()).unwrap())
            .await,
        Err(SessionError::Unauthenticated)
    ));
    sqlx::query("UPDATE account_profiles SET visibility='public' WHERE user_id=$1")
        .bind(Uuid::from_u128(target_id.0))
        .execute(&fixture.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE session_accounts SET enabled=FALSE WHERE user_id=$1")
        .bind(Uuid::from_u128(target_id.0))
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(matches!(
        first
            .other_account_profile(viewer, AccountHandle::new("target_handle".into()).unwrap())
            .await,
        Err(SessionError::Unauthenticated)
    ));
    fixture.close().await;
}
#[tokio::test]
#[ignore = "requires real disposable PostgreSQL16; missing setup fails"]
async fn profile_revision_receipt_replay_cannot_overwrite_or_bypass_revocation() {
    let fixture = Fixture::new().await;
    let store = PgSessionStore::new(fixture.pool.clone());
    let (op, user) = fixture.enroll("editor", "editor_handle").await;
    let id = AccountOperationId::generate().unwrap();
    for _ in 0..2 {
        store
            .update_account_profile(
                op,
                id,
                1,
                AccountDisplayName::new("Updated".into()).unwrap(),
                AccountProfileVisibility::Friends,
            )
            .await
            .unwrap();
    }
    let observed_before: i64 =
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(
        store
            .update_account_profile(
                op,
                id,
                1,
                AccountDisplayName::new("Wrong receipt payload".into()).unwrap(),
                AccountProfileVisibility::Friends
            )
            .await,
        Err(SessionError::InvalidInput)
    );
    let observed_after: i64 =
        sqlx::query_scalar("SELECT last_observed_at_ms FROM session_accounts WHERE user_id=$1")
            .bind(Uuid::from_u128(user.0))
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert!(
        observed_after >= observed_before,
        "rejected mismatched operation must retain its trusted observed clock"
    );
    assert_eq!(
        store
            .update_account_profile(
                op,
                AccountOperationId::generate().unwrap(),
                1,
                AccountDisplayName::new("Stale".into()).unwrap(),
                AccountProfileVisibility::Public
            )
            .await,
        Err(SessionError::Conflict)
    );
    let (guard, current) = store.self_account_profile(op).await.unwrap();
    let current = current.expect("enrollment created the requested self profile");
    assert_eq!(current.revision, 2);
    assert_eq!(current.display_name.as_str(), "Updated");
    drop(guard);
    store
        .invalidate_account_epoch(user, AccountEpoch::new(0).unwrap())
        .await
        .unwrap();
    assert_eq!(
        store
            .update_account_profile(
                op,
                id,
                1,
                AccountDisplayName::new("Updated".into()).unwrap(),
                AccountProfileVisibility::Friends
            )
            .await,
        Err(SessionError::Unauthenticated)
    );
    fixture.close().await;
}

#[tokio::test]
#[ignore = "requires real disposable PostgreSQL16; missing setup fails"]
async fn multi_account_backend_loss_keeps_target_exclusion_until_expired_frame_is_suppressed() {
    let fixture = Fixture::new().await;
    let first = PgSessionStore::new(fixture.pool.clone());
    let second = PgSessionStore::new(fixture.other.clone());
    let (viewer, _) = fixture.enroll("loss_viewer", "loss_viewer").await;
    let (target, _) = fixture.enroll("loss_target", "loss_target").await;
    let (mut publication, _) = first
        .other_account_profile(viewer, AccountHandle::new("loss_target".into()).unwrap())
        .await
        .unwrap();
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(publication.backend_pid())
        .fetch_one(&fixture.admin)
        .await
        .unwrap();
    assert!(terminated);
    let write = tokio::spawn(async move {
        second
            .update_account_profile(
                target,
                AccountOperationId::generate().unwrap(),
                1,
                AccountDisplayName::new("Retired target".into()).unwrap(),
                AccountProfileVisibility::Private,
            )
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(75)).await;
    assert!(
        !write.is_finished(),
        "lost publication backend must retain committed peer exclusion"
    );
    tokio::time::timeout(std::time::Duration::from_secs(4), write)
        .await
        .expect("bounded durable exclusion must expire")
        .unwrap()
        .unwrap();
    let mut invoked = false;
    assert_eq!(
        publication
            .publish(|_| {
                invoked = true;
            })
            .unwrap_err(),
        SessionError::Unauthenticated
    );
    assert!(
        !invoked,
        "expired private frame must not be built after peer policy mutation"
    );
    drop(publication);
    fixture.close().await;
}
