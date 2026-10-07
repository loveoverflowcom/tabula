package com.tabula.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.tabula.app.model.GameCategory
import com.tabula.app.ui.components.*
import com.tabula.app.ui.theme.*
import com.tabula.app.viewmodel.AppNavDestination
import com.tabula.app.viewmodel.TabulaViewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            TabulaTheme {
                TabulaAppRoot()
            }
        }
    }
}

@Composable
fun TabulaAppRoot(viewModel: TabulaViewModel = viewModel()) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()
    val savedMatches by viewModel.savedMatches.collectAsStateWithLifecycle()

    val showBottomBar = uiState.currentDestination in listOf(
        AppNavDestination.DISCOVERY,
        AppNavDestination.ROOMS,
        AppNavDestination.HISTORY,
        AppNavDestination.SETTINGS
    )

    Scaffold(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing),
        containerColor = TabulaDarkBackground,
        bottomBar = {
            if (showBottomBar) {
                NavigationBar(
                    containerColor = TabulaDarkSurface,
                    contentColor = TabulaOnSurface,
                    tonalElevation = 6.dp,
                    modifier = Modifier.testTag("bottom_nav_bar")
                ) {
                    NavigationBarItem(
                        selected = uiState.currentDestination == AppNavDestination.DISCOVERY,
                        onClick = { viewModel.navigateTo(AppNavDestination.DISCOVERY) },
                        icon = { Icon(Icons.Default.Explore, contentDescription = "Discover") },
                        label = { Text("Discover") },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = TabulaOnPrimary,
                            selectedTextColor = TabulaPrimary,
                            indicatorColor = TabulaPrimary
                        )
                    )
                    NavigationBarItem(
                        selected = uiState.currentDestination == AppNavDestination.ROOMS,
                        onClick = { viewModel.navigateTo(AppNavDestination.ROOMS) },
                        icon = { Icon(Icons.Default.Groups, contentDescription = "Rooms") },
                        label = { Text("Rooms") },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = TabulaOnPrimary,
                            selectedTextColor = TabulaPrimary,
                            indicatorColor = TabulaPrimary
                        )
                    )
                    NavigationBarItem(
                        selected = uiState.currentDestination == AppNavDestination.HISTORY,
                        onClick = { viewModel.navigateTo(AppNavDestination.HISTORY) },
                        icon = { Icon(Icons.Default.History, contentDescription = "History") },
                        label = { Text("Replays") },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = TabulaOnPrimary,
                            selectedTextColor = TabulaPrimary,
                            indicatorColor = TabulaPrimary
                        )
                    )
                    NavigationBarItem(
                        selected = uiState.currentDestination == AppNavDestination.SETTINGS,
                        onClick = { viewModel.navigateTo(AppNavDestination.SETTINGS) },
                        icon = { Icon(Icons.Default.Person, contentDescription = "Profile") },
                        label = { Text("Profile") },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = TabulaOnPrimary,
                            selectedTextColor = TabulaPrimary,
                            indicatorColor = TabulaPrimary
                        )
                    )
                }
            }
        }
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            when (uiState.currentDestination) {
                AppNavDestination.DISCOVERY -> {
                    DiscoveryScreen(
                        uiState = uiState,
                        onCategorySelected = { viewModel.setCategory(it) },
                        onGameSelected = { viewModel.openGameDetail(it) },
                        onPlayQuickMatch = { gameId ->
                            if (gameId == "chess") viewModel.startChessMatch(againstBot = true)
                            else if (gameId == "tictactoe") viewModel.startTicTacToeMatch(againstBot = true)
                            else viewModel.openGameDetail(gameId)
                        }
                    )
                }
                AppNavDestination.GAME_DETAIL -> {
                    GameDetailScreen(
                        gameId = uiState.selectedGameId,
                        onBack = { viewModel.navigateTo(AppNavDestination.DISCOVERY) },
                        onPlay = { gameId ->
                            if (gameId == "chess") viewModel.startChessMatch(againstBot = true)
                            else if (gameId == "tictactoe") viewModel.startTicTacToeMatch(againstBot = true)
                            else viewModel.navigateTo(AppNavDestination.ROOMS)
                        }
                    )
                }
                AppNavDestination.CHESS_MATCH -> {
                    ChessMatchScreen(
                        state = uiState.chessState,
                        isBotThinking = uiState.botIsThinking,
                        onSquareClick = { viewModel.handleChessSquareClick(it) },
                        onResetMatch = { viewModel.resetChess() },
                        onBack = { viewModel.navigateTo(AppNavDestination.DISCOVERY) }
                    )
                }
                AppNavDestination.TICTACTOE_MATCH -> {
                    TicTacToeMatchScreen(
                        state = uiState.tttState,
                        isBotThinking = uiState.botIsThinking,
                        onCellClick = { viewModel.handleTicTacToeCellClick(it) },
                        onReset = { viewModel.resetTicTacToe() },
                        onBack = { viewModel.navigateTo(AppNavDestination.DISCOVERY) }
                    )
                }
                AppNavDestination.ROOMS -> {
                    RoomsScreen(
                        onJoinRoom = { room ->
                            if (room.gameId == "chess") viewModel.startChessMatch(againstBot = false)
                            else if (room.gameId == "tictactoe") viewModel.startTicTacToeMatch(againstBot = false)
                            else viewModel.openGameDetail(room.gameId)
                        }
                    )
                }
                AppNavDestination.HISTORY -> {
                    HistoryScreen(
                        savedMatches = savedMatches
                    )
                }
                AppNavDestination.SETTINGS -> {
                    SettingsScreen(
                        playerRating = uiState.playerRating,
                        wins = uiState.wins,
                        losses = uiState.losses,
                        draws = uiState.draws
                    )
                }
            }
        }
    }
}
