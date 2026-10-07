//! Synthetic ES256 vectors for verifier rejection partitions. The signing key
//! exists only in test-process memory, has no provider/account access and is
//! never persisted or substituted for actual Kanidm CI proof.
use super::*;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};

struct SigningFixture {
    der: Vec<u8>,
    x: String,
    y: String,
}
fn fixture() -> &'static SigningFixture {
    use ring::signature::KeyPair as _;
    static FIXTURE: std::sync::OnceLock<SigningFixture> = std::sync::OnceLock::new();
    FIXTURE.get_or_init(|| {
        let random = ring::rand::SystemRandom::new();
        let der = ring::signature::EcdsaKeyPair::generate_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            &random,
        )
        .unwrap();
        let pair = ring::signature::EcdsaKeyPair::from_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            der.as_ref(),
            &random,
        )
        .unwrap();
        let public = pair.public_key().as_ref();
        assert_eq!(public.len(), 65);
        assert_eq!(public[0], 4);
        SigningFixture {
            der: der.as_ref().to_vec(),
            x: URL_SAFE_NO_PAD.encode(&public[1..33]),
            y: URL_SAFE_NO_PAD.encode(&public[33..65]),
        }
    })
}
fn config() -> KanidmConfig {
    KanidmConfig::new(
        "https://idm.example",
        "https://app.example",
        "tabula",
        "synthetic-only-secret".into(),
        vec!["invited-subject".into()],
    )
    .unwrap()
}
fn flow(now: u64) -> Pending {
    let c = config();
    Pending {
        binding: "non-authorizing-test-binding".into(),
        nonce: "nonce-bound-to-this-attempt".into(),
        verifier: "never-a-real-provider-verifier".into(),
        started_seconds: now,
        expires: Instant::now() + FLOW_LIFETIME,
        epochs: [(c.admitted[0].clone(), AccountEpoch::new(3).unwrap())].into(),
        enrollment_epoch: None,
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}
fn claims(now: u64) -> Value {
    json!({"iss":config().issuer(),"sub":"invited-subject","aud":"tabula","exp":now+300,"iat":now,"auth_time":now,"nonce":"nonce-bound-to-this-attempt"})
}
fn key() -> EncodingKey {
    EncodingKey::from_ec_der(&fixture().der)
}
fn tokens(claims: &Value, header: &Header) -> TokenResponse {
    TokenResponse {
        id_token: encode(header, claims, &key()).unwrap(),
        token_type: "Bearer".into(),
        access_token: "synthetic-access-token".into(),
    }
}
fn header() -> Header {
    let mut h = Header::new(Algorithm::ES256);
    h.kid = Some("synthetic-key".into());
    h
}
fn jwks() -> JwkSet {
    serde_json::from_value(json!({"keys":[{"kty":"EC","crv":"P-256","x":fixture().x,"y":fixture().y,"kid":"synthetic-key","use":"sig","alg":"ES256","key_ops":["verify"]}]})).unwrap()
}
fn check(value: &Value, now: u64) -> Result<ProviderIdentityKey, SessionError> {
    verify_id_token(
        &config(),
        &flow(now),
        &tokens(value, &header()),
        &jwks(),
        now,
    )
}
#[test]
fn actual_es256_signature_and_exact_identity_pass() {
    let now = clock_seconds().unwrap();
    assert_eq!(check(&claims(now), now).unwrap(), config().admitted[0]);
    let mut array = claims(now);
    array["aud"] = json!(["tabula"]);
    assert!(check(&array, now).is_ok());
}
#[test]
fn issuer_subject_audience_nonce_and_authentication_time_are_required() {
    let now = clock_seconds().unwrap();
    for field in ["iss", "sub", "aud", "exp", "iat", "auth_time", "nonce"] {
        let mut raw = claims(now);
        raw.as_object_mut().unwrap().remove(field);
        assert_eq!(
            check(&raw, now),
            Err(SessionError::Unauthenticated),
            "missing {field}"
        );
    }
    for (field, bad) in [
        ("iss", json!("https://idm.example/oauth2/openid/other")),
        ("aud", json!("other")),
        ("aud", json!(["tabula", "other"])),
        ("nonce", json!("another-attempt")),
        ("exp", json!(now)),
        ("iat", json!(now + 6)),
        ("auth_time", json!(now - 6)),
        ("auth_time", json!(now + 6)),
        ("nbf", json!(now + 1)),
        ("azp", json!("other")),
    ] {
        let mut raw = claims(now);
        raw[field] = bad;
        assert_eq!(
            check(&raw, now),
            Err(SessionError::Unauthenticated),
            "bad {field}"
        );
    }
    let mut raw = claims(now);
    raw["sub"] = json!("");
    assert!(check(&raw, now).is_err());
}
#[test]
fn cryptographic_tampering_and_untrusted_keys_fail_closed() {
    let now = clock_seconds().unwrap();
    let mut token = tokens(&claims(now), &header());
    let mut parts = token
        .id_token
        .split('.')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut signature = URL_SAFE_NO_PAD.decode(&parts[2]).unwrap();
    signature[0] ^= 1;
    parts[2] = URL_SAFE_NO_PAD.encode(signature);
    token.id_token = parts.join(".");
    assert_eq!(
        verify_id_token(&config(), &flow(now), &token, &jwks(), now),
        Err(SessionError::Unauthenticated)
    );
    // A different valid P-256 point: SEC2 secp256r1 generator (public, no access).
    let wrong: JwkSet=serde_json::from_value(json!({"keys":[{"kty":"EC","crv":"P-256","x":"axfR8uEsQkf4vOblY6RA8ncDfYEt6zOg9KE5RdiYwpY","y":"T-NC4v4af5uO5-tKfA-eFivOM1drMV7Oy7ZAaDe_UfU","kid":"synthetic-key","use":"sig","alg":"ES256"}]})).unwrap();
    assert_eq!(
        verify_id_token(
            &config(),
            &flow(now),
            &tokens(&claims(now), &header()),
            &wrong,
            now
        ),
        Err(SessionError::Unauthenticated)
    );
    for raw in [
        json!({"keys":[]}),
        json!({"keys":[{"kty":"EC","crv":"P-256","x":fixture().x,"y":fixture().y,"kid":"other"}]}),
        json!({"keys":[{"kty":"EC","crv":"P-384","x":fixture().x,"y":fixture().y,"kid":"synthetic-key"}]}),
        json!({"keys":[{"kty":"EC","crv":"P-256","x":fixture().x,"y":fixture().y,"kid":"synthetic-key","use":"enc"}]}),
        json!({"keys":[{"kty":"EC","crv":"P-256","x":fixture().x,"y":fixture().y,"kid":"synthetic-key","alg":"HS256"}]}),
        json!({"keys":[{"kty":"EC","crv":"P-256","x":fixture().x,"y":fixture().y,"kid":"synthetic-key","key_ops":["sign"]}]}),
    ] {
        let keys: JwkSet = serde_json::from_value(raw).unwrap();
        assert!(verify_id_token(
            &config(),
            &flow(now),
            &tokens(&claims(now), &header()),
            &keys,
            now
        )
        .is_err());
    }
    let mut dup = jwks();
    dup.keys.push(
        serde_json::from_value(
            json!({"kty":"EC","crv":"P-256","x":fixture().x,"y":fixture().y,"kid":"synthetic-key"}),
        )
        .unwrap(),
    );
    assert_eq!(
        verify_id_token(
            &config(),
            &flow(now),
            &tokens(&claims(now), &header()),
            &dup,
            now
        ),
        Err(SessionError::Unauthenticated)
    );
}
#[test]
fn hostile_header_key_urls_and_duplicate_claims_cannot_bypass_signature_validation() {
    let now = clock_seconds().unwrap();
    for field in ["jku", "jwk", "x5u", "x5c", "crit", "b64"] {
        let mut raw = json!({"alg":"ES256","kid":"synthetic-key","typ":"JWT"});
        raw[field] = json!("attacker");
        let h = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&raw).unwrap());
        let p = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims(now)).unwrap());
        let message = format!("{h}.{p}");
        let sig = jsonwebtoken::crypto::sign(message.as_bytes(), &key(), Algorithm::ES256).unwrap();
        let token = TokenResponse {
            id_token: format!("{message}.{sig}"),
            token_type: "Bearer".into(),
            access_token: "test".into(),
        };
        assert_eq!(
            verify_id_token(&config(), &flow(now), &token, &jwks(), now),
            Err(SessionError::Unauthenticated),
            "header {field}"
        );
    }
    let h = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header()).unwrap());
    let raw=format!("{{\"iss\":\"{}\",\"sub\":\"invited-subject\",\"aud\":\"tabula\",\"exp\":{},\"iat\":{now},\"auth_time\":{now},\"nonce\":\"nonce-bound-to-this-attempt\",\"nonce\":\"attacker\"}}",config().issuer(),now+300);
    let message = format!("{h}.{}", URL_SAFE_NO_PAD.encode(raw));
    let sig = jsonwebtoken::crypto::sign(message.as_bytes(), &key(), Algorithm::ES256).unwrap();
    let token = TokenResponse {
        id_token: format!("{message}.{sig}"),
        token_type: "Bearer".into(),
        access_token: "test".into(),
    };
    assert_eq!(
        verify_id_token(&config(), &flow(now), &token, &jwks(), now),
        Err(SessionError::Unauthenticated)
    );
}
#[test]
fn algorithm_confusion_typ_and_noncanonical_encodings_reject() {
    let now = clock_seconds().unwrap();
    let mut h = header();
    h.typ = Some("at+jwt".into());
    assert!(verify_id_token(
        &config(),
        &flow(now),
        &tokens(&claims(now), &h),
        &jwks(),
        now
    )
    .is_err());
    let mut h = header();
    h.kid = None;
    assert!(verify_id_token(
        &config(),
        &flow(now),
        &tokens(&claims(now), &h),
        &jwks(),
        now
    )
    .is_err());
    for raw in ["a.b", "a.b.c.d", "e30=.e30=.AA", "..", "none"] {
        let token = TokenResponse {
            id_token: raw.into(),
            token_type: "Bearer".into(),
            access_token: "test".into(),
        };
        assert!(verify_id_token(&config(), &flow(now), &token, &jwks(), now).is_err());
    }
    let mut hs = Header::new(Algorithm::HS256);
    hs.kid = Some("synthetic-key".into());
    let token = TokenResponse {
        id_token: encode(
            &hs,
            &claims(now),
            &EncodingKey::from_secret(fixture().x.as_bytes()),
        )
        .unwrap(),
        token_type: "Bearer".into(),
        access_token: "test".into(),
    };
    assert_eq!(
        verify_id_token(&config(), &flow(now), &token, &jwks(), now),
        Err(SessionError::Unauthenticated)
    );
    let h = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","kid":"synthetic-key"}"#);
    let p = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims(now)).unwrap());
    let token = TokenResponse {
        id_token: format!("{h}.{p}."),
        token_type: "Bearer".into(),
        access_token: "test".into(),
    };
    assert_eq!(
        verify_id_token(&config(), &flow(now), &token, &jwks(), now),
        Err(SessionError::Unauthenticated)
    );
}
#[test]
fn access_token_hash_matches_only_the_same_code_exchange() {
    let now = clock_seconds().unwrap();
    let mut raw = claims(now);
    raw["at_hash"] =
        json!(URL_SAFE_NO_PAD.encode(&Sha256::digest(b"synthetic-access-token")[..16]));
    assert!(check(&raw, now).is_ok());
    raw["at_hash"] = json!("other");
    assert!(check(&raw, now).is_err());
}
#[test]
fn discovered_endpoints_and_supported_security_contract_are_exact() {
    let c = config();
    let good = json!({"issuer":c.issuer(),"authorization_endpoint":"https://idm.example/ui/oauth2","token_endpoint":"https://idm.example/oauth2/token","jwks_uri":"https://idm.example/oauth2/openid/tabula/public_key.jwk","response_types_supported":["code"],"response_modes_supported":["query"],"id_token_signing_alg_values_supported":["ES256"],"token_endpoint_auth_methods_supported":["client_secret_basic"],"code_challenge_methods_supported":["S256"]});
    let m: Metadata = serde_json::from_value(good.clone()).unwrap();
    assert!(m.validate(&c).is_ok());
    for (name, bad) in [
        ("issuer", json!("https://other")),
        ("authorization_endpoint", json!("https://other/ui/oauth2")),
        ("token_endpoint", json!("https://idm.example/redirect")),
        ("jwks_uri", json!("http://idm.example/key")),
        ("id_token_signing_alg_values_supported", json!(["RS256"])),
        ("code_challenge_methods_supported", json!(["plain"])),
        ("response_modes_supported", json!(["fragment"])),
        ("response_types_supported", json!(["token"])),
    ] {
        let mut raw = good.clone();
        raw[name] = bad;
        let m: Metadata = serde_json::from_value(raw).unwrap();
        assert_eq!(
            m.validate(&c),
            Err(SessionError::Unavailable),
            "metadata {name}"
        );
    }
}
#[test]
fn pkce_s256_matches_rfc7636_appendix_b_independent_vector() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert_eq!(
        URL_SAFE_NO_PAD.encode(Sha256::digest(verifier)),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}
#[test]
fn nonce_state_verifier_use_independent_csprng_and_canonical_encoding() {
    let values = (0..128)
        .map(|_| random_value().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(values.len(), 128);
    for value in values {
        assert_eq!(value.len(), 43);
        assert!(SessionCredential::parse(&value).is_ok());
    }
}
#[test]
fn epoch_capture_state_and_cancellation_have_bounded_lifetime() {
    let now = clock_seconds().unwrap();
    let c = config();
    let mut f = flow(now);
    f.expires = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    let flag = f.cancelled.clone();
    let mut state = PendingState::default();
    state.reservations.insert(
        "test".into(),
        Reservation {
            cancelled: flag,
            expires: f.expires,
        },
    );
    state.flows.insert("state".into(), f);
    prune(&mut state);
    assert!(state.flows.is_empty());
    assert!(state.reservations.is_empty());
    assert_eq!(c.admitted[0].subject(), "invited-subject");
}
