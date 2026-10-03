//! Deterministic pseudo-legal generation, attack detection, and perft.

use std::{error::Error, fmt};

use super::state::{CastlingRights, Color, Piece, PieceKind, PositionKey, Square, State, Status};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Move {
    pub from: Square,
    pub to: Square,
    pub promotion: Option<PieceKind>,
    pub en_passant_capture: Option<Square>,
    pub rook: Option<(Square, Square)>,
}

/// A local FEN parsing error. FEN exists only to make independently published
/// perft positions convenient test fixtures; it is not a wire protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FenError(&'static str);

impl fmt::Display for FenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for FenError {}

impl State {
    /// Builds a standard FEN position for local perft fixtures.
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        from_fen(fen)
    }

    /// Position identity for repetition claims; its en-passant cell is kept
    /// only when a legal en-passant capture exists.
    #[must_use]
    pub fn position_key(&self) -> PositionKey {
        position_key(self)
    }
}

/// Parses a six-field standard FEN into canonical chess state.
pub fn from_fen(fen: &str) -> Result<State, FenError> {
    let mut fields = fen.split_whitespace();
    let placement = fields.next().ok_or(FenError("missing piece placement"))?;
    let turn = match fields.next().ok_or(FenError("missing side to move"))? {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err(FenError("invalid side to move")),
    };
    let castling_field = fields.next().ok_or(FenError("missing castling rights"))?;
    let ep_field = fields.next().ok_or(FenError("missing en-passant square"))?;
    let halfmove_clock = fields
        .next()
        .ok_or(FenError("missing halfmove clock"))?
        .parse()
        .map_err(|_| FenError("invalid halfmove clock"))?;
    let fullmove_number = fields
        .next()
        .ok_or(FenError("missing fullmove number"))?
        .parse()
        .map_err(|_| FenError("invalid fullmove number"))?;
    if fullmove_number == 0 {
        return Err(FenError("fullmove number must be at least one"));
    }
    if fields.next().is_some() {
        return Err(FenError("too many FEN fields"));
    }

    let board = parse_board(placement)?;
    let castling = CastlingRights {
        white_king: castling_field.contains('K'),
        white_queen: castling_field.contains('Q'),
        black_king: castling_field.contains('k'),
        black_queen: castling_field.contains('q'),
    };
    if !valid_castling_field(castling_field) {
        return Err(FenError("invalid castling rights"));
    }
    let en_passant = if ep_field == "-" {
        None
    } else {
        parse_square(ep_field)?
    };
    let mut state = State {
        board,
        turn,
        castling,
        en_passant,
        halfmove_clock,
        fullmove_number,
        repetition: Vec::new(),
        status: Status::Playing,
        draw_offer: None,
        clock: None,
    };
    if king_count(&state, Color::White) != 1 || king_count(&state, Color::Black) != 1 {
        return Err(FenError("position must contain one king of each color"));
    }
    if state.board.iter().enumerate().any(|(square, piece)| {
        piece.is_some_and(|piece| {
            piece.kind == PieceKind::Pawn && (square / 8 == 0 || square / 8 == 7)
        })
    }) {
        return Err(FenError("pawns cannot occupy a promotion rank"));
    }
    if kings_are_adjacent(&state) || in_check(&state, turn.other()) {
        return Err(FenError("position has an invalid king placement"));
    }
    if !castling_rights_match_board(&state, castling_field) {
        return Err(FenError("castling rights do not match the board"));
    }
    if !en_passant_matches_board(&state) {
        return Err(FenError("en-passant target does not match the board"));
    }
    state.repetition.push(position_key(&state));
    Ok(state)
}

