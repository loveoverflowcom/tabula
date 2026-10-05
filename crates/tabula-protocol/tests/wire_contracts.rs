//! Hostile and ordinary-serde trust-boundary partitions (doc 05 §9.1).
use serde::Serialize;
use tabula_core::{GameId, GameVersion, MatchId};
use tabula_protocol::{
    decode_client, decode_server, encode_client, encode_server, ClientEnvelope, Codec, ErrorCode,
    GameCommandFrame, ProtocolVersion, ServerEnvelope, ServerMessage, WireError, MAX_EVENTS,
    MAX_EVENT_BYTES, MAX_GAME_ID_BYTES, MAX_GAME_PAYLOAD_BYTES, MAX_GAME_VERSION_BYTES,
    MAX_INBOUND_FRAME_BYTES, MAX_OUTBOUND_FRAME_BYTES, MAX_VIEW_BYTES, PROTOCOL_VERSION,
};

fn command(payload: Vec<u8>) -> GameCommandFrame {
    GameCommandFrame::new(
        MatchId(42),
        GameId::new("com.example.test").unwrap(),
        GameVersion::new("1.2.3").unwrap(),
        payload,
    )
    .unwrap()
}

fn client() -> ClientEnvelope {
    ClientEnvelope::new(7, 9, command(vec![0, 1, 255])).unwrap()
}

#[derive(Serialize)]
struct RawCommand<'a> {
    match_id: MatchId,
    game: &'a str,
    game_version: &'a str,
    payload: Vec<u8>,
}

#[derive(Serialize)]
struct RawClient<'a> {
    v: ProtocolVersion,
    seq: u64,
    corr: u64,
    command: RawCommand<'a>,
}

fn raw_client() -> RawClient<'static> {
    RawClient {
        v: PROTOCOL_VERSION,
        seq: 7,
        corr: 9,
        command: RawCommand {
            match_id: MatchId(42),
            game: "com.example.test",
            game_version: "1.2.3",
            payload: vec![0, 1, 255],
        },
    }
}

fn serde_rejects(raw: &impl Serialize) {
    let json = serde_json::to_vec(raw).unwrap();
    assert!(serde_json::from_slice::<ClientEnvelope>(&json).is_err());
    let postcard = postcard::to_allocvec(raw).unwrap();
    assert!(postcard::from_bytes::<ClientEnvelope>(&postcard).is_err());
    for (codec, bytes) in [(Codec::Json, json), (Codec::Postcard, postcard)] {
        assert!(decode_client(codec, &bytes).is_err());
    }
}

#[test]
fn normal_serde_cannot_bypass_version_sequence_identity_or_payload_checks() {
    let mut raw = raw_client();
    raw.seq = 0;
    serde_rejects(&raw);
    for version in [
        ProtocolVersion { major: 1, minor: 1 },
        ProtocolVersion { major: 0, minor: 2 },
    ] {
        let mut raw = raw_client();
        raw.v = version;
        serde_rejects(&raw);
    }
    for game in ["Com.Example.Test", "one", "a..b"] {
        let mut raw = raw_client();
        raw.command.game = game;
        serde_rejects(&raw);
    }
    for version in ["1.2", "01.2.3"] {
        let mut raw = raw_client();
        raw.command.game_version = version;
        serde_rejects(&raw);
    }
    let oversized_id = format!("a.{}", "b".repeat(MAX_GAME_ID_BYTES - 1));
    let mut raw = raw_client();
    raw.command.game = &oversized_id;
    serde_rejects(&raw);
    let oversized_version = format!("1.2.3+{}", "x".repeat(MAX_GAME_VERSION_BYTES - 5));
    raw.command.game = "com.example.test";
    raw.command.game_version = &oversized_version;
    serde_rejects(&raw);
    raw.command.game_version = "1.2.3";
    raw.command.payload = vec![0; MAX_GAME_PAYLOAD_BYTES + 1];
    serde_rejects(&raw);
}

