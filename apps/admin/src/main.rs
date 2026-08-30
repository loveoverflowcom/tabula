//! # `tabula-admin` — operator tooling
//!
//! > ## PHASE 5
//!
//! A **separate bundle** from `apps/web`, deliberately: operator tooling should
//! not ship to every player, and its authorisation role is different. It reuses
//! `tabula-design` so the two surfaces still look like one product.
//!
//! ## What an operator must be able to do
//!
//! | Task | Needs a deploy? | Why |
//! |---|---|---|
//! | Disable a game | **No** | `rollout.enabled` is a DB row; the manifest is only the default (doc 02 §9.1) |
//! | Restrict to beta / staff / `percentage:10` | **No** | same table |
//! | Hide from the catalog | **No** | same table |
//! | Inspect a live match | No | `Envelope::Inspect`, `Viewer::Audit` only |
//! | Cancel a match | No | `Input::Admin(Cancel)` — the game decides what that means |
//! | Pause / resume | No | only if `capabilities.pausable` |
//! | Review moderation queue | No | Phase 7 |
//!
//! "An admin can disable a game without a deploy" is a **Phase 5 exit
//! criterion**. If disabling a misbehaving game needs a release, the incident
//! lasts as long as the release does.
//!
//! ## Two security requirements, not conveniences
//!
//! 1. **`Viewer::Audit` is never reachable from a game-client session.** It is
//!    authorised by an internal role and every access is logged (doc 00 §9.4).
//! 2. **Match debug dumps redact the seed, even for `Viewer::Audit`**
//!    (doc 03 §21). An operator who can read a seed can predict every shuffle in
//!    that match.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/main.rs      mount + role gate
//! src/rollout.rs   enable/disable, audience targeting, percentage rollout
//! src/matches.rs   live match list, inspect, cancel, force-end
//! src/replays.rs   replay lookup, divergence reports
//! src/users.rs     account status, suspensions
//! src/moderation.rs  Phase 7: chat/abuse queue
//! ```

fn main() {
    eprintln!(
        "tabula-admin is a Phase 5 deliverable (docs/architecture/07-phases-and-implementation-roadmap.md)."
    );
    std::process::exit(1);
}
