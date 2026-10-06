//! The default/WASM-facing surface is a versioned DTO contract only.

use tabula_session_http::{
    AccountCapabilities, ContextResponse, NativeRefreshResponse, PublicProblem,
    SelfProfileResponse, SessionDisposition, HTTP_CONTRACT_VERSION,
};

#[test]
fn context_is_explicit_and_contains_no_session_or_provider_fields() {
    let context = ContextResponse {
        version: HTTP_CONTRACT_VERSION,
        disposition: SessionDisposition::SignedOut,
        account_id: None,
        csrf_token: Some("document-memory-synchronizer-token".to_owned()),
        capabilities: AccountCapabilities {
            login: false,
            register: false,
            friends: false,
            read_self_profile: false,
        },
    };
    let value = serde_json::to_value(&context).unwrap();
    assert_eq!(value["version"], 1);
    assert_eq!(value["disposition"], "signed_out");
    assert!(value["account_id"].is_null());
    assert_eq!(value.as_object().unwrap().len(), 5);
    assert_eq!(value["capabilities"].as_object().unwrap().len(), 4);
    assert_eq!(
        serde_json::from_value::<ContextResponse>(value).unwrap(),
        context
    );
}

#[test]
fn profile_is_only_version_and_immutable_account_id() {
    let profile = SelfProfileResponse {
        version: HTTP_CONTRACT_VERSION,
        account_id: "00000000000000000000000000000001".to_owned(),
    };
    let value = serde_json::to_value(&profile).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 2);
    assert_eq!(
        serde_json::from_value::<SelfProfileResponse>(value).unwrap(),
        profile
    );
}

#[test]
fn typed_responses_reject_unknown_fields_and_native_debug_redacts_credentials() {
    for (target, mut value) in [
        (
            "context",
            serde_json::json!({"version":1,"disposition":"unavailable","account_id":null,"csrf_token":null,"capabilities":{"login":false,"register":false,"friends":false,"read_self_profile":false}}),
        ),
        (
            "profile",
            serde_json::json!({"version":1,"account_id":"fixture"}),
        ),
        (
            "problem",
            serde_json::json!({"version":1,"status":503,"title":"Unavailable","code":"unavailable"}),
        ),
        (
            "native",
            serde_json::json!({"version":1,"credential":"fixture-secret"}),
        ),
    ] {
        value["provider_subject"] = serde_json::json!("unrequested");
        let rejected = match target {
            "context" => serde_json::from_value::<ContextResponse>(value).is_err(),
            "profile" => serde_json::from_value::<SelfProfileResponse>(value).is_err(),
            "problem" => serde_json::from_value::<PublicProblem>(value).is_err(),
            "native" => serde_json::from_value::<NativeRefreshResponse>(value).is_err(),
            _ => unreachable!(),
        };
        assert!(rejected, "{target} accepted an unknown field");
    }
    let native = NativeRefreshResponse {
        version: HTTP_CONTRACT_VERSION,
        credential: "fixture-secret".to_owned(),
    };
    assert_eq!(format!("{native:?}"), "NativeRefreshResponse([REDACTED])");
}

#[test]
fn exact_v1_json_vector_is_stable_and_browser_shape_validation_fails_closed() {
    let context = ContextResponse {
        version: 1,
        disposition: SessionDisposition::Authenticated,
        account_id: Some("00000000000000000000000000000001".to_owned()),
        csrf_token: Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned()),
        capabilities: AccountCapabilities {
            login: false,
            register: false,
            friends: false,
            read_self_profile: true,
        },
    };
    assert!(context.validate_for_browser().is_ok());
    let vector = "{\"version\":1,\"disposition\":\"authenticated\",\"account_id\":\"00000000000000000000000000000001\",\"csrf_token\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\",\"capabilities\":{\"login\":false,\"register\":false,\"friends\":false,\"read_self_profile\":true}}";
    assert_eq!(serde_json::to_string(&context).unwrap(), vector);
    let base = serde_json::to_value(&context).unwrap();
    for (pointer, hostile) in [
        ("/version", serde_json::json!(2)),
        ("/disposition", serde_json::json!("signed_out")),
        ("/disposition", serde_json::json!("unavailable")),
        ("/account_id", serde_json::Value::Null),
        (
            "/account_id",
            serde_json::json!("00000000000000000000000000000000"),
        ),
        (
            "/account_id",
            serde_json::json!("0000000000000000000000000000000A"),
        ),
        (
            "/account_id",
            serde_json::json!("0000000000000000000000000000000g"),
        ),
        (
            "/account_id",
            serde_json::json!("0000000000000000000000000000001"),
        ),
        ("/csrf_token", serde_json::Value::Null),
        ("/csrf_token", serde_json::json!("AAAA")),
        (
            "/csrf_token",
            serde_json::json!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB"),
        ),
        (
            "/csrf_token",
            serde_json::json!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
        ),
        ("/capabilities/login", serde_json::json!(true)),
        ("/capabilities/register", serde_json::json!(true)),
        ("/capabilities/read_self_profile", serde_json::json!(false)),
    ] {
        let mut value = base.clone();
        *value.pointer_mut(pointer).unwrap() = hostile;
        let response: ContextResponse = serde_json::from_value(value).unwrap();
        assert!(
            response.validate_for_browser().is_err(),
            "accepted hostile {pointer}"
        );
    }
    assert!(serde_json::from_str::<ContextResponse>(
        "{\"version\":1,\"version\":1,\"disposition\":\"signed_out\",\"account_id\":null,\"csrf_token\":null,\"capabilities\":{\"login\":false,\"register\":false,\"friends\":false,\"read_self_profile\":false}}"
    ).is_err());
}

