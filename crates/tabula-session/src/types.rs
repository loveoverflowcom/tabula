use std::fmt;

/// Generic boundary outcomes, with no identity, credential or storage detail.
/// (ADR-0031 §3, ADR-0036)
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("unavailable")]
    Unavailable,
    #[error("conflict")]
    Conflict,
    #[error("invalid input")]
    InvalidInput,
}

/// Exact, bounded provider issuer+subject lookup key (ADR-0034).
///
/// Construction validates structure only. It establishes no verified provider
/// login, account permission, email equivalence or issuer trust. No trimming,
/// case folding, Unicode normalization or URL normalization is performed.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderIdentityKey {
    issuer: String,
    subject: String,
}

impl ProviderIdentityKey {
    /// Structural bound for each UTF-8 identity component in bytes.
    /// Two exact-key components fit the durable `PostgreSQL` index budget; values
    /// beyond this internal bound are rejected, never normalized or truncated.
    pub const MAX_COMPONENT_BYTES: usize = 1024;

    pub fn new(
        issuer: impl Into<String>,
        subject: impl Into<String>,
    ) -> Result<Self, SessionError> {
        let issuer = issuer.into();
        let subject = subject.into();
        if !Self::valid_component(&issuer) || !Self::valid_component(&subject) {
            return Err(SessionError::InvalidInput);
        }
        Ok(Self { issuer, subject })
    }

    fn valid_component(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= Self::MAX_COMPONENT_BYTES
            && !value.chars().any(char::is_control)
            && !value.chars().all(char::is_whitespace)
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }
}

impl fmt::Debug for ProviderIdentityKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderIdentityKey([REDACTED])")
    }
}

macro_rules! bounded_counter {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            /// Checks the shared signed-64-bit durable storage domain.
            pub fn new(value: u64) -> Result<Self, SessionError> {
                if value > i64::MAX as u64 {
                    return Err(SessionError::InvalidInput);
                }
                Ok(Self(value))
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

bounded_counter!(
    UnixMillis,
    "Server wall-clock milliseconds, sampled inside the adapter's ordering boundary (ADR-0036)."
);
bounded_counter!(
    AccountEpoch,
    "Account authorization epoch: stale in-flight issuance is fenced by CAS (ADR-0031 §5)."
);
bounded_counter!(
    CredentialGeneration,
    "Credential rotation generation, independent of established connection binding (ADR-0031 §5)."
);

impl AccountEpoch {
    /// Never wraps or saturates: exhaustion fails closed.
    pub fn checked_next(self) -> Result<Self, SessionError> {
        self.get()
            .checked_add(1)
            .and_then(|value| Self::new(value).ok())
            .ok_or(SessionError::Unavailable)
    }
}

impl CredentialGeneration {
    /// Never wraps or saturates: exhaustion fails closed.
    pub fn checked_next(self) -> Result<Self, SessionError> {
        self.get()
            .checked_add(1)
            .and_then(|value| Self::new(value).ok())
            .ok_or(SessionError::Unavailable)
    }
}

macro_rules! nonzero_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u128);

        impl $name {
            pub fn new(value: u128) -> Result<Self, SessionError> {
                if value == 0 {
                    return Err(SessionError::InvalidInput);
                }
                Ok(Self(value))
            }

            pub const fn get(self) -> u128 {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
    };
}

nonzero_id!(AuthSessionId, "Internal durable session record identity; never a credential or connection routing ID (ADR-0031 §1).");
nonzero_id!(SessionContextId, "Stable non-authorizing session context binding ID. This is not a CSRF token, bearer or token verifier; token issuance stays gated (ADR-0036).");

/// The credential transport channel; browser and native cannot cross-use it.
/// (ADR-0031 §2–§3)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SessionChannel {
    BrowserCookie,
    NativeBearer,
}

impl SessionChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BrowserCookie => "browser_cookie",
            Self::NativeBearer => "native_bearer",
        }
    }

    /// Checked storage/input conversion; unknown channels never select a fallback.
    pub fn parse(value: &str) -> Result<Self, SessionError> {
        match value {
            "browser_cookie" => Ok(Self::BrowserCookie),
            "native_bearer" => Ok(Self::NativeBearer),
            _ => Err(SessionError::InvalidInput),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_identity_is_exact_and_structural_only() {
        let key = ProviderIdentityKey::new("Issuer/", "Subject A").unwrap();
        assert_eq!(key.issuer(), "Issuer/");
        assert_eq!(key.subject(), "Subject A");
        for other in [
            ("issuer/", "Subject A"),
            ("Issuer", "Subject A"),
            ("Issuer/", "Subject a"),
        ] {
            assert_ne!(key, ProviderIdentityKey::new(other.0, other.1).unwrap());
        }
        assert_ne!(
            ProviderIdentityKey::new("I", "é").unwrap(),
            ProviderIdentityKey::new("I", "e\u{301}").unwrap()
        );
        assert_eq!(
            ProviderIdentityKey::new(" I ", " S ").unwrap().subject(),
            " S "
        );
        for invalid in ["", " ", "\n", "a\0b", "a\u{7f}b"] {
            assert_eq!(
                ProviderIdentityKey::new(invalid, "S"),
                Err(SessionError::InvalidInput)
            );
            assert_eq!(
                ProviderIdentityKey::new("I", invalid),
                Err(SessionError::InvalidInput)
            );
        }
        let limit = "x".repeat(ProviderIdentityKey::MAX_COMPONENT_BYTES);
        assert!(ProviderIdentityKey::new(&limit, "S").is_ok());
        assert!(ProviderIdentityKey::new(format!("{limit}x"), "S").is_err());
        assert_eq!(format!("{key:?}"), "ProviderIdentityKey([REDACTED])");
    }

    #[test]
    fn numeric_ids_and_counters_reject_boundaries_without_wrapping() {
        assert_eq!(AuthSessionId::new(0), Err(SessionError::InvalidInput));
        assert_eq!(SessionContextId::new(0), Err(SessionError::InvalidInput));
        assert!(AuthSessionId::new(u128::MAX).is_ok());
        for value in [i64::MAX as u64 + 1, u64::MAX] {
            assert!(UnixMillis::new(value).is_err());
            assert!(AccountEpoch::new(value).is_err());
            assert!(CredentialGeneration::new(value).is_err());
        }
        assert_eq!(
            AccountEpoch::new(i64::MAX as u64).unwrap().checked_next(),
            Err(SessionError::Unavailable)
        );
        assert_eq!(
            CredentialGeneration::new(i64::MAX as u64)
                .unwrap()
                .checked_next(),
            Err(SessionError::Unavailable)
        );
        assert_eq!(
            CredentialGeneration::new(0)
                .unwrap()
                .checked_next()
                .unwrap()
                .get(),
            1
        );
    }
}