#[test]
fn constructor_limits_include_exact_boundaries() {
    assert_eq!(
        ClientEnvelope::new(0, 9, command(vec![])),
        Err(WireError::ZeroCounter)
    );
    let id = GameId::new(format!("a.{}", "b".repeat(MAX_GAME_ID_BYTES - 2))).unwrap();
    let version =
        GameVersion::new(format!("1.2.3+{}", "x".repeat(MAX_GAME_VERSION_BYTES - 6))).unwrap();
    let maximal = GameCommandFrame::new(
        MatchId(1),
        id.clone(),
        version.clone(),
        vec![0; MAX_GAME_PAYLOAD_BYTES],
    )
    .unwrap();
    let envelope = ClientEnvelope::new(u64::MAX, 0, maximal).unwrap();
    for codec in [Codec::Postcard, Codec::Json] {
        let encoded = encode_client(codec, &envelope).unwrap();
        assert_eq!(decode_client(codec, &encoded).unwrap(), envelope);
    }
    assert_eq!(
        GameCommandFrame::new(
            MatchId(1),
            id.clone(),
            version.clone(),
            vec![0; MAX_GAME_PAYLOAD_BYTES + 1]
        ),
        Err(WireError::LimitExceeded)
    );
    let id_over = GameId::new(format!("a.{}", "b".repeat(MAX_GAME_ID_BYTES - 1))).unwrap();
    assert_eq!(
        GameCommandFrame::new(MatchId(1), id_over, version.clone(), vec![]),
        Err(WireError::LimitExceeded)
    );
    let version_over =
        GameVersion::new(format!("1.2.3+{}", "x".repeat(MAX_GAME_VERSION_BYTES - 5))).unwrap();
    assert_eq!(
        GameCommandFrame::new(MatchId(1), id, version_over, vec![]),
        Err(WireError::LimitExceeded)
    );
}

#[test]
fn both_codecs_reject_truncated_corrupt_and_extra_data() {
    for codec in [Codec::Postcard, Codec::Json] {
        let encoded = encode_client(codec, &client()).unwrap();
        assert!(decode_client(codec, &encoded[..encoded.len() - 1]).is_err());
        assert!(decode_client(codec, &[]).is_err());
        assert!(decode_client(codec, &[255; 32]).is_err());
    }
    let mut postcard = encode_client(Codec::Postcard, &client()).unwrap();
    postcard.push(0);
    assert_eq!(
        decode_client(Codec::Postcard, &postcard),
        Err(WireError::TrailingBytes)
    );
    let envelope = ServerEnvelope::new(None, 1, ServerMessage::Ack { seq: 7 }).unwrap();
    let mut postcard = encode_server(Codec::Postcard, &envelope).unwrap();
    postcard.push(0);
    assert_eq!(
        decode_server(Codec::Postcard, &postcard),
        Err(WireError::TrailingBytes)
    );
    let mut json = encode_client(Codec::Json, &client()).unwrap();
    json.extend_from_slice(b" {}");
    assert!(decode_client(Codec::Json, &json).is_err());
}

#[test]
fn json_accepts_unknown_fields_at_all_struct_boundaries() {
    let json = br#"{"v":{"major":0,"minor":1,"future":true},"seq":7,"corr":9,"future":[1,2],"command":{"match_id":42,"game":"com.example.test","game_version":"1.2.3","payload":[0,1,255],"future":{}}}"#;
    assert_eq!(decode_client(Codec::Json, json).unwrap(), client());
    let json = br#"{"v":{"major":0,"minor":1},"corr":null,"frame":1,"future":true,"body":{"MatchUpdate":{"revision":0,"view":[],"events":[],"future":true}}}"#;
    assert!(decode_server(Codec::Json, json).is_ok());
}

#[test]
fn actual_frame_limits_include_json_whitespace_and_unknown_fields() {
    let mut json = encode_client(Codec::Json, &client()).unwrap();
    json.resize(MAX_INBOUND_FRAME_BYTES, b' ');
    assert_eq!(decode_client(Codec::Json, &json).unwrap(), client());
    json.push(b' ');
    assert_eq!(
        decode_client(Codec::Json, &json),
        Err(WireError::LimitExceeded)
    );
    for codec in [Codec::Postcard, Codec::Json] {
        assert_eq!(
            decode_client(codec, &vec![0; MAX_INBOUND_FRAME_BYTES + 1]),
            Err(WireError::LimitExceeded)
        );
        assert_eq!(
            decode_server(codec, &vec![0; MAX_OUTBOUND_FRAME_BYTES + 1]),
            Err(WireError::LimitExceeded)
        );
    }
    let opaque = ClientEnvelope::new(1, 0, command(vec![255; MAX_GAME_PAYLOAD_BYTES])).unwrap();
    assert!(encode_client(Codec::Postcard, &opaque).is_ok());
    assert_eq!(
        encode_client(Codec::Json, &opaque),
        Err(WireError::LimitExceeded)
    );
    let output = ServerEnvelope::new(
        None,
        1,
        ServerMessage::MatchUpdate {
            revision: 0,
            view: vec![255; MAX_VIEW_BYTES],
            events: vec![],
        },
    )
    .unwrap();
    assert!(encode_server(Codec::Postcard, &output).is_ok());
    assert_eq!(
        encode_server(Codec::Json, &output),
        Err(WireError::LimitExceeded)
    );
}

