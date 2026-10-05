use std::fmt;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest as _, Sha256};

use crate::SessionError;

/// A SHA-256 verifier. Durable storage accepts this, never a bearer value.
/// Debug is redacted and no wire serialization is implemented (ADR-0031 §1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialDigest([u8; 32]);

impl CredentialDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Checked digest-column conversion; malformed persisted rows fail closed.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, SessionError> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| SessionError::Unavailable)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for CredentialDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialDigest([REDACTED])")
    }
}

/// A 32-byte opaque transport credential, never a public record identifier.
///
/// The only production minting path uses OS entropy. `parse` validates encoding,
/// not authority; only a durable authority lookup can authenticate it. Deliberate
/// exposure is available solely for the transport adapter. No `Display`, serde,
/// `Clone`, byte accessor or recoverable successor is provided (ADR-0031 §1/§5).
#[derive(PartialEq, Eq)]
pub struct SessionCredential([u8; 32]);

impl SessionCredential {
    /// Generates exactly 32 bytes from the OS CSPRNG; entropy failure is unavailable.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate() -> Result<Self, SessionError> {
        Self::generate_with(|bytes| getrandom::fill(bytes).map_err(|_| SessionError::Unavailable))
    }

    /// This foundation cannot mint credentials in a browser/WASM target.
    #[cfg(target_arch = "wasm32")]
    pub fn generate() -> Result<Self, SessionError> {
        Err(SessionError::Unavailable)
    }

    #[cfg(any(not(target_arch = "wasm32"), test))]
    fn generate_with(
        fill: impl FnOnce(&mut [u8; 32]) -> Result<(), SessionError>,
    ) -> Result<Self, SessionError> {
        let mut bytes = [0; 32];
        fill(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// Accepts exactly canonical URL-safe unpadded base64 (43 characters).
    pub fn parse(encoded: &str) -> Result<Self, SessionError> {
        if encoded.len() != 43 {
            return Err(SessionError::InvalidInput);
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| SessionError::InvalidInput)?;
        let bytes: [u8; 32] = decoded.try_into().map_err(|_| SessionError::InvalidInput)?;
        if URL_SAFE_NO_PAD.encode(bytes) != encoded {
            return Err(SessionError::InvalidInput);
        }
        Ok(Self(bytes))
    }

    /// Intentional bearer exposure for a credential's authorized transport channel.
    /// Never log, persist or send this through a generic wire serializer.
    pub fn expose_encoded(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    pub fn digest(&self) -> CredentialDigest {
        CredentialDigest(Sha256::digest(self.0).into())
    }
}

impl fmt::Debug for SessionCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionCredential([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_uses_pinned_canonical_encoding_and_sha256() {
        let credential = SessionCredential([0; 32]);
        let encoded = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        assert_eq!(credential.expose_encoded(), encoded);
        assert_eq!(SessionCredential::parse(encoded).unwrap(), credential);
        // Independent SHA-256 vector for 32 zero bytes.
        assert_eq!(
            credential.digest().as_bytes(),
            &[
                0x66, 0x68, 0x7a, 0xad, 0xf8, 0x62, 0xbd, 0x77, 0x6c, 0x8f, 0xc1, 0x8b, 0x8e, 0x9f,
                0x8e, 0x20, 0x08, 0x97, 0x14, 0x85, 0x6e, 0xe2, 0x33, 0xb3, 0x90, 0x2a, 0x59, 0x1d,
                0x0d, 0x5f, 0x29, 0x25,
            ]
        );
        for invalid in [
            "",
            "AAAA",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA+",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA/",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n",
        ] {
            assert_eq!(
                SessionCredential::parse(invalid),
                Err(SessionError::InvalidInput)
            );
        }
    }

    #[test]
    fn secrets_and_verifiers_are_redacted() {
        let credential = SessionCredential([0xab; 32]);
        assert_eq!(format!("{credential:?}"), "SessionCredential([REDACTED])");
        assert_eq!(
            format!("{:?}", credential.digest()),
            "CredentialDigest([REDACTED])"
        );
        assert_eq!(
            CredentialDigest::from_slice(&[1; 31]),
            Err(SessionError::Unavailable)
        );
        assert_eq!(
            CredentialDigest::from_slice(&[1; 33]),
            Err(SessionError::Unavailable)
        );
    }

    #[test]
    fn entropy_failure_never_mints_a_zero_or_partial_credential() {
        assert_eq!(
            SessionCredential::generate_with(|bytes| {
                bytes[0] = 42;
                Err(SessionError::Unavailable)
            }),
            Err(SessionError::Unavailable)
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn production_generation_roundtrips_exactly_32_bytes() {
        let credential = SessionCredential::generate().unwrap();
        let encoded = credential.expose_encoded();
        assert_eq!(encoded.len(), 43);
        assert_eq!(SessionCredential::parse(&encoded).unwrap(), credential);
    }
}
