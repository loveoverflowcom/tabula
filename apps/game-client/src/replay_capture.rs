//! Canonical replay evidence — the attempt/accepted distinction
//! [`LocalMatch`](crate::LocalMatch) produces from real gameplay (doc 05 §8).
//!
//! # Two collections, two invariants
//!
//! - [`RecordedInput`] is the **attempt audit**: one entry per canonical
//!   input attempt that reached `GameRules::apply`, whether accepted or
//!   rejected. Every attempt consumes a unique [`InputIndex`], so this log's
//!   indices are contiguous from 1.
//! - [`AcceptedReplayInput`], collected into a [`LocalReplayTrace`], is
//!   **replay evidence**: one entry per attempt `GameRules::apply` actually
//!   *accepted*. A rejected attempt still consumed an `InputIndex` —
//!   `DetRng::for_input(seed, index)` derivation depends on the original
//!   value — so this log's indices can, and after any rejection *must*, be
//!   non-contiguous. Renumbering them to close the gap would silently shift
//!   every later accepted input onto the wrong RNG-domain stream position: a
//!   determinism bug invisible in the visible input sequence.
//!
//! These are deliberately two types rather than one struct with an
//! `accepted: bool` flag: a replay consumer that wants "the accepted
//! transitions" should not have to filter and re-derive an invariant
//! (contiguity does NOT hold; checkpoint hashes only exist here) that the
//! type system can just assert once.
//!
//! # What this module does not do
//!
//! It does not choose a replay file format, does not perform I/O, and does
//! not decide whether a session becomes a `.tbr` — see
//! `crates/tabula-testkit/src/replay.rs` for the existing `.tbr`
//! reader/writer, which this module does not depend on or duplicate. Both
//! types here are produced only inside `LocalMatch::apply_canonical`, at the
//! instant `GameRules::apply` returns, independent of presentation,
//! rendering, or effect interpretation succeeding.

use tabula_core::{InputIndex, LogicalTime, StateHash};
use tabula_game_api::Input;

/// One recorded canonical input **attempt**, whether `GameRules::apply`
/// accepted or rejected it — the attempt-audit log `LocalMatch` has kept
/// since Phase 2 (doc 00 §3.1).
///
/// The typed input is deterministic replay-adjacent data: `Input` and its
/// game command are canonical serializable values. This runtime deliberately
/// does not choose a replay file format, and does not itself decide which
/// attempts are canonical replay frames — see [`AcceptedReplayInput`].
#[derive(Clone, Debug)]
pub struct RecordedInput<C> {
    /// The unique input-log/RNG-domain ordinal consumed by this attempt.
    pub index: InputIndex,
    /// The monotonic logical time supplied to the rules transition.
    pub now: LogicalTime,
    /// The complete canonical input, including timer and bot-originated input.
    pub input: Input<C>,
}

/// One canonical input `GameRules::apply` **accepted**, carrying the
/// post-transition checkpoint hash — the unit of independently-replayable
/// evidence.
///
/// This is not a general-purpose DTO. It is evidence that a specific,
/// trusted transition boundary (`LocalMatch::apply_canonical`) actually
/// observed. Construction is restricted to the parent local-runtime module so
/// sibling modules cannot assert a replay fact that was never actually
/// accepted live — a proof barrier, not merely another struct. Read access is
/// unrestricted: a caller may inspect every field to build a `.tbr` or any
/// other export; it just cannot fabricate one with
/// `AcceptedReplayInput::new(arbitrary_index, arbitrary_hash, ...)`.
#[derive(Clone, Debug)]
pub struct AcceptedReplayInput<C> {
    index: InputIndex,
    now: LogicalTime,
    input: Input<C>,
    state_hash: StateHash,
}

impl<C> AcceptedReplayInput<C> {
    pub(super) const fn new(
        index: InputIndex,
        now: LogicalTime,
        input: Input<C>,
        state_hash: StateHash,
    ) -> Self {
        Self {
            index,
            now,
            input,
            state_hash,
        }
    }

    /// The original attempt index this input was accepted at. May be
    /// non-contiguous with a neighboring entry — see the module docs.
    #[must_use]
    pub const fn index(&self) -> InputIndex {
        self.index
    }

    /// The exact logical time `GameRules::apply` used for this transition —
    /// never a later frame-arrival time (this crate's timer-fidelity
    /// contract: a due timer replays at its deadline, not the sampled frame
    /// that happened to observe it).
    #[must_use]
    pub const fn now(&self) -> LogicalTime {
        self.now
    }

    /// The canonical input, exactly as `GameRules::apply` accepted it.
    #[must_use]
    pub const fn input(&self) -> &Input<C> {
        &self.input
    }

    /// The canonical state hash immediately after this transition.
    #[must_use]
    pub const fn state_hash(&self) -> StateHash {
        self.state_hash
    }
}

/// Deterministic replay evidence accumulated over one live
/// [`LocalMatch`](crate::LocalMatch) session: enough to reconstruct exactly
/// what the canonical rules accepted, independent of presentation.
///
/// Never carries canonical `State`, a `Viewer`, a `View`, a `ViewEvent`, or
/// any presentation fact — replay evidence and the projection boundary are
/// different concerns (I-5). Nothing in this type, or in
/// [`AcceptedReplayInput`], can be constructed outside this crate with
/// values that were never actually accepted by a live transition.
#[derive(Clone, Debug)]
pub struct LocalReplayTrace<C> {
    initial_state_hash: StateHash,
    accepted: Vec<AcceptedReplayInput<C>>,
}

impl<C> LocalReplayTrace<C> {
    pub(crate) const fn new(initial_state_hash: StateHash) -> Self {
        Self {
            initial_state_hash,
            accepted: Vec::new(),
        }
    }

    pub(super) fn record(&mut self, entry: AcceptedReplayInput<C>) {
        self.accepted.push(entry);
    }

    /// The canonical state hash immediately after `GameRules::create`,
    /// before any input was accepted — distinguishes a divergence at
    /// creation from a divergence at the first accepted input.
    #[must_use]
    pub const fn initial_state_hash(&self) -> StateHash {
        self.initial_state_hash
    }

    /// Every accepted canonical transition, in acceptance order. Indices may
    /// be non-contiguous; see the module docs on why that must be preserved,
    /// never compacted.
    #[must_use]
    pub fn accepted_inputs(&self) -> &[AcceptedReplayInput<C>] {
        &self.accepted
    }

    /// The canonical state hash after the most recent accepted transition,
    /// or [`initial_state_hash`](Self::initial_state_hash) if none has been
    /// accepted yet.
    ///
    /// The single authority for "the current canonical checkpoint": always
    /// derived from `accepted_inputs`, never tracked as a second field that
    /// could drift from it.
    #[must_use]
    pub fn final_state_hash(&self) -> StateHash {
        self.accepted
            .last()
            .map_or(self.initial_state_hash, AcceptedReplayInput::state_hash)
    }
}
