//! Pure byte-integrity verification for manifest-declared physical asset files. (doc 04 §12.2)
//!
//! @ai.role trust-boundary
//! @ai.domain assets.integrity
//! @ai.pure true
//! @ai.invariant verified-asset-byte-size-and-blake3
//! @ai.evidence tests::integrity_verification_happy_path
//! @ai.evidence tests::integrity_verification_rejects_size_mismatch
//! @ai.evidence tests::integrity_verification_rejects_hash_mismatch

#![allow(clippy::doc_markdown)]

use crate::manifest::{AssetByteSize, AssetContentHash, AssetFile};

/// Why raw asset payload bytes failed integrity verification against an [`AssetFile`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssetIntegrityError {
    /// The actual byte length did not match the manifest-declared size.
    #[error("asset byte size mismatch: expected {expected}, found {found}")]
    SizeMismatch {
        /// The size declared in the manifest.
        expected: AssetByteSize,
        /// The actual byte length found.
        found: u64,
    },
    /// The computed BLAKE3 digest did not match the manifest-declared digest.
    #[error("asset content hash mismatch: expected {expected}, found {found}")]
    HashMismatch {
        /// The BLAKE3 digest declared in the manifest.
        expected: AssetContentHash,
        /// The computed BLAKE3 digest of the supplied bytes.
        found: AssetContentHash,
    },
}

/// Proof that raw bytes match an exact [`AssetFile`]'s declared size and BLAKE3 hash.
///
/// This witness guarantees:
/// 1. `bytes.len()` equals `file.bytes().get()`;
/// 2. `blake3::hash(bytes)` equals `*file.hash()`.
///
/// It does **not** guarantee that the bytes are a valid image/audio format,
/// that atlas coordinates are within decoded pixel bounds, that the source was
/// authentic, or that backend upload will succeed. Those remain later decoding
/// and renderer boundaries.
///
/// Fields are private and cannot be constructed unchecked. The only public
/// constructor is [`AssetFile::verify_bytes`].
///
/// @ai.role validated-domain-value
/// @ai.domain assets.integrity
/// @ai.pure true
/// @ai.invariant bytes-match-declared-asset-file
/// @ai.evidence tests::verified_asset_bytes_witness_preserves_exact_file_and_payload
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedAssetBytes<'file, 'bytes> {
    file: &'file AssetFile,
    bytes: &'bytes [u8],
}

impl<'file, 'bytes> VerifiedAssetBytes<'file, 'bytes> {
    pub(crate) const fn new(file: &'file AssetFile, bytes: &'bytes [u8]) -> Self {
        Self { file, bytes }
    }

    /// Returns the exact [`AssetFile`] metadata verified against these bytes.
    #[must_use]
    pub const fn file(&self) -> &'file AssetFile {
        self.file
    }

    /// Returns the verified byte payload.
    #[must_use]
    pub const fn bytes(&self) -> &'bytes [u8] {
        self.bytes
    }
}

impl AsRef<[u8]> for VerifiedAssetBytes<'_, '_> {
    fn as_ref(&self) -> &[u8] {
        self.bytes
    }
}