fn parse_board(placement: &str) -> Result<[Option<Piece>; 64], FenError> {
    let mut board = [None; 64];
    let ranks: Vec<_> = placement.split('/').collect();
    if ranks.len() != 8 {
        return Err(FenError("piece placement must contain eight ranks"));
    }
    for (fen_rank, encoded) in ranks.iter().enumerate() {
        let mut file = 0u8;
        for ch in encoded.chars() {
            if let Some(gap) = ch.to_digit(10) {
                if gap == 0 || gap > 8 {
                    return Err(FenError("invalid empty-square count"));
                }
                let gap = u8::try_from(gap).map_err(|_| FenError("invalid empty-square count"))?;
                file = file.checked_add(gap).ok_or(FenError("rank too wide"))?;
                continue;
            }
            let piece = fen_piece(ch).ok_or(FenError("invalid piece character"))?;
            if file >= 8 {
                return Err(FenError("rank too wide"));
            }
            board[(7 - fen_rank) * 8 + usize::from(file)] = Some(piece);
            file += 1;
        }
        if file != 8 {
            return Err(FenError("rank does not contain eight squares"));
        }
    }
    Ok(board)
}

fn fen_piece(ch: char) -> Option<Piece> {
    let color = if ch.is_ascii_uppercase() {
        Color::White
    } else {
        Color::Black
    };
    let kind = match ch.to_ascii_lowercase() {
        'p' => PieceKind::Pawn,
        'n' => PieceKind::Knight,
        'b' => PieceKind::Bishop,
        'r' => PieceKind::Rook,
        'q' => PieceKind::Queen,
        'k' => PieceKind::King,
        _ => return None,
    };
    Some(Piece { color, kind })
}

fn valid_castling_field(field: &str) -> bool {
    if field == "-" {
        return true;
    }
    let mut seen = 0u8;
    for (flag, bit) in [('K', 1), ('Q', 2), ('k', 4), ('q', 8)] {
        if field.matches(flag).count() > 1 {
            return false;
        }
        if field.contains(flag) {
            seen |= bit;
        }
    }
    seen != 0 && field.chars().all(|ch| matches!(ch, 'K' | 'Q' | 'k' | 'q'))
}

fn parse_square(value: &str) -> Result<Option<Square>, FenError> {
    let bytes = value.as_bytes();
    if bytes.len() != 2 || !(b'a'..=b'h').contains(&bytes[0]) || !(b'1'..=b'8').contains(&bytes[1])
    {
        return Err(FenError("invalid square"));
    }
    Ok(Square::new((bytes[0] - b'a') + (bytes[1] - b'1') * 8))
}

/// All legal moves in stable board-index/direction/promotion order.
#[must_use]
pub(crate) fn legal_moves(state: &State) -> Vec<Move> {
    legal_moves_internal(state)
}

pub(crate) fn legal_moves_internal(state: &State) -> Vec<Move> {
    if !matches!(state.status, Status::Playing) {
        return Vec::new();
    }
    let side = state.turn;
    pseudo_moves(state)
        .into_iter()
        .filter(|candidate| {
            let mut after = state.clone();
            apply_move(&mut after, *candidate, false);
            !in_check(&after, side)
        })
        .collect()
}

fn pseudo_moves(state: &State) -> Vec<Move> {
    let mut moves = Vec::new();
    for index in 0..64u8 {
        let from = Square(index);
        let Some(piece) = state.board[usize::from(index)] else {
            continue;
        };
        if piece.color != state.turn {
            continue;
        }
        match piece.kind {
            PieceKind::Pawn => pawn_moves(state, from, piece.color, &mut moves),
            PieceKind::Knight => jump_moves(state, from, piece.color, &KNIGHT_STEPS, &mut moves),
            PieceKind::Bishop => ray_moves(state, from, piece.color, &DIAGONALS, &mut moves),
            PieceKind::Rook => ray_moves(state, from, piece.color, &ORTHOGONALS, &mut moves),
            PieceKind::Queen => {
                ray_moves(state, from, piece.color, &DIAGONALS, &mut moves);
                ray_moves(state, from, piece.color, &ORTHOGONALS, &mut moves);
            }
            PieceKind::King => king_moves(state, from, piece.color, &mut moves),
        }
    }
    moves
}

