use super::*;

fn time(ms: u64) -> UnixMillis {
    UnixMillis::new(ms).unwrap()
}
fn epoch(value: u64) -> AccountEpoch {
    AccountEpoch::new(value).unwrap()
}
fn generation(value: u64) -> CredentialGeneration {
    CredentialGeneration::new(value).unwrap()
}
fn digest(value: u8) -> CredentialDigest {
    CredentialDigest::from_bytes([value; 32])
}
fn account() -> AccountRecord {
    AccountRecord::new(UserId(10), epoch(0), true, time(1_000)).unwrap()
}
fn record() -> SessionRecord {
    SessionRecord::issue(
        AuthSessionId::new(20).unwrap(),
        UserId(10),
        epoch(0),
        SessionChannel::BrowserCookie,
        digest(1),
        SessionContextId::new(30).unwrap(),
        time(1_000),
    )
    .unwrap()
}

#[test]
fn issue_initializes_literal_deadlines_without_public_id_authority() {
    let record = record();
    let raw = record.clone().into_raw();
    assert_eq!(raw.created_at_ms, 1_000);
    assert_eq!(raw.last_activity_at_ms, 1_000);
    assert_eq!(raw.last_observed_at_ms, 1_000);
    assert_eq!(raw.idle_deadline_ms, 1_801_000);
    assert_eq!(raw.absolute_deadline_ms, 86_401_000);
    assert_eq!(raw.credential_generation, 0);
    assert_eq!(raw.credential_digest, vec![1; 32]);
    assert_eq!(raw.context_id, 30);
    assert_eq!(SessionRecord::try_from(raw).unwrap(), record);
    assert!(SessionRecord::issue(
        AuthSessionId::new(20).unwrap(),
        UserId(0),
        epoch(0),
        SessionChannel::NativeBearer,
        digest(1),
        SessionContextId::new(30).unwrap(),
        time(1_000)
    )
    .is_err());
    assert_eq!(
        SessionRecord::issue(
            AuthSessionId::new(20).unwrap(),
            UserId(10),
            epoch(0),
            SessionChannel::NativeBearer,
            digest(1),
            SessionContextId::new(30).unwrap(),
            time(i64::MAX as u64 - 86_399_999)
        ),
        Err(SessionError::InvalidInput)
    );
    assert!(crate::SessionCredential::parse("00000000-0000-0000-0000-000000000014").is_err());
}

#[test]
fn idle_deadline_partitions_include_equality_and_terminal_marker() {
    for (ms, expected) in [
        (1_800_999, Ok(())),
        (1_801_000, Err(SessionError::Unauthenticated)),
        (1_801_001, Err(SessionError::Unauthenticated)),
    ] {
        let mut record = record();
        assert_eq!(record.observe(&account(), time(ms)).map(|_| ()), expected);
        assert_eq!(record.last_activity_at().get(), 1_000);
        assert_eq!(record.idle_deadline().get(), 1_801_000);
        assert_eq!(record.expired_at().is_some(), ms >= 1_801_000);
        assert_eq!(
            SessionRecord::try_from(record.clone().into_raw()).unwrap(),
            record
        );
    }
}

