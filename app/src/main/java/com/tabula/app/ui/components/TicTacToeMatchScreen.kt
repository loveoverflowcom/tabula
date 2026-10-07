package com.tabula.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tabula.app.model.MatchStatus
import com.tabula.app.model.SeatRole
import com.tabula.app.model.TicTacToeState
import com.tabula.app.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TicTacToeMatchScreen(
    state: TicTacToeState,
    isBotThinking: Boolean,
    onCellClick: (Int) -> Unit,
    onReset: () -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Tic-Tac-Toe & Caro", fontWeight = FontWeight.Bold) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = onReset) {
                        Icon(Icons.Default.Refresh, contentDescription = "Reset Match")
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
        modifier = modifier.testTag("tictactoe_match_screen")
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(20.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            // Opponent HUD (O / Bot)
            PlayerHudBar(
                name = if (state.isAgainstBot) "Tabula Pure Bot (O)" else "Player 2 (O)",
                isTurn = state.turn == SeatRole.PLAYER_O && state.status == MatchStatus.PLAYING,
                timeText = if (isBotThinking) "Thinking..." else "Ready",
                isThinking = isBotThinking,
                avatar = "🤖"
            )

            // Status message
            val statusText = when (state.status) {
                MatchStatus.PLAYING -> if (state.turn == SeatRole.PLAYER_X) "Your Turn (X)" else "Bot Turn (O)"
                MatchStatus.WON_X -> "🎉 Victory! Player X won."
                MatchStatus.WON_O -> "Defeat! Player O won."
                MatchStatus.DRAW -> "Match Drawn! Full board."
            }

            Surface(
                shape = RoundedCornerShape(12.dp),
                color = when (state.status) {
                    MatchStatus.PLAYING -> TabulaDarkSurfaceVariant
                    MatchStatus.WON_X -> TabulaPrimaryContainer
                    else -> TabulaDarkSurfaceHighlight
                },
                border = androidx.compose.foundation.BorderStroke(1.dp, TabulaPrimary),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(
                    text = statusText,
                    style = MaterialTheme.typography.titleMedium,
                    color = TabulaOnSurface,
                    modifier = Modifier.padding(12.dp).align(Alignment.CenterHorizontally),
                    fontWeight = FontWeight.Bold
                )
            }

            // 3x3 Grid
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(1f)
                    .clip(RoundedCornerShape(16.dp))
                    .background(TabulaDarkSurface)
                    .border(2.dp, TabulaOutline, RoundedCornerShape(16.dp))
                    .padding(8.dp)
            ) {
                Column(modifier = Modifier.fillMaxSize()) {
                    for (row in 0..2) {
                        Row(modifier = Modifier.weight(1f)) {
                            for (col in 0..2) {
                                val cell = row * 3 + col
                                val value = state.board[cell]

                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .fillMaxHeight()
                                        .padding(4.dp)
                                        .clip(RoundedCornerShape(12.dp))
                                        .background(TabulaDarkSurfaceVariant)
                                        .border(1.dp, TabulaOutline, RoundedCornerShape(12.dp))
                                        .clickable { onCellClick(cell) }
                                        .testTag("cell_$cell"),
                                    contentAlignment = Alignment.Center
                                ) {
                                    if (value != null) {
                                        Text(
                                            text = if (value == SeatRole.PLAYER_X) "X" else "O",
                                            fontSize = 44.sp,
                                            fontWeight = FontWeight.Bold,
                                            color = if (value == SeatRole.PLAYER_X) TabulaPrimary else TabulaSecondary
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Player HUD (X)
            PlayerHudBar(
                name = "You (X)",
                isTurn = state.turn == SeatRole.PLAYER_X && state.status == MatchStatus.PLAYING,
                timeText = "Ready",
                isThinking = false,
                avatar = "👤"
            )

            // Reset Button
            Button(
                onClick = onReset,
                colors = ButtonDefaults.buttonColors(containerColor = TabulaPrimary),
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier.fillMaxWidth().height(48.dp)
            ) {
                Text("Start New Match", fontWeight = FontWeight.Bold)
            }
        }
    }
}