impl AssetFile {
    /// Verifies that untrusted raw bytes match this file's declared size and BLAKE3 digest.
    ///
    /// Verification order:
    /// 1. Byte length is verified against [`AssetFile::bytes`]. Truncated or oversized
    ///    payloads fail immediately without hashing.
    /// 2. BLAKE3 digest of the exact supplied bytes is verified against [`AssetFile::hash`].
    ///
    /// On success, returns a [`VerifiedAssetBytes`] witness binding this exact [`AssetFile`]
    /// and the verified byte slice.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain assets.integrity
    /// @ai.pure true
    /// @ai.invariant verified-asset-byte-size-and-blake3
    /// @ai.evidence tests::integrity_verification_happy_path
    /// @ai.evidence tests::integrity_verification_rejects_size_mismatch
    /// @ai.evidence tests::integrity_verification_rejects_hash_mismatch
    pub fn verify_bytes<'file, 'bytes>(
        &'file self,
        bytes: &'bytes [u8],
    ) -> Result<VerifiedAssetBytes<'file, 'bytes>, AssetIntegrityError> {
        let Ok(found_size) = u64::try_from(bytes.len()) else {
            return Err(AssetIntegrityError::SizeMismatch {
                expected: self.bytes(),
                found: u64::MAX,
            });
        };

        if found_size != self.bytes().get() {
            return Err(AssetIntegrityError::SizeMismatch {
                expected: self.bytes(),
                found: found_size,
            });
        }

        let computed_digest = *blake3::hash(bytes).as_bytes();
        let computed_hash = AssetContentHash::from_bytes(computed_digest);

        if computed_hash != *self.hash() {
            return Err(AssetIntegrityError::HashMismatch {
                expected: *self.hash(),
                found: computed_hash,
            });
        }

        Ok(VerifiedAssetBytes::new(self, bytes))
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use tabula_core::GameId;
    use tabula_game_api::AssetRef;

    use super::*;
    use crate::manifest::{
        AssetDensity, AssetPackId, AssetPackManifest, AssetPackRef, AssetPriority,
    };

    const FIXTURE_BYTES: &[u8] = b"tabula-pure-asset-byte-integrity-fixture-payload-data-v1";

    fn sample_manifest(file_name: &str, file_path: &str, bytes: &[u8]) -> AssetPackManifest {
        let hash_hex = blake3::hash(bytes).to_hex();
        let size = bytes.len();
        let toml_content = format!(
            r#"
pack    = "sample"
version = "1.0.0"
game    = "com.example.sample"

[[files]]
name     = "{file_name}"
path     = "{file_path}"
hash     = "{hash_hex}"
bytes    = {size}
priority = "critical"
density  = 2

[[resources]]
id = "pieces/white-knight"

[[resources.variants]]
file = "{file_name}"
region = {{ x = 0, y = 0, width = 128, height = 128 }}
"#
        );
        AssetPackManifest::from_toml(&toml_content).expect("valid fixture manifest must parse")
    }

    #[test]
    fn integrity_verification_happy_path() {
        let manifest = sample_manifest(
            "pieces@2x.atlas",
            "sample/1.0.0/pieces@2x.png",
            FIXTURE_BYTES,
        );
        let file = &manifest.files()[0];

        let verified = file
            .verify_bytes(FIXTURE_BYTES)
            .expect("matching bytes and size must verify successfully");

        assert_eq!(verified.file(), file);
        assert_eq!(verified.bytes(), FIXTURE_BYTES);
        assert_eq!(verified.as_ref(), FIXTURE_BYTES);
        assert!(ptr::eq(verified.bytes(), FIXTURE_BYTES));
    }

    #[test]
    fn verified_asset_bytes_witness_preserves_exact_file_and_payload() {
        let manifest = sample_manifest(
            "pieces@2x.atlas",
            "sample/1.0.0/pieces@2x.png",
            FIXTURE_BYTES,
        );
        let file = &manifest.files()[0];

        let verified = file.verify_bytes(FIXTURE_BYTES).unwrap();

        assert_eq!(verified.file().name().as_str(), "pieces@2x.atlas");
        assert_eq!(
            verified.file().path().as_str(),
            "sample/1.0.0/pieces@2x.png"
        );
        assert_eq!(verified.file().bytes().get(), FIXTURE_BYTES.len() as u64);
        assert_eq!(verified.file().priority(), AssetPriority::Critical);
        assert_eq!(
            verified.file().density(),
            Some(AssetDensity::new(2).unwrap())
        );
        assert_eq!(verified.file().hash(), file.hash());

        // Copy and Clone semantics
        let copied = verified;
        let cloned = verified;
        assert_eq!(verified, copied);
        assert_eq!(verified, cloned);
        assert_eq!(copied.bytes(), FIXTURE_BYTES);
    }

    #[test]
    fn integrity_verification_rejects_size_mismatch() {
        let manifest = sample_manifest(
            "pieces@2x.atlas",
            "sample/1.0.0/pieces@2x.png",
            FIXTURE_BYTES,
        );
        let file = &manifest.files()[0];
        let expected_size = file.bytes();

        // 1. Declared size > actual size (truncated payload)
        let truncated = &FIXTURE_BYTES[..FIXTURE_BYTES.len() - 10];
        let err = file.verify_bytes(truncated).unwrap_err();
        assert_eq!(
            err,
            AssetIntegrityError::SizeMismatch {
                expected: expected_size,
                found: truncated.len() as u64,
            }
        );

        // 2. Declared size < actual size (oversized payload)
        let mut oversized = FIXTURE_BYTES.to_vec();
        oversized.extend_from_slice(b"-extra-trailing-bytes");
        let err = file.verify_bytes(&oversized).unwrap_err();
        assert_eq!(
            err,
            AssetIntegrityError::SizeMismatch {
                expected: expected_size,
                found: oversized.len() as u64,
            }
        );

        // 3. Actual bytes empty while manifest size > 0
        let empty: &[u8] = &[];
        let err = file.verify_bytes(empty).unwrap_err();
        assert_eq!(
            err,
            AssetIntegrityError::SizeMismatch {
                expected: expected_size,
                found: 0,
            }
        );
    }

    #[test]
    fn size_checked_before_hashing_even_if_hash_would_match() {
        // Mutation test: if hashing occurred before size validation, supplying a slice whose
        // hash matches the declared hash but whose length differs from declared size would pass
        // hash check or fail at the wrong step.
        let full_payload = b"0123456789012345678901234567890123456789"; // 40 bytes
        let slice_30 = &full_payload[..30]; // 30 bytes
        let hash_of_slice_30 = blake3::hash(slice_30).to_hex();

        // Manifest declares the hash of slice_30, but declares size 40
        let toml_content = format!(
            r#"
pack    = "sample"
version = "1.0.0"
game    = "com.example.sample"

[[files]]
name     = "test.bin"
path     = "sample/1.0.0/test.bin"
hash     = "{hash_of_slice_30}"
bytes    = 40
priority = "high"

[[resources]]
id = "test/resource"
[[resources.variants]]
file = "test.bin"
"#
        );
        let manifest = AssetPackManifest::from_toml(&toml_content).unwrap();
        let file = &manifest.files()[0];

        // Pass slice_30 (30 bytes). Its hash matches declared hash, but length (30) != declared (40).
        let err = file.verify_bytes(slice_30).unwrap_err();
        assert_eq!(
            err,
            AssetIntegrityError::SizeMismatch {
                expected: AssetByteSize::new(40).unwrap(),
                found: 30,
            }
        );
    }

    #[test]
    fn integrity_verification_rejects_hash_mismatch() {
        let manifest = sample_manifest(
            "pieces@2x.atlas",
            "sample/1.0.0/pieces@2x.png",
            FIXTURE_BYTES,
        );
        let file = &manifest.files()[0];

        // Same size as FIXTURE_BYTES, but completely corrupted contents
        let corrupted = vec![0x55_u8; FIXTURE_BYTES.len()];
        let err = file.verify_bytes(&corrupted).unwrap_err();

        let expected_hash = *file.hash();
        let found_hash = AssetContentHash::from_bytes(*blake3::hash(&corrupted).as_bytes());

        assert_eq!(
            err,
            AssetIntegrityError::HashMismatch {
                expected: expected_hash,
                found: found_hash,
            }
        );
        assert_ne!(expected_hash, found_hash);
    }

    #[test]
    fn single_byte_corruption_at_every_position_is_rejected() {
        let manifest = sample_manifest(
            "pieces@2x.atlas",
            "sample/1.0.0/pieces@2x.png",
            FIXTURE_BYTES,
        );
        let file = &manifest.files()[0];

        // Prove that changing any single byte at any position fails hash verification
        for byte_idx in 0..FIXTURE_BYTES.len() {
            let mut corrupted = FIXTURE_BYTES.to_vec();
            corrupted[byte_idx] ^= 0x01;

            assert_eq!(corrupted.len(), FIXTURE_BYTES.len());

            let err = file.verify_bytes(&corrupted).unwrap_err();

            match err {
                AssetIntegrityError::HashMismatch { expected, found } => {
                    assert_eq!(expected, *file.hash());
                    assert_ne!(expected, found);
                }
                AssetIntegrityError::SizeMismatch { .. } => {
                    panic!("size did not change; must fail with HashMismatch");
                }
            }
        }
    }

    #[test]
    fn same_size_file_substitution_is_rejected() {
        let payload_a = b"same-size-payload-alpha-fixture-1";
        let payload_b = b"same-size-payload-bravo-fixture-2";
        assert_eq!(payload_a.len(), payload_b.len());
        assert_ne!(payload_a, payload_b);

        let size = payload_a.len();
        let hash_a = blake3::hash(payload_a).to_hex();
        let hash_b = blake3::hash(payload_b).to_hex();

        let toml_content = format!(
            r#"
pack    = "sample"
version = "1.0.0"
game    = "com.example.sample"

[[files]]
name     = "file_a.bin"
path     = "sample/1.0.0/file_a.bin"
hash     = "{hash_a}"
bytes    = {size}
priority = "critical"

[[files]]
name     = "file_b.bin"
path     = "sample/1.0.0/file_b.bin"
hash     = "{hash_b}"
bytes    = {size}
priority = "critical"

[[resources]]
id = "res/a"
[[resources.variants]]
file = "file_a.bin"

[[resources]]
id = "res/b"
[[resources.variants]]
file = "file_b.bin"
"#
        );

        let manifest = AssetPackManifest::from_toml(&toml_content).unwrap();
        let file_a = &manifest.files()[0];
        let file_b = &manifest.files()[1];

        // Valid pairs succeed
        assert!(file_a.verify_bytes(payload_a).is_ok());
        assert!(file_b.verify_bytes(payload_b).is_ok());

        // Substituted same-size payloads fail with HashMismatch
        let err_a = file_a.verify_bytes(payload_b).unwrap_err();
        assert_eq!(
            err_a,
            AssetIntegrityError::HashMismatch {
                expected: *file_a.hash(),
                found: AssetContentHash::from_bytes(*blake3::hash(payload_b).as_bytes()),
            }
        );

        let err_b = file_b.verify_bytes(payload_a).unwrap_err();
        assert_eq!(
            err_b,
            AssetIntegrityError::HashMismatch {
                expected: *file_b.hash(),
                found: AssetContentHash::from_bytes(*blake3::hash(payload_a).as_bytes()),
            }
        );
    }

    #[test]
    fn shared_atlas_semantics_verifies_physical_file_once() {
        const ATLAS_PAYLOAD: &[u8] = b"shared-atlas-texture-data-representing-multiple-items";
        let atlas_hash = blake3::hash(ATLAS_PAYLOAD).to_hex();
        let atlas_size = ATLAS_PAYLOAD.len();

        let toml_content = format!(
            r#"
pack    = "sample"
version = "1.0.0"
game    = "com.example.sample"

[[files]]
name     = "pieces@2x.atlas"
path     = "sample/1.0.0/pieces@2x.png"
hash     = "{atlas_hash}"
bytes    = {atlas_size}
priority = "critical"
density  = 2

[[resources]]
id = "pieces/white-knight"
[[resources.variants]]
file = "pieces@2x.atlas"
region = {{ x = 0, y = 0, width = 128, height = 128 }}

[[resources]]
id = "pieces/white-queen"
[[resources.variants]]
file = "pieces@2x.atlas"
region = {{ x = 128, y = 0, width = 128, height = 128 }}

[[resources]]
id = "pieces/black-king"
[[resources.variants]]
file = "pieces@2x.atlas"
region = {{ x = 256, y = 0, width = 128, height = 128 }}
"#
        );

        let manifest = AssetPackManifest::from_toml(&toml_content).unwrap();
        let pack_ref = AssetPackRef::new(
            AssetPackId::new("sample").unwrap(),
            crate::manifest::AssetPackVersion::new("1.0.0").unwrap(),
        );
        let game_id = GameId::new("com.example.sample").unwrap();
        let bound = manifest.validate_binding(&pack_ref, &game_id).unwrap();

        let res_knight = bound
            .resolve(
                &AssetRef::from_static("pieces/white-knight"),
                AssetDensity::new(2).unwrap(),
            )
            .unwrap();
        let res_queen = bound
            .resolve(
                &AssetRef::from_static("pieces/white-queen"),
                AssetDensity::new(2).unwrap(),
            )
            .unwrap();
        let res_king = bound
            .resolve(
                &AssetRef::from_static("pieces/black-king"),
                AssetDensity::new(2).unwrap(),
            )
            .unwrap();

        // All 3 logical resources point to the exact same physical AssetFile
        assert_eq!(res_knight.file(), res_queen.file());
        assert_eq!(res_queen.file(), res_king.file());
        assert!(ptr::eq(res_knight.file(), res_queen.file()));
        assert!(ptr::eq(res_queen.file(), res_king.file()));

        // Verification happens once at the physical AssetFile level
        let verified = res_knight
            .file()
            .verify_bytes(ATLAS_PAYLOAD)
            .expect("physical atlas bytes verify once");

        assert_eq!(verified.file(), res_knight.file());
        assert_eq!(verified.file(), res_queen.file());
        assert_eq!(verified.file(), res_king.file());
        assert_eq!(verified.bytes(), ATLAS_PAYLOAD);
    }

    #[test]
    fn verification_works_independently_of_binding_and_resolution() {
        let manifest = sample_manifest(
            "pieces@2x.atlas",
            "sample/1.0.0/pieces@2x.png",
            FIXTURE_BYTES,
        );

        // Direct file access without pack binding or resource resolution
        let file = &manifest.files()[0];
        let verified = file.verify_bytes(FIXTURE_BYTES).unwrap();

        assert_eq!(verified.file().name().as_str(), "pieces@2x.atlas");
        assert_eq!(verified.bytes(), FIXTURE_BYTES);
    }

    #[test]
    fn error_display_formatting_is_deterministic_and_informative() {
        let expected_size = AssetByteSize::new(1024).unwrap();
        let size_err = AssetIntegrityError::SizeMismatch {
            expected: expected_size,
            found: 512,
        };
        assert_eq!(
            size_err.to_string(),
            "asset byte size mismatch: expected 1024, found 512"
        );

        let hash_a = AssetContentHash::from_bytes([0x11; 32]);
        let hash_b = AssetContentHash::from_bytes([0x22; 32]);
        let hash_err = AssetIntegrityError::HashMismatch {
            expected: hash_a,
            found: hash_b,
        };
        assert_eq!(
            hash_err.to_string(),
            format!("asset content hash mismatch: expected {hash_a}, found {hash_b}")
        );
        assert!(hash_err
            .to_string()
            .contains("1111111111111111111111111111111111111111111111111111111111111111"));
        assert!(hash_err
            .to_string()
            .contains("2222222222222222222222222222222222222222222222222222222222222222"));
    }

    #[test]
    fn non_raster_audio_and_multi_priority_verification() {
        const AUDIO_BYTES: &[u8] =
            b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00mock-audio-payload";
        let audio_hash = blake3::hash(AUDIO_BYTES).to_hex();
        let audio_size = AUDIO_BYTES.len();

        let toml_content = format!(
            r#"
pack    = "sample"
version = "1.0.0"
game    = "com.example.sample"

[[files]]
name     = "move.ogg"
path     = "sample/1.0.0/move.ogg"
hash     = "{audio_hash}"
bytes    = {audio_size}
priority = "low"

[[resources]]
id = "audio/move"
[[resources.variants]]
file = "move.ogg"
"#
        );

        let manifest = AssetPackManifest::from_toml(&toml_content).unwrap();
        let audio_file = &manifest.files()[0];

        assert_eq!(audio_file.density(), None);
        assert_eq!(audio_file.priority(), AssetPriority::Low);

        let verified = audio_file.verify_bytes(AUDIO_BYTES).unwrap();
        assert_eq!(verified.file().name().as_str(), "move.ogg");
        assert_eq!(verified.bytes(), AUDIO_BYTES);
    }
}