#[test]
fn absolute_equality_expires_even_after_accepted_activity() {
    let mut record = record();
    let account = account();
    // Every accepted effect precedes its current idle deadline.
    for step in 1..=48 {
        let ms = 1_000 + step * 1_799_999;
        record
            .record_activity(&account, ActivityKind::GameCommandAccepted, time(ms))
            .unwrap();
    }
    assert_eq!(record.absolute_deadline().get(), 86_401_000);
    assert_eq!(record.idle_deadline().get(), 86_401_000);
    assert!(record.observe(&account, time(86_400_999)).is_ok());
    assert_eq!(
        record.observe(&account, time(86_401_000)),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(record.expired_at().unwrap().get(), 86_401_000);
    assert_eq!(
        record.record_activity(
            &account,
            ActivityKind::ProtectedMutationCommitted,
            time(86_401_001)
        ),
        Err(SessionError::Unauthenticated)
    );
}

#[test]
fn expiry_is_durable_and_clock_regression_cannot_restore_it() {
    let mut record = record();
    assert_eq!(
        record.observe(&account(), time(1_801_000)),
        Err(SessionError::Unauthenticated)
    );
    let mut loaded = SessionRecord::try_from(record.clone().into_raw()).unwrap();
    let before = loaded.clone();
    assert_eq!(
        loaded.observe(&account(), time(1_800_999)),
        Err(SessionError::Unavailable)
    );
    assert_eq!(loaded, before);
    assert_eq!(
        loaded.observe(&account(), time(1_801_001)),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(loaded.expired_at(), record.expired_at());
    assert_eq!(loaded.last_activity_at().get(), 1_000);
}

#[test]
fn observation_floor_is_separate_from_activity_and_regression_is_unavailable() {
    let mut record = record();
    let mut account = account();
    assert!(record.observe(&account, time(2_000)).is_ok());
    let before = record.clone();
    assert_eq!(
        record.observe(&account, time(1_999)),
        Err(SessionError::Unavailable)
    );
    assert_eq!(record, before);
    account.observe(time(3_000)).unwrap();
    assert_eq!(
        record.observe(&account, time(2_999)),
        Err(SessionError::Unavailable)
    );
    assert_eq!(record, before);
    assert_eq!(account.observe(time(2_999)), Err(SessionError::Unavailable));
    assert_eq!(account.last_observed_at().get(), 3_000);
    assert_eq!(record.last_activity_at().get(), 1_000);
}

#[test]
fn only_server_classified_successful_effects_extend_idle() {
    for activity in [
        ActivityKind::Read,
        ActivityKind::Refresh,
        ActivityKind::Rejected,
        ActivityKind::Duplicate,
        ActivityKind::Control,
    ] {
        let mut record = record();
        assert!(!activity.extends_idle());
        record
            .record_activity(&account(), activity, time(2_000))
            .unwrap();
        assert_eq!(record.last_activity_at().get(), 1_000);
        assert_eq!(record.idle_deadline().get(), 1_801_000);
        assert_eq!(record.absolute_deadline().get(), 86_401_000);
        assert_eq!(record.last_observed_at().get(), 2_000);
    }
    for activity in [
        ActivityKind::ProtectedMutationCommitted,
        ActivityKind::GameCommandAccepted,
    ] {
        let mut record = record();
        assert!(activity.extends_idle());
        record
            .record_activity(&account(), activity, time(2_000))
            .unwrap();
        assert_eq!(record.last_activity_at().get(), 2_000);
        assert_eq!(record.idle_deadline().get(), 1_802_000);
        assert_eq!(record.absolute_deadline().get(), 86_401_000);
    }
}

#[test]
fn rotation_preserves_binding_context_and_deadlines_but_rejects_old_credential() {
    let mut record = record();
    let binding = record.binding();
    let snapshot = record
        .rotate(
            &account(),
            digest(1),
            SessionChannel::BrowserCookie,
            generation(0),
            digest(2),
            time(2_000),
        )
        .unwrap();
    assert_eq!(snapshot.id().get(), 20);
    assert_eq!(snapshot.user_id(), UserId(10));
    assert_eq!(snapshot.authorization_epoch().get(), 0);
    assert_eq!(snapshot.credential_generation().get(), 1);
    assert_eq!(snapshot.context_id().get(), 30);
    assert_eq!(snapshot.binding(), binding);
    assert_eq!(snapshot.last_activity_at().get(), 1_000);
    assert_eq!(snapshot.idle_deadline().get(), 1_801_000);
    assert_eq!(snapshot.absolute_deadline().get(), 86_401_000);
    assert_eq!(
        record.observe_credential(
            &account(),
            digest(1),
            SessionChannel::BrowserCookie,
            time(2_001)
        ),
        Err(SessionError::Unauthenticated)
    );
    assert!(record
        .observe_credential(
            &account(),
            digest(2),
            SessionChannel::BrowserCookie,
            time(2_001)
        )
        .is_ok());
    assert!(record.matches_binding(binding));
    assert!(record.observe(&account(), time(2_002)).is_ok());
    assert_eq!(
        record.rotate(
            &account(),
            digest(1),
            SessionChannel::BrowserCookie,
            generation(0),
            digest(3),
            time(2_002)
        ),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(
        record.rotate(
            &account(),
            digest(2),
            SessionChannel::BrowserCookie,
            generation(0),
            digest(3),
            time(2_002)
        ),
        Err(SessionError::Conflict)
    );
    assert_eq!(record.credential_digest(), digest(2));
    assert_eq!(record.credential_generation().get(), 1);
}

#[test]
fn channels_wrong_verifier_and_generation_exhaustion_fail_closed() {
    let mut record = record();
    let before = record.clone();
    assert_eq!(
        record.observe_credential(
            &account(),
            digest(1),
            SessionChannel::NativeBearer,
            time(2_000)
        ),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(
        record.observe_credential(
            &account(),
            digest(2),
            SessionChannel::BrowserCookie,
            time(2_000)
        ),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(record.last_activity_at(), before.last_activity_at());
    assert_eq!(record.idle_deadline(), before.idle_deadline());
    assert_eq!(record.last_observed_at().get(), 2_000);
    assert_eq!(
        record.rotate(
            &account(),
            digest(1),
            SessionChannel::BrowserCookie,
            generation(0),
            digest(1),
            time(2_000)
        ),
        Err(SessionError::InvalidInput)
    );
    let mut raw = record.into_raw();
    raw.credential_generation = i64::MAX as u64;
    let mut loaded = SessionRecord::try_from(raw).unwrap();
    assert_eq!(
        loaded.rotate(
            &account(),
            digest(1),
            SessionChannel::BrowserCookie,
            generation(i64::MAX as u64),
            digest(2),
            time(2_001)
        ),
        Err(SessionError::Unavailable)
    );
    assert_eq!(loaded.credential_digest(), digest(1));
    assert_eq!(loaded.credential_generation().get(), i64::MAX as u64);
    assert_eq!(loaded.last_activity_at().get(), 1_000);
}

#[test]
fn current_session_revocation_is_idempotent_and_does_not_affect_another_device() {
    let mut revoked = record();
    let mut other = SessionRecord::issue(
        AuthSessionId::new(21).unwrap(),
        UserId(10),
        epoch(0),
        SessionChannel::NativeBearer,
        digest(2),
        SessionContextId::new(31).unwrap(),
        time(1_000),
    )
    .unwrap();
    revoked.revoke(time(2_000)).unwrap();
    revoked.revoke(time(2_001)).unwrap();
    assert_eq!(revoked.revoked_at().unwrap().get(), 2_000);
    assert_eq!(revoked.last_activity_at().get(), 1_000);
    assert_eq!(
        revoked.observe(&account(), time(2_001)),
        Err(SessionError::Unauthenticated)
    );
    assert!(other.observe(&account(), time(2_001)).is_ok());
    assert_eq!(
        SessionRecord::try_from(revoked.clone().into_raw()).unwrap(),
        revoked
    );
}

#[test]
fn epoch_and_suspension_fence_all_older_sessions_and_cas_never_wraps() {
    let mut account = account();
    let mut old_record = record();
    account.invalidate(epoch(0), time(2_000)).unwrap();
    assert_eq!(account.authorization_epoch().get(), 1);
    let after = account.clone();
    assert_eq!(
        account.invalidate(epoch(0), time(2_001)),
        Err(SessionError::Conflict)
    );
    assert_eq!(account.authorization_epoch(), after.authorization_epoch());
    assert_eq!(account.last_observed_at().get(), 2_001);
    assert_eq!(account.observe(time(2_000)), Err(SessionError::Unavailable));
    assert_eq!(
        old_record.observe(&account, time(2_001)),
        Err(SessionError::Unauthenticated)
    );
    let disabled = AccountRecord::new(UserId(10), epoch(0), false, time(1_000)).unwrap();
    assert_eq!(
        record().observe(&disabled, time(2_000)),
        Err(SessionError::Unauthenticated)
    );
    let mut maximum =
        AccountRecord::new(UserId(10), epoch(i64::MAX as u64), true, time(1_000)).unwrap();
    let before = maximum.clone();
    assert_eq!(
        maximum.invalidate(epoch(i64::MAX as u64), time(2_000)),
        Err(SessionError::Unavailable)
    );
    assert_eq!(maximum.authorization_epoch(), before.authorization_epoch());
    assert_eq!(maximum.last_observed_at().get(), 2_000);
    assert_eq!(
        maximum.invalidate(epoch(i64::MAX as u64), time(999)),
        Err(SessionError::Unavailable)
    );
}

#[test]
fn every_raw_session_invalid_partition_is_rejected_as_unavailable() {
    let base = record().into_raw();
    let invalid = [
        RawSessionRecord {
            id: 0,
            ..base.clone()
        },
        RawSessionRecord {
            user_id: 0,
            ..base.clone()
        },
        RawSessionRecord {
            context_id: 0,
            ..base.clone()
        },
        RawSessionRecord {
            channel: "browser".into(),
            ..base.clone()
        },
        RawSessionRecord {
            authorization_epoch: u64::MAX,
            ..base.clone()
        },
        RawSessionRecord {
            credential_generation: u64::MAX,
            ..base.clone()
        },
        RawSessionRecord {
            credential_digest: vec![0; 31],
            ..base.clone()
        },
        RawSessionRecord {
            credential_digest: vec![0; 33],
            ..base.clone()
        },
        RawSessionRecord {
            created_at_ms: u64::MAX,
            ..base.clone()
        },
        RawSessionRecord {
            last_activity_at_ms: 999,
            ..base.clone()
        },
        RawSessionRecord {
            last_observed_at_ms: 999,
            ..base.clone()
        },
        RawSessionRecord {
            idle_deadline_ms: 1_801_001,
            ..base.clone()
        },
        RawSessionRecord {
            absolute_deadline_ms: 86_401_001,
            ..base.clone()
        },
        RawSessionRecord {
            revoked_at_ms: Some(999),
            ..base.clone()
        },
        RawSessionRecord {
            revoked_at_ms: Some(1_001),
            ..base.clone()
        },
        RawSessionRecord {
            expired_at_ms: Some(1_800_999),
            last_observed_at_ms: 1_801_000,
            ..base.clone()
        },
        RawSessionRecord {
            expired_at_ms: Some(1_801_000),
            ..base.clone()
        },
    ];
    for raw in invalid {
        assert_eq!(SessionRecord::try_from(raw), Err(SessionError::Unavailable));
    }
    assert_eq!(format!("{base:?}"), "RawSessionRecord([REDACTED])");
    assert_eq!(format!("{:?}", record()), "SessionRecord([REDACTED])");
    assert_eq!(
        format!("{:?}", record().snapshot()),
        "SessionSnapshot([REDACTED])"
    );
    assert_eq!(
        format!("{:?}", record().binding()),
        "SessionBinding([REDACTED])"
    );
}

#[test]
fn raw_account_conversion_checks_every_counter_and_id_path() {
    let base = account().into_raw();
    for raw in [
        RawAccountRecord {
            user_id: 0,
            ..base.clone()
        },
        RawAccountRecord {
            authorization_epoch: u64::MAX,
            ..base.clone()
        },
        RawAccountRecord {
            last_observed_at_ms: u64::MAX,
            ..base.clone()
        },
    ] {
        assert_eq!(AccountRecord::try_from(raw), Err(SessionError::Unavailable));
    }
    assert_eq!(AccountRecord::try_from(base.clone()).unwrap(), account());
    assert_eq!(format!("{base:?}"), "RawAccountRecord([REDACTED])");
}

#[test]
fn all_boundary_errors_are_payload_free_generic_results() {
    for (error, expected) in [
        (SessionError::Unauthenticated, "unauthenticated"),
        (SessionError::Unavailable, "unavailable"),
        (SessionError::Conflict, "conflict"),
        (SessionError::InvalidInput, "invalid input"),
    ] {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn rejected_wrong_channel_and_revocation_persist_due_expiry() {
    let mut wrong_channel = record();
    assert_eq!(
        wrong_channel.observe_credential(
            &account(),
            digest(1),
            SessionChannel::NativeBearer,
            time(1_801_000)
        ),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(wrong_channel.expired_at().unwrap().get(), 1_801_000);
    let loaded = SessionRecord::try_from(wrong_channel.into_raw()).unwrap();
    assert_eq!(loaded.expired_at().unwrap().get(), 1_801_000);
    let mut revoked = record();
    revoked.revoke(time(1_801_000)).unwrap();
    assert_eq!(revoked.expired_at().unwrap().get(), 1_801_000);
    assert_eq!(revoked.revoked_at().unwrap().get(), 1_801_000);
    assert!(SessionRecord::try_from(revoked.into_raw()).is_ok());
}

#[test]
fn impossible_future_session_epoch_is_retired_before_account_can_catch_up() {
    let mut account = account();
    let mut raw = record().into_raw();
    raw.authorization_epoch = 1;
    let mut corrupt = SessionRecord::try_from(raw).unwrap();
    assert_eq!(
        corrupt.observe(&account, time(2_000)),
        Err(SessionError::Unavailable)
    );
    assert_eq!(corrupt.revoked_at().unwrap().get(), 2_000);
    assert_eq!(corrupt.last_observed_at().get(), 2_000);
    assert_eq!(corrupt.last_activity_at().get(), 1_000);
    // Persist/reload the retirement, then let the legitimate account catch up.
    let mut loaded = SessionRecord::try_from(corrupt.into_raw()).unwrap();
    account.invalidate(epoch(0), time(3_000)).unwrap();
    assert_eq!(account.authorization_epoch().get(), 1);
    assert_eq!(
        loaded.observe(&account, time(3_000)),
        Err(SessionError::Unauthenticated)
    );
    assert_eq!(loaded.revoked_at().unwrap().get(), 2_000);
    // Legitimate historical session epochs still have ordinary auth denial.
    assert_eq!(
        record().observe(&account, time(3_000)),
        Err(SessionError::Unauthenticated)
    );
}
