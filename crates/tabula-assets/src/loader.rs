//! Async-capable orchestration from an [`crate::AssetSource`] to owned verified bytes.
//!
//! Fetching remains an effectful source operation; the integrity transition is
//! delegated to the pure [`crate::AssetFile::verify_owned_bytes`] boundary.
//! The future is deliberately not required to be `Send`, preserving browser
//! and single-threaded WASM compatibility.
//!
//! @ai.role orchestration-boundary
//! @ai.domain assets.loading
//! @ai.pure false
//! @ai.invariant source-and-integrity-errors-remain-distinct
//! @ai.evidence tests::load_verified_preserves_source_errors
//! @ai.evidence tests::load_verified_preserves_integrity_errors

#![allow(clippy::doc_markdown)]

use crate::{AssetFile, AssetSource, OwnedVerifiedAssetBytes};

/// Why a verified asset load failed.
///
/// Source acquisition and pure integrity failures remain separate so callers
/// can choose source recovery behavior without treating corrupt bytes as a
/// transport failure.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetLoadError<SourceError> {
    /// The source could not provide bytes for the requested path.
    #[error("asset source failed: {0}")]
    Source(#[source] SourceError),
    /// The source returned bytes that failed the manifest contract.
    #[error("asset integrity verification failed: {0}")]
    Integrity(#[source] crate::AssetIntegrityError),
}

/// Fetches one physical asset and returns it only after size and BLAKE3
/// verification against the exact supplied [`AssetFile`].
///
/// This is a thin imperative shell: [`AssetSource::fetch`] acquires owned but
/// untrusted bytes, then [`AssetFile::verify_owned_bytes`] performs the pure
/// trust transition. No filesystem, HTTP, cache, retry, decoder, or renderer
/// policy is introduced here.
///
/// @ai.role trust-transition
/// @ai.domain assets.loading
/// @ai.pure false
/// @ai.requires source-output-is-unverified
/// @ai.ensures bytes-match-owned-asset-file
/// @ai.evidence tests::load_verified_returns_owned_payload_after_source_scope
/// @ai.evidence tests::load_verified_preserves_source_errors
/// @ai.evidence tests::load_verified_preserves_integrity_errors
pub async fn load_verified<S>(
    file: &AssetFile,
    source: &S,
) -> Result<OwnedVerifiedAssetBytes, AssetLoadError<S::Error>>
where
    S: AssetSource,
{
    let unverified = source
        .fetch(file.path())
        .await
        .map_err(AssetLoadError::Source)?;

    file.verify_owned_bytes(unverified)
        .map_err(AssetLoadError::Integrity)
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use super::*;
    use crate::{
        AssetIntegrityError, AssetPackManifest, AssetPath, MemoryAssetSource,
        MemoryAssetSourceError, UnverifiedAssetBytes,
    };

    const PAYLOAD: &[u8] = b"owned-loader-fixture";
    const FILE_NAME: &str = "fixture.bin";
    const FILE_PATH: &str = "sample/1.0.0/fixture.bin";

    fn sample_manifest(payload: &[u8]) -> AssetPackManifest {
        let hash = blake3::hash(payload).to_hex();
        let source = format!(
            r#"
pack = "sample"
version = "1.0.0"
game = "com.example.sample"

[[files]]
name = "{FILE_NAME}"
path = "{FILE_PATH}"
hash = "{hash}"
bytes = {}
priority = "critical"

[[resources]]
id = "fixture"
[[resources.variants]]
file = "{FILE_NAME}"
"#,
            payload.len()
        );
        AssetPackManifest::from_toml(&source).expect("valid fixture manifest must parse")
    }

    fn two_file_manifest(first: &[u8], second: &[u8]) -> AssetPackManifest {
        let first_hash = blake3::hash(first).to_hex();
        let second_hash = blake3::hash(second).to_hex();
        let source = format!(
            r#"
pack = "sample"
version = "1.0.0"
game = "com.example.sample"

[[files]]
name = "first.bin"
path = "sample/1.0.0/first.bin"
hash = "{first_hash}"
bytes = {}
priority = "critical"

[[files]]
name = "second.bin"
path = "sample/1.0.0/second.bin"
hash = "{second_hash}"
bytes = {}
priority = "critical"

[[resources]]
id = "first"
[[resources.variants]]
file = "first.bin"

[[resources]]
id = "second"
[[resources.variants]]
file = "second.bin"
"#,
            first.len(),
            second.len()
        );
        AssetPackManifest::from_toml(&source).expect("valid fixture manifest must parse")
    }

    fn block_on_ready<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => result,
            Poll::Pending => panic!("memory source load must be ready without an executor"),
        }
    }

    #[test]
    fn load_verified_returns_owned_payload_after_source_scope() {
        let manifest = sample_manifest(PAYLOAD);
        let file = manifest.files()[0].clone();

        let verified = {
            let mut source = MemoryAssetSource::new();
            source.insert(file.path().clone(), PAYLOAD.to_vec());
            block_on_ready(load_verified(&file, &source)).expect("matching bytes must verify")
        };

        assert_eq!(verified.file(), &file);
        assert_eq!(verified.bytes(), PAYLOAD);
    }

    #[test]
    fn load_verified_preserves_source_errors() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let source = MemoryAssetSource::new();

        let error = block_on_ready(load_verified(file, &source)).unwrap_err();

        assert_eq!(
            error,
            AssetLoadError::Source(MemoryAssetSourceError::NotFound(file.path().clone()))
        );
    }

    #[test]
    fn load_verified_preserves_integrity_errors() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let mut source = MemoryAssetSource::new();
        source.insert(file.path().clone(), b"truncated".to_vec());

        let error = block_on_ready(load_verified(file, &source)).unwrap_err();

        assert_eq!(
            error,
            AssetLoadError::Integrity(AssetIntegrityError::SizeMismatch {
                expected: file.bytes(),
                found: 9,
            })
        );
    }

    #[test]
    fn load_verified_rejects_oversized_payloads() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let mut oversized = PAYLOAD.to_vec();
        oversized.push(0);
        let mut source = MemoryAssetSource::new();
        source.insert(file.path().clone(), oversized);

        let error = block_on_ready(load_verified(file, &source)).unwrap_err();

        assert!(matches!(
            error,
            AssetLoadError::Integrity(AssetIntegrityError::SizeMismatch { .. })
        ));
    }

    #[test]
    fn load_verified_rejects_same_size_corruption() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let mut corrupted = PAYLOAD.to_vec();
        corrupted[0] ^= 0x01;
        let mut source = MemoryAssetSource::new();
        source.insert(file.path().clone(), corrupted);

        let error = block_on_ready(load_verified(file, &source)).unwrap_err();

        assert!(matches!(
            error,
            AssetLoadError::Integrity(AssetIntegrityError::HashMismatch { .. })
        ));
    }

    #[test]
    fn load_verified_rejects_same_size_bytes_for_another_asset_file() {
        let first = b"file-a-payload";
        let second = b"file-b-payload";
        assert_eq!(first.len(), second.len());
        let manifest = two_file_manifest(first, second);
        let first_file = &manifest.files()[0];
        let second_file = &manifest.files()[1];
        let mut source = MemoryAssetSource::new();
        source.insert(first_file.path().clone(), second.to_vec());
        source.insert(second_file.path().clone(), second.to_vec());

        let error = block_on_ready(load_verified(first_file, &source)).unwrap_err();

        assert_eq!(
            error,
            AssetLoadError::Integrity(AssetIntegrityError::HashMismatch {
                expected: *first_file.hash(),
                found: *second_file.hash(),
            })
        );
    }

    #[test]
    fn repeated_memory_loads_are_deterministic_and_equivalent() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let mut source = MemoryAssetSource::new();
        source.insert(file.path().clone(), PAYLOAD.to_vec());

        let first = block_on_ready(load_verified(file, &source)).expect("first load must verify");
        let second = block_on_ready(load_verified(file, &source)).expect("second load must verify");

        assert_eq!(first, second);
        assert_eq!(first.bytes(), second.bytes());
        assert_eq!(first.file(), second.file());
    }

    #[test]
    fn unverified_source_value_is_consumed_by_owned_verification() {
        let manifest = sample_manifest(PAYLOAD);
        let file = &manifest.files()[0];
        let unverified = UnverifiedAssetBytes::new(PAYLOAD.to_vec());
        let verified = file
            .verify_owned_bytes(unverified)
            .expect("matching bytes must verify");

        assert_eq!(verified.bytes(), PAYLOAD);
    }

    #[test]
    fn missing_path_error_is_structured_at_source_boundary() {
        let path = AssetPath::new(FILE_PATH).expect("fixture path must be valid");
        let source = MemoryAssetSource::new();

        let error = block_on_ready(source.fetch(&path)).unwrap_err();

        assert_eq!(error, MemoryAssetSourceError::NotFound(path));
    }
}
