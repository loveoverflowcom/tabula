//! Real disposable `PostgreSQL` acceptance, never a conditional green skip.
use super::*;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tabula_session::{
    AccountEpoch, AccountProfileAuthority, AccountProfileVisibility, AuthSessionId,
    CredentialDigest, IssueSession, ProviderIdentityKey, SessionBinding, SessionChannel,
    SessionContextBinding, SessionContextId, SessionPublication,
};

static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    admin: PgPool,
    pool: PgPool,
    second: PgPool,
    store: PgSessionStore,
    schema: String,
}
#[derive(Clone, Copy)]
struct Actor {
    operation: CredentialOperation,
    binding: SessionBinding,
    user: UserId,
}
impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("DATABASE_URL")
            .expect("real social PostgreSQL acceptance requires DATABASE_URL; setup cannot skip");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("social acceptance admin must connect");
        let version: i32 =
            sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
                .fetch_one(&admin)
                .await
                .unwrap();
        assert!(
            version >= 160_000,
            "social acceptance requires PostgreSQL16+"
        );
        let schema = format!(
            "tabula_social_acceptance_{}_{}",
            std::process::id(),
            NEXT_SCHEMA.fetch_add(1, Ordering::SeqCst)
        );
        assert!(schema
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'));
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&admin)
            .await
            .expect("isolated social schema must be created");
        let pool = Self::pool(&url, &schema).await;
        let second = Self::pool(&url, &schema).await;
        let store = PgSessionStore::new(pool.clone());
        store
            .migrate_social()
            .await
            .expect("all strict social prerequisites must migrate");
        store
            .migrate_social()
            .await
            .expect("identical migration versions/checksums must repeat");
        Self {
            admin,
            pool,
            second,
            store,
            schema,
        }
    }
    async fn pool(url: &str, schema: &str) -> PgPool {
        let path = format!("{schema},public");
        PgPoolOptions::new()
            .max_connections(8)
            .after_connect(move |connection, _| {
                let path = path.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path',$1,false)")
                        .bind(path)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(url)
            .await
            .expect("isolated social pool must connect")
    }
    async fn actor(&self, number: u8, handle: &str) -> Actor {
        let user = Uuid::from_u128(u128::from(number));
        sqlx::query("INSERT INTO session_accounts(user_id,authorization_epoch,enabled,last_observed_at_ms) VALUES($1,0,TRUE,0)").bind(user).execute(&self.pool).await.unwrap();
        let identity =
            ProviderIdentityKey::new("https://social.invalid", format!("subject-{number}"))
                .unwrap();
        sqlx::query(
            "INSERT INTO session_provider_identities(issuer,subject,user_id) VALUES($1,$2,$3)",
        )
        .bind(identity.issuer())
        .bind(identity.subject())
        .bind(user)
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO account_profiles(user_id,handle,display_name,visibility,revision) VALUES($1,$2,$3,'public',1)").bind(user).bind(handle).bind(format!("Name {number}")).execute(&self.pool).await.unwrap();
        let digest = CredentialDigest::from_bytes([number; 32]);
        let snapshot = self
            .store
            .issue_session(IssueSession {
                identity,
                expected_epoch: AccountEpoch::new(0).unwrap(),
                id: AuthSessionId::new(100 + u128::from(number)).unwrap(),
                channel: SessionChannel::BrowserCookie,
                credential_digest: digest,
                context_id: SessionContextId::new(200 + u128::from(number)).unwrap(),
            })
            .await
            .unwrap();
        Actor {
            operation: CredentialOperation {
                digest,
                channel: SessionChannel::BrowserCookie,
                context: Some(SessionContextBinding {
                    context_id: snapshot.context_id(),
                    authorization_epoch: snapshot.authorization_epoch(),
                }),
            },
            binding: snapshot.binding(),
            user: UserId(user.as_u128()),
        }
    }
    async fn close(self) {
        self.pool.close().await;
        self.second.close().await;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP SCHEMA {} CASCADE",
            self.schema
        )))
        .execute(&self.admin)
        .await
        .unwrap();
        self.admin.close().await;
    }
}
fn operation(number: u128, action: SocialAction) -> SocialMutation {
    SocialMutation {
        operation_id: format!("{number:032x}"),
        action,
    }
}
fn sent(number: u128, target: Actor) -> SocialMutation {
    operation(
        number,
        SocialAction::Send {
            target_user_id: format!("{:032x}", target.user.0),
        },
    )
}
fn release<T, P: SessionPublication>(mut candidate: GuardedSocial<T, P>) -> T {
    candidate
        .publication
        .publish(|_| ())
        .expect("candidate must be live at actual frame handoff");
    candidate.value
}
async fn send(fixture: &Fixture, sender: Actor, target: Actor, number: u128) -> FriendRequestView {
    release(
        fixture
            .store
            .social_mutate(sender.operation, sent(number, target))
            .await
            .expect("advertised legal Send must commit"),
    )
    .request
}
async fn accept(
    fixture: &Fixture,
    recipient: Actor,
    request: &FriendRequestView,
    number: u128,
) -> FriendRequestView {
    release(
        fixture
            .store
            .social_mutate(
                recipient.operation,
                operation(
                    number,
                    SocialAction::Accept {
                        request_id: request.request_id.clone(),
                        expected_revision: request.revision,
                    },
                ),
            )
            .await
            .expect("advertised legal Accept must commit"),
    )
    .request
}

