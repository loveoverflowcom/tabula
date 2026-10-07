//! Operator-supplied, exact Kanidm trust configuration (ADR-0038).
//! This constructor provisions no account, client, grant or credential.

use std::{collections::BTreeSet, fmt};
use tabula_session::{ProviderIdentityKey, SessionError};
use url::Url;

/// The sole configured provider/client and bounded previously invited identities.
/// No callback URL, issuer or endpoint is accepted from a request.
#[derive(Clone)]
pub struct KanidmConfig {
    pub(crate) provider_origin: String,
    pub(crate) browser_origin: String,
    pub(crate) issuer: String,
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
    pub(crate) callback_url: String,
    pub(crate) admitted: Vec<ProviderIdentityKey>,
    pub(crate) root_certificate: Option<Vec<u8>>,
    pub(crate) enrollment_enabled: bool,
}
impl fmt::Debug for KanidmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KanidmConfig")
            .field("provider_origin", &self.provider_origin)
            .field("browser_origin", &self.browser_origin)
            .field("invited_count", &self.admitted.len())
            .finish_non_exhaustive()
    }
}
impl KanidmConfig {
    /// Confidential app-specific client with S256 PKCE and exact invited subjects.
    /// The secret is supplied by its operator, never returned to browser code.
    pub fn new(
        provider_origin: &str,
        browser_origin: &str,
        client_id: &str,
        client_secret: String,
        admitted_subjects: Vec<String>,
    ) -> Result<Self, SessionError> {
        Self::configured(
            provider_origin,
            browser_origin,
            client_id,
            client_secret,
            admitted_subjects,
            false,
        )
    }

    /// Explicit Kanidm-backed Tabula enrollment mode. Provider credentials and
    /// provisioning remain Kanidm-owned; an empty static invitation list is valid.
    pub fn new_with_enrollment(
        provider_origin: &str,
        browser_origin: &str,
        client_id: &str,
        client_secret: String,
        admitted_subjects: Vec<String>,
    ) -> Result<Self, SessionError> {
        Self::configured(
            provider_origin,
            browser_origin,
            client_id,
            client_secret,
            admitted_subjects,
            true,
        )
    }
    fn configured(
        provider_origin: &str,
        browser_origin: &str,
        client_id: &str,
        client_secret: String,
        admitted_subjects: Vec<String>,
        enrollment_enabled: bool,
    ) -> Result<Self, SessionError> {
        canonical_https_origin(provider_origin)?;
        canonical_https_origin(browser_origin)?;
        if client_id.is_empty()
            || client_id.len() > 64
            || !client_id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
            || !(16..=4096).contains(&client_secret.len())
            || !client_secret.bytes().all(|b| b.is_ascii_graphic())
            || (!enrollment_enabled && admitted_subjects.is_empty())
            || admitted_subjects.len() > 32
        {
            return Err(SessionError::InvalidInput);
        }
        let issuer = format!("{provider_origin}/oauth2/openid/{client_id}");
        let mut unique = BTreeSet::new();
        let mut admitted = Vec::with_capacity(admitted_subjects.len());
        for subject in admitted_subjects {
            let identity = ProviderIdentityKey::new(&issuer, subject)?;
            if !unique.insert(identity.clone()) {
                return Err(SessionError::InvalidInput);
            }
            admitted.push(identity);
        }
        Ok(Self {
            provider_origin: provider_origin.to_owned(),
            browser_origin: browser_origin.to_owned(),
            issuer,
            client_id: client_id.to_owned(),
            client_secret,
            callback_url: format!("{browser_origin}/api/v1/auth/oidc/callback"),
            admitted,
            root_certificate: None,
            enrollment_enabled,
        })
    }

    /// Add an explicitly trusted private CA. Certificate/hostname verification
    /// remains enabled; there is no insecure TLS or redirect fallback.
    pub fn with_root_certificate(mut self, pem: Vec<u8>) -> Result<Self, SessionError> {
        if pem.is_empty() || pem.len() > 16_384 {
            return Err(SessionError::InvalidInput);
        }
        reqwest::Certificate::from_pem(&pem).map_err(|_| SessionError::InvalidInput)?;
        self.root_certificate = Some(pem);
        Ok(self)
    }
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn callback_url(&self) -> &str {
        &self.callback_url
    }
    pub fn browser_origin(&self) -> &str {
        &self.browser_origin
    }
    pub fn admitted_identities(&self) -> &[ProviderIdentityKey] {
        &self.admitted
    }
}

fn canonical_https_origin(input: &str) -> Result<(), SessionError> {
    let url = Url::parse(input).map_err(|_| SessionError::InvalidInput)?;
    if url.scheme() != "https"
        || !url.origin().is_tuple()
        || url.origin().ascii_serialization() != input
    {
        return Err(SessionError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(origin: &str) -> Result<KanidmConfig, SessionError> {
        KanidmConfig::new(
            origin,
            "https://app.example",
            "tabula",
            "test-only-client-secret".into(),
            vec!["invited-subject".into()],
        )
    }
    #[test]
    fn exact_provider_urls_and_redacted_secret() {
        let c = config("https://idm.example").unwrap();
        assert_eq!(c.issuer(), "https://idm.example/oauth2/openid/tabula");
        assert_eq!(
            c.callback_url(),
            "https://app.example/api/v1/auth/oidc/callback"
        );
        let debug = format!("{c:?}");
        assert!(!debug.contains("test-only-client-secret"));
        assert!(!debug.contains("invited-subject"));
    }
    #[test]
    fn rejects_origin_aliases_client_paths_and_duplicate_invites() {
        for bad in [
            "http://idm.example",
            "https://IDM.example",
            "https://idm.example:443",
            "https://idm.example/",
            "https://user@idm.example",
            "https://idm.example?callback=evil",
        ] {
            assert_eq!(config(bad).unwrap_err(), SessionError::InvalidInput);
        }
        for bad in ["../tabula", "Tabula", "", "tabula?evil", "tabula.client"] {
            assert!(KanidmConfig::new(
                "https://idm.example",
                "https://app.example",
                bad,
                "test-only-client-secret".into(),
                vec!["one".into()]
            )
            .is_err());
        }
        assert!(KanidmConfig::new(
            "https://idm.example",
            "https://app.example",
            "tabula",
            "test-only-client-secret".into(),
            vec!["one".into(), "one".into()]
        )
        .is_err());
    }
}

#[cfg(test)]
mod enrollment_tests {
    use super::*;
    #[test]
    fn enrollment_is_an_explicit_constructor_and_never_invents_provider_invites() {
        assert!(KanidmConfig::new(
            "https://idm.example",
            "https://app.example",
            "tabula",
            "synthetic-only-secret".into(),
            vec![]
        )
        .is_err());
        let enrollment = KanidmConfig::new_with_enrollment(
            "https://idm.example",
            "https://app.example",
            "tabula",
            "synthetic-only-secret".into(),
            vec![],
        )
        .unwrap();
        assert!(enrollment.enrollment_enabled);
        assert!(enrollment.admitted_identities().is_empty());
        assert_eq!(
            enrollment.callback_url(),
            "https://app.example/api/v1/auth/oidc/callback"
        );
        assert_eq!(
            enrollment.issuer(),
            "https://idm.example/oauth2/openid/tabula"
        );
    }
}
