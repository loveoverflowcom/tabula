//! Client-only decoders for existing projected wire shapes (I-5/I-6, ADR-0041).
//! Outside canonical rules source: no rules identity, encoding or behavior change.
use serde::{Deserialize, Deserializer};
use tabula_core::{MatchOutcome, Millis, SeatId};
use crate::{CastlingRights, ClockState, Color, Command, Piece, PieceKind, Square, Status, View, ViewEvent};

#[derive(Deserialize)]
#[serde(remote = "View")]
struct ViewWire {
    #[serde(deserialize_with = "board")]
    board: [Option<Piece>; 64],
    turn: Color,
    castling: CastlingRights,
    en_passant: Option<Square>,
    halfmove_clock: u16,
    fullmove_number: u16,
    status: Status,
    draw_offer: Option<Color>,
    clock: Option<ClockState>,
    you: Option<Color>,
    in_check: bool,
    legal_moves: Vec<Command>,
    actions: Vec<Command>,
}
impl<'de> Deserialize<'de> for View {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> { ViewWire::deserialize(deserializer) }
}
#[derive(Deserialize)]
#[serde(remote = "ViewEvent")]
enum ViewEventWire {
    Moved { seat: SeatId, from: Square, to: Square, promotion: Option<PieceKind>, captured: Option<Piece> },
    ClockUpdated { seat: SeatId, remaining: Millis },
    DrawOffered { seat: SeatId },
    DrawDeclined { seat: SeatId },
    Ended { outcome: MatchOutcome },
}
impl<'de> Deserialize<'de> for ViewEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> { ViewEventWire::deserialize(deserializer) }
}
fn board<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[Option<Piece>; 64], D::Error> {
    struct Board;
    impl<'de> serde::de::Visitor<'de> for Board {
        type Value = [Option<Piece>; 64];
        fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { f.write_str("exactly 64 projected board squares") }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            use serde::de::Error as _;
            if sequence.size_hint().is_some_and(|length| length != 64) { return Err(A::Error::custom("projected board must have 64 squares")); }
            let mut board = [None; 64];
            for square in &mut board { *square = sequence.next_element()?.ok_or_else(|| A::Error::custom("projected board is truncated"))?; }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() { return Err(A::Error::custom("projected board has extra squares")); }
            Ok(board)
        }
    }
    deserializer.deserialize_seq(Board)
}
#[cfg(test)]
mod tests {
    use super::*;
    use tabula_core::{canonical_decode, canonical_encode};
    #[test]
    fn projected_view_bytes_decode_and_reencode_without_wire_change() {
        let view = View { board: [None; 64], turn: Color::White, castling: CastlingRights::initial(), en_passant: None, halfmove_clock: 0, fullmove_number: 1, status: Status::Playing, draw_offer: None, clock: None, you: Some(Color::White), in_check: false, legal_moves: Vec::new(), actions: Vec::new() };
        let bytes = canonical_encode(&view).unwrap();
        let decoded = canonical_decode::<View>(&bytes).unwrap();
        assert_eq!(canonical_encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.board.len(), 64);
        assert_eq!(decoded.you, Some(Color::White));
    }
    #[test]
    fn public_event_variants_preserve_existing_positions_and_fields() {
        for event in [
            ViewEvent::Moved { seat: SeatId(0), from: Square(12), to: Square(28), promotion: None, captured: None },
            ViewEvent::ClockUpdated { seat: SeatId(1), remaining: Millis(9) },
            ViewEvent::DrawOffered { seat: SeatId(0) },
            ViewEvent::DrawDeclined { seat: SeatId(1) },
            ViewEvent::Ended { outcome: MatchOutcome::new(tabula_core::OutcomeKind::Aborted { reason: tabula_core::AbortReason::OperatorCancelled }, smallvec::SmallVec::default(), "cancelled".into()).unwrap() },
        ] {
            let bytes = canonical_encode(&event).unwrap();
            let decoded = canonical_decode::<ViewEvent>(&bytes).unwrap();
            assert_eq!(canonical_encode(&decoded).unwrap(), bytes);
        }
    }
    #[test]
    fn board_length_mismatch_is_rejected() {
        let valid = canonical_encode(&vec![None::<Piece>; 64]).unwrap();
        let valid: BoardOnly = canonical_decode(&valid).unwrap();
        assert_eq!(valid.0.len(), 64);
        let too_short = canonical_encode(&vec![None::<Piece>; 63]).unwrap();
        let result: Result<BoardOnly, _> = canonical_decode(&too_short);
        assert!(result.is_err());
    }
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct BoardOnly(#[serde(deserialize_with = "board")] [Option<Piece>; 64]);
}