#[test]
fn virtual_expiry_is_stable_and_cannot_revive_an_old_pending_request() {
    let pending = FriendRequestRecord::new(1, UserId(2), UserId(3), 100).unwrap();
    let expired = observed_record(pending.clone(), pending.expires_at_ms).unwrap();
    assert_eq!(expired.status, FriendRequestStatus::Expired);
    assert_eq!(expired.updated_at_ms, pending.expires_at_ms);
    assert_eq!(
        observed_record(pending.clone(), pending.expires_at_ms + 100).unwrap(),
        expired
    );
    assert_eq!(observed_record(expired, 99), Err(SocialError::Unavailable));
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL16+ via DATABASE_URL"]
#[allow(clippy::too_many_lines)] // One real fixture covers the complete participant lifecycle.
async fn postgres_requests_permissions_terminals_idempotency_and_search_projection() {
    let f = Fixture::new().await;
    let alice = f.actor(1, "alice").await;
    let bob = f.actor(2, "bob").await;
    let carol = f.actor(3, "carol").await;
    let request = send(&f, alice, bob, 1).await;
    assert_eq!(request.status, FriendRequestStatus::Pending);
    for viewer in [alice, bob] {
        let snapshot = release(
            f.store
                .social_snapshot(SocialSession::Connection(viewer.binding), vec![])
                .await
                .unwrap(),
        );
        assert!(snapshot.friends.is_empty());
        assert_eq!(
            snapshot.requests.len(),
            1,
            "fresh snapshot must show each participant's committed pending request"
        );
        assert_eq!(snapshot.requests[0].request_id, request.request_id);
        assert_eq!(snapshot.requests[0].status, FriendRequestStatus::Pending);
        assert_eq!(
            snapshot.requests[0].sender.user_id,
            format!("{:032x}", alice.user.0)
        );
        assert_eq!(
            snapshot.requests[0].recipient.user_id,
            format!("{:032x}", bob.user.0)
        );
    }
    for actor in [alice, carol] {
        let result = f
            .store
            .social_mutate(
                actor.operation,
                operation(
                    10 + actor.user.0,
                    SocialAction::Accept {
                        request_id: request.request_id.clone(),
                        expected_revision: 1,
                    },
                ),
            )
            .await;
        assert_eq!(
            result.unwrap_err(),
            SocialError::Denied,
            "sender/outsider may not accept"
        );
    }
    assert_eq!(
        f.store
            .social_mutate(
                bob.operation,
                operation(
                    20,
                    SocialAction::Cancel {
                        request_id: request.request_id.clone(),
                        expected_revision: 1
                    }
                )
            )
            .await
            .unwrap_err(),
        SocialError::Denied
    );
    let accepted = accept(&f, bob, &request, 21).await;
    assert_eq!(accepted.status, FriendRequestStatus::Accepted);
    assert_eq!(accepted.revision, 2);
    let retry = release(
        f.store
            .social_mutate(alice.operation, sent(1, bob))
            .await
            .unwrap(),
    );
    assert!(retry.duplicate);
    assert_eq!(retry.request.request_id, accepted.request_id);
    assert_eq!(retry.request.status, FriendRequestStatus::Accepted);
    assert_eq!(
        f.store
            .social_mutate(alice.operation, sent(1, carol))
            .await
            .unwrap_err(),
        SocialError::Conflict
    );
    assert_eq!(
        f.store
            .social_mutate(alice.operation, sent(22, bob))
            .await
            .unwrap_err(),
        SocialError::Conflict
    );
    let declined = send(&f, alice, carol, 30).await;
    let declined = release(
        f.store
            .social_mutate(
                carol.operation,
                operation(
                    31,
                    SocialAction::Decline {
                        request_id: declined.request_id,
                        expected_revision: 1,
                    },
                ),
            )
            .await
            .unwrap(),
    )
    .request;
    assert_eq!(declined.status, FriendRequestStatus::Declined);
    let cancelled = send(&f, alice, carol, 32).await;
    let cancelled = release(
        f.store
            .social_mutate(
                alice.operation,
                operation(
                    33,
                    SocialAction::Cancel {
                        request_id: cancelled.request_id,
                        expected_revision: 1,
                    },
                ),
            )
            .await
            .unwrap(),
    )
    .request;
    assert_eq!(cancelled.status, FriendRequestStatus::Cancelled);
    let expiring = send(&f, alice, carol, 34).await;
    sqlx::query(
        "UPDATE tabula_friend_requests SET expires_at_ms=created_at_ms+1 WHERE request_id=$1",
    )
    .bind(id(&expiring.request_id).unwrap())
    .execute(&f.pool)
    .await
    .unwrap();
    let expired = f
        .store
        .social_mutate(
            carol.operation,
            operation(
                35,
                SocialAction::Accept {
                    request_id: expiring.request_id.clone(),
                    expected_revision: 1,
                },
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(expired, SocialError::Conflict);
    let stored: String =
        sqlx::query_scalar("SELECT status FROM tabula_friend_requests WHERE request_id=$1")
            .bind(id(&expiring.request_id).unwrap())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(stored, "expired");
    let empty = release(
        f.store
            .social_search(SocialSession::Credential(alice.operation), String::new())
            .await
            .unwrap(),
    );
    assert!(empty.results.is_empty());
    let unicode = release(
        f.store
            .social_search(SocialSession::Credential(alice.operation), "Việt".into())
            .await
            .unwrap(),
    );
    assert!(unicode.results.is_empty());
    let literal = release(
        f.store
            .social_search(SocialSession::Credential(alice.operation), "%".into())
            .await
            .unwrap(),
    );
    assert!(literal.results.is_empty());
    let found = release(
        f.store
            .social_search(SocialSession::Credential(alice.operation), "bo".into())
            .await
            .unwrap(),
    );
    assert_eq!(found.results.len(), 1);
    assert_eq!(found.results[0].relationship, SocialRelationship::Accepted);
    assert_eq!(found.results[0].identity.handle, "bob");
    f.store
        .update_account_profile(
            bob.operation,
            AccountOperationId::parse(&format!("{:032x}", 48)).unwrap(),
            1,
            AccountDisplayName::new("Friends Name".into()).unwrap(),
            AccountProfileVisibility::Friends,
        )
        .await
        .unwrap();
    let permitted = release(
        f.store
            .social_search(SocialSession::Credential(alice.operation), "bo".into())
            .await
            .unwrap(),
    );
    assert_eq!(permitted.results.len(), 1);
    assert_eq!(
        permitted.results[0].identity.display_name.as_deref(),
        Some("Friends Name")
    );
    let denied = release(
        f.store
            .social_search(SocialSession::Credential(carol.operation), "bo".into())
            .await
            .unwrap(),
    );
    assert!(
        denied.results.is_empty(),
        "nonfriend directory search must not enumerate Friends visibility"
    );
    f.store
        .update_account_profile(
            bob.operation,
            AccountOperationId::parse(&format!("{:032x}", 50)).unwrap(),
            2,
            AccountDisplayName::new("Hidden Name".into()).unwrap(),
            AccountProfileVisibility::Private,
        )
        .await
        .unwrap();
    let hidden = release(
        f.store
            .social_search(SocialSession::Credential(alice.operation), "bo".into())
            .await
            .unwrap(),
    );
    assert!(
        hidden.results.is_empty(),
        "private profiles must not appear in directory search"
    );
    let participants = release(
        f.store
            .social_snapshot(
                SocialSession::Credential(alice.operation),
                vec![bob.binding.into()],
            )
            .await
            .unwrap(),
    );
    assert_eq!(participants.friends[0].identity.handle, "bob");
    assert_eq!(participants.friends[0].identity.display_name, None);
    assert_eq!(
        participants
            .requests
            .iter()
            .find(|request| request.request_id == accepted.request_id)
            .unwrap()
            .recipient
            .display_name,
        None
    );
    f.store
        .update_account_profile(
            bob.operation,
            AccountOperationId::parse(&format!("{:032x}", 49)).unwrap(),
            3,
            AccountDisplayName::new("Public Name".into()).unwrap(),
            AccountProfileVisibility::Public,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE session_accounts SET enabled=FALSE,authorization_epoch=authorization_epoch+1 WHERE user_id=$1").bind(Uuid::from_u128(bob.user.0)).execute(&f.pool).await.unwrap();
    let denied = release(
        f.store
            .social_search(SocialSession::Credential(alice.operation), "bo".into())
            .await
            .unwrap(),
    );
    assert!(
        denied.results.is_empty(),
        "disabled public profiles must not appear in directory search"
    );
    let disabled = release(
        f.store
            .social_snapshot(
                SocialSession::Credential(alice.operation),
                vec![bob.binding.into()],
            )
            .await
            .unwrap(),
    );
    assert_eq!(disabled.friends[0].identity.display_name, None);
    assert_eq!(disabled.friends[0].presence, PresenceObservation::Unknown);
    assert_eq!(
        f.store
            .social_mutate(carol.operation, sent(51, bob))
            .await
            .unwrap_err(),
        SocialError::Denied
    );
    let dave = f.actor(4, "dave").await;
    let pending = send(&f, dave, carol, 52).await;
    sqlx::query("UPDATE session_accounts SET enabled=FALSE,authorization_epoch=authorization_epoch+1 WHERE user_id=$1")
        .bind(Uuid::from_u128(dave.user.0)).execute(&f.pool).await.unwrap();
    let denied = f
        .store
        .social_mutate(
            carol.operation,
            operation(
                53,
                SocialAction::Accept {
                    request_id: pending.request_id.clone(),
                    expected_revision: pending.revision,
                },
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(
        denied,
        SocialError::Denied,
        "current caller cannot accept a disabled peer's old pending request"
    );
    let status: String =
        sqlx::query_scalar("SELECT status FROM tabula_friend_requests WHERE request_id=$1")
            .bind(id(&pending.request_id).unwrap())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
    let declined = release(
        f.store
            .social_mutate(
                carol.operation,
                operation(
                    54,
                    SocialAction::Decline {
                        request_id: pending.request_id,
                        expected_revision: pending.revision,
                    },
                ),
            )
            .await
            .unwrap(),
    );
    assert_eq!(
        declined.request.status,
        FriendRequestStatus::Declined,
        "a disabled peer must not trap participant-owned terminal cleanup"
    );
    let edward = f.actor(5, "edward").await;
    let pending = send(&f, edward, carol, 55).await;
    sqlx::query("UPDATE session_accounts SET enabled=FALSE,authorization_epoch=authorization_epoch+1 WHERE user_id=$1")
        .bind(Uuid::from_u128(edward.user.0)).execute(&f.pool).await.unwrap();
    sqlx::query("UPDATE tabula_friend_requests SET created_at_ms=(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint-1000,updated_at_ms=(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint-1000,expires_at_ms=(EXTRACT(EPOCH FROM clock_timestamp())*1000)::bigint WHERE request_id=$1")
        .bind(id(&pending.request_id).unwrap()).execute(&f.pool).await.unwrap();
    assert_eq!(
        f.store
            .social_mutate(
                carol.operation,
                operation(
                    56,
                    SocialAction::Accept {
                        request_id: pending.request_id.clone(),
                        expected_revision: pending.revision,
                    }
                )
            )
            .await
            .unwrap_err(),
        SocialError::Conflict
    );
    let status: String =
        sqlx::query_scalar("SELECT status FROM tabula_friend_requests WHERE request_id=$1")
            .bind(id(&pending.request_id).unwrap())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(
        status, "expired",
        "trusted deadline retirement precedes disabled-peer acceptance denial"
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL16+ via DATABASE_URL"]
#[allow(clippy::too_many_lines)] // Keep both orders of the real permission/publication race together.
async fn postgres_peer_revocation_and_visibility_publication_fence() {
    let f = Fixture::new().await;
    let alice = f.actor(1, "alice").await;
    let bob = f.actor(2, "bob").await;
    let request = send(&f, alice, bob, 1).await;
    accept(&f, bob, &request, 2).await;
    let live = release(
        f.store
            .social_snapshot(
                SocialSession::Connection(alice.binding),
                vec![bob.binding.into()],
            )
            .await
            .unwrap(),
    );
    assert!(matches!(
        live.friends[0].presence,
        PresenceObservation::Online { .. }
    ));
    let stale_candidate = SocialPresenceCandidate {
        binding: bob.binding,
        stale_as_of_ms: Some(live.generated_at_ms),
        fresh_until: None,
    };
    let stale = release(
        f.store
            .social_snapshot(
                SocialSession::Connection(alice.binding),
                vec![stale_candidate],
            )
            .await
            .unwrap(),
    );
    assert_eq!(
        stale.friends[0].presence,
        PresenceObservation::Stale {
            as_of_ms: Some(live.generated_at_ms),
            last_seen_ms: None,
        },
        "unconfirmed age alone must preserve a permitted positive timestamp as Stale"
    );
    let another_tab = release(
        f.store
            .social_snapshot(
                SocialSession::Connection(alice.binding),
                vec![stale_candidate, bob.binding.into()],
            )
            .await
            .unwrap(),
    );
    assert!(
        matches!(
            another_tab.friends[0].presence,
            PresenceObservation::Online { .. }
        ),
        "one stale tab must not override another actually fresh attachment"
    );
    let torn_down = release(
        f.store
            .social_snapshot(SocialSession::Connection(alice.binding), vec![])
            .await
            .unwrap(),
    );
    assert!(
        matches!(
            torn_down.friends[0].presence,
            PresenceObservation::Offline { .. }
        ),
        "only explicit absence after attachment teardown permits Offline"
    );
    f.store.revoke_session(bob.binding).await.unwrap();
    let revoked = release(
        f.store
            .social_snapshot(
                SocialSession::Connection(alice.binding),
                vec![bob.binding.into()],
            )
            .await
            .unwrap(),
    );
    assert!(
        matches!(
            revoked.friends[0].presence,
            PresenceObservation::Offline { .. }
        ),
        "revoked binding cannot fabricate Online"
    );
    // Reissue Bob using a separately verified provider mapping; old binding remains terminal.
    let snapshot = f
        .store
        .issue_session(IssueSession {
            identity: ProviderIdentityKey::new("https://social.invalid", "subject-2").unwrap(),
            expected_epoch: AccountEpoch::new(0).unwrap(),
            id: AuthSessionId::new(302).unwrap(),
            channel: SessionChannel::BrowserCookie,
            credential_digest: CredentialDigest::from_bytes([42; 32]),
            context_id: SessionContextId::new(402).unwrap(),
        })
        .await
        .unwrap();
    let bob_op = CredentialOperation {
        digest: CredentialDigest::from_bytes([42; 32]),
        channel: SessionChannel::BrowserCookie,
        context: Some(SessionContextBinding {
            context_id: snapshot.context_id(),
            authorization_epoch: snapshot.authorization_epoch(),
        }),
    };
    let candidate = f
        .store
        .social_snapshot(
            SocialSession::Credential(alice.operation),
            vec![snapshot.binding().into()],
        )
        .await
        .unwrap();
    assert_eq!(
        candidate.value.friends[0].identity.display_name.as_deref(),
        Some("Name 2")
    );
    let other = PgSessionStore::new(f.second.clone());
    let narrowing = tokio::spawn(async move {
        other
            .update_account_profile(
                bob_op,
                AccountOperationId::parse(&format!("{:032x}", 3)).unwrap(),
                1,
                AccountDisplayName::new("Secret".into()).unwrap(),
                AccountProfileVisibility::Private,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !narrowing.is_finished(),
        "peer privacy commit must wait behind actual held frame fence"
    );
    let old = release(candidate);
    assert_eq!(
        old.friends[0].identity.display_name.as_deref(),
        Some("Name 2")
    );
    tokio::time::timeout(Duration::from_secs(5), narrowing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let hidden = release(
        f.store
            .social_snapshot(
                SocialSession::Credential(alice.operation),
                vec![snapshot.binding().into()],
            )
            .await
            .unwrap(),
    );
    assert_eq!(hidden.friends[0].identity.display_name, None);
    assert_eq!(hidden.friends[0].presence, PresenceObservation::Unknown);
    let private_expired = release(
        f.store
            .social_snapshot(
                SocialSession::Credential(alice.operation),
                vec![SocialPresenceCandidate {
                    binding: snapshot.binding(),
                    stale_as_of_ms: None,
                    fresh_until: Some(
                        std::time::Instant::now()
                            .checked_sub(Duration::from_secs(1))
                            .unwrap(),
                    ),
                }],
            )
            .await
            .unwrap(),
    );
    assert_eq!(
        private_expired.friends[0].presence,
        PresenceObservation::Unknown,
        "hidden activity cannot alter response availability through transport age"
    );
    f.store.revoke_session(alice.binding).await.unwrap();
    assert_eq!(
        f.store
            .social_snapshot(SocialSession::Connection(alice.binding), vec![])
            .await
            .unwrap_err(),
        SocialError::Unauthenticated
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL16+ via DATABASE_URL"]
async fn postgres_competing_actions_and_expiry_after_receipt_wait() {
    let f = Fixture::new().await;
    let alice = f.actor(1, "alice").await;
    let bob = f.actor(2, "bob").await;
    let request = send(&f, alice, bob, 1).await;
    let accept_store = f.store.clone();
    let cancel_store = PgSessionStore::new(f.second.clone());
    let accept_id = request.request_id.clone();
    let cancel_id = request.request_id.clone();
    let accepted = tokio::spawn(async move {
        accept_store
            .social_mutate(
                bob.operation,
                operation(
                    2,
                    SocialAction::Accept {
                        request_id: accept_id,
                        expected_revision: 1,
                    },
                ),
            )
            .await
            .map(|candidate| release(candidate).request.status)
    });
    let cancelled = tokio::spawn(async move {
        cancel_store
            .social_mutate(
                alice.operation,
                operation(
                    3,
                    SocialAction::Cancel {
                        request_id: cancel_id,
                        expected_revision: 1,
                    },
                ),
            )
            .await
            .map(|candidate| release(candidate).request.status)
    });
    let a = accepted.await.unwrap();
    let b = cancelled.await.unwrap();
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "exactly one competing transition commits"
    );
    assert!(a == Err(SocialError::Conflict) || b == Err(SocialError::Conflict));
    // A separate pair gives a fresh pending row regardless of the race winner.
    let carol = f.actor(3, "carol").await;
    let request = send(&f, alice, carol, 4).await;
    sqlx::query("UPDATE tabula_friend_requests SET expires_at_ms=((extract(epoch from clock_timestamp())*1000)::bigint)+250 WHERE request_id=$1").bind(id(&request.request_id).unwrap()).execute(&f.pool).await.unwrap();
    // Independent SQL oracle injects a resource/receipt wait AFTER the row decision.
    sqlx::raw_sql("CREATE FUNCTION social_acceptance_delay() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(0.35); RETURN NEW; END $$; CREATE TRIGGER social_receipt_delay BEFORE INSERT ON tabula_social_operations FOR EACH ROW EXECUTE FUNCTION social_acceptance_delay();").execute(&f.pool).await.unwrap();
    let result = f
        .store
        .social_mutate(
            carol.operation,
            operation(
                5,
                SocialAction::Accept {
                    request_id: request.request_id.clone(),
                    expected_revision: 1,
                },
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(
        result,
        SocialError::Conflict,
        "deadline at actual commit must fence an earlier pending decision"
    );
    let stored: (String, i64) =
        sqlx::query_as("SELECT status,revision FROM tabula_friend_requests WHERE request_id=$1")
            .bind(id(&request.request_id).unwrap())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(stored, ("expired".into(), 2));
    let receipt: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tabula_social_operations WHERE actor_id=$1 AND operation_id=$2)").bind(Uuid::from_u128(carol.user.0)).bind(Uuid::from_u128(5)).fetch_one(&f.pool).await.unwrap();
    assert!(!receipt);
    f.close().await;
}

#[tokio::test]
#[ignore = "requires isolated real PostgreSQL16+ via DATABASE_URL"]
async fn postgres_expired_and_repeated_socket_frame_handoffs_never_transfer() {
    use tabula_session::{BoundedSocketFrame, SocketFramePublication};
    let f = Fixture::new().await;
    let alice = f.actor(1, "alice").await;
    let mut expired = f
        .store
        .social_snapshot(SocialSession::Connection(alice.binding), vec![])
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        expired
            .publication
            .handoff(BoundedSocketFrame::new("{}".into()).unwrap(), |_| calls
                .set(calls.get() + 1)),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(calls.get(), 0);
    drop(expired);
    let mut current = f
        .store
        .social_snapshot(SocialSession::Connection(alice.binding), vec![])
        .await
        .unwrap();
    current
        .publication
        .handoff(BoundedSocketFrame::new("{}".into()).unwrap(), |_| {
            calls.set(calls.get() + 1);
        })
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        current
            .publication
            .handoff(BoundedSocketFrame::new("{}".into()).unwrap(), |_| calls
                .set(calls.get() + 1)),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(calls.get(), 1);
    drop(current);
    let inner = f
        .store
        .begin_accounts_publication(AccountsPrincipal::Binding(alice.binding), vec![])
        .await
        .unwrap();
    let mut presence_expired = SocialPublication {
        inner,
        online_until: Some(std::time::Instant::now() + Duration::from_millis(50)),
    };
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        presence_expired.publish(|_| calls.set(calls.get() + 1)),
        Err(SessionError::Unavailable)
    );
    assert_eq!(
        presence_expired.handoff(BoundedSocketFrame::new("{}".into()).unwrap(), |_| calls
            .set(calls.get() + 1)),
        Err(SessionError::Unavailable)
    );
    assert_eq!(calls.get(), 1, "current account authority cannot release a transport observation that expired while queued");
    drop(presence_expired);
    f.close().await;
}
