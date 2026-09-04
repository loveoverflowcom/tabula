//! Canonical encoding and state hashing. (doc 05 §7, ADR-021, ADR-026)
//!
//! ## Why this file is load-bearing
//!
//! The state hash is **the only mechanism that detects determinism drift in
//! production**. A snapshot carries one; every N-th event row carries one
//! (default N = 20). The nightly replay job re-runs sampled real matches and
//! compares. When they diverge, the failing input index is known exactly, so the
//! bug is a bisect away rather than a mystery. (doc 05 §7.3)
//!
//! ## The canonical encoding
//!
//! ```text
//! canonical(x) = ENCODING_VERSION.to_le_bytes() ‖ postcard(x)
//! ```
//!
//! with:
//!
//! - the type's **derived** `Serialize` (never a custom human-friendly impl)
//! - all maps as `BTreeMap` (sorted keys) — `HashMap` is banned in these types (I-2)
//! - no floats anywhere (doc 00 §5.1)
//!
//! Used for `match_inputs.payload`, `match_events.payload`,
//! `match_snapshots.payload`, `.tbr` replay files, and everything hashed.
//!
//! **Always Postcard, regardless of the connection codec.** JSON is a transport
//! convenience, never a storage format — its key ordering and float formatting
//! are not stable enough to hash. (doc 05 §4.3)
//!
//! ### The one instability Postcard does not fix for us
//!
//! Postcard is non-self-describing with fixed varint encoding, so it has no key
//! ordering or float formatting freedom of its own. It does, however, serialize a
//! map in **iteration order**. A `HashMap` in canonical state would therefore
//! encode nondeterministically, and no encoder can fix that.
//!
//! It is prevented upstream (`clippy.toml` bans `HashMap`/`HashSet` in every
//! rules-tier crate) and *caught* by `determinism_same_inputs`, which builds the
//! state twice from scratch: two independently constructed `HashMap`s in one
//! thread get different `RandomState` seeds, so their iteration orders differ and
//! the hashes diverge.
//!
//! ## The state hash
//!
//! ```text
//! StateHash = blake3( STATE_HASH_DOMAIN          // 15 bytes, fixed
//!                   ‖ rules_version.to_le_bytes() // u32, 4 bytes, fixed
//!                   ‖ canonical(state) )
//! ```
//!
//! Both prefixes are fixed-width, so the preimage is unambiguous without length
//! prefixing. What participates and what does not — `GameId` and `Config` do not —
//! is decided and argued in ADR-026 §2.
//!
//! ## Frozen
//!
//! Doc 09 §4 lists "`DetRng` algorithm, canonical encoding, `state_hash`" as FROZEN
//! FOREVER. Changing any of it invalidates every stored replay. If it must
//! change, bump [`ENCODING_VERSION`] and write a migration — never edit in place.
//! The stability vectors at the bottom of this file are what make an accidental
//! change fail loudly.

use alloc::vec::Vec;

use serde::Serialize;

use crate::ids::RulesVersion;

/// Prefix on every canonical encoding, little-endian. Bump only with a migration
/// plan — it invalidates every stored byte in the system.
pub const ENCODING_VERSION: u16 = 1;

/// Domain-separation prefix for state hashes. Part of the frozen contract.
pub const STATE_HASH_DOMAIN: &[u8] = b"tabula.state.v1";

/// 32 bytes of blake3 over a canonical encoding.
///
/// Only meaningful **within one `RulesVersion`**: the version is in the preimage,
/// so two versions cannot collide on a structurally identical state, and
/// comparing hashes across versions is a category error rather than a divergence.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, serde::Deserialize,
)]
pub struct StateHash(pub [u8; 32]);

/// Encode a value canonically: `ENCODING_VERSION` prefix, then Postcard.
///
/// The caller is responsible for the type discipline the encoding assumes — no
/// floats, no `HashMap`/`HashSet`, derived `Serialize` only. The type system
/// cannot check it, so `clippy.toml`'s `disallowed-types` list and review do.
///
/// # Errors
/// [`CanonicalError::Encode`] only for genuinely unencodable values — a sequence
/// whose length is not known up front, or a custom `Serialize` that fails. For
/// types that follow the discipline above this is unreachable.
pub fn canonical_encode<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&ENCODING_VERSION.to_le_bytes());
    postcard::to_extend(value, out).map_err(|_| CanonicalError::Encode("postcard serialization"))
}

