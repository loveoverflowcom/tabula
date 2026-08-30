//! Deterministic randomness. (doc 00 §5.2, doc 02 §2)
//!
//! ## The contract
//!
//! One [`MatchSeed`] (32 bytes) is generated **server-side from OS entropy at
//! match creation** and stored in the match record. It is never sent to a client
//! while it could reveal future hidden information, and it is encrypted at rest
//! (doc 03 §19.4). A client that could predict the deck can cheat at every card
//! and dice game we will ever ship — this is the single highest-value secret in
//! the platform.
//!
//! ## Why domain separation matters
//!
//! [`DetRng::for_input`] derives a fresh stream from `(seed, input_index)`. So a
//! given input always draws from the same stream position **regardless of how
//! many draws earlier inputs made**. Without this, adding one `rng.below(6)` to
//! an early rule shifts every subsequent random result in the match, and every
//! stored replay for that game silently diverges.
//!
//! [`DetRng::stream`] then splits further by purpose (`DOMAIN_SHUFFLE`,
//! `DOMAIN_ROLES`, …) so that adding a draw in one subsystem cannot shift another.
//!
//! ## What games must never do
//!
//! - Derive randomness from a state hash, from logical time, or from player input.
//! - Use `rand::SliceRandom::shuffle` — its algorithm is not pinned across crate
//!   versions. Use [`DetRng::shuffle`], which is implemented here precisely so we
//!   own the algorithm forever.
//!
//! **These algorithms are frozen** (doc 09 §4: "Deterministic replay rules —
//! FROZEN FOREVER"). Changing `below` or `shuffle` invalidates every stored
//! replay in existence. If it ever must change, it changes behind a new
//! `ENCODING_VERSION` and a migration, never in place.

use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Conventional domain constants. Games define their own; these are the ones the
/// platform itself uses so they cannot collide with a game's choices.
pub mod domain {
    /// Reserved for the platform. Games start at 1000.
    pub const PLATFORM: u32 = 0;
    /// Suggested game-side base. `DOMAIN_SHUFFLE = domain::GAME_BASE + 1`, etc.
    pub const GAME_BASE: u32 = 1_000;
}

/// The root of all match randomness. 32 bytes of OS entropy, server-side only.
///
/// Not `Debug`-printable on purpose: a seed in a log line is a leaked deck.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSeed([u8; 32]);

impl MatchSeed {
    /// Construct from bytes the *shell* obtained from OS entropy.
    ///
    /// This crate cannot generate a seed itself — `getrandom` is banned here
    /// (I-1/I-4). Generation lives in `tabula-storage`/`services/tabula-server`
    /// at match creation, which is correct: the seed is a platform secret, not a
    /// rules concept.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

// Manual Debug so a seed can never be printed accidentally by a `#[derive(Debug)]`
// on some enclosing struct. (doc 06 §9.3: never log seeds.)
impl core::fmt::Debug for MatchSeed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("MatchSeed(<redacted>)")
    }
}

/// Deterministic, domain-separated PRNG.
///
/// `ChaCha8`: cryptographic-quality stream, stable algorithm across versions and
/// platforms, and fast enough that its cost is irrelevant at board-game rates.
/// (doc 01 §1.1)
pub struct DetRng {
    // Unread until the methods below are implemented in Phase 0. Kept as a field
    // now so the type is honest about what backs it — ChaCha8 is a LOCK NOW
    // decision (doc 01 §1.1), not an implementation detail to be chosen later.
    #[allow(dead_code)]
    inner: ChaCha8Rng,
}

impl core::fmt::Debug for DetRng {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DetRng")
    }
}

impl DetRng {
    /// Root stream for one input application: `blake3(seed || b"input" || index)`.
    ///
    /// TODO(phase 0): implement exactly as specified. Write the stability test in
    /// the same commit — a committed vector of the first 16 `next_u64()` values
    /// for a known seed and index. That test is what stops a future dependency
    /// bump from silently re-seeding every match ever played.
    #[must_use]
    pub fn for_input(_seed: &MatchSeed, _index: crate::ids::InputIndex) -> Self {
        todo!("doc 00 §5.2: blake3(seed || b\"input\" || input_index) -> ChaCha8Rng::from_seed")
    }

    /// Independent substream for a named purpose.
    ///
    /// Take one per subsystem — `stream(DOMAIN_SHUFFLE)` for the deck,
    /// `stream(DOMAIN_ROLES)` for role assignment — so the two cannot interfere.
    #[must_use]
    pub fn stream(&self, _domain: u32) -> Self {
        todo!("doc 00 §5.2: derive a substream keyed by the domain, not by consuming self")
    }

    pub fn next_u32(&mut self) -> u32 {
        todo!("delegate to rand_core::RngCore on self.inner")
    }

    pub fn next_u64(&mut self) -> u64 {
        todo!("delegate to rand_core::RngCore on self.inner")
    }

    /// Uniform in `[0, n)` with **rejection sampling**.
    ///
    /// Rejection, not modulo: modulo bias is small but real, and "small but real"
    /// in a card game is a cheating accusation we cannot disprove. The exact
    /// rejection loop is part of the frozen contract — do not "optimise" it.
    ///
    /// # Panics
    /// Never. `n == 0` returns 0 by convention; callers should not rely on it.
    pub fn below(&mut self, _n: u32) -> u32 {
        todo!("doc 02 §2: pinned rejection-sampling algorithm; add a stability vector test")
    }

    /// Fisher-Yates, implemented **here** rather than via `rand::SliceRandom` so
    /// the algorithm is pinned to this repository and not to a dependency's
    /// changelog. (doc 02 §12.2)
    ///
    /// Iterate `i` from `len-1` down to `1`, swap with `below(i+1)`. Write it
    /// once, test it against a committed vector, never touch it again.
    pub fn shuffle<T>(&mut self, _slice: &mut [T]) {
        todo!("doc 02 §2: pinned Fisher-Yates; add a stability vector test")
    }
}
