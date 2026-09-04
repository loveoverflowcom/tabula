//! `GameRules` — the functional core. (doc 02 §3)

use serde::{de::DeserializeOwned, Serialize};
use smallvec::SmallVec;
use tabula_core::{state_hash, RuleError, RulesVersion, SeatId, SeatRoster, StateHash, Viewer};

use crate::{
    a11y::A11yDescription,
    ctx::Ctx,
    effect::Effect,
    error::{InitError, MigrateError},
    input::Input,
};

/// The deterministic heart of a game. **Pure, synchronous, total.**
///
/// # The contract
///
/// Tested mechanically by `tabula_testkit::conformance!` (doc 02 §11.1). A game
/// may not be registered until it passes.
///
/// | # | Rule | Test |
/// |---|---|---|
/// | **R1** | `apply` is deterministic: same `(state, input, ctx inputs)` ⇒ same `(state', events, effects)` | `determinism_same_inputs` |
/// | **R2** | `apply` is **transactional**: if it returns `Err`, `state` is byte-identical to before | `error_is_transactional` |
/// | **R3** | `apply` never panics on any input, including hostile and nonsensical ones | `no_panic_on_hostile_input` |
/// | **R4** | `apply` never reads wall-clock time, OS randomness, the environment, or files | I-1 dep ban + clippy |
/// | **R5** | `project` never returns information the viewer is not authorised to know | `projection_hides_secrets` |
/// | **R6** | `view_event` is the only path from `Event` to a client | `view_event_never_bypasses` |
/// | **R7** | All iteration that affects output is over ordered collections | I-2, clippy `disallowed_types` |
/// | **R8** | A rejected input is a **total no-op**: state, `state_version`, and the RNG stream are all unaffected | `error_is_transactional`, `rejection_does_not_disturb_rng` |
///
/// R2 is the one people get wrong. **Validate fully, then mutate.** A rejection
/// that has already half-applied corrupts the match, and the corruption is
/// invisible until a replay diverges weeks later.
///
/// # The transition, precisely
///
/// ```text
/// State  ×  Input<Command>  ×  Ctx { now, index, rng }
///                    ↓
///     Ok(Outcome { events, effects })   — state mutated in place, events ORDERED
///     Err(RuleError)                    — state byte-identical to before (R2)
/// ```
///
/// Event order is part of the contract, not an implementation detail: `events` is
/// a `SmallVec` written to the log verbatim and replayed in that order. Never
/// build it by iterating an unordered collection (R7).
///
/// # Why `&mut State` rather than returning a new state
///
/// Both shapes were re-evaluated in ADR-026 §1 rather than inherited. The
/// deciding point: a pure `apply(&State, ..) -> Result<Transition, _>` gives R2
/// for free, but only in the spelling that rebuilds the whole state per command —
/// the cheap move-based spelling lets a game mutate the moved-in value before
/// returning `Err` and so buys nothing. Carcassonne-style boards and 20-seat
/// werewolf states make that rebuild a permanent per-command cost, and it defeats
/// the incremental structures (union-find feature graphs, zobrist hashes) that
/// `&mut` exists to allow.
///
/// So purity is preserved by contract plus tests rather than by types — and the
/// testkit's R2 check is **not opt-in**: `conformance!` wires
/// `assert_transactional_on_error` into every game, comparing the canonical
/// encoding before and after every rejected input. (doc 02 §3.3, ADR-026 §1)
///
/// # Why there is no `is_terminal`
///
/// Terminality is already expressed by [`Effect::EndMatch`]. A second,
/// independently computed answer to "is this match over" is a divergence source:
/// a game could report `false` after emitting `EndMatch` and the platform would
/// have two authorities. Self-play observes the effect. (ADR-026 §1)
pub trait GameRules: Sized + Send + Sync + 'static {
    /// Canonical, full-information state. **Server-only. Never serialised to a
    /// client** (I-5). The wire has no representation for this type.
    type State: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Player intent. Decoded from opaque wire bytes **by the module**, never by
    /// the platform (ADR-008). That is what lets the gateway route a command for
    /// a game it knows nothing about.
    type Command: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Canonical record of what happened. Full information. Written to the log
    /// verbatim, and therefore the input to replay.
    ///
    /// Emit *semantic* events and let presentation elaborate. One event per pixel
    /// of feedback bloats the log and the replay cost. (doc 02 §13)
    type Event: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Per-viewer redacted state. The **only** state form a client ever sees.
    ///
    /// Must be a *different type* from `State`, not `State` with fields blanked.
    /// Model the knowledge, not the value: `HandSummary { count: u8 }`, never
    /// `Option<Vec<Card>>` set to `None` — one careless refactor fills that in.
    /// (doc 02 §7.1)
    type View: Clone + Serialize + Send + Sync + 'static;

    /// Per-viewer redacted event.
    type ViewEvent: Clone + Serialize + Send + Sync + 'static;

    /// Match creation options chosen in the lobby: time control, variant, role
    /// set. Validated by [`crate::module::GameModule::validate_config`] before
    /// the match exists, so a bad config fails at creation rather than mid-match.
    type Config: Clone + Serialize + DeserializeOwned + Default + Send + Sync + 'static;

    /// Bumped on **any** change to `State`/`Event` encoding or to rule behaviour.
    /// Stored per match; a match runs one version for its whole life. (doc 02 §9.2)
    const RULES_VERSION: RulesVersion;

    /// Build-derived identity of the rules-half source for replay compatibility.
    ///
    /// A zero value means that this game has not supplied an authoritative
    /// identity yet; replay tooling must not call that an exact match. Real
    /// game crates override this with the hash produced by their build script
    /// over their canonical rules-source subtree and [`Self::RULES_VERSION`].
    /// (doc 05 §6.2)
    const RULES_HASH: [u8; 32] = [0; 32];

    /// Build the initial state. May draw randomness (shuffle, role assignment)
    /// from `ctx.rng` — this is the one place most games use RNG at all.
    ///
    /// # Errors
    /// Return [`InitError`] when the roster or config cannot produce a valid
    /// opening position. The lobby has already called `validate_config`, so this
    /// should be rare.
    fn create(
        config: &Self::Config,
        roster: &SeatRoster,
        ctx: &mut Ctx<'_>,
    ) -> Result<Init<Self>, InitError>;

    /// **The single mutation entry point for the entire platform.**
    ///
    /// Every player command, timer expiry, seat lifecycle change, and admin
    /// action arrives here, in one totally ordered stream (ADR-003). There is no
    /// second channel by which state mutates.
    ///
    /// Dispatch to sub-functions per phase. A whole rulebook in one match arm is
    /// untestable. (doc 02 §13)
    ///
    /// # Errors
    /// [`RuleError`] for any rejection. The state must be unchanged (R2).
    fn apply(
        state: &mut Self::State,
        input: Input<Self::Command>,
        ctx: &mut Ctx<'_>,
    ) -> Result<Outcome<Self>, RuleError>;

    /// **THE SECURITY BOUNDARY.** (doc 00 §4.2, ADR-005)
    ///
    /// If a secret can be derived from what a client receives, this function is
    /// broken and it is a *security defect*, not a gameplay bug.
    ///
    /// Consider all four viewer cases explicitly, spectators included. Spectators
    /// seeing hidden hands is the single most common projection bug in board-game
    /// platforms. (doc 02 §7.1)
    fn project(state: &Self::State, viewer: Viewer) -> Self::View;

    /// **The other half of the security boundary.**
    ///
    /// `None` hides the *existence* of the event — use it when even the fact that
    /// something happened is secret (werewolf night actions: timing analysis
    /// alone reveals who acted).
    ///
    /// Prefer *degrading* to hiding where possible: `Drew { seat, card }` becomes
    /// `Drew { seat, card: Hidden }` for other seats, so the card back still flies
    /// across the table and the client has something to animate. (doc 02 §7.2)
    ///
    /// Receives `state_after` so the decision can depend on the post-event world
    /// — after a reveal, the same event becomes fully visible on a later pass.
    fn view_event(
        state_after: &Self::State,
        event: &Self::Event,
        viewer: Viewer,
    ) -> Option<Self::ViewEvent>;

    // ---------------- provided methods: override only when useful --------------

    /// Hints for UI affordances and bots. **Never consulted for authority.**
    ///
    /// Cheap approximations are fine and `Unknown` is legal. Do not call this
    /// from `apply` — that doubles the cost and re-creates the validate/apply
    /// split that doc 02 §3.2 exists to prevent.
    ///
    /// Implementing it is high-leverage: it unlocks move highlighting, drag-drop
    /// legality, a free `Trivial` bot, and self-play fuzzing (doc 02 §6, §11.3).
    fn legal_commands(_state: &Self::State, _seat: SeatId) -> LegalCommands<Self::Command> {
        LegalCommands::Unknown
    }

    /// The canonical semantic hash of `state`. **The only mechanism that detects
    /// determinism drift in production.** (doc 05 §7.2)
    ///
    /// ```text
    /// blake3( b"tabula.state.v1" ‖ RULES_VERSION_le ‖ ENCODING_VERSION_le ‖ postcard(state) )
    /// ```
    ///
    /// The rules version is supplied by the default from `Self::RULES_VERSION`,
    /// not by the author, so it cannot be omitted — two rules versions of one
    /// game can never collide on a structurally identical state. (ADR-026 §2)
    ///
    /// What is hashed is **authoritative semantic state and nothing else**.
    /// `GameId` and `Config` are deliberately excluded (ADR-026 §2); presentation,
    /// animation, and camera state are excluded because I-10 keeps them out of
    /// `State` entirely. A derived cache held *inside* `State` does participate,
    /// and should: a divergent cache is a divergence.
    ///
    /// Override only for huge states where an incremental structural hash pays
    /// for itself (tiles' feature graph — doc 02 §12.4). The incremental
    /// structure must be part of the hash so divergence is still caught.
    fn state_hash(state: &Self::State) -> StateHash {
        state_hash(Self::RULES_VERSION, state)
    }

    /// Accessibility mirror: a text/tree description for screen readers and the
    /// "Board Reader" fallback. (doc 04 §10.4)
    ///
    /// Games without this are flagged in CI and may not be marked accessible in
    /// the catalog.
    fn describe(_state: &Self::State, _viewer: Viewer) -> A11yDescription {
        A11yDescription::unsupported()
    }

    /// Load a snapshot written by an older `RULES_VERSION`.
    ///
    /// The default marks old replays unreplayable, which is the honest answer.
    /// **We never fake a replay** (doc 05 §10.2) — showing a plausible but wrong
    /// reconstruction destroys the audit value of the whole system.
    ///
    /// # Errors
    /// [`MigrateError::Unsupported`] when this version cannot read that one.
    fn migrate(_from: RulesVersion, _bytes: &[u8]) -> Result<Self::State, MigrateError> {
        Err(MigrateError::Unsupported)
    }
}