const KNIGHT_STEPS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];
const DIAGONALS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, -1), (-1, 1)];
const ORTHOGONALS: [(i8, i8); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

fn pawn_moves(state: &State, from: Square, color: Color, moves: &mut Vec<Move>) {
    let direction = if color == Color::White { 1 } else { -1 };
    let start_rank = if color == Color::White { 1 } else { 6 };
    let promotion_rank = if color == Color::White { 7 } else { 0 };
    if let Some(one) = offset(from, 0, direction) {
        if state.board[usize::from(one.0)].is_none() {
            push_pawn_move(moves, from, one, promotion_rank, None);
            if from.rank() == start_rank {
                if let Some(two) = offset(from, 0, direction * 2) {
                    if state.board[usize::from(two.0)].is_none() {
                        moves.push(basic_move(from, two));
                    }
                }
            }
        }
    }
    for file_delta in [-1, 1] {
        let Some(to) = offset(from, file_delta, direction) else {
            continue;
        };
        if state.board[usize::from(to.0)]
            .is_some_and(|piece| piece.color != color && piece.kind != PieceKind::King)
        {
            push_pawn_move(moves, from, to, promotion_rank, None);
        } else if state.en_passant == Some(to) {
            let captured = offset(to, 0, -direction);
            if captured.is_some_and(|square| {
                state.board[usize::from(square.0)]
                    == Some(Piece {
                        color: color.other(),
                        kind: PieceKind::Pawn,
                    })
            }) {
                moves.push(Move {
                    from,
                    to,
                    promotion: None,
                    en_passant_capture: captured,
                    rook: None,
                });
            }
        }
    }
}

fn push_pawn_move(
    moves: &mut Vec<Move>,
    from: Square,
    to: Square,
    promotion_rank: u8,
    en_passant_capture: Option<Square>,
) {
    if to.rank() == promotion_rank {
        for promotion in [
            PieceKind::Queen,
            PieceKind::Rook,
            PieceKind::Bishop,
            PieceKind::Knight,
        ] {
            moves.push(Move {
                from,
                to,
                promotion: Some(promotion),
                en_passant_capture,
                rook: None,
            });
        }
    } else {
        moves.push(Move {
            from,
            to,
            promotion: None,
            en_passant_capture,
            rook: None,
        });
    }
}

fn jump_moves(
    state: &State,
    from: Square,
    color: Color,
    steps: &[(i8, i8)],
    moves: &mut Vec<Move>,
) {
    for &(file, rank) in steps {
        let Some(to) = offset(from, file, rank) else {
            continue;
        };
        if state.board[usize::from(to.0)]
            .is_none_or(|piece| piece.color != color && piece.kind != PieceKind::King)
        {
            moves.push(basic_move(from, to));
        }
    }
}

fn ray_moves(
    state: &State,
    from: Square,
    color: Color,
    directions: &[(i8, i8)],
    moves: &mut Vec<Move>,
) {
    for &(file, rank) in directions {
        let mut cursor = from;
        while let Some(to) = offset(cursor, file, rank) {
            match state.board[usize::from(to.0)] {
                None => moves.push(basic_move(from, to)),
                Some(piece) if piece.color != color && piece.kind != PieceKind::King => {
                    moves.push(basic_move(from, to));
                    break;
                }
                Some(_) => break,
            }
            cursor = to;
        }
    }
}

fn king_moves(state: &State, from: Square, color: Color, moves: &mut Vec<Move>) {
    jump_moves(
        state,
        from,
        color,
        &[
            (1, 1),
            (1, 0),
            (1, -1),
            (0, -1),
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, 1),
        ],
        moves,
    );
    let (home, king_to, queen_to, king_rook, queen_rook) = match color {
        Color::White => (Square(4), Square(6), Square(2), Square(7), Square(0)),
        Color::Black => (Square(60), Square(62), Square(58), Square(63), Square(56)),
    };
    if from != home || in_check(state, color) {
        return;
    }
    let (allow_king, allow_queen) = match color {
        Color::White => (state.castling.white_king, state.castling.white_queen),
        Color::Black => (state.castling.black_king, state.castling.black_queen),
    };
    if allow_king
        && state.board[usize::from(king_rook.0)]
            == Some(Piece {
                color,
                kind: PieceKind::Rook,
            })
        && state.board[usize::from(home.0 + 1)].is_none()
        && state.board[usize::from(king_to.0)].is_none()
        && !is_attacked(state, Square(home.0 + 1), color.other())
    {
        moves.push(Move {
            from,
            to: king_to,
            promotion: None,
            en_passant_capture: None,
            rook: Some((king_rook, Square(home.0 + 1))),
        });
    }
    if allow_queen
        && state.board[usize::from(queen_rook.0)]
            == Some(Piece {
                color,
                kind: PieceKind::Rook,
            })
        && state.board[usize::from(home.0 - 1)].is_none()
        && state.board[usize::from(queen_to.0)].is_none()
        && state.board[usize::from(home.0 - 3)].is_none()
        && !is_attacked(state, Square(home.0 - 1), color.other())
    {
        moves.push(Move {
            from,
            to: queen_to,
            promotion: None,
            en_passant_capture: None,
            rook: Some((queen_rook, Square(home.0 - 1))),
        });
    }
}