fn server_serde_rejects(body: ServerMessage) {
    // The enum permits manually built variants, but both ordinary serde and
    // the private enclosing envelope enforce its invariant at the boundary.
    let json = serde_json::to_vec(&body).unwrap();
    assert!(serde_json::from_slice::<ServerMessage>(&json).is_err());
    let postcard = postcard::to_allocvec(&body).unwrap();
    assert!(postcard::from_bytes::<ServerMessage>(&postcard).is_err());
    assert!(ServerEnvelope::new(None, 1, body).is_err());
}

#[test]
fn regular_server_serde_cannot_bypass_view_event_count_or_aggregate_limits() {
    server_serde_rejects(ServerMessage::Ack { seq: 0 });
    server_serde_rejects(ServerMessage::Reject {
        seq: 0,
        error: ErrorCode::Malformed,
    });
    server_serde_rejects(ServerMessage::MatchUpdate {
        revision: 0,
        view: vec![0; MAX_VIEW_BYTES + 1],
        events: vec![],
    });
    server_serde_rejects(ServerMessage::MatchUpdate {
        revision: 0,
        view: vec![],
        events: vec![vec![0; MAX_EVENT_BYTES + 1]],
    });
    server_serde_rejects(ServerMessage::MatchUpdate {
        revision: 0,
        view: vec![],
        events: vec![vec![]; MAX_EVENTS + 1],
    });
    // Field bounds each pass; the aggregate plus metadata exceeds one frame.
    server_serde_rejects(ServerMessage::MatchUpdate {
        revision: 0,
        view: vec![0; MAX_VIEW_BYTES],
        events: vec![vec![0; MAX_EVENT_BYTES]; 32],
    });
    assert_eq!(
        ServerEnvelope::new(None, 0, ServerMessage::Ack { seq: 1 }),
        Err(WireError::ZeroCounter)
    );
    for invalid_json in [
        r#"{"v":{"major":0,"minor":2},"corr":null,"frame":1,"body":{"Ack":{"seq":1}}}"#,
        r#"{"v":{"major":0,"minor":1},"corr":null,"frame":0,"body":{"Ack":{"seq":1}}}"#,
    ] {
        assert!(serde_json::from_str::<ServerEnvelope>(invalid_json).is_err());
    }
}

#[test]
fn server_exact_field_limits_are_accepted_in_both_codecs() {
    for body in [
        ServerMessage::Ack { seq: u64::MAX },
        ServerMessage::Reject {
            seq: u64::MAX,
            error: ErrorCode::Terminal,
        },
        ServerMessage::MatchUpdate {
            revision: 0,
            view: vec![0; MAX_VIEW_BYTES - 128],
            events: vec![],
        },
        ServerMessage::MatchUpdate {
            revision: u64::MAX,
            view: vec![],
            events: vec![vec![0; MAX_EVENT_BYTES]],
        },
        ServerMessage::MatchUpdate {
            revision: 0,
            view: vec![],
            events: vec![vec![]; MAX_EVENTS],
        },
    ] {
        let envelope = ServerEnvelope::new(Some(0), u64::MAX, body).unwrap();
        for codec in [Codec::Postcard, Codec::Json] {
            let bytes = encode_server(codec, &envelope).unwrap();
            assert_eq!(decode_server(codec, &bytes).unwrap(), envelope);
        }
    }
    // Exact view cap is legal, but its JSON representation needs more than one
    // MiB after mandatory envelope overhead; the selected codec rejects it.
    assert!(ServerEnvelope::new(
        None,
        1,
        ServerMessage::MatchUpdate {
            revision: 0,
            view: vec![0; MAX_VIEW_BYTES],
            events: vec![]
        }
    )
    .is_ok());
}

