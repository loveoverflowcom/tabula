//! Platform-neutral asset-byte sources and the explicit unverified byte state.
//!
//! A source only retrieves owned bytes for a validated physical [`AssetPath`].
//! It does not know logical resources or manifest integrity metadata. Every
//! returned payload must still pass [`crate::AssetFile::verify_bytes`] before
//! it can become [`crate::VerifiedAssetBytes`].
//!
//! @ai.role source-port
//! @ai.domain assets.source
//! @ai.pure false
//! @ai.invariant source-output-is-unverified
//! @ai.evidence tests::source_output_requires_integrity_verification

#![allow(clippy::doc_markdown)]

use std::collections::BTreeMap;
use std::future::Future;

use crate::AssetPath;

/// Owned bytes obtained from an external or test source before integrity
/// verification.
///
/// This type records only ownership and the fact that bytes were returned. It
/// does not imply authenticity, file-format validity, successful decoding, or
/// a match with any particular [`crate::AssetFile`]. Use
/// [`crate::AssetFile::verify_bytes`] to cross the integrity boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnverifiedAssetBytes {
    bytes: Vec<u8>,
}

impl UnverifiedAssetBytes {
    /// Wraps an owned payload without making any integrity claim about it.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Returns the unverified payload without transferring ownership.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the owned unverified payload.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }

    /// Returns the payload length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl AsRef<[u8]> for UnverifiedAssetBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

/// A platform-neutral port for obtaining physical asset bytes.
///
/// Implementations may use a filesystem, HTTP, browser API, cache, or test
/// fixture in a later layer. The port deliberately accepts only a validated
/// physical [`AssetPath`] and always returns explicitly unverified owned bytes.
/// The returned future is not required to be `Send`, so browser implementations
/// remain portable to WASM.
pub trait AssetSource {
    /// The source-specific error returned when bytes cannot be obtained.
    type Error;

    /// Retrieves owned, unverified bytes for a physical asset path.
    fn fetch<'a>(
        &'a self,
        path: &'a AssetPath,
    ) -> impl Future<Output = Result<UnverifiedAssetBytes, Self::Error>> + 'a;
}

/// A deterministic in-memory [`AssetSource`] for tests and local reference
/// flows.
///
/// This is a source fixture, not a cache. Entries are keyed only by physical
/// [`AssetPath`] values, and every read returns a fresh owned unverified payload.
#[derive(Clone, Debug, Default)]
pub struct MemoryAssetSource {
    entries: BTreeMap<AssetPath, Vec<u8>>,
}

impl MemoryAssetSource {
    /// Creates an empty in-memory source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores or replaces the bytes associated with a physical asset path.
    ///
    /// The bytes remain unverified, including when read back through
    /// [`AssetSource::fetch`].
    pub fn insert(&mut self, path: AssetPath, bytes: Vec<u8>) {
        self.entries.insert(path, bytes);
    }
}

impl AssetSource for MemoryAssetSource {
    type Error = MemoryAssetSourceError;

    fn fetch<'a>(
        &'a self,
        path: &'a AssetPath,
    ) -> impl Future<Output = Result<UnverifiedAssetBytes, Self::Error>> + 'a {
        std::future::ready(
            self.entries
                .get(path)
                .cloned()
                .map(UnverifiedAssetBytes::new)
                .ok_or_else(|| MemoryAssetSourceError::NotFound(path.clone())),
        )
    }
}

