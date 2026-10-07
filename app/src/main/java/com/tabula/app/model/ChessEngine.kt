package com.tabula.app.model

import kotlin.math.abs

enum class PieceColor { WHITE, BLACK }
enum class PieceType { PAWN, KNIGHT, BISHOP, ROOK, QUEEN, KING }

data class ChessPiece(val type: PieceType, val color: PieceColor) {
    val symbol: String = when (color) {
        PieceColor.WHITE -> when (type) {
            PieceType.KING -> "♔"
            PieceType.QUEEN -> "♕"
            PieceType.ROOK -> "♖"
            PieceType.BISHOP -> "♗"
            PieceType.KNIGHT -> "♘"
            PieceType.PAWN -> "♙"
        }
        PieceColor.BLACK -> when (type) {
            PieceType.KING -> "♚"
            PieceType.QUEEN -> "♛"
            PieceType.ROOK -> "♜"
            PieceType.BISHOP -> "♝"
            PieceType.KNIGHT -> "♞"
            PieceType.PAWN -> "♟"
        }
    }
}

data class ChessMove(val from: Int, val to: Int, val isCapture: Boolean = false)

data class ChessState(
    val board: List<ChessPiece?> = initialBoard(),
    val turn: PieceColor = PieceColor.WHITE,
    val selectedSquare: Int? = null,
    val legalTargets: List<Int> = emptyList(),
    val moveHistory: List<String> = emptyList(),
    val lastMoveFrom: Int? = null,
    val lastMoveTo: Int? = null,
    val isCheck: Boolean = false,
    val isGameOver: Boolean = false,
    val winner: PieceColor? = null,
    val whiteTimeMillis: Long = 300_000L, // 5 min
    val blackTimeMillis: Long = 300_000L,
    val isAgainstBot: Boolean = true
) {
    companion object {
        fun initialBoard(): List<ChessPiece?> {
            val b = MutableList<ChessPiece?>(64) { null }
            // Black pieces
            val backRankBlack = listOf(
                PieceType.ROOK, PieceType.KNIGHT, PieceType.BISHOP, PieceType.QUEEN,
                PieceType.KING, PieceType.BISHOP, PieceType.KNIGHT, PieceType.ROOK
            )
            for (col in 0..7) {
                b[col] = ChessPiece(backRankBlack[col], PieceColor.BLACK)
                b[8 + col] = ChessPiece(PieceType.PAWN, PieceColor.BLACK)
            }
            // White pieces
            val backRankWhite = listOf(
                PieceType.ROOK, PieceType.KNIGHT, PieceType.BISHOP, PieceType.QUEEN,
                PieceType.KING, PieceType.BISHOP, PieceType.KNIGHT, PieceType.ROOK
            )
            for (col in 0..7) {
                b[48 + col] = ChessPiece(PieceType.PAWN, PieceColor.WHITE)
                b[56 + col] = ChessPiece(backRankWhite[col], PieceColor.WHITE)
            }
            return b
        }
    }

    fun selectSquare(sq: Int): ChessState {
        if (isGameOver) return this
        val piece = board[sq]
        if (selectedSquare == null) {
            if (piece != null && piece.color == turn) {
                val targets = computeLegalTargets(sq)
                return copy(selectedSquare = sq, legalTargets = targets)
            }
            return this
        } else {
            if (sq in legalTargets) {
                return executeMove(selectedSquare, sq)
            } else if (piece != null && piece.color == turn) {
                val targets = computeLegalTargets(sq)
                return copy(selectedSquare = sq, legalTargets = targets)
            } else {
                return copy(selectedSquare = null, legalTargets = emptyList())
            }
        }
    }

    fun executeMove(from: Int, to: Int): ChessState {
        val movingPiece = board[from] ?: return this
        val captured = board[to]
        val newBoard = board.toMutableList()
        newBoard[to] = movingPiece
        newBoard[from] = null

        // Promotion rule: Pawn reaches back rank -> Queen
        if (movingPiece.type == PieceType.PAWN) {
            val toRow = to / 8
            if ((movingPiece.color == PieceColor.WHITE && toRow == 0) ||
                (movingPiece.color == PieceColor.BLACK && toRow == 7)
            ) {
                newBoard[to] = ChessPiece(PieceType.QUEEN, movingPiece.color)
            }
        }

        val fromNotation = squareToNotation(from)
        val toNotation = squareToNotation(to)
        val notation = "${movingPiece.type.name.take(1)}$fromNotation${if (captured != null) "x" else "-"}$toNotation"

        val nextTurn = if (turn == PieceColor.WHITE) PieceColor.BLACK else PieceColor.WHITE

        // Simple win if king is taken
        val isKingDead = captured?.type == PieceType.KING
        val newGameOver = isKingDead
        val newWinner = if (isKingDead) movingPiece.color else null

        return copy(
            board = newBoard,
            turn = nextTurn,
            selectedSquare = null,
            legalTargets = emptyList(),
            moveHistory = moveHistory + notation,
            lastMoveFrom = from,
            lastMoveTo = to,
            isGameOver = newGameOver,
            winner = newWinner
        )
    }

    private fun computeLegalTargets(sq: Int): List<Int> {
        val piece = board[sq] ?: return emptyList()
        val r = sq / 8
        val c = sq % 8
        val targets = mutableListOf<Int>()

        when (piece.type) {
            PieceType.PAWN -> {
                val dir = if (piece.color == PieceColor.WHITE) -1 else 1
                val startRow = if (piece.color == PieceColor.WHITE) 6 else 1
                val oneStep = (r + dir) * 8 + c
                if (oneStep in 0..63 && board[oneStep] == null) {
                    targets.add(oneStep)
                    val twoStep = (r + 2 * dir) * 8 + c
                    if (r == startRow && board[twoStep] == null) {
                        targets.add(twoStep)
                    }
                }
                // Captures
                listOf(c - 1, c + 1).filter { it in 0..7 }.forEach { capCol ->
                    val capSq = (r + dir) * 8 + capCol
                    if (capSq in 0..63) {
                        val capPiece = board[capSq]
                        if (capPiece != null && capPiece.color != piece.color) {
                            targets.add(capSq)
                        }
                    }
                }
            }
            PieceType.KNIGHT -> {
                val jumps = listOf(
                    Pair(-2, -1), Pair(-2, 1), Pair(-1, -2), Pair(-1, 2),
                    Pair(1, -2), Pair(1, 2), Pair(2, -1), Pair(2, 1)
                )
                for ((dr, dc) in jumps) {
                    val nr = r + dr
                    val nc = c + dc
                    if (nr in 0..7 && nc in 0..7) {
                        val nsq = nr * 8 + nc
                        val target = board[nsq]
                        if (target == null || target.color != piece.color) targets.add(nsq)
                    }
                }
            }
            PieceType.BISHOP -> addRayTargets(r, c, listOf(Pair(-1, -1), Pair(-1, 1), Pair(1, -1), Pair(1, 1)), piece.color, targets)
            PieceType.ROOK -> addRayTargets(r, c, listOf(Pair(-1, 0), Pair(1, 0), Pair(0, -1), Pair(0, 1)), piece.color, targets)
            PieceType.QUEEN -> addRayTargets(r, c, listOf(Pair(-1, -1), Pair(-1, 1), Pair(1, -1), Pair(1, 1), Pair(-1, 0), Pair(1, 0), Pair(0, -1), Pair(0, 1)), piece.color, targets)
            PieceType.KING -> {
                for (dr in -1..1) {
                    for (dc in -1..1) {
                        if (dr == 0 && dc == 0) continue
                        val nr = r + dr
                        val nc = c + dc
                        if (nr in 0..7 && nc in 0..7) {
                            val nsq = nr * 8 + nc
                            val target = board[nsq]
                            if (target == null || target.color != piece.color) targets.add(nsq)
                        }
                    }
                }
            }
        }
        return targets
    }

    private fun addRayTargets(
        r: Int, c: Int,
        directions: List<Pair<Int, Int>>,
        myColor: PieceColor,
        out: MutableList<Int>
    ) {
        for ((dr, dc) in directions) {
            var currR = r + dr
            var currC = c + dc
            while (currR in 0..7 && currC in 0..7) {
                val sq = currR * 8 + currC
                val target = board[sq]
                if (target == null) {
                    out.add(sq)
                } else {
                    if (target.color != myColor) out.add(sq)
                    break
                }
                currR += dr
                currC += dc
            }
        }
    }

    fun getBestBotMove(): ChessMove? {
        val botPieces = board.indices.filter { board[it] != null && board[it]?.color == turn }
        val allMoves = mutableListOf<ChessMove>()
        for (sq in botPieces) {
            val targets = computeLegalTargets(sq)
            for (t in targets) {
                val isCap = board[t] != null
                allMoves.add(ChessMove(sq, t, isCap))
            }
        }
        if (allMoves.isEmpty()) return null

        // 1. King capture / Winning capture
        val kingCap = allMoves.find { board[it.to]?.type == PieceType.KING }
        if (kingCap != null) return kingCap

        // 2. High value captures
        val captures = allMoves.filter { it.isCapture }.sortedByDescending {
            when (board[it.to]?.type) {
                PieceType.QUEEN -> 9
                PieceType.ROOK -> 5
                PieceType.BISHOP, PieceType.KNIGHT -> 3
                PieceType.PAWN -> 1
                else -> 0
            }
        }
        if (captures.isNotEmpty()) return captures.first()

        // 3. Center development moves
        val centerMoves = allMoves.filter { (it.to / 8) in 2..5 && (it.to % 8) in 2..5 }
        if (centerMoves.isNotEmpty()) return centerMoves.random()

        return allMoves.random()
    }

    private fun squareToNotation(sq: Int): String {
        val col = ('a' + (sq % 8))
        val row = 8 - (sq / 8)
        return "$col$row"
    }
}