/// Decode a value written by [`canonical_encode`], **checking the version prefix**.
///
/// Reading the prefix rather than skipping it is the point: a snapshot written
/// under a different `ENCODING_VERSION` must fail loudly here, not silently
/// deserialize into a plausible-looking wrong state. That is the difference
/// between "this replay is unreplayable" (honest, doc 05 §10.2) and a fake
/// replay, which is the one thing doc 05 forbids outright.
///
/// # Errors
/// - [`CanonicalError::Truncated`] if `bytes` is shorter than the prefix.
/// - [`CanonicalError::EncodingVersion`] if the prefix is not [`ENCODING_VERSION`].
/// - [`CanonicalError::Decode`] if the Postcard body does not match `T`.
pub fn canonical_decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, CanonicalError> {
    let (prefix, body) = bytes
        .split_first_chunk::<2>()
        .ok_or(CanonicalError::Truncated)?;

    let found = u16::from_le_bytes(*prefix);
    if found != ENCODING_VERSION {
        return Err(CanonicalError::EncodingVersion {
            found,
            expected: ENCODING_VERSION,
        });
    }

    postcard::from_bytes(body).map_err(|_| CanonicalError::Decode("postcard deserialization"))
}

/// `blake3(STATE_HASH_DOMAIN ‖ rules_version_le ‖ canonical(state))`.
///
/// The rules version is a **typed parameter rather than a free-form tag** so it
/// cannot be omitted. The previous `canonical_hash(tag: &str, ..)` shape allowed
/// exactly that, and the shipped default did it: `canonical_hash("state", state)`
/// hashed every rules version identically, which would have made a legitimate
/// behaviour change indistinguishable from determinism rot. (ADR-026 §2)
///
/// This is the default implementation of `GameRules::state_hash`. Games override
/// it only when the state is large enough that an incremental structural hash is
/// worth the complexity (doc 02 §12.4, tiles) — and then the incremental
/// structure must itself be in the hash, or divergence stops being caught.
///
/// # Panics
/// If `state` cannot be canonically encoded. Unreachable for types that follow
/// the canonical discipline; a game that trips it fails `state_roundtrip` and
/// `determinism_same_inputs` in the conformance suite, long before production.
/// Failing fast is deliberate: the alternative is a silently wrong hash, and a
/// wrong hash is worse than no hash because it makes divergence undetectable.
#[must_use]
pub fn state_hash<T: Serialize + ?Sized>(rules_version: RulesVersion, state: &T) -> StateHash {
    let encoded = canonical_encode(state)
        .expect("canonical state must be encodable; see tabula_core::hash::state_hash");

    let mut hasher = blake3::Hasher::new();
    hasher.update(STATE_HASH_DOMAIN);
    hasher.update(&rules_version.as_u32().to_le_bytes());
    hasher.update(&encoded);
    StateHash(*hasher.finalize().as_bytes())
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CanonicalError {
    #[error("canonical encoding failed: {0}")]
    Encode(&'static str),
    #[error("canonical decoding failed: {0}")]
    Decode(&'static str),
    #[error("canonical blob is shorter than its 2-byte version prefix")]
    Truncated,
    #[error("canonical encoding version {found} is not the supported {expected}")]
    EncodingVersion { found: u16, expected: u16 },
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    #[derive(Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
    struct Board {
        cells: [u8; 4],
        turn: u8,
    }

    // ---------------------------------------------------------------------
    // Stability vectors. FROZEN — doc 09 §4.
    //
    // These literals are the contract. If a dependency bump or an "optimisation"
    // changes them, every stored replay in existence has been invalidated, and
    // this test is the only thing standing between that and a silent production
    // divergence months later. Do not update the expected values to match new
    // output; change the code back, or write the ADR and bump ENCODING_VERSION.
    // ---------------------------------------------------------------------

    #[test]
    fn canonical_encoding_is_stable() {
        let board = Board {
            cells: [0, 1, 2, 3],
            turn: 1,
        };
        // 0x01 0x00      ENCODING_VERSION = 1, little-endian
        // 00 01 02 03    fixed-size array: elements only, no length prefix
        // 01             turn
        assert_eq!(
            canonical_encode(&board).unwrap(),
            vec![0x01, 0x00, 0x00, 0x01, 0x02, 0x03, 0x01]
        );
    }

    #[test]
    fn encoding_version_prefix_is_little_endian() {
        // A one-byte payload, so the prefix is unambiguous.
        assert_eq!(canonical_encode(&7u8).unwrap(), vec![0x01, 0x00, 0x07]);
    }

    #[test]
    fn state_hash_is_stable() {
        let board = Board {
            cells: [0, 1, 2, 3],
            turn: 1,
        };
        let h = state_hash(RulesVersion(1), &board);
        assert_eq!(
            h.0,
            [
                0x58, 0x9f, 0x55, 0x51, 0xdc, 0xa1, 0x90, 0xa9, 0x1b, 0x32, 0x8e, 0x60, 0x29, 0x1b,
                0x8e, 0x7a, 0xda, 0x9a, 0x18, 0x3c, 0xb8, 0x1c, 0x95, 0x23, 0xb8, 0xa7, 0x59, 0xa5,
                0xa3, 0x14, 0xfe, 0x76,
            ],
            "state hash is FROZEN (doc 09 §4) — see the comment above this module's tests"
        );
    }

    /// Rebuilds the documented preimage by hand:
    /// `blake3(STATE_HASH_DOMAIN ‖ rules_version_le ‖ canonical(state))`.
    ///
    /// This is what ties the frozen vector above to the *specification* rather
    /// than to whatever the implementation happened to emit — it would catch a
    /// reordered or omitted prefix that a captured literal alone would not.
    #[test]
    fn state_hash_matches_its_documented_preimage() {
        let board = Board {
            cells: [4, 5, 6, 7],
            turn: 0,
        };

        let mut preimage = Vec::new();
        preimage.extend_from_slice(STATE_HASH_DOMAIN);
        preimage.extend_from_slice(&2u32.to_le_bytes());
        preimage.extend_from_slice(&canonical_encode(&board).unwrap());

        assert_eq!(
            state_hash(RulesVersion(2), &board).0,
            *blake3::hash(&preimage).as_bytes()
        );
    }

    /// Confirms the `blake3` dependency is standard BLAKE3. Published vector for
    /// empty input; if it fails, every frozen value in this crate is suspect.
    #[test]
    fn blake3_dependency_is_standard() {
        assert_eq!(
            blake3::hash(b"").to_hex().as_str(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    // ---------------------------------------------------------------------
    // Properties
    // ---------------------------------------------------------------------

    #[test]
    fn rules_version_separates_the_hash() {
        let board = Board {
            cells: [0, 1, 2, 3],
            turn: 1,
        };
        // The whole reason the version is in the preimage: a structurally
        // identical state under a different rules version must not collide, or a
        // legitimate behaviour change looks like determinism rot.
        assert_ne!(
            state_hash(RulesVersion(1), &board).0,
            state_hash(RulesVersion(2), &board).0
        );
    }

    #[test]
    fn different_states_hash_differently() {
        let a = Board {
            cells: [0, 1, 2, 3],
            turn: 1,
        };
        let b = Board {
            cells: [0, 1, 2, 3],
            turn: 0,
        };
        assert_ne!(
            state_hash(RulesVersion(1), &a).0,
            state_hash(RulesVersion(1), &b).0
        );
    }

    #[test]
    fn encoding_round_trips_and_preserves_the_hash() {
        let board = Board {
            cells: [3, 1, 4, 1],
            turn: 1,
        };
        let bytes = canonical_encode(&board).unwrap();
        let back: Board = canonical_decode(&bytes).unwrap();

        assert_eq!(board, back, "round-trip lost semantic state");
        assert_eq!(
            state_hash(RulesVersion(1), &board).0,
            state_hash(RulesVersion(1), &back).0,
            "round-trip preserved the value but not the hash"
        );
    }

    #[test]
    fn decode_rejects_a_foreign_encoding_version() {
        let mut bytes = canonical_encode(&Board {
            cells: [0; 4],
            turn: 0,
        })
        .unwrap();
        bytes[0] = 0xff; // a blob written by some future ENCODING_VERSION

        // Must fail loudly rather than decode into a plausible wrong state —
        // "unreplayable" is honest, a fake replay is not (doc 05 §10.2).
        assert!(matches!(
            canonical_decode::<Board>(&bytes),
            Err(CanonicalError::EncodingVersion { found: 255, .. })
        ));
    }

    #[test]
    fn decode_rejects_a_truncated_blob() {
        assert!(matches!(
            canonical_decode::<Board>(&[0x01]),
            Err(CanonicalError::Truncated)
        ));
    }

    #[test]
    fn hashing_is_pure() {
        let board = Board {
            cells: [9, 8, 7, 6],
            turn: 0,
        };
        assert_eq!(
            state_hash(RulesVersion(3), &board).0,
            state_hash(RulesVersion(3), &board).0
        );
    }
}
