package com.tabula.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tabula.app.model.ChessPiece
import com.tabula.app.model.ChessState
import com.tabula.app.model.PieceColor
import com.tabula.app.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChessMatchScreen(
    state: ChessState,
    isBotThinking: Boolean,
    onSquareClick: (Int) -> Unit,
    onResetMatch: () -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Chess (Cờ Vua)", fontWeight = FontWeight.Bold) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = onResetMatch) {
                        Icon(Icons.Default.Refresh, contentDescription = "Reset Board")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = TabulaDarkSurface,
                    titleContentColor = TabulaOnSurface,
                    navigationIconContentColor = TabulaOnSurface
                )
            )
        },
        containerColor = TabulaDarkBackground,
        modifier = modifier.testTag("chess_match_screen")
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            // Opponent HUD (Black / Bot)
            PlayerHudBar(
                name = if (state.isAgainstBot) "Tabula Engine Bot (Black)" else "Player 2 (Black)",
                isTurn = state.turn == PieceColor.BLACK && !state.isGameOver,
                timeText = "04:48",
                isThinking = isBotThinking && state.turn == PieceColor.BLACK,
                avatar = "🤖"
            )

            // Status Banner if game over or in check
            if (state.isGameOver) {
                Surface(
                    shape = RoundedCornerShape(12.dp),
                    color = TabulaPrimaryContainer,
                    border = androidx.compose.foundation.BorderStroke(1.dp, TabulaPrimary),
                    modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)
                ) {
                    Text(
                        text = "Match Concluded • ${if (state.winner == PieceColor.WHITE) "Victory (White)" else "Defeat (Black)"}",
                        style = MaterialTheme.typography.titleMedium,
                        color = TabulaOnPrimaryContainer,
                        modifier = Modifier.padding(12.dp).align(Alignment.CenterHorizontally),
                        fontWeight = FontWeight.Bold
                    )
                }
            }

            // 8x8 Chess Board
            ChessBoardGrid(
                state = state,
                onSquareClick = onSquareClick,
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(1f)
                    .clip(RoundedCornerShape(12.dp))
                    .border(2.dp, TabulaOutline, RoundedCornerShape(12.dp))
            )

            // Player HUD (White)
            PlayerHudBar(
                name = "You (White)",
                isTurn = state.turn == PieceColor.WHITE && !state.isGameOver,
                timeText = "04:56",
                isThinking = false,
                avatar = "👤"
            )

            // Notation move history tape
            MoveHistoryTape(moves = state.moveHistory)
        }
    }
}

@Composable
fun ChessBoardGrid(
    state: ChessState,
    onSquareClick: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier) {
        for (row in 0..7) {
            Row(modifier = Modifier.weight(1f)) {
                for (col in 0..7) {
                    val sq = row * 8 + col
                    val isLight = (row + col) % 2 == 0
                    val isSelected = state.selectedSquare == sq
                    val isLegal = sq in state.legalTargets
                    val isLastMove = sq == state.lastMoveFrom || sq == state.lastMoveTo
                    val piece = state.board[sq]

                    val bgColor = when {
                        isSelected -> TabulaSelected
                        isLastMove -> TabulaLastAction.copy(alpha = 0.5f)
                        isLight -> Color(0xFF28253B)
                        else -> Color(0xFF161424)
                    }

                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxHeight()
                            .background(bgColor)
                            .clickable { onSquareClick(sq) }
                            .testTag("square_$sq"),
                        contentAlignment = Alignment.Center
                    ) {
                        // Legal target indicator dot or capture ring
                        if (isLegal) {
                            if (piece == null) {
                                Box(
                                    modifier = Modifier
                                        .size(14.dp)
                                        .clip(RoundedCornerShape(7.dp))
                                        .background(TabulaPrimary.copy(alpha = 0.8f))
                                )
                            } else {
                                Box(
                                    modifier = Modifier
                                        .size(38.dp)
                                        .border(3.dp, TabulaIllegalTarget, RoundedCornerShape(19.dp))
                                )
                            }
                        }

                        // Chess Piece
                        if (piece != null) {
                            Text(
                                text = piece.symbol,
                                fontSize = 32.sp,
                                color = if (piece.color == PieceColor.WHITE) Color.White else Color(0xFFFFB547),
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun PlayerHudBar(
    name: String,
    isTurn: Boolean,
    timeText: String,
    isThinking: Boolean,
    avatar: String,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        color = if (isTurn) TabulaDarkSurfaceVariant else TabulaDarkSurface,
        border = androidx.compose.foundation.BorderStroke(
            1.dp,
            if (isTurn) TabulaTurnActive else TabulaOutline
        )
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(text = avatar, fontSize = 20.sp)
                Spacer(modifier = Modifier.width(10.dp))
                Column {
                    Text(
                        text = name,
                        style = MaterialTheme.typography.titleSmall,
                        color = TabulaOnSurface,
                        fontWeight = FontWeight.Bold
                    )
                    Text(
                        text = if (isThinking) "Thinking next move..." else if (isTurn) "Turn Active" else "Waiting",
                        style = MaterialTheme.typography.bodySmall,
                        color = if (isTurn) TabulaTurnActive else TabulaOnSurfaceVariant
                    )
                }
            }
            Surface(
                shape = RoundedCornerShape(8.dp),
                color = TabulaDarkBackground,
                border = androidx.compose.foundation.BorderStroke(1.dp, TabulaOutline)
            ) {
                Text(
                    text = timeText,
                    style = MaterialTheme.typography.labelMedium,
                    color = if (isTurn) TabulaTurnActive else TabulaOnSurface,
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
                    fontWeight = FontWeight.Bold
                )
            }
        }
    }
}

@Composable
fun MoveHistoryTape(moves: List<String>, modifier: Modifier = Modifier) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(10.dp),
        color = TabulaDarkSurfaceVariant,
        border = androidx.compose.foundation.BorderStroke(1.dp, TabulaOutline)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "MOVES:",
                style = MaterialTheme.typography.labelSmall,
                color = TabulaSecondary,
                fontWeight = FontWeight.Bold
            )
            Spacer(modifier = Modifier.width(8.dp))
            if (moves.isEmpty()) {
                Text(
                    text = "Awaiting opening move...",
                    style = MaterialTheme.typography.bodySmall,
                    color = TabulaOnSurfaceVariant
                )
            } else {
                LazyRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    items(moves.takeLast(10)) { m ->
                        Surface(
                            shape = RoundedCornerShape(4.dp),
                            color = TabulaDarkSurface
                        ) {
                            Text(
                                text = m,
                                style = MaterialTheme.typography.labelSmall,
                                color = TabulaOnSurface,
                                modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                            )
                        }
                    }
                }
            }
        }
    }
}
