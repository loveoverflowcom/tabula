//! Exact HTTP-v1 vectors and ordinary-serde hostile partitions (I-13).
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeMap, fmt::Debug};
use tabula_core::{GameId, GameVersion, MatchId};
use tabula_match_http::*;
use tabula_protocol::{ClientEnvelope, ErrorCode, GameCommandFrame, ServerEnvelope, ServerMessage};
#[allow(clippy::needless_pass_by_value)] // Reviewed vectors own complete carriers.
fn vector<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: T, expected: &str) {
    assert_eq!(serde_json::to_string(&value).unwrap(), expected);
    let decoded: T = serde_json::from_str(expected).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(serde_json::to_string(&decoded).unwrap(), expected);
    let mut json: serde_json::Value = serde_json::from_str(expected).unwrap();
    json["version"] = 2.into();
    assert!(serde_json::from_value::<T>(json).is_err());
    let mut json: serde_json::Value = serde_json::from_str(expected).unwrap();
    json["seat_claim"] = 0.into();
    assert!(serde_json::from_value::<T>(json).is_err());
}
fn command() -> ClientEnvelope {
    ClientEnvelope::new(
        1,
        7,
        GameCommandFrame::new(
            MatchId(42),
            GameId::new("com.example.test").unwrap(),
            GameVersion::new("1.2.3").unwrap(),
            vec![0, 1, 255],
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn every_http_carrier_has_reviewed_exact_json_vector() {
    let id = "0000000000000000000000000000002a";
    vector(
        MatchCreateRequest::new(
            "com.example.test".into(),
            2,
            BTreeMap::from([("clock".into(), "none".into())]),
        )
        .unwrap(),
        r#"{"version":1,"game_id":"com.example.test","seats":2,"config":{"clock":"none"}}"#,
    );
    vector(
        MatchJoinRequest::new("ABCD2345EFGH".into()).unwrap(),
        r#"{"version":1,"code":"ABCD2345EFGH"}"#,
    );
    vector(MatchGrantRequest::new().unwrap(), r#"{"version":1}"#);
    let grant = "A".repeat(64);
    vector(
        MatchAttachRequest::new(grant.clone()).unwrap(),
        r#"{"version":1,"binding_id":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#,
    );
    vector(
        MatchPollRequest::new(id.into()).unwrap(),
        r#"{"version":1,"attachment_id":"0000000000000000000000000000002a"}"#,
    );
    vector(
        MatchCommandRequest::new(id.into(), command()).unwrap(),
        r#"{"version":1,"attachment_id":"0000000000000000000000000000002a","command":{"v":{"major":0,"minor":1},"seq":1,"corr":7,"command":{"match_id":42,"game":"com.example.test","game_version":"1.2.3","payload":[0,1,255]}}}"#,
    );
    vector(
        MatchAdmission::new(
            id.into(),
            "com.example.test".into(),
            "1.2.3".into(),
            0,
            Some("ABCD2345EFGH".into()),
            false,
        )
        .unwrap(),
        r#"{"version":1,"match_id":"0000000000000000000000000000002a","game_id":"com.example.test","game_version":"1.2.3","seat":0,"join_code":"ABCD2345EFGH","ready":false}"#,
    );
    vector(
        MatchAdmission::new(
            id.into(),
            "com.example.test".into(),
            "1.2.3".into(),
            1,
            None,
            true,
        )
        .unwrap(),
        r#"{"version":1,"match_id":"0000000000000000000000000000002a","game_id":"com.example.test","game_version":"1.2.3","seat":1,"join_code":null,"ready":true}"#,
    );
    vector(
        MatchGrant::new(false, None, 0, "com.example.test".into(), "1.2.3".into()).unwrap(),
        r#"{"version":1,"ready":false,"binding_id":null,"seat":0,"game_id":"com.example.test","game_version":"1.2.3"}"#,
    );
    vector(
        MatchGrant::new(
            true,
            Some(grant),
            0,
            "com.example.test".into(),
            "1.2.3".into(),
        )
        .unwrap(),
        r#"{"version":1,"ready":true,"binding_id":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","seat":0,"game_id":"com.example.test","game_version":"1.2.3"}"#,
    );
    let update = ServerEnvelope::new(
        None,
        1,
        ServerMessage::MatchUpdate {
            revision: 0,
            view: vec![16, 17],
            events: vec![],
        },
    )
    .unwrap();
    vector(
        MatchAttachment::new(id.into(), 0, 1, vec![update]).unwrap(),
        r#"{"version":1,"attachment_id":"0000000000000000000000000000002a","seat":0,"next_seq":1,"frames":[{"v":{"major":0,"minor":1},"corr":null,"frame":1,"body":{"MatchUpdate":{"revision":0,"view":[16,17],"events":[]}}}]}"#,
    );
    let ack = ServerEnvelope::new(Some(7), 2, ServerMessage::Ack { seq: 1 }).unwrap();
    let reject = ServerEnvelope::new(
        Some(8),
        3,
        ServerMessage::Reject {
            seq: 2,
            error: ErrorCode::RuleRejected,
        },
    )
    .unwrap();
    vector(
        MatchFrames::new(vec![ack, reject]).unwrap(),
        r#"{"version":1,"frames":[{"v":{"major":0,"minor":1},"corr":7,"frame":2,"body":{"Ack":{"seq":1}}},{"v":{"major":0,"minor":1},"corr":8,"frame":3,"body":{"Reject":{"seq":2,"error":"RuleRejected"}}}]}"#,
    );
    vector(
        MatchFrames::new(vec![]).unwrap(),
        r#"{"version":1,"frames":[]}"#,
    );
}
#[test]
fn boundary_and_hostile_constructor_partitions() {
    let game = "x".repeat(128);
    assert!(MatchCreateRequest::new(game.clone(), 2, BTreeMap::new()).is_ok());
    assert!(MatchCreateRequest::new(format!("{game}x"), 2, BTreeMap::new()).is_err());
    for seats in [0, 1, 9, u8::MAX] {
        assert!(MatchCreateRequest::new(game.clone(), seats, BTreeMap::new()).is_err());
    }
    for seats in [2, 8] {
        assert!(MatchCreateRequest::new(game.clone(), seats, BTreeMap::new()).is_ok());
    }
    let mut config = (0..16)
        .map(|n| (format!("key{n}"), "v".repeat(128)))
        .collect::<BTreeMap<_, _>>();
    assert!(MatchCreateRequest::new(game.clone(), 2, config.clone()).is_ok());
    config.insert("seventeenth".into(), "v".into());
    assert!(MatchCreateRequest::new(game.clone(), 2, config).is_err());
    assert!(MatchJoinRequest::new("x".repeat(64)).is_ok());
    assert!(MatchJoinRequest::new("x".repeat(65)).is_err());
    for value in [
        "00000000000000000000000000000000",
        "0000000000000000000000000000002A",
        "42",
        "0000000000000000000000000000002g",
    ] {
        assert!(parse_match_id(value).is_err());
    }
    assert!(MatchAttachRequest::new("A".repeat(63)).is_err());
    assert!(MatchAttachRequest::new("A".repeat(2048)).is_ok());
    assert!(MatchAttachRequest::new("A".repeat(2049)).is_err());
    assert!(MatchGrant::new(true, None, 0, "com.example.test".into(), "1.2.3".into()).is_err());
    let ack = ServerEnvelope::new(None, 1, ServerMessage::Ack { seq: 1 }).unwrap();
    assert!(MatchFrames::new(vec![ack.clone(); 16]).is_ok());
    assert!(MatchFrames::new(vec![ack; 17]).is_err());
}
#[test]
fn grants_and_context_are_redacted_and_no_canonical_counters_appear() {
    let secret = "B".repeat(64);
    let request = MatchAttachRequest::new(secret.clone()).unwrap();
    assert!(!format!("{request:?}").contains(&secret));
    let response = MatchGrant::new(
        true,
        Some(secret.clone()),
        0,
        "com.example.test".into(),
        "1.2.3".into(),
    )
    .unwrap();
    assert!(!format!("{response:?}").contains(&secret));
    let wire = serde_json::to_string(
        &MatchFrames::new(vec![ServerEnvelope::new(
            None,
            1,
            ServerMessage::MatchUpdate {
                revision: 0,
                view: vec![1],
                events: vec![],
            },
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    for key in [
        "seed",
        "canonical",
        "state_version",
        "input_index",
        "state_hash",
        "logical_time",
        "ledger",
    ] {
        assert!(!wire.contains(key));
    }
}
#[test]
fn ordinary_serde_rejects_duplicate_config_and_incremental_oversize_fields() {
    assert!(serde_json::from_str::<MatchCreateRequest>(r#"{"version":1,"game_id":"com.example.test","seats":2,"config":{"clock":"none","clock":"other"}}"#).is_err());
    let config = (0..17)
        .map(|n| format!("\"key{n}\":\"v\""))
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        "{{\"version\":1,\"game_id\":\"com.example.test\",\"seats\":2,\"config\":{{{config}}}}}"
    );
    assert!(serde_json::from_str::<MatchCreateRequest>(&body).is_err());
    let body = format!("{{\"version\":1,\"binding_id\":\"{}\"}}", "A".repeat(2049));
    assert!(serde_json::from_str::<MatchAttachRequest>(&body).is_err());
    let ack = serde_json::to_string(
        &ServerEnvelope::new(None, 1, ServerMessage::Ack { seq: 1 }).unwrap(),
    )
    .unwrap();
    let body = format!("{{\"version\":1,\"frames\":[{}]}}", vec![ack; 17].join(","));
    assert!(serde_json::from_str::<MatchFrames>(&body).is_err());
}