/// What [`GameRules::create`] produces.
#[derive(Debug)]
pub struct Init<R: GameRules> {
    pub state: R::State,
    pub events: SmallVec<[R::Event; 4]>,
    pub effects: SmallVec<[Effect; 4]>,
}

/// What a successful [`GameRules::apply`] produces.
#[derive(Debug)]
pub struct Outcome<R: GameRules> {
    /// Canonical events, in order. Appended to the log verbatim.
    pub events: SmallVec<[R::Event; 4]>,
    /// Requests to the platform, executed **after** the state transition is
    /// persisted, and therefore required to be idempotent. (doc 03 §7.1)
    pub effects: SmallVec<[Effect; 2]>,
}

impl<R: GameRules> Outcome<R> {
    /// Accepted, nothing happened.
    ///
    /// This is a meaningful rules decision, not a stub: chess returns it for
    /// `Input::Seat { change: Disconnected }` precisely because "do nothing" means
    /// "the clock keeps burning". (doc 02 §12.1)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            events: SmallVec::new(),
            effects: SmallVec::new(),
        }
    }
}

/// UI/bot affordance hints. Never authoritative.
#[derive(Clone, Debug)]
pub enum LegalCommands<C> {
    /// Not computed. Always safe.
    Unknown,
    /// Fully enumerated — chess's ~30 moves. Powers highlighting and a free bot.
    Enumerated(Vec<C>),
    /// Structured hints when enumeration is too large — tiles' (position ×
    /// rotation) pairs. Enough to highlight without listing every command.
    /// (doc 02 §12.4)
    Hints(Vec<CommandHint>),
    /// This seat cannot act right now.
    None,
}