const fn basic_move(from: Square, to: Square) -> Move {
    Move {
        from,
        to,
        promotion: None,
        en_passant_capture: None,
        rook: None,
    }
}

fn offset(square: Square, file_delta: i8, rank_delta: i8) -> Option<Square> {
    let file = i16::from(square.file()) + i16::from(file_delta);
    let rank = i16::from(square.rank()) + i16::from(rank_delta);
    if !(0..8).contains(&file) || !(0..8).contains(&rank) {
        return None;
    }
    u8::try_from(rank * 8 + file).ok().and_then(Square::new)
}

pub(crate) fn in_check(state: &State, color: Color) -> bool {
    let king = state.board.iter().position(|piece| {
        *piece
            == Some(Piece {
                color,
                kind: PieceKind::King,
            })
    });
    let Some(index) = king else {
        return true;
    };
    let Some(square) = u8::try_from(index).ok().and_then(Square::new) else {
        return true;
    };
    is_attacked(state, square, color.other())
}

fn is_attacked(state: &State, target: Square, attacker: Color) -> bool {
    let pawn_rank = if attacker == Color::White { -1 } else { 1 };
    for file in [-1, 1] {
        if offset(target, file, pawn_rank).is_some_and(|from| {
            state.board[usize::from(from.0)]
                == Some(Piece {
                    color: attacker,
                    kind: PieceKind::Pawn,
                })
        }) {
            return true;
        }
    }
    if KNIGHT_STEPS.iter().any(|&(file, rank)| {
        offset(target, file, rank).is_some_and(|from| {
            state.board[usize::from(from.0)]
                == Some(Piece {
                    color: attacker,
                    kind: PieceKind::Knight,
                })
        })
    }) {
        return true;
    }
    if [
        (1, 1),
        (1, 0),
        (1, -1),
        (0, -1),
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, 1),
    ]
    .iter()
    .any(|&(file, rank)| {
        offset(target, file, rank).is_some_and(|from| {
            state.board[usize::from(from.0)]
                == Some(Piece {
                    color: attacker,
                    kind: PieceKind::King,
                })
        })
    }) {
        return true;
    }
    ray_attacked(
        state,
        target,
        attacker,
        &DIAGONALS,
        PieceKind::Bishop,
        PieceKind::Queen,
    ) || ray_attacked(
        state,
        target,
        attacker,
        &ORTHOGONALS,
        PieceKind::Rook,
        PieceKind::Queen,
    )
}

fn ray_attacked(
    state: &State,
    target: Square,
    attacker: Color,
    directions: &[(i8, i8)],
    first: PieceKind,
    second: PieceKind,
) -> bool {
    directions.iter().any(|&(file, rank)| {
        let mut cursor = target;
        while let Some(from) = offset(cursor, file, rank) {
            match state.board[usize::from(from.0)] {
                None => cursor = from,
                Some(Piece { color, kind }) => {
                    return color == attacker && (kind == first || kind == second)
                }
            }
        }
        false
    })
}

