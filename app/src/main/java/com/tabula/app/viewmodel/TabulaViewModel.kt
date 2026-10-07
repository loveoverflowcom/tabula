package com.tabula.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.room.Room
import com.tabula.app.data.SavedMatchEntity
import com.tabula.app.data.TabulaDatabase
import com.tabula.app.model.*
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch

enum class AppNavDestination {
    DISCOVERY,
    GAME_DETAIL,
    CHESS_MATCH,
    TICTACTOE_MATCH,
    ROOMS,
    HISTORY,
    SETTINGS
}

data class TabulaUiState(
    val currentDestination: AppNavDestination = AppNavDestination.DISCOVERY,
    val selectedGameId: String = "chess",
    val selectedCategory: GameCategory = GameCategory.ALL,
    val searchQuery: String = "",
    val chessState: ChessState = ChessState(),
    val tttState: TicTacToeState = TicTacToeState(),
    val activeRoomId: String? = null,
    val botIsThinking: Boolean = false,
    val playerRating: Int = 1480,
    val wins: Int = 12,
    val losses: Int = 5,
    val draws: Int = 3
)

class TabulaViewModel(application: Application) : AndroidViewModel(application) {

    private val db = Room.databaseBuilder(
        application,
        TabulaDatabase::class.java,
        "tabula.db"
    ).build()

    private val _uiState = MutableStateFlow(TabulaUiState())
    val uiState: StateFlow<TabulaUiState> = _uiState.asStateFlow()

    val savedMatches: StateFlow<List<SavedMatchEntity>> = db.savedMatchDao()
        .getAllMatches()
        .stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(5000),
            initialValue = emptyList()
        )

    fun navigateTo(dest: AppNavDestination) {
        _uiState.update { it.copy(currentDestination = dest) }
    }

    fun openGameDetail(gameId: String) {
        _uiState.update { it.copy(selectedGameId = gameId, currentDestination = AppNavDestination.GAME_DETAIL) }
    }

    fun setCategory(cat: GameCategory) {
        _uiState.update { it.copy(selectedCategory = cat) }
    }

    fun setSearch(q: String) {
        _uiState.update { it.copy(searchQuery = q) }
    }

    fun startChessMatch(againstBot: Boolean = true) {
        _uiState.update {
            it.copy(
                chessState = ChessState(isAgainstBot = againstBot),
                currentDestination = AppNavDestination.CHESS_MATCH
            )
        }
    }

    fun handleChessSquareClick(sq: Int) {
        val currState = _uiState.value.chessState
        if (currState.isGameOver || _uiState.value.botIsThinking) return

        val nextState = currState.selectSquare(sq)
        _uiState.update { it.copy(chessState = nextState) }

        // If move was made and it's black's turn & playing against bot
        if (nextState.turn == PieceColor.BLACK && nextState.isAgainstBot && !nextState.isGameOver) {
            triggerBotChessMove()
        }

        if (nextState.isGameOver) {
            recordMatchResult("chess", "Chess (Cờ Vua)", nextState.winner == PieceColor.WHITE)
        }
    }

    private fun triggerBotChessMove() {
        viewModelScope.launch {
            _uiState.update { it.copy(botIsThinking = true) }
            delay(600) // Realistic think time
            val botMove = _uiState.value.chessState.getBestBotMove()
            if (botMove != null) {
                val afterBot = _uiState.value.chessState.executeMove(botMove.from, botMove.to)
                _uiState.update { it.copy(chessState = afterBot, botIsThinking = false) }
                if (afterBot.isGameOver) {
                    recordMatchResult("chess", "Chess (Cờ Vua)", afterBot.winner == PieceColor.WHITE)
                }
            } else {
                _uiState.update { it.copy(botIsThinking = false) }
            }
        }
    }

    fun startTicTacToeMatch(againstBot: Boolean = true) {
        _uiState.update {
            it.copy(
                tttState = TicTacToeState(isAgainstBot = againstBot),
                currentDestination = AppNavDestination.TICTACTOE_MATCH
            )
        }
    }

    fun handleTicTacToeCellClick(cell: Int) {
        val curr = _uiState.value.tttState
        if (curr.board[cell] != null || curr.status != MatchStatus.PLAYING || _uiState.value.botIsThinking) return

        val next = curr.makeMove(cell)
        _uiState.update { it.copy(tttState = next) }

        if (next.turn == SeatRole.PLAYER_O && next.isAgainstBot && next.status == MatchStatus.PLAYING) {
            triggerBotTicTacToeMove()
        } else if (next.status != MatchStatus.PLAYING) {
            recordMatchResult("tictactoe", "Tic-Tac-Toe", next.status == MatchStatus.WON_X)
        }
    }

    private fun triggerBotTicTacToeMove() {
        viewModelScope.launch {
            _uiState.update { it.copy(botIsThinking = true) }
            delay(400)
            val move = _uiState.value.tttState.getBestBotMove()
            if (move != null) {
                val next = _uiState.value.tttState.makeMove(move)
                _uiState.update { it.copy(tttState = next, botIsThinking = false) }
                if (next.status != MatchStatus.PLAYING) {
                    recordMatchResult("tictactoe", "Tic-Tac-Toe", next.status == MatchStatus.WON_X)
                }
            } else {
                _uiState.update { it.copy(botIsThinking = false) }
            }
        }
    }

    fun resetChess() {
        _uiState.update { it.copy(chessState = ChessState(isAgainstBot = it.chessState.isAgainstBot)) }
    }

    fun resetTicTacToe() {
        _uiState.update { it.copy(tttState = TicTacToeState(isAgainstBot = it.tttState.isAgainstBot)) }
    }

    private fun recordMatchResult(gameId: String, title: String, isWin: Boolean) {
        val ratingDelta = if (isWin) "+15 Elo" else "-10 Elo"
        viewModelScope.launch {
            db.savedMatchDao().insertMatch(
                SavedMatchEntity(
                    gameId = gameId,
                    gameTitle = title,
                    opponent = "Tabula Engine Bot",
                    result = if (isWin) "Victory" else "Defeat",
                    ratingChange = ratingDelta,
                    moveCount = if (gameId == "chess") _uiState.value.chessState.moveHistory.size else _uiState.value.tttState.moveCount,
                    durationText = "Realtime"
                )
            )
            _uiState.update {
                it.copy(
                    playerRating = if (isWin) it.playerRating + 15 else (it.playerRating - 10).coerceAtLeast(1000),
                    wins = if (isWin) it.wins + 1 else it.wins,
                    losses = if (!isWin) it.losses + 1 else it.losses
                )
            }
        }
    }
}
