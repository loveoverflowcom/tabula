//! Canonical encoding and state hashing. (doc 05 §7, ADR-021)
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
//! canonical(x) = postcard(x) with:
//!    - the type's DERIVED Serialize (never a custom human-friendly impl)
//!    - a 2-byte encoding-version prefix
//!    - all maps as BTreeMap (sorted keys) — HashMap is banned in these types (I-2)
//!    - no floats anywhere (doc 00 §5.1)
//! ```
//!
//! Used for `match_inputs.payload`, `match_events.payload`,
//! `match_snapshots.payload`, `.tbr` replay files, and everything hashed.
//!
//! **Always Postcard, regardless of the connection codec.** JSON is a transport
//! convenience, never a storage format — its key ordering and float formatting
//! are not stable enough to hash. (doc 05 §4.3)
//!
//! ## Frozen
//!
//! Doc 09 §4 lists "`DetRng` algorithm, canonical encoding, `state_hash`" as FROZEN
//! FOREVER. Changing any of it invalidates every stored replay. If it must
//! change, bump [`ENCODING_VERSION`] and write a migration — never edit in place.

use serde::Serialize;

/// Prefix on every canonical encoding. Bump only with a migration plan.
pub const ENCODING_VERSION: u16 = 1;

/// Domain-separation prefix for state hashes. Part of the frozen contract.
pub const STATE_HASH_DOMAIN: &[u8] = b"tabula.state.v1";

/// 32 bytes of blake3 over a canonical encoding.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, serde::Deserialize,
)]
pub struct StateHash(pub [u8; 32]);

/// Encode a value canonically.
///
/// TODO(phase 0): postcard with the `ENCODING_VERSION` prefix. Reject at review
/// any type reaching here that contains a float or a `HashMap` — the type system
/// cannot catch it, so the review and the clippy `disallowed_types` list must.
///
/// # Errors
/// Returns an error only for genuinely unencodable values (e.g. a sequence whose
/// length is not known); in practice this is infallible for canonical types.
pub fn canonical_encode<T: Serialize>(_value: &T) -> Result<alloc::vec::Vec<u8>, CanonicalError> {
    todo!("doc 05 §7.1: ENCODING_VERSION prefix || postcard::to_allocvec(value)")
}

/// `blake3(STATE_HASH_DOMAIN || tag || canonical(value))`.
///
/// The `tag` carries the rules version so two rules versions of the same game
/// can never produce a colliding hash for structurally identical states — which
/// would make a legitimate behaviour change look like determinism rot.
///
/// This is the default implementation of `GameRules::state_hash`. Games override
/// it only when the state is large enough that an incremental structural hash is
/// worth the complexity (doc 02 §12.4, tiles).
#[must_use]
pub fn canonical_hash<T: Serialize>(_tag: &str, _value: &T) -> StateHash {
    todo!("doc 05 §7.2: blake3(STATE_HASH_DOMAIN || tag || canonical(value))")
}

#[derive(Debug, thiserror::Error)]
pub enum CanonicalError {
    #[error("canonical encoding failed: {0}")]
    Encode(&'static str),
}
