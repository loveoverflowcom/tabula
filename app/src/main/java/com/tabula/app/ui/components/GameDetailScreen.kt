package com.tabula.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Security
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tabula.app.model.GameCatalog
import com.tabula.app.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GameDetailScreen(
    gameId: String,
    onBack: () -> Unit,
    onPlay: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    val game = GameCatalog.getById(gameId) ?: return

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(game.title, fontWeight = FontWeight.Bold) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
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
        modifier = modifier.testTag("game_detail_screen")
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Header Card
            item {
                Card(
                    modifier = Modifier
                        .fillMaxWidth()
                        .border(1.dp, TabulaOutline, RoundedCornerShape(16.dp)),
                    colors = CardDefaults.cardColors(containerColor = TabulaDarkSurface)
                ) {
                    Column(modifier = Modifier.padding(20.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Box(
                                modifier = Modifier
                                    .size(56.dp)
                                    .clip(RoundedCornerShape(14.dp))
                                    .background(TabulaPrimaryContainer)
                                    .border(1.dp, TabulaPrimary, RoundedCornerShape(14.dp)),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(text = game.bannerIcon, fontSize = 28.sp)
                            }
                            Spacer(modifier = Modifier.width(16.dp))
                            Column {
                                Text(
                                    text = game.title,
                                    style = MaterialTheme.typography.titleLarge,
                                    color = TabulaOnSurface,
                                    fontWeight = FontWeight.Bold
                                )
                                Text(
                                    text = game.subtitle,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = TabulaSecondary
                                )
                            }
                        }

                        Spacer(modifier = Modifier.height(16.dp))

                        Row(
                            horizontalArrangement = Arrangement.spacedBy(16.dp),
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            MetaStatItem("Players", game.players, Modifier.weight(1f))
                            MetaStatItem("Duration", game.estMinutes, Modifier.weight(1f))
                            MetaStatItem("Complexity", game.complexity.label, Modifier.weight(1f))
                        }
                    }
                }
            }

            // Description
            item {
                DetailSection(title = "ARCHITECTURAL OVERVIEW") {
                    Text(
                        text = game.description,
                        style = MaterialTheme.typography.bodyMedium,
                        color = TabulaOnSurfaceVariant
                    )
                }
            }

            // Platform Capabilities Contract
            item {
                DetailSection(title = "PLATFORM CAPABILITIES (GAME.TOML)") {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        game.capabilities.forEach { cap ->
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(
                                    Icons.Default.Security,
                                    contentDescription = null,
                                    tint = TabulaTurnActive,
                                    modifier = Modifier.size(16.dp)
                                )
                                Spacer(modifier = Modifier.width(8.dp))
                                Text(
                                    text = cap,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = TabulaOnSurface
                                )
                            }
                        }
                    }
                }
            }

            // Actions
            item {
                Spacer(modifier = Modifier.height(8.dp))
                if (game.isPlayableInApp) {
                    Button(
                        onClick = { onPlay(game.id) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(52.dp)
                            .testTag("detail_play_button"),
                        colors = ButtonDefaults.buttonColors(containerColor = TabulaPrimary),
                        shape = RoundedCornerShape(14.dp)
                    ) {
                        Icon(Icons.Default.PlayArrow, contentDescription = null)
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = "Launch Match in Engine",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold
                        )
                    }
                } else {
                    OutlinedButton(
                        onClick = onBack,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(48.dp),
                        shape = RoundedCornerShape(14.dp)
                    ) {
                        Text("Back to Catalog")
                    }
                }
            }
        }
    }
}

@Composable
private fun MetaStatItem(label: String, value: String, modifier: Modifier = Modifier) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(10.dp),
        color = TabulaDarkSurfaceVariant,
        border = androidx.compose.foundation.BorderStroke(1.dp, TabulaOutline)
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                text = label.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = TabulaOnSurfaceVariant,
                fontSize = 10.sp
            )
            Text(
                text = value,
                style = MaterialTheme.typography.labelMedium,
                color = TabulaOnSurface,
                fontWeight = FontWeight.Bold
            )
        }
    }
}

@Composable
private fun DetailSection(
    title: String,
    content: @Composable () -> Unit
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .border(1.dp, TabulaOutline, RoundedCornerShape(14.dp)),
        colors = CardDefaults.cardColors(containerColor = TabulaDarkSurface),
        shape = RoundedCornerShape(14.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(
                text = title,
                style = MaterialTheme.typography.labelSmall,
                color = TabulaSecondary,
                letterSpacing = 1.sp,
                fontWeight = FontWeight.Bold
            )
            Spacer(modifier = Modifier.height(8.dp))
            content()
        }
    }
}
