package com.tabula.app.ui.components

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tabula.app.ui.theme.*

@Composable
fun SettingsScreen(
    playerRating: Int,
    wins: Int,
    losses: Int,
    draws: Int,
    modifier: Modifier = Modifier
) {
    var hapticFeedback by remember { mutableStateOf(true) }
    var soundEffects by remember { mutableStateOf(true) }
    var showLegalHighlights by remember { mutableStateOf(true) }
    var reducedMotion by remember { mutableStateOf(false) }

    LazyColumn(
        modifier = modifier
            .fillMaxSize()
            .testTag("settings_screen"),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        item {
            Column {
                Text(
                    text = "PLAYER IDENTITY",
                    style = MaterialTheme.typography.labelSmall,
                    color = TabulaSecondary,
                    letterSpacing = 2.sp
                )
                Text(
                    text = "Profile & Engine Settings",
                    style = MaterialTheme.typography.headlineMedium,
                    color = TabulaOnSurface,
                    fontWeight = FontWeight.Bold
                )
            }
        }

        // Player Stats Card
        item {
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, TabulaOutline, RoundedCornerShape(16.dp)),
                colors = CardDefaults.cardColors(containerColor = TabulaDarkSurface),
                shape = RoundedCornerShape(16.dp)
            ) {
                Column(modifier = Modifier.padding(18.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column {
                            Text(
                                text = "Tabula Grandmaster",
                                style = MaterialTheme.typography.titleLarge,
                                color = TabulaOnSurface,
                                fontWeight = FontWeight.Bold
                            )
                            Text(
                                text = "Competitive Board Gamer",
                                style = MaterialTheme.typography.bodySmall,
                                color = TabulaOnSurfaceVariant
                            )
                        }

                        Surface(
                            shape = RoundedCornerShape(12.dp),
                            color = TabulaPrimaryContainer,
                            border = androidx.compose.foundation.BorderStroke(1.dp, TabulaPrimary)
                        ) {
                            Text(
                                text = "$playerRating Elo",
                                style = MaterialTheme.typography.titleSmall,
                                color = TabulaOnPrimaryContainer,
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }

                    Spacer(modifier = Modifier.height(16.dp))

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        StatBadge("Wins", "$wins", TabulaTurnActive, Modifier.weight(1f))
                        StatBadge("Losses", "$losses", TabulaIllegalTarget, Modifier.weight(1f))
                        StatBadge("Draws", "$draws", TabulaTertiary, Modifier.weight(1f))
                    }
                }
            }
        }

        // Board & Presentation Preferences
        item {
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, TabulaOutline, RoundedCornerShape(16.dp)),
                colors = CardDefaults.cardColors(containerColor = TabulaDarkSurface),
                shape = RoundedCornerShape(16.dp)
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text(
                        text = "GAMEPLAY PRESENTATION",
                        style = MaterialTheme.typography.labelSmall,
                        color = TabulaSecondary,
                        letterSpacing = 1.sp,
                        fontWeight = FontWeight.Bold
                    )

                    Spacer(modifier = Modifier.height(10.dp))

                    SettingToggleRow(
                        title = "Move Highlight Affordances",
                        desc = "Show dots and danger rings for legal destinations",
                        checked = showLegalHighlights,
                        onCheckedChange = { showLegalHighlights = it }
                    )

                    HorizontalDivider(color = TabulaOutlineVariant, modifier = Modifier.padding(vertical = 8.dp))

                    SettingToggleRow(
                        title = "Haptic Move Confirmation",
                        desc = "Tactile feedback upon piece drop",
                        checked = hapticFeedback,
                        onCheckedChange = { hapticFeedback = it }
                    )

                    HorizontalDivider(color = TabulaOutlineVariant, modifier = Modifier.padding(vertical = 8.dp))

                    SettingToggleRow(
                        title = "Piece Slide Motion",
                        desc = "Follow spring token trajectories (<280ms)",
                        checked = !reducedMotion,
                        onCheckedChange = { reducedMotion = !it }
                    )

                    HorizontalDivider(color = TabulaOutlineVariant, modifier = Modifier.padding(vertical = 8.dp))

                    SettingToggleRow(
                        title = "Audio Effects & Clocks",
                        desc = "Play click and low-clock warning sound effects",
                        checked = soundEffects,
                        onCheckedChange = { soundEffects = it }
                    )
                }
            }
        }

        // Architecture info
        item {
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .border(1.dp, TabulaOutline, RoundedCornerShape(16.dp)),
                colors = CardDefaults.cardColors(containerColor = TabulaDarkSurfaceVariant),
                shape = RoundedCornerShape(16.dp)
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text(
                        text = "ENGINE PROTOCOL DETAILS",
                        style = MaterialTheme.typography.labelSmall,
                        color = TabulaOnSurfaceVariant,
                        letterSpacing = 1.sp
                    )
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        text = "• Rule Engine: Pure sync total function apply()\n• Encoding: Postcard v1 canonical hash\n• Offline Store: Room SQLite (com.tabula.app)\n• Target Runtime: Kotlin + Jetpack Compose",
                        style = MaterialTheme.typography.bodySmall,
                        color = TabulaOnSurface,
                        lineHeight = 20.sp
                    )
                }
            }
        }
    }
}

@Composable
fun StatBadge(label: String, value: String, color: androidx.compose.ui.graphics.Color, modifier: Modifier = Modifier) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(10.dp),
        color = TabulaDarkSurfaceVariant,
        border = androidx.compose.foundation.BorderStroke(1.dp, TabulaOutline)
    ) {
        Column(
            modifier = Modifier.padding(8.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text(
                text = value,
                style = MaterialTheme.typography.titleMedium,
                color = color,
                fontWeight = FontWeight.Bold
            )
            Text(
                text = label,
                style = MaterialTheme.typography.labelSmall,
                color = TabulaOnSurfaceVariant
            )
        }
    }
}

@Composable
fun SettingToggleRow(
    title: String,
    desc: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyMedium,
                color = TabulaOnSurface,
                fontWeight = FontWeight.SemiBold
            )
            Text(
                text = desc,
                style = MaterialTheme.typography.bodySmall,
                color = TabulaOnSurfaceVariant
            )
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = TabulaOnPrimary,
                checkedTrackColor = TabulaPrimary,
                uncheckedTrackColor = TabulaDarkSurfaceVariant
            )
        )
    }
}
