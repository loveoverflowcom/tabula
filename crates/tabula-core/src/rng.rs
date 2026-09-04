//! Deterministic randomness. (doc 00 §5.2, doc 02 §2, ADR-026 §4)
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
//! ## Derivation
//!
//! ```text
//! for_input(seed, index) = from_key( blake3( seed     ‖ b"input"  ‖ index_le  ) )
//! stream(&self, domain)  = from_key( blake3( self.key ‖ b"stream" ‖ domain_le ) )
//! from_key(k)            = ChaCha8Rng::from_seed(k), remembering k
//! ```
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
//! It takes `&self` and derives from the retained key, so it **never consumes the
//! parent** and substreams compose to any depth.
//!
//! ## Rejected inputs need no rewind (contract R8)
//!
//! Because each input's stream is derived from `(seed, index)` alone, the number
//! of draws made while applying input *N* cannot affect input *N+1*. A game that
//! drew from `ctx.rng` and then returned `Err` has consumed nothing any later
//! input can observe, so a rejection is a total no-op without any rollback
//! machinery. This property is why per-input derivation is not negotiable — it
//! would not hold for a single match-long stream. (ADR-026 §5)
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
//! `ENCODING_VERSION` and a migration, never in place. The stability vectors at
//! the bottom of this file are what make an accidental change fail loudly —
//! including a `rand_chacha` bump that silently altered the stream.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};

/// The size of `next_u32`'s output space. Not representable in `u32`, which is
/// why [`DetRng::below`]'s rejection bound is computed in `u64`.
const U32_RANGE: u64 = 1 << 32;

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
///
/// `ChaCha8Rng` is reached **only** through this wrapper, so `rand_chacha`'s
/// semantics cannot change under us without failing the stability vectors below.
#[derive(Clone)]
pub struct DetRng {
    /// The 32 bytes `inner` was seeded from. Retained so [`DetRng::stream`] can be
    /// a pure derivation from the root rather than a draw — which is what letting
    /// it take `&self` requires, and what makes substreams composable.
    ///
    /// Private and never exposed: it is seed-derived material.
    key: [u8; 32],
    inner: ChaCha8Rng,
}

impl core::fmt::Debug for DetRng {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never render `key` — it is seed-derived material, and a substream key
        // plus a known domain narrows the search for the root seed.
        f.write_str("DetRng")
    }
}

impl DetRng {
    fn from_key(key: [u8; 32]) -> Self {
        Self {
            inner: ChaCha8Rng::from_seed(key),
            key,
        }
    }

