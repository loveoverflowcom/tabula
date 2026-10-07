package com.tabula.app.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tabula.app.R
import com.tabula.app.model.GameCatalog
import com.tabula.app.model.GameCategory
import com.tabula.app.model.GameSummary
import com.tabula.app.ui.theme.*
import com.tabula.app.viewmodel.TabulaUiState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DiscoveryScreen(
    uiState: TabulaUiState,
    onCategorySelected: (GameCategory) -> Unit,
    onGameSelected: (String) -> Unit,
    onPlayQuickMatch: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val filteredGames = GameCatalog.games.filter { game ->
        (uiState.selectedCategory == GameCategory.ALL || game.category == uiState.selectedCategory) &&
                (uiState.searchQuery.isEmpty() || game.title.contains(uiState.searchQuery, ignoreCase = true) || game.description.contains(uiState.searchQuery, ignoreCase = true))
    }

    LazyColumn(
        modifier = modifier
            .fillMaxSize()
            .testTag("discovery_screen"),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp)
    ) {
        // Hero Studio Banner
        item {
            HeroStudioBanner(
                playerRating = uiState.playerRating,
                onQuickPlayChess = { onPlayQuickMatch("chess") },
                onQuickPlayTtt = { onPlayQuickMatch("tictactoe") }
            )
        }

        // Category Filter Chips
        item {
            Text(
                text = "DISCOVER GAMES",
                style = MaterialTheme.typography.labelMedium,
                color = TabulaSecondary,
                letterSpacing = 2.sp
            )
            Spacer(modifier = Modifier.height(8.dp))
            LazyRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.fillMaxWidth()
            ) {
                items(GameCategory.values()) { category ->
                    val isSelected = uiState.selectedCategory == category
                    FilterChip(
                        selected = isSelected,
                        onClick = { onCategorySelected(category) },
                        label = { Text(category.title, fontSize = 13.sp) },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = TabulaPrimary,
                            selectedLabelColor = TabulaOnPrimary,
                            containerColor = TabulaDarkSurfaceVariant,
                            labelColor = TabulaOnSurfaceVariant
                        ),
                        border = FilterChipDefaults.filterChipBorder(
                            enabled = true,
                            selected = isSelected,
                            borderColor = if (isSelected) TabulaSecondary else TabulaOutline
                        )
                    )
                }
            }
        }

        // Game Catalog List
        items(filteredGames) { game ->
            GameCard(
                game = game,
                onClick = { onGameSelected(game.id) },
                onPlayNow = { onPlayQuickMatch(game.id) }
            )
        }
    }
}

