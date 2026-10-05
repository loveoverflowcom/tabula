//! Reviewed positional Postcard and stable JSON vectors (I-13; doc 05 §3.3).
use core::fmt::Debug;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tabula_core::{GameId, GameVersion, MatchId};
use tabula_protocol::{
    decode_client, decode_server, encode_client, encode_server, ClientEnvelope, Codec, ErrorCode,
    GameCommandFrame, ServerEnvelope, ServerMessage, PROTOCOL_VERSION,
};

#[derive(Deserialize)]
struct Vector {
    name: String,
    postcard: String,
    json: String,
}

fn vectors() -> Vec<Vector> {
    serde_json::from_str(include_str!("vectors/0.1/wire.json")).unwrap()
}

fn find<'a>(vectors: &'a [Vector], name: &str) -> &'a Vector {
    vectors.iter().find(|vector| vector.name == name).unwrap()
}

fn bytes(vector: &Vector) -> Vec<u8> {
    assert_eq!(vector.postcard.len() % 2, 0);
    vector
        .postcard
        .as_bytes()
        .chunks_exact(2)
        .map(|hex| u8::from_str_radix(core::str::from_utf8(hex).unwrap(), 16).unwrap())
        .collect()
}

fn assert_vector<T: Serialize + DeserializeOwned + PartialEq + Debug>(vector: &Vector, value: &T) {
    let expected = bytes(vector);
    assert_eq!(
        postcard::to_allocvec(value).unwrap(),
        expected,
        "{} Postcard changed",
        vector.name
    );
    let (decoded, trailing) = postcard::take_from_bytes::<T>(&expected).unwrap();
    assert_eq!(&decoded, value);
    assert!(trailing.is_empty());
    assert_eq!(postcard::to_allocvec(&decoded).unwrap(), expected);
    assert_eq!(
        serde_json::to_string(value).unwrap(),
        vector.json,
        "{} JSON changed",
        vector.name
    );
    let decoded: T = serde_json::from_str(&vector.json).unwrap();
    assert_eq!(&decoded, value);
    assert_eq!(serde_json::to_string(&decoded).unwrap(), vector.json);
}

#[test]
fn every_wire_type_and_enum_variant_has_an_exact_vector() {
    let vectors = vectors();
    assert_eq!(
        vectors.len(),
        22,
        "vector coverage changed: review and bump protocol"
    );
    assert_vector(find(&vectors, "version"), &PROTOCOL_VERSION);
    assert_vector(find(&vectors, "codec-postcard"), &Codec::Postcard);
    assert_vector(find(&vectors, "codec-json"), &Codec::Json);
    for (name, error) in [
        ("error-malformed", ErrorCode::Malformed),
        ("error-unauthorized", ErrorCode::Unauthorized),
        ("error-wrong-match", ErrorCode::WrongMatch),
        ("error-wrong-game", ErrorCode::WrongGame),
        ("error-busy", ErrorCode::Busy),
        ("error-stale-seq", ErrorCode::StaleSeq),
        ("error-seq-too-far", ErrorCode::SeqTooFar),
        ("error-operation-conflict", ErrorCode::OperationConflict),
        ("error-rule-rejected", ErrorCode::RuleRejected),
        ("error-unavailable", ErrorCode::Unavailable),
        ("error-terminal", ErrorCode::Terminal),
    ] {
        assert_vector(find(&vectors, name), &error);
    }
    let command = GameCommandFrame::new(
        MatchId(42),
        GameId::new("com.example.test").unwrap(),
        GameVersion::new("1.2.3").unwrap(),
        vec![0, 1, 255],
    )
    .unwrap();
    assert_vector(find(&vectors, "command"), &command);
    let client = ClientEnvelope::new(7, 9, command).unwrap();
    assert_vector(find(&vectors, "client"), &client);
    for (name, corr, frame, body) in [
        ("ack", Some(9), 1, ServerMessage::Ack { seq: 7 }),
        (
            "reject",
            None,
            2,
            ServerMessage::Reject {
                seq: 7,
                error: ErrorCode::RuleRejected,
            },
        ),
        (
            "update",
            None,
            3,
            ServerMessage::MatchUpdate {
                revision: 1,
                view: vec![16, 17],
                events: vec![vec![32], vec![33, 34]],
            },
        ),
    ] {
        assert_vector(find(&vectors, &format!("message-{name}")), &body);
        let server = ServerEnvelope::new(corr, frame, body).unwrap();
        let vector = find(&vectors, &format!("server-{name}"));
        assert_vector(vector, &server);
        for (codec, encoded) in [
            (Codec::Postcard, bytes(vector)),
            (Codec::Json, vector.json.as_bytes().to_vec()),
        ] {
            assert_eq!(encode_server(codec, &server).unwrap(), encoded);
            assert_eq!(decode_server(codec, &encoded).unwrap(), server);
        }
    }
    let vector = find(&vectors, "client");
    for (codec, encoded) in [
        (Codec::Postcard, bytes(vector)),
        (Codec::Json, vector.json.as_bytes().to_vec()),
    ] {
        assert_eq!(encode_client(codec, &client).unwrap(), encoded);
        assert_eq!(decode_client(codec, &encoded).unwrap(), client);
    }
}