#[test]
fn isolated_account_capabilities_are_scoped_to_their_session_disposition() {
    let mut context = ContextResponse {
        version: 1,
        disposition: SessionDisposition::SignedOut,
        account_id: None,
        csrf_token: Some("A".repeat(43)),
        capabilities: AccountCapabilities {
            login: true,
            register: true,
            friends: false,
            read_self_profile: false,
        },
    };
    assert!(context.validate_for_browser().is_ok());
    context.capabilities.friends = true;
    assert!(context.validate_for_browser().is_err());
    context.capabilities.friends = false;
    context.csrf_token = None;
    assert!(context.validate_for_browser().is_err());
    context.csrf_token = Some("A".repeat(43));
    context.disposition = SessionDisposition::Authenticated;
    context.account_id = Some("00000000000000000000000000000001".into());
    context.capabilities.login = false;
    context.capabilities.register = false;
    context.capabilities.read_self_profile = true;
    context.capabilities.friends = true;
    assert!(context.validate_for_browser().is_ok());
    context.capabilities.register = true;
    assert!(context.validate_for_browser().is_err());
    context.capabilities.register = false;
    context.disposition = SessionDisposition::Unavailable;
    context.account_id = None;
    context.csrf_token = None;
    context.capabilities.read_self_profile = false;
    assert!(context.validate_for_browser().is_err());
}

#[test]
fn signed_out_unavailable_and_profile_validation_keep_authority_and_shape_distinct() {
    let mut context = ContextResponse {
        version: 1,
        disposition: SessionDisposition::SignedOut,
        account_id: None,
        csrf_token: None,
        capabilities: AccountCapabilities {
            login: false,
            register: false,
            friends: false,
            read_self_profile: false,
        },
    };
    assert!(context.validate_for_browser().is_ok());
    context.csrf_token = Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned());
    assert!(context.validate_for_browser().is_ok());
    context.disposition = SessionDisposition::Unavailable;
    assert!(context.validate_for_browser().is_err());
    context.csrf_token = None;
    assert!(context.validate_for_browser().is_ok());
    context.account_id = Some("00000000000000000000000000000001".to_owned());
    assert!(context.validate_for_browser().is_err());

    let profile = SelfProfileResponse {
        version: 1,
        account_id: "00000000000000000000000000000001".to_owned(),
    };
    assert!(profile.validate().is_ok());
    assert_eq!(
        serde_json::to_string(&profile).unwrap(),
        "{\"version\":1,\"account_id\":\"00000000000000000000000000000001\"}"
    );
    for account_id in [
        "",
        "1",
        "00000000000000000000000000000000",
        "0000000000000000000000000000000A",
        "0000000000000000000000000000000g",
    ] {
        assert!(SelfProfileResponse {
            version: 1,
            account_id: account_id.to_owned()
        }
        .validate()
        .is_err());
    }
    assert!(SelfProfileResponse {
        version: 2,
        account_id: profile.account_id
    }
    .validate()
    .is_err());
}

#[test]
fn opt_in_login_context_requires_signed_out_preauth_csrf_and_closed_other_caps() {
    let mut context = ContextResponse {
        version: 1,
        disposition: SessionDisposition::SignedOut,
        account_id: None,
        csrf_token: Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned()),
        capabilities: AccountCapabilities {
            login: true,
            register: false,
            friends: false,
            read_self_profile: false,
        },
    };
    assert!(context.validate_for_browser().is_ok());
    context.csrf_token = None;
    assert!(context.validate_for_browser().is_err());
    context.csrf_token = Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned());
    context.disposition = SessionDisposition::Unavailable;
    assert!(context.validate_for_browser().is_err());
}

#[test]
fn login_start_url_is_bounded_https_without_authority_or_navigation_confusion() {
    use tabula_session_http::LoginStartResponse;
    for url in [
        "https://kanidm.invalid/oauth2/authorise?state=abc",
        "https://localhost:8443/",
        "https://[::1]:8443/",
        "https://192.0.2.1/",
    ] {
        assert!(
            LoginStartResponse {
                version: 1,
                authorization_url: url.to_owned()
            }
            .validate()
            .is_ok(),
            "{url}"
        );
    }
    for url in [
        "https://",
        "https://:443/",
        "https://[broken/",
        "https://idm.invalid:65536/",
        "https://idm.invalid:abc/",
        "http://idm.invalid/",
        "https://idm.invalid/#fragment",
        "https://user@idm.invalid/",
        "https://%40idm.invalid/",
        "https://idm.invalid\\evil.invalid/",
        "https://idm.invalid/\\evil",
        "https://idm.invalid/ space",
        "javascript:alert(1)",
    ] {
        assert!(
            LoginStartResponse {
                version: 1,
                authorization_url: url.to_owned()
            }
            .validate()
            .is_err(),
            "{url}"
        );
    }
    assert!(LoginStartResponse {
        version: 1,
        authorization_url: format!("https://idm.invalid/{}", "x".repeat(4096))
    }
    .validate()
    .is_err());
    assert!(serde_json::from_str::<LoginStartResponse>(
        "{\"version\":1,\"authorization_url\":\"https://idm.invalid/\",\"credential\":\"x\"}"
    )
    .is_err());
    assert!(serde_json::from_str::<LoginStartResponse>(
        "{\"version\":1,\"version\":1,\"authorization_url\":\"https://idm.invalid/\"}"
    )
    .is_err());
}
