//! `State`, `Command`, `Event`, `View`, `ViewEvent`, `Config`. (doc 02 §10.2)
//!
//! Read this file as the template for every game: **six types, in one file,
//! before any logic.** Getting these right is most of the work; `apply` is
//! usually the easy part once the types say the right thing.

use serde::{Deserialize, Serialize};
use tabula_core::{MatchOutcome, SeatId};

/// Canonical, full-information state. Server-only, never on the wire (I-5).
///
/// Fixed-size array rather than `Vec`: smaller canonical encoding, no length
/// prefix, and no way to represent an invalid board size.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub board: [Option<Mark>; 9],
    pub turn: SeatId,
    pub status: Status,
    pub moves: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mark {
    X,
    O,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Playing,
    Won(SeatId),
    Drawn,
}

/// Player intent. Decoded from opaque wire bytes by this crate, never by the
/// platform (ADR-008).
///
/// Note there is no `Command::Debug*` variant. Shipping one ships an exploit —
/// test-only commands go behind `#[cfg(test)]` and are excluded from the decoder.
/// (doc 02 §13)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Place { cell: u8 },
    Resign,
}

/// Canonical record of what happened. Written to the log verbatim.
///
/// Semantic events, not per-pixel feedback: "Placed", not "`PieceStartedMoving`" +
/// "`PieceArrived`" + "`SoundPlayed`". Presentation elaborates. (doc 02 §13)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Placed { seat: SeatId, cell: u8, mark: Mark },
    Ended { outcome: MatchOutcome },
}

/// Per-viewer redacted state.
///
/// **Nothing is hidden in tic-tac-toe, and it is still a distinct type** — see
/// doc 02 §7.1. Two reasons that matter even here:
///
/// 1. `View` omits `moves`, an implementation detail the client has no use for.
/// 2. `View` adds `you`, which only makes sense per-viewer.
///
/// The habit is the point. A game that starts with `type View = State` and later
/// grows a secret has to redesign its projection under pressure.
#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub board: [Option<Mark>; 9],
    pub turn: SeatId,
    pub status: Status,
    /// `None` for spectators. Drives "your turn" affordances client-side.
    pub you: Option<SeatId>,
}

/// No redaction needed, so the view event is the canonical event.
///
/// This alias is only correct because `hidden_information = false`. Any game with
/// secrets needs a genuinely separate type — see doc 02 §12.2 for what that looks
/// like when it is real.
pub type ViewEvent = Event;

/// Lobby-chosen options.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Per-move deadline. Enforced by `Effect::SetTimer` + `Input::Timer`,
    /// never by reading a clock (doc 02 §13).
    pub move_timeout_ms: u64,
}