/// A structured legality hint: a game-defined kind plus a canonically encoded
/// game-defined payload.
///
/// # Why the shape stayed opaque
///
/// The requirement is "enough for the client to highlight legal targets
/// without enumerating commands", and doc 02 §12.4 deliberately deferred the
/// shape until a game needed it. Tiles is that game (Phase 3), and writing it
/// did **not** produce a reason to give this type game-shaped fields: tiles
/// groups its legal (position × rotation) pairs into one hint per position,
/// which the opaque `(kind, data)` pair already expresses. A second consumer
/// may still justify structure here; one did not.
///
/// # What writing the first consumer did produce
///
/// The type was previously `#[non_exhaustive]` with public fields, which made
/// it **unconstructible outside this crate** (E0639) — so
/// [`LegalCommands::Hints`] could not be returned by any game at all. The
/// fields are now private behind [`CommandHint::new`], which both fixes that
/// and matches how every other refined type in this workspace is built:
/// evolvable, and with exactly one place a value can come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandHint {
    kind: compact_str::CompactString,
    data: Vec<u8>,
}

impl CommandHint {
    /// A hint of `kind` (e.g. `"place-tile"`) carrying `data`, which the game
    /// is expected to have produced with [`tabula_core::canonical_encode`] so
    /// that its own presentation and bots can decode it again.
    ///
    /// Infallible on purpose: hints are affordances, never authority
    /// (see [`GameRules::legal_commands`]), so a malformed one degrades the
    /// UI rather than the match, and a `Result` here would buy nothing.
    #[must_use]
    pub fn new(kind: impl Into<compact_str::CompactString>, data: Vec<u8>) -> Self {
        Self {
            kind: kind.into(),
            data,
        }
    }

    /// Game-defined kind discriminator.
    #[must_use]
    pub fn kind(&self) -> &str {
        self.kind.as_str()
    }

    /// Game-defined canonically encoded payload.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::CommandHint;

    /// The accessors round-trip what `new` was given. The *cross-crate*
    /// evidence that `LegalCommands::Hints` is usable at all lives in
    /// `games/tiles/tests/rules.rs::legal_commands_hints_decode_to_exactly_the_accepted_placements`
    /// — a same-crate test cannot observe the E0639 that made this type
    /// unconstructible for every game.
    #[test]
    fn command_hint_exposes_the_kind_and_payload_it_was_built_from() {
        let hint = CommandHint::new("place-tile", vec![7, 9]);
        assert_eq!(hint.kind(), "place-tile");
        assert_eq!(hint.data(), &[7, 9]);
    }
}
