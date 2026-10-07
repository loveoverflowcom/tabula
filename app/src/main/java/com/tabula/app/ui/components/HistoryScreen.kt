package com.tabula.app.ui.components

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tabula.app.data.SavedMatchEntity
import com.tabula.app.model.MatchHistoryCatalog
import com.tabula.app.ui.theme.*

@Composable
fun HistoryScreen(
    savedMatches: List<SavedMatchEntity>,
    modifier: Modifier = Modifier
) {
    LazyColumn(
        modifier = modifier
            .fillMaxSize()
            .testTag("history_screen"),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        item {
            Column {
                Text(
                    text = "DETERMINISTIC LOGS",
                    style = MaterialTheme.typography.labelSmall,
                    color = TabulaSecondary,
                    letterSpacing = 2.sp
                )
                Text(
                    text = "Match History & Replays",
                    style = MaterialTheme.typography.headlineMedium,
                    color = TabulaOnSurface,
                    fontWeight = FontWeight.Bold
                )
                Text(
                    text = "All matches store verified event sequences. Any game can be re-run byte-identically.",
                    style = MaterialTheme.typography.bodySmall,
                    color = TabulaOnSurfaceVariant
                )
            }
        }

        // Live locally recorded matches from Room DB
        if (savedMatches.isNotEmpty()) {
            item {
                Text(
                    text = "RECENT RUNTIME MATCHES (${savedMatches.size})",
                    style = MaterialTheme.typography.labelSmall,
                    color = TabulaPrimary,
                    fontWeight = FontWeight.Bold
                )
            }
            items(savedMatches) { m ->
                MatchCardItem(
                    title = m.gameTitle,
                    opponent = m.opponent,
                    result = m.result,
                    ratingDelta = m.ratingChange,
                    moves = "${m.moveCount} moves",
                    time = m.durationText
                )
            }
        }

        // Standard Catalog sample replays
        item {
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = "COMMITTED .TBR REPLAYS",
                style = MaterialTheme.typography.labelSmall,
                color = TabulaSecondary,
                fontWeight = FontWeight.Bold
            )
        }

        items(MatchHistoryCatalog.sampleHistory) { item ->
            MatchCardItem(
                title = item.gameTitle,
                opponent = item.opponent,
                result = item.result,
                ratingDelta = item.scoreChange,
                moves = "${item.movesCount} moves",
                time = "${item.playedAt} (${item.duration})"
            )
        }
    }
}

@Composable
fun MatchCardItem(
    title: String,
    opponent: String,
    result: String,
    ratingDelta: String,
    moves: String,
    time: String,
    modifier: Modifier = Modifier
) {
    val isWin = result == "Victory"

    Card(
        modifier = modifier
            .fillMaxWidth()
            .border(1.dp, TabulaOutline, RoundedCornerShape(12.dp)),
        colors = CardDefaults.cardColors(containerColor = TabulaDarkSurface),
        shape = RoundedCornerShape(12.dp)
    ) {
        Row(
            modifier = Modifier
                .padding(14.dp)
                .fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Column {
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleSmall,
                    color = TabulaOnSurface,
                    fontWeight = FontWeight.Bold
                )
                Text(
                    text = "vs $opponent • $moves",
                    style = MaterialTheme.typography.bodySmall,
                    color = TabulaOnSurfaceVariant
                )
                Text(
                    text = time,
                    style = MaterialTheme.typography.labelSmall,
                    color = TabulaOnSurfaceVariant.copy(alpha = 0.7f),
                    fontSize = 11.sp
                )
            }

            Column(horizontalAlignment = Alignment.End) {
                Surface(
                    shape = RoundedCornerShape(6.dp),
                    color = if (isWin) TabulaTurnActive.copy(alpha = 0.2f) else TabulaIllegalTarget.copy(alpha = 0.2f)
                ) {
                    Text(
                        text = result.uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        color = if (isWin) TabulaTurnActive else TabulaIllegalTarget,
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
                        fontWeight = FontWeight.Bold
                    )
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = ratingDelta,
                    style = MaterialTheme.typography.labelSmall,
                    color = if (isWin) TabulaTurnActive else TabulaOnSurfaceVariant,
                    fontWeight = FontWeight.Bold
                )
            }
        }
    }
}
