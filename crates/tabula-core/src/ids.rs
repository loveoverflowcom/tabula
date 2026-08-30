//! Identifiers. (doc 02 §2)
//!
//! Design rule: **newtypes over small integers, not `Uuid` everywhere.** Compact
//! state, fast comparison, deterministic encoding. `Uuid` appears only at platform
//! boundaries (the database, the HTTP API) and is converted at that boundary by
//! `tabula-storage` / `tabula-protocol` — never inside rules. (doc 01 §1.1)
//!
//! Every id here is `Copy + Ord`, so they can be `BTreeMap` keys without a clone
//! and iterate deterministically (I-2).

use alloc::string::String;

use serde::{Deserialize, Serialize};

/// An addressable participant slot in a match.
///
/// Seats are **stable**; occupants are not. A seat outlives the human sitting in
/// it — that is what makes reconnect, substitution, and bot takeover expressible
/// without a second identity system. (doc 00 §13)
///
/// `u8` because werewolf needs ~20 and nothing plausible needs 256.
/// Never invent a game-side "player index" alongside this. (doc 02 §13)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SeatId(pub u8);

/// A registered account. Carries `UUIDv7` bytes; conversion lives at the boundary.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct UserId(pub u128);

/// One instance of one game being played. The unit of ownership, ordering, and
/// persistence. (doc 00 §13)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct MatchId(pub u128);

/// A timer the *game* asked for. Game-scoped: `TimerId(1)` means whatever the
/// game says it means, and two games' timer ids never collide because they never
/// share a match. (doc 00 §6.3)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct TimerId(pub u16);

/// Monotonic per-match counter: **+1 per successfully applied input, and never
/// otherwise** (I-7). Drives reconnect, idempotency, and ordering.
///
/// A rejected input must leave this unchanged — that is half of contract R2.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct StateVersion(pub u64);

/// Position of an input in the match's log.
///
/// Two jobs: it is the event-log row ordinal, and it is the **RNG domain root**
/// (`DetRng::for_input`). That second job is why it must be assigned by the log,
/// not by a counter that could drift. (doc 02 §3.1)
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct InputIndex(pub u64);

/// Process-local connection id. Cheap on purpose — it never leaves the process
/// and never appears in the event log. (doc 03 §4)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SessionId(pub u64);

/// Reverse-DNS game identity, e.g. `com.tabula.chess`. (doc 02 §4.1)
///
/// **No platform crate may compare this against a literal** (I-9). It exists to
/// be looked up in `tabula-registry`, never to be branched on.
/// `xtask check-no-game-ids` greps for exactly that mistake.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct GameId(pub String);

/// Semver of the game *package*: presentation, bots, assets, docs, fixes.
///
/// Distinct from [`RulesVersion`] on purpose — see doc 02 §9.2. A presentation
/// bug fix bumps this and nothing else, and live matches are unaffected.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct GameVersion(pub String);

/// Monotonic integer, bumped on **any** change to `State`/`Command`/`Event`
/// encoding or to `apply`/`project` behaviour. (doc 02 §9.2)
///
/// A match runs exactly one `RulesVersion` for its whole life. Upgrading a
/// running match's rules is not supported and never will be.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RulesVersion(pub u32);

impl RulesVersion {
    /// The version as it enters a hash preimage, little-endian.
    ///
    /// [`crate::hash::state_hash`] takes a `RulesVersion` directly rather than a
    /// free-form tag, so domain separation between two rules versions of one game
    /// is structural: there is no way for a caller to leave the version out.
    /// (ADR-026 §2 — the earlier `tag() -> &'static str` idea could not be
    /// written for a runtime value, and the `&str` shape it was papering over is
    /// what allowed the version-blind default this replaced.)
    #[must_use]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}