@Composable
fun HeroStudioBanner(
    playerRating: Int,
    onQuickPlayChess: () -> Unit,
    onQuickPlayTtt: () -> Unit,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(20.dp))
            .border(1.dp, TabulaPrimary.copy(alpha = 0.5f), RoundedCornerShape(20.dp)),
        colors = CardDefaults.cardColors(containerColor = TabulaDarkSurfaceVariant)
    ) {
        Column {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(140.dp)
            ) {
                Image(
                    painter = painterResource(id = R.drawable.img_hero_tabula_1791356971686),
                    contentDescription = "Tabula Hero Banner",
                    modifier = Modifier.fillMaxSize(),
                    contentScale = ContentScale.Crop
                )
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(
                            androidx.compose.ui.graphics.Brush.verticalGradient(
                                colors = listOf(
                                    Color.Transparent,
                                    TabulaDarkSurfaceVariant.copy(alpha = 0.95f)
                                )
                            )
                        )
                )
                // Rating badge
                Surface(
                    shape = RoundedCornerShape(12.dp),
                    color = TabulaDarkSurface.copy(alpha = 0.85f),
                    border = androidx.compose.foundation.BorderStroke(1.dp, TabulaTertiary),
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(12.dp)
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                    ) {
                        Icon(
                            Icons.Default.Star,
                            contentDescription = "Rating",
                            tint = TabulaTertiary,
                            modifier = Modifier.size(16.dp)
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(
                            text = "$playerRating Elo",
                            style = MaterialTheme.typography.labelMedium,
                            color = TabulaOnSurface,
                            fontWeight = FontWeight.Bold
                        )
                    }
                }
            }

            Column(modifier = Modifier.padding(16.dp)) {
                Text(
                    text = "TABULA GAME RUNTIME",
                    style = MaterialTheme.typography.labelSmall,
                    color = TabulaSecondary,
                    letterSpacing = 1.5.sp
                )
                Text(
                    text = "Deterministic Server-Authoritative Games",
                    style = MaterialTheme.typography.titleLarge,
                    color = TabulaOnSurface,
                    fontWeight = FontWeight.Bold
                )
                Text(
                    text = "Pure function rules with zero floating point drift, cryptographic projections, and instant rollback-free replays.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = TabulaOnSurfaceVariant,
                    modifier = Modifier.padding(vertical = 8.dp)
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    Button(
                        onClick = onQuickPlayChess,
                        colors = ButtonDefaults.buttonColors(containerColor = TabulaPrimary),
                        shape = RoundedCornerShape(12.dp),
                        modifier = Modifier
                            .weight(1f)
                            .testTag("quick_play_chess")
                    ) {
                        Text("Play Chess ♟️", fontWeight = FontWeight.Bold)
                    }
                    OutlinedButton(
                        onClick = onQuickPlayTtt,
                        colors = ButtonDefaults.outlinedButtonColors(contentColor = TabulaSecondary),
                        border = androidx.compose.foundation.BorderStroke(1.dp, TabulaSecondary),
                        shape = RoundedCornerShape(12.dp),
                        modifier = Modifier
                            .weight(1f)
                            .testTag("quick_play_tictactoe")
                    ) {
                        Text("Tic-Tac-Toe ⭕", fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

@Composable
fun GameCard(
    game: GameSummary,
    onClick: () -> Unit,
    onPlayNow: () -> Unit,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .border(1.dp, TabulaOutline, RoundedCornerShape(16.dp)),
        colors = CardDefaults.cardColors(containerColor = TabulaDarkSurface),
        shape = RoundedCornerShape(16.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth()
            ) {
                // Game Icon badge
                Box(
                    modifier = Modifier
                        .size(48.dp)
                        .clip(RoundedCornerShape(12.dp))
                        .background(TabulaPrimaryContainer)
                        .border(1.dp, TabulaPrimary, RoundedCornerShape(12.dp)),
                    contentAlignment = Alignment.Center
                ) {
                    Text(text = game.bannerIcon, fontSize = 24.sp)
                }

                Spacer(modifier = Modifier.width(12.dp))

                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = game.title,
                        style = MaterialTheme.typography.titleMedium,
                        color = TabulaOnSurface,
                        fontWeight = FontWeight.Bold
                    )
                    Text(
                        text = game.subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = TabulaOnSurfaceVariant
                    )
                }

                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = TabulaDarkSurfaceVariant,
                    border = androidx.compose.foundation.BorderStroke(1.dp, TabulaOutline)
                ) {
                    Text(
                        text = game.complexity.label,
                        style = MaterialTheme.typography.labelSmall,
                        color = TabulaSecondary,
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp)
                    )
                }
            }

            Spacer(modifier = Modifier.height(10.dp))

            Text(
                text = game.description,
                style = MaterialTheme.typography.bodyMedium,
                color = TabulaOnSurfaceVariant,
                maxLines = 2
            )

            Spacer(modifier = Modifier.height(12.dp))

            // Capabilities tags
            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                modifier = Modifier.fillMaxWidth()
            ) {
                game.capabilities.take(2).forEach { cap ->
                    Surface(
                        shape = RoundedCornerShape(6.dp),
                        color = TabulaDarkSurfaceVariant.copy(alpha = 0.6f)
                    ) {
                        Text(
                            text = "• $cap",
                            style = MaterialTheme.typography.labelSmall,
                            color = TabulaOnSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(14.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "👥 ${game.players}",
                        style = MaterialTheme.typography.labelSmall,
                        color = TabulaOnSurfaceVariant
                    )
                    Text(
                        text = "⏱️ ${game.estMinutes}",
                        style = MaterialTheme.typography.labelSmall,
                        color = TabulaOnSurfaceVariant
                    )
                }

                if (game.isPlayableInApp) {
                    Button(
                        onClick = onPlayNow,
                        colors = ButtonDefaults.buttonColors(containerColor = TabulaPrimary),
                        shape = RoundedCornerShape(10.dp),
                        contentPadding = PaddingValues(horizontal = 14.dp, vertical = 6.dp)
                    ) {
                        Text("Play Local", fontSize = 12.sp, fontWeight = FontWeight.Bold)
                    }
                } else {
                    OutlinedButton(
                        onClick = onClick,
                        shape = RoundedCornerShape(10.dp),
                        contentPadding = PaddingValues(horizontal = 14.dp, vertical = 6.dp)
                    ) {
                        Text("Explore Rules", fontSize = 12.sp)
                    }
                }
            }
        }
    }
}
