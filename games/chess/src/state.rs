//! Chess domain types. Canonical state contains only rules data (I-10).

use serde::{Deserialize, Serialize};
use tabula_core::{LogicalTime, MatchOutcome, Millis, SeatId};

/// A board square using the stable `a1 = 0`, `h8 = 63` representation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Square(pub u8);

impl Square {
    /// Returns a square only for the 64 representable board cells.
    #[must_use]
    pub const fn new(value: u8) -> Option<Self> {
        if value < 64 {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn file(self) -> u8 {
        self.0 % 8
    }

    #[must_use]
    pub const fn rank(self) -> u8 {
        self.0 / 8
    }
}

/// The two chess sides. White is always `SeatId(0)` and Black `SeatId(1)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    White,
    Black,
}

impl Color {
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    #[must_use]
    pub const fn seat(self) -> SeatId {
        match self {
            Self::White => SeatId(0),
            Self::Black => SeatId(1),
        }
    }

    #[must_use]
    pub const fn from_seat(seat: SeatId) -> Option<Self> {
        match seat.0 {
            0 => Some(Self::White),
            1 => Some(Self::Black),
            _ => None,
        }
    }
}

/// A chess piece category. Only Queen/Rook/Bishop/Knight are valid promotions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

/// One occupied cell in [`State::board`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceKind,
}

/// Castling permissions, revoked permanently by king/rook moves and rook captures.
#[allow(clippy::struct_excessive_bools)] // Four independent, FIDE-defined rights; bit flags obscure them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastlingRights {
    pub white_king: bool,
    pub white_queen: bool,
    pub black_king: bool,
    pub black_queen: bool,
}

impl CastlingRights {
    #[must_use]
    pub const fn initial() -> Self {
        Self {
            white_king: true,
            white_queen: true,
            black_king: true,
            black_queen: true,
        }
    }
}

/// A clock configuration reserved for the next Phase 1 clock slice.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockConfig {
    pub initial: Millis,
    pub increment: Millis,
}

/// Canonical clock state. It remains `None` until clock rules are implemented.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockState {
    pub remaining: [Millis; 2],
    pub last_move_at: LogicalTime,
}

/// Lobby configuration. Standard chess is the default and only accepted mode now.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub clock: Option<ClockConfig>,
}

/// A repetition identity: exactly position-defining fields, never counters/offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionKey {
    #[serde(with = "board_serde")]
    pub board: [Option<Piece>; 64],
    pub turn: Color,
    pub castling: CastlingRights,
    pub en_passant: Option<Square>,
}

/// Terminal/non-terminal game status. `Ended` is authoritative terminality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Playing,
    Ended { outcome: MatchOutcome },
}

/// Canonical server-only chess state (doc 02 §12.1).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "board_serde")]
    pub board: [Option<Piece>; 64],
    pub turn: Color,
    pub castling: CastlingRights,
    pub en_passant: Option<Square>,
    pub halfmove_clock: u16,
    pub fullmove_number: u16,
    pub repetition: Vec<PositionKey>,
    pub status: Status,
    pub draw_offer: Option<Color>,
    pub clock: Option<ClockState>,
}

impl State {
    /// The standard opening position.
    #[must_use]
    pub fn initial() -> Self {
        let mut board = [None; 64];
        let back = [
            PieceKind::Rook,
            PieceKind::Knight,
            PieceKind::Bishop,
            PieceKind::Queen,
            PieceKind::King,
            PieceKind::Bishop,
            PieceKind::Knight,
            PieceKind::Rook,
        ];
        for file in 0..8 {
            board[file] = Some(Piece {
                color: Color::White,
                kind: back[file],
            });
            board[8 + file] = Some(Piece {
                color: Color::White,
                kind: PieceKind::Pawn,
            });
            board[48 + file] = Some(Piece {
                color: Color::Black,
                kind: PieceKind::Pawn,
            });
            board[56 + file] = Some(Piece {
                color: Color::Black,
                kind: back[file],
            });
        }
        let mut state = Self {
            board,
            turn: Color::White,
            castling: CastlingRights::initial(),
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            repetition: Vec::new(),
            status: Status::Playing,
            draw_offer: None,
            clock: None,
        };
        state.repetition.push(state.position_key());
        state
    }

    /// Builds a standard FEN position for local perft fixtures.
    pub fn from_fen(fen: &str) -> Result<Self, crate::movegen::FenError> {
        crate::movegen::from_fen(fen)
    }

    /// Position identity for threefold repetition; its en-passant cell is kept
    /// only when a legal en-passant capture exists.
    #[must_use]
    pub fn position_key(&self) -> PositionKey {
        crate::movegen::position_key(self)
    }
}

/// Player intent. `u8` source/target fields deliberately permit hostile wire
/// values; [`crate::ChessRules`] validates them before changing state (R2/R3).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Move {
        from: u8,
        to: u8,
        promotion: Option<PieceKind>,
    },
    Resign,
    OfferDraw,
    AcceptDraw,
    DeclineDraw,
    ClaimDraw,
}

/// Canonical facts written to the match log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Moved {
        seat: SeatId,
        from: Square,
        to: Square,
        promotion: Option<PieceKind>,
        captured: Option<Piece>,
    },
    DrawOffered {
        seat: SeatId,
    },
    DrawDeclined {
        seat: SeatId,
    },
    Ended {
        outcome: MatchOutcome,
    },
}

/// Public Chess state. It is intentionally distinct from [`State`] despite no secrets.
#[derive(Clone, Debug, Serialize)]
pub struct View {
    #[serde(with = "board_serde")]
    pub board: [Option<Piece>; 64],
    pub turn: Color,
    pub castling: CastlingRights,
    pub en_passant: Option<Square>,
    pub halfmove_clock: u16,
    pub fullmove_number: u16,
    pub status: Status,
    pub draw_offer: Option<Color>,
    pub clock: Option<ClockState>,
    pub you: Option<Color>,
    pub legal_moves: Vec<Command>,
}

/// Public event form. Kept distinct to preserve the `view_event` boundary.
#[derive(Clone, Debug, Serialize)]
pub enum ViewEvent {
    Moved {
        seat: SeatId,
        from: Square,
        to: Square,
        promotion: Option<PieceKind>,
        captured: Option<Piece>,
    },
    DrawOffered {
        seat: SeatId,
    },
    DrawDeclined {
        seat: SeatId,
    },
    Ended {
        outcome: MatchOutcome,
    },
}

/// Serde currently implements array traits only through a fixed small length.
/// This local codec preserves the `[Option<Piece>; 64]` domain invariant while
/// rejecting malformed snapshot vectors at the decode boundary.
mod board_serde {
    use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

    use super::Piece;

    pub fn serialize<S>(board: &[Option<Piece>; 64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        board.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[Option<Piece>; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        let board = Vec::<Option<Piece>>::deserialize(deserializer)?;
        board
            .try_into()
            .map_err(|_| D::Error::custom("a chess board must have exactly 64 squares"))
    }
}
