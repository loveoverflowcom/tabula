package com.tabula.app.model

enum class SeatRole(val label: String) {
    PLAYER_X("Player X (Black)"),
    PLAYER_O("Player O (White)")
}

enum class MatchStatus {
    PLAYING,
    WON_X,
    WON_O,
    DRAW
}

data class TicTacToeState(
    val board: List<SeatRole?> = List(9) { null },
    val turn: SeatRole = SeatRole.PLAYER_X,
    val status: MatchStatus = MatchStatus.PLAYING,
    val moveCount: Int = 0,
    val moveHistory: List<Int> = emptyList(),
    val isAgainstBot: Boolean = true
) {
    companion object {
        val WIN_LINES = listOf(
            listOf(0, 1, 2), listOf(3, 4, 5), listOf(6, 7, 8),
            listOf(0, 3, 6), listOf(1, 4, 7), listOf(2, 5, 8),
            listOf(0, 4, 8), listOf(2, 4, 6)
        )
    }

    fun makeMove(cell: Int): TicTacToeState {
        if (cell !in 0..8 || board[cell] != null || status != MatchStatus.PLAYING) return this

        val newBoard = board.toMutableList()
        newBoard[cell] = turn
        val newCount = moveCount + 1
        val newHistory = moveHistory + cell

        // Check for win
        val won = WIN_LINES.any { line ->
            val (a, b, c) = line
            newBoard[a] != null && newBoard[a] == newBoard[b] && newBoard[b] == newBoard[c]
        }

        val newStatus = when {
            won -> if (turn == SeatRole.PLAYER_X) MatchStatus.WON_X else MatchStatus.WON_O
            newCount >= 9 -> MatchStatus.DRAW
            else -> MatchStatus.PLAYING
        }

        val nextTurn = if (turn == SeatRole.PLAYER_X) SeatRole.PLAYER_O else SeatRole.PLAYER_X

        return copy(
            board = newBoard,
            turn = nextTurn,
            status = newStatus,
            moveCount = newCount,
            moveHistory = newHistory
        )
    }

    fun getBestBotMove(): Int? {
        val freeCells = board.indices.filter { board[it] == null }
        if (freeCells.isEmpty()) return null

        // 1. Immediate win for O
        for (cell in freeCells) {
            val testBoard = board.toMutableList()
            testBoard[cell] = SeatRole.PLAYER_O
            if (WIN_LINES.any { (a, b, c) -> testBoard[a] == SeatRole.PLAYER_O && testBoard[b] == SeatRole.PLAYER_O && testBoard[c] == SeatRole.PLAYER_O }) {
                return cell
            }
        }

        // 2. Immediate block for X
        for (cell in freeCells) {
            val testBoard = board.toMutableList()
            testBoard[cell] = SeatRole.PLAYER_X
            if (WIN_LINES.any { (a, b, c) -> testBoard[a] == SeatRole.PLAYER_X && testBoard[b] == SeatRole.PLAYER_X && testBoard[c] == SeatRole.PLAYER_X }) {
                return cell
            }
        }

        // 3. Center preference
        if (4 in freeCells) return 4

        // 4. Corners preference
        val corners = listOf(0, 2, 6, 8).filter { it in freeCells }
        if (corners.isNotEmpty()) return corners.first()

        return freeCells.first()
    }
}