/// Why a [`MemoryAssetSource`] could not return bytes for a requested path.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MemoryAssetSourceError {
    /// No in-memory entry exists for the requested physical path.
    #[error("asset source path not found: {0}")]
    NotFound(AssetPath),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AssetIntegrityError, AssetPackManifest};

    const PAYLOAD: &[u8] = b"ABCDEF";
    const FILE_NAME: &str = "fixture.bin";
    const FILE_PATH: &str = "sample/1.0.0/fixture.bin";

    fn sample_manifest(bytes: &[u8]) -> AssetPackManifest {
        let hash_hex = blake3::hash(bytes).to_hex();
        let content = format!(
            r#"
pack = "sample"
version = "1.0.0"
game = "com.example.sample"

[[files]]
name = "{FILE_NAME}"
path = "{FILE_PATH}"
hash = "{hash_hex}"
bytes = {}
priority = "critical"

[[resources]]
id = "pieces/white-knight"
[[resources.variants]]
file = "{FILE_NAME}"
"#,
            bytes.len()
        );
        AssetPackManifest::from_toml(&content).expect("valid fixture manifest must parse")
    }

    fn fetch_now(
        source: &MemoryAssetSource,
        path: &AssetPath,
    ) -> Result<UnverifiedAssetBytes, MemoryAssetSourceError> {
        let mut future = Box::pin(source.fetch(path));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(result) => result,
            std::task::Poll::Pending => panic!("memory source future must be ready"),
        }
    }

    #[test]
    fn unverified_asset_bytes_exposes_owned_payload_operations() {
        let bytes = UnverifiedAssetBytes::new(PAYLOAD.to_vec());

        assert_eq!(bytes.as_slice(), PAYLOAD);
        assert_eq!(bytes.as_ref(), PAYLOAD);
        assert_eq!(bytes.len(), PAYLOAD.len());
        assert!(!bytes.is_empty());
        assert_eq!(bytes.into_vec(), PAYLOAD);
        assert!(UnverifiedAssetBytes::new(Vec::new()).is_empty());
    }

    #[test]
    fn source_output_requires_integrity_verification() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let mut source = MemoryAssetSource::new();
        source.insert(file.path().clone(), PAYLOAD.to_vec());

        let unverified = fetch_now(&source, file.path()).expect("stored path must be found");
        let verified = file
            .verify_bytes(unverified.as_slice())
            .expect("matching source bytes must verify");

        assert_eq!(verified.file(), file);
        assert_eq!(verified.bytes(), PAYLOAD);
    }

    #[test]
    fn corrupt_source_output_is_rejected_by_integrity_boundary() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let mut source = MemoryAssetSource::new();
        source.insert(file.path().clone(), b"ABXDEF".to_vec());

        let unverified = fetch_now(&source, file.path()).expect("stored path must be found");
        let error = file.verify_bytes(unverified.as_slice()).unwrap_err();

        assert!(matches!(error, AssetIntegrityError::HashMismatch { .. }));
    }

    #[test]
    fn truncated_source_output_is_rejected_by_integrity_boundary() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let mut source = MemoryAssetSource::new();
        source.insert(file.path().clone(), b"ABCDE".to_vec());

        let unverified = fetch_now(&source, file.path()).expect("stored path must be found");
        let error = file.verify_bytes(unverified.as_slice()).unwrap_err();

        assert_eq!(
            error,
            AssetIntegrityError::SizeMismatch {
                expected: file.bytes(),
                found: 5,
            }
        );
    }

    #[test]
    fn missing_path_returns_source_error_with_requested_path() {
        let path = AssetPath::new(FILE_PATH).expect("fixture path must be valid");
        let source = MemoryAssetSource::new();

        let error = fetch_now(&source, &path).unwrap_err();

        assert_eq!(error, MemoryAssetSourceError::NotFound(path));
    }

    #[test]
    fn repeated_reads_of_one_path_are_deterministic_and_not_consuming() {
        let path = AssetPath::new(FILE_PATH).expect("fixture path must be valid");
        let mut source = MemoryAssetSource::new();
        source.insert(path.clone(), PAYLOAD.to_vec());

        let first = fetch_now(&source, &path).expect("stored path must be found");
        let second = fetch_now(&source, &path).expect("stored path must remain available");

        assert_eq!(first.as_slice(), PAYLOAD);
        assert_eq!(second.as_slice(), PAYLOAD);
        assert_eq!(first, second);
    }
}