#[test]
fn outputs_expose_only_observable_counters_and_fixed_error_codes() {
    for body in [
        ServerMessage::Ack { seq: 1 },
        ServerMessage::Reject {
            seq: 1,
            error: ErrorCode::RuleRejected,
        },
        ServerMessage::MatchUpdate {
            revision: 1,
            view: vec![4],
            events: vec![vec![5]],
        },
    ] {
        let encoded = String::from_utf8(
            encode_server(Codec::Json, &ServerEnvelope::new(None, 1, body).unwrap()).unwrap(),
        )
        .unwrap();
        for forbidden in [
            "state_version",
            "input_index",
            "seed",
            "authority",
            "user_id",
            "seat_id",
            "diagnostic",
            "reason",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "unexpected private metadata: {encoded}"
            );
        }
    }
}

#[test]
fn huge_postcard_length_hints_are_rejected_without_trying_to_read_elements() {
    // 0.1, seq=1, corr=0, match=1, game=a.b, version=1.2.3, then
    // a malicious usize::MAX vector length and no elements.
    let raw = RawClient {
        v: PROTOCOL_VERSION,
        seq: 1,
        corr: 0,
        command: RawCommand {
            match_id: MatchId(1),
            game: "a.b",
            game_version: "1.2.3",
            payload: vec![],
        },
    };
    let mut encoded = postcard::to_allocvec(&raw).unwrap();
    encoded.pop();
    encoded.extend(postcard::to_allocvec(&usize::MAX).unwrap());
    assert!(postcard::from_bytes::<ClientEnvelope>(&encoded).is_err());
    assert!(decode_client(Codec::Postcard, &encoded).is_err());
}

#[test]
fn complete_postcard_frame_accepts_exact_cap_and_rejects_one_extra_byte_of_data() {
    #[derive(Serialize)]
    struct RawServer<'a> {
        v: ProtocolVersion,
        corr: Option<u64>,
        frame: u64,
        body: &'a ServerMessage,
    }
    let mut body = ServerMessage::MatchUpdate {
        revision: 0,
        view: vec![0; MAX_VIEW_BYTES],
        events: vec![vec![0; MAX_EVENT_BYTES]; 32],
    };
    let serialize = |body: &ServerMessage| {
        postcard::to_allocvec(&RawServer {
            v: PROTOCOL_VERSION,
            corr: None,
            frame: 1,
            body,
        })
        .unwrap()
    };
    let excess = serialize(&body).len() - MAX_OUTBOUND_FRAME_BYTES;
    let ServerMessage::MatchUpdate { events, .. } = &mut body else {
        unreachable!()
    };
    // Crossing the 16 KiB varint threshold saves one length-prefix byte.
    events
        .last_mut()
        .unwrap()
        .truncate(MAX_EVENT_BYTES - excess + 1);
    let encoded = serialize(&body);
    assert_eq!(encoded.len(), MAX_OUTBOUND_FRAME_BYTES);
    let envelope = ServerEnvelope::new(None, 1, body.clone()).unwrap();
    assert_eq!(encode_server(Codec::Postcard, &envelope).unwrap(), encoded);
    assert_eq!(decode_server(Codec::Postcard, &encoded).unwrap(), envelope);
    assert_eq!(
        postcard::from_bytes::<ServerEnvelope>(&encoded).unwrap(),
        envelope
    );
    let ServerMessage::MatchUpdate { events, .. } = &mut body else {
        unreachable!()
    };
    events.last_mut().unwrap().push(0);
    let encoded = serialize(&body);
    assert_eq!(encoded.len(), MAX_OUTBOUND_FRAME_BYTES + 1);
    assert_eq!(
        ServerEnvelope::new(None, 1, body),
        Err(WireError::LimitExceeded)
    );
    assert!(postcard::from_bytes::<ServerEnvelope>(&encoded).is_err());
    assert_eq!(
        decode_server(Codec::Postcard, &encoded),
        Err(WireError::LimitExceeded)
    );
}