/// Applies a candidate already generated for `state`. The caller owns legality.
pub(crate) fn apply_move(
    state: &mut State,
    candidate: Move,
    record_position: bool,
) -> Option<Piece> {
    let Some(moving) = state.board[usize::from(candidate.from.0)] else {
        // Defensive totality for a malformed internal caller. Rule application
        // reaches this only with a generated candidate, which proves a source.
        return None;
    };
    let castling_rook = candidate
        .rook
        .and_then(|(rook_from, _)| state.board[usize::from(rook_from.0)]);
    if candidate.rook.is_some() && castling_rook.is_none() {
        return None;
    }
    let captured = state.board[usize::from(candidate.to.0)];
    state.board[usize::from(candidate.from.0)] = None;
    if let Some(square) = candidate.en_passant_capture {
        state.board[usize::from(square.0)] = None;
    }
    state.board[usize::from(candidate.to.0)] = Some(Piece {
        color: moving.color,
        kind: candidate.promotion.unwrap_or(moving.kind),
    });
    if let (Some((rook_from, rook_to)), Some(rook)) = (candidate.rook, castling_rook) {
        state.board[usize::from(rook_from.0)] = None;
        state.board[usize::from(rook_to.0)] = Some(rook);
    }
    revoke_castling(
        &mut state.castling,
        moving,
        candidate.from,
        candidate.to,
        captured,
    );
    state.en_passant = if moving.kind == PieceKind::Pawn
        && candidate.from.rank().abs_diff(candidate.to.rank()) == 2
    {
        offset(
            candidate.from,
            0,
            if moving.color == Color::White { 1 } else { -1 },
        )
    } else {
        None
    };
    state.halfmove_clock = if moving.kind == PieceKind::Pawn
        || captured.is_some()
        || candidate.en_passant_capture.is_some()
    {
        0
    } else {
        state.halfmove_clock.saturating_add(1)
    };
    if moving.color == Color::Black {
        state.fullmove_number = state.fullmove_number.saturating_add(1);
    }
    state.turn = state.turn.other();
    state.draw_offer = None;
    if record_position {
        // A pawn move or capture makes every earlier position unreachable, so
        // those keys need not remain in the canonical history. This keeps the
        // repetition vector bounded by the no-progress window.
        if moving.kind == PieceKind::Pawn || captured.is_some() {
            state.repetition.clear();
        }
        state.repetition.push(position_key(state));
    }
    captured.or_else(|| {
        candidate.en_passant_capture.map(|_| Piece {
            color: moving.color.other(),
            kind: PieceKind::Pawn,
        })
    })
}

fn revoke_castling(
    rights: &mut CastlingRights,
    moving: Piece,
    from: Square,
    to: Square,
    captured: Option<Piece>,
) {
    if moving.kind == PieceKind::King {
        match moving.color {
            Color::White => {
                rights.white_king = false;
                rights.white_queen = false;
            }
            Color::Black => {
                rights.black_king = false;
                rights.black_queen = false;
            }
        }
    }
    if moving.kind == PieceKind::Rook {
        revoke_rook_square(rights, from);
    }
    if captured.is_some_and(|piece| piece.kind == PieceKind::Rook) {
        revoke_rook_square(rights, to);
    }
}

fn revoke_rook_square(rights: &mut CastlingRights, square: Square) {
    match square.0 {
        0 => rights.white_queen = false,
        7 => rights.white_king = false,
        56 => rights.black_queen = false,
        63 => rights.black_king = false,
        _ => {}
    }
}

/// Canonical repetition identity with FIDE-relevant en-passant availability.
#[must_use]
pub fn position_key(state: &State) -> PositionKey {
    let en_passant = state.en_passant.filter(|_| {
        legal_moves_internal(state)
            .iter()
            .any(|candidate| candidate.en_passant_capture.is_some())
    });
    let mut key = 0;
    for (square, piece) in state.board.iter().enumerate() {
        if let Some(piece) = piece {
            key ^= zobrist_piece(square, *piece);
        }
    }
    key ^= if state.turn == Color::White {
        zobrist_component(0x0f0f_0f0f_0f0f_0f0f)
    } else {
        zobrist_component(0xf0f0_f0f0_f0f0_f0f0)
    };
    for (enabled, salt) in [
        (state.castling.white_king, 0x1000),
        (state.castling.white_queen, 0x1001),
        (state.castling.black_king, 0x1002),
        (state.castling.black_queen, 0x1003),
    ] {
        if enabled {
            key ^= zobrist_component(salt);
        }
    }
    if let Some(square) = en_passant {
        key ^= zobrist_component(0x2000 + u64::from(square.0));
    }
    PositionKey(key)
}