    /// Root stream for one input application: `blake3(seed || b"input" || index_le)`.
    #[must_use]
    pub fn for_input(seed: &MatchSeed, index: crate::ids::InputIndex) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(seed.as_bytes());
        hasher.update(b"input");
        hasher.update(&index.0.to_le_bytes());
        Self::from_key(*hasher.finalize().as_bytes())
    }

    /// Independent substream for a named purpose:
    /// `blake3(self.key || b"stream" || domain_le)`.
    ///
    /// Take one per subsystem — `stream(DOMAIN_SHUFFLE)` for the deck,
    /// `stream(DOMAIN_ROLES)` for role assignment — so the two cannot interfere.
    /// Takes `&self`, so it derives rather than draws: calling it does not move
    /// the parent stream, and a substream can be split again.
    #[must_use]
    pub fn stream(&self, domain: u32) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.key);
        hasher.update(b"stream");
        hasher.update(&domain.to_le_bytes());
        Self::from_key(*hasher.finalize().as_bytes())
    }

    pub fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    pub fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    /// Uniform in `[0, n)` with **rejection sampling**.
    ///
    /// Rejection, not modulo: modulo bias is small but real, and "small but real"
    /// in a card game is a cheating accusation we cannot disprove. The exact
    /// rejection loop is part of the frozen contract — do not "optimise" it.
    ///
    /// `zone` is the largest multiple of `n` that fits in the 2^32 output space;
    /// draws at or above it are discarded, so every residue is equally likely.
    /// The arithmetic is done in `u64` because 2^32 is not representable in `u32`.
    ///
    /// # Panics
    /// Never. `n == 0` returns 0 by convention; callers should not rely on it.
    pub fn below(&mut self, n: u32) -> u32 {
        if n <= 1 {
            return 0;
        }
        let zone = U32_RANGE - (U32_RANGE % u64::from(n));
        loop {
            let x = self.next_u32();
            if u64::from(x) < zone {
                return x % n;
            }
        }
    }

    /// Fisher-Yates, implemented **here** rather than via `rand::SliceRandom` so
    /// the algorithm is pinned to this repository and not to a dependency's
    /// changelog. (doc 02 §12.2)
    ///
    /// Iterates `i` from `len-1` down to `1`, swapping with `below(i+1)`.
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        // A slice longer than u32::MAX cannot be shuffled by this pinned
        // algorithm, and no deck, tile bag, or role list comes close. Returning
        // early rather than truncating keeps the function total.
        if slice.len() < 2 || u32::try_from(slice.len()).is_err() {
            return;
        }
        for i in (1..slice.len()).rev() {
            // `i < len <= u32::MAX`, so both conversions are exact.
            let bound = u32::try_from(i + 1).unwrap_or(u32::MAX);
            let j = self.below(bound) as usize;
            slice.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;
    use crate::ids::InputIndex;

    const SEED: MatchSeed = MatchSeed::from_bytes([7u8; 32]);

    // ---------------------------------------------------------------------
    // Stability vectors. FROZEN — doc 09 §4.
    //
    // These literals ARE the contract. A `rand_chacha` bump, a "cleaner"
    // rejection loop, or a reordered blake3 preimage all change them, and every
    // stored replay in existence is invalidated the moment they do. This test is
    // the only thing that catches it. Do not update the expected values to match
    // new output; change the code back, or write the ADR and bump
    // ENCODING_VERSION with a migration.
    // ---------------------------------------------------------------------

    #[test]
    fn for_input_stream_is_stable() {
        let mut rng = DetRng::for_input(&SEED, InputIndex(0));
        let got: Vec<u64> = (0..16).map(|_| rng.next_u64()).collect();
        assert_eq!(got, VECTOR_FOR_INPUT_0, "DetRng is FROZEN (doc 09 §4)");
    }

    #[test]
    fn below_is_stable() {
        let mut rng = DetRng::for_input(&SEED, InputIndex(1));
        let got: Vec<u32> = (0..24).map(|_| rng.below(6)).collect();
        assert_eq!(got, VECTOR_BELOW_6, "DetRng::below is FROZEN (doc 09 §4)");
    }

    #[test]
    fn shuffle_is_stable() {
        let mut deck: Vec<u8> = (0..13).collect();
        let mut rng = DetRng::for_input(&SEED, InputIndex(2)).stream(domain::GAME_BASE + 1);
        rng.shuffle(&mut deck);
        assert_eq!(deck, VECTOR_SHUFFLE_13, "shuffle is FROZEN (doc 09 §4)");
    }

    // ---------------------------------------------------------------------
    // Properties
    // ---------------------------------------------------------------------

    #[test]
    fn same_seed_and_index_reproduce_the_stream() {
        let mut a = DetRng::for_input(&SEED, InputIndex(42));
        let mut b = DetRng::for_input(&SEED, InputIndex(42));
        for _ in 0..64 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_index_gives_a_different_stream() {
        let mut a = DetRng::for_input(&SEED, InputIndex(1));
        let mut b = DetRng::for_input(&SEED, InputIndex(2));
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn different_seed_gives_a_different_stream() {
        let other = MatchSeed::from_bytes([9u8; 32]);
        let mut a = DetRng::for_input(&SEED, InputIndex(1));
        let mut b = DetRng::for_input(&other, InputIndex(1));
        assert_ne!(a.next_u64(), b.next_u64());
    }

    /// The property the whole design exists for: draws in one subsystem must not
    /// shift another. This is what lets a game add a `rng.below(6)` to an early
    /// rule without invalidating every stored replay.
    #[test]
    fn substreams_are_independent_and_do_not_consume_the_parent() {
        let root = DetRng::for_input(&SEED, InputIndex(3));

        let mut shuffle = root.stream(1001);
        let mut roles_before = root.stream(1002);
        let roles_expected: Vec<u64> = (0..8).map(|_| roles_before.next_u64()).collect();

        // Draw heavily from one substream...
        for _ in 0..1000 {
            let _ = shuffle.next_u64();
        }

        // ...the other is unaffected, and the parent was never consumed.
        let mut roles_after = root.stream(1002);
        let roles_actual: Vec<u64> = (0..8).map(|_| roles_after.next_u64()).collect();
        assert_eq!(roles_expected, roles_actual);
    }

    #[test]
    fn substreams_compose_to_any_depth() {
        let root = DetRng::for_input(&SEED, InputIndex(4));
        let mut deep = root.stream(1).stream(2).stream(3);
        let mut same = root.stream(1).stream(2).stream(3);
        assert_eq!(deep.next_u64(), same.next_u64());

        let mut different = root.stream(1).stream(3).stream(2);
        assert_ne!(deep.next_u64(), different.next_u64());
    }

    #[test]
    fn below_respects_its_bound() {
        let mut rng = DetRng::for_input(&SEED, InputIndex(5));
        for n in 1..40u32 {
            for _ in 0..50 {
                assert!(rng.below(n) < n, "below({n}) escaped its bound");
            }
        }
    }

    #[test]
    fn below_degenerate_bounds_are_total() {
        let mut rng = DetRng::for_input(&SEED, InputIndex(6));
        assert_eq!(rng.below(0), 0);
        assert_eq!(rng.below(1), 0);
    }

    #[test]
    fn below_is_not_visibly_biased() {
        // Not a statistical proof — a smoke test that the rejection loop did not
        // collapse to a constant or drop a residue class entirely.
        let mut rng = DetRng::for_input(&SEED, InputIndex(7));
        let mut counts = [0u32; 6];
        for _ in 0..60_000 {
            counts[rng.below(6) as usize] += 1;
        }
        for (face, count) in counts.iter().enumerate() {
            assert!(
                (9_000..11_000).contains(count),
                "face {face} came up {count} times in 60000 rolls of a d6"
            );
        }
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut rng = DetRng::for_input(&SEED, InputIndex(8));
        for len in 0..40usize {
            let mut deck: Vec<usize> = (0..len).collect();
            rng.shuffle(&mut deck);
            let mut sorted = deck.clone();
            sorted.sort_unstable();
            assert_eq!(
                sorted,
                (0..len).collect::<Vec<_>>(),
                "len {len} lost a card"
            );
        }
    }

    #[test]
    fn shuffle_of_short_slices_is_total() {
        let mut rng = DetRng::for_input(&SEED, InputIndex(9));
        let mut empty: [u8; 0] = [];
        rng.shuffle(&mut empty);
        let mut single = [1u8];
        rng.shuffle(&mut single);
        assert_eq!(single, [1]);
    }

    /// The derivation is documented as `blake3(seed ‖ b"input" ‖ index_le)`.
    /// Rebuilding that preimage by hand here proves the *composition and order*
    /// are what the docs claim, so the frozen vectors above are not merely
    /// "whatever the code happened to emit".
    #[test]
    fn for_input_matches_its_documented_preimage() {
        let mut preimage = Vec::new();
        preimage.extend_from_slice(SEED.as_bytes());
        preimage.extend_from_slice(b"input");
        preimage.extend_from_slice(&11u64.to_le_bytes());
        let expected_key = blake3::hash(&preimage);

        let mut from_docs = DetRng::from_key(*expected_key.as_bytes());
        let mut from_api = DetRng::for_input(&SEED, InputIndex(11));
        assert_eq!(from_docs.next_u64(), from_api.next_u64());
    }

    /// Likewise for `blake3(self.key ‖ b"stream" ‖ domain_le)`.
    #[test]
    fn stream_matches_its_documented_preimage() {
        let root = DetRng::for_input(&SEED, InputIndex(12));

        let mut preimage = Vec::new();
        preimage.extend_from_slice(&root.key);
        preimage.extend_from_slice(b"stream");
        preimage.extend_from_slice(&1001u32.to_le_bytes());
        let expected_key = blake3::hash(&preimage);

        let mut from_docs = DetRng::from_key(*expected_key.as_bytes());
        let mut from_api = root.stream(1001);
        assert_eq!(from_docs.next_u64(), from_api.next_u64());
    }

    /// Confirms the `blake3` dependency is standard BLAKE3 and not something that
    /// merely calls itself that. This is the published vector for empty input; if
    /// it ever fails, every frozen value in this crate is suspect.
    #[test]
    fn blake3_dependency_is_standard() {
        assert_eq!(
            blake3::hash(b"").to_hex().as_str(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    // Captured from the pinned algorithm above, then frozen. The two
    // `matches_its_documented_preimage` tests are what tie these numbers to the
    // specification rather than to the implementation.
    const VECTOR_FOR_INPUT_0: [u64; 16] = [
        5_752_095_252_383_972_965,
        16_303_605_752_607_551_179,
        72_207_660_447_056_274,
        481_734_133_635_975_366,
        11_766_688_698_697_978_466,
        6_599_567_892_713_766_936,
        7_603_945_128_116_586_449,
        2_393_136_378_176_873_240,
        14_562_788_512_568_573_072,
        13_398_465_651_320_855_040,
        14_843_202_601_514_672_510,
        4_428_321_696_437_434_379,
        16_910_956_866_707_019_063,
        5_670_090_176_197_503_859,
        5_145_105_673_920_517_076,
        14_263_003_136_144_502_859,
    ];
    const VECTOR_BELOW_6: [u32; 24] = [
        0, 5, 0, 5, 4, 3, 0, 0, 4, 0, 2, 1, 5, 4, 2, 5, 4, 1, 0, 3, 3, 0, 5, 0,
    ];
    const VECTOR_SHUFFLE_13: [u8; 13] = [3, 9, 11, 8, 2, 6, 4, 7, 12, 10, 0, 5, 1];
}