/// Counts leaf nodes using the same legal generation as rule application.
#[must_use]
pub fn perft(position: &State, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    legal_moves_internal(position)
        .into_iter()
        .map(|candidate| {
            let mut next = position.clone();
            apply_move(&mut next, candidate, false);
            perft(&next, depth - 1)
        })
        .sum()
}

fn king_count(state: &State, color: Color) -> usize {
    state
        .board
        .iter()
        .filter(|piece| {
            **piece
                == Some(Piece {
                    color,
                    kind: PieceKind::King,
                })
        })
        .count()
}

const ZOBRIST_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

fn zobrist_piece(square: usize, piece: Piece) -> u64 {
    let color = match piece.color {
        Color::White => 0,
        Color::Black => 1,
    };
    let kind = match piece.kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    };
    zobrist_component(0x3000 + (square as u64 * 12) + color * 6 + kind)
}

fn zobrist_component(value: u64) -> u64 {
    splitmix64(ZOBRIST_SEED.wrapping_add(value))
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn kings_are_adjacent(state: &State) -> bool {
    let Some(white) = state.board.iter().position(|piece| {
        *piece
            == Some(Piece {
                color: Color::White,
                kind: PieceKind::King,
            })
    }) else {
        return true;
    };
    let Some(black) = state.board.iter().position(|piece| {
        *piece
            == Some(Piece {
                color: Color::Black,
                kind: PieceKind::King,
            })
    }) else {
        return true;
    };
    let white = Square(u8::try_from(white).unwrap_or(0));
    let black = Square(u8::try_from(black).unwrap_or(0));
    white.file().abs_diff(black.file()) <= 1 && white.rank().abs_diff(black.rank()) <= 1
}

fn castling_rights_match_board(state: &State, field: &str) -> bool {
    let expected = [
        (
            'K',
            state.castling.white_king,
            Some(Piece {
                color: Color::White,
                kind: PieceKind::King,
            }),
            Some(Piece {
                color: Color::White,
                kind: PieceKind::Rook,
            }),
            4,
            7,
        ),
        (
            'Q',
            state.castling.white_queen,
            Some(Piece {
                color: Color::White,
                kind: PieceKind::King,
            }),
            Some(Piece {
                color: Color::White,
                kind: PieceKind::Rook,
            }),
            4,
            0,
        ),
        (
            'k',
            state.castling.black_king,
            Some(Piece {
                color: Color::Black,
                kind: PieceKind::King,
            }),
            Some(Piece {
                color: Color::Black,
                kind: PieceKind::Rook,
            }),
            60,
            63,
        ),
        (
            'q',
            state.castling.black_queen,
            Some(Piece {
                color: Color::Black,
                kind: PieceKind::King,
            }),
            Some(Piece {
                color: Color::Black,
                kind: PieceKind::Rook,
            }),
            60,
            56,
        ),
    ];
    expected
        .into_iter()
        .all(|(flag, enabled, king, rook, king_square, rook_square)| {
            !enabled
                || (field.contains(flag)
                    && state.board[king_square] == king
                    && state.board[rook_square] == rook)
        })
}

fn en_passant_matches_board(state: &State) -> bool {
    let Some(target) = state.en_passant else {
        return true;
    };
    let expected_rank = if state.turn == Color::White { 5 } else { 2 };
    if target.rank() != expected_rank || state.board[usize::from(target.0)].is_some() {
        return false;
    }
    let pawn_square = if state.turn == Color::White {
        target.0.saturating_sub(8)
    } else {
        target.0.saturating_add(8)
    };
    let origin_square = if state.turn == Color::White {
        target.0.saturating_add(8)
    } else {
        target.0.saturating_sub(8)
    };
    state.board.get(usize::from(pawn_square))
        == Some(&Some(Piece {
            color: state.turn.other(),
            kind: PieceKind::Pawn,
        }))
        && state.board.get(usize::from(origin_square)) == Some(&None)
}
