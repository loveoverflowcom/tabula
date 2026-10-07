package com.tabula.app.ui.components

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.MeetingRoom
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.tabula.app.model.RoomCatalog
import com.tabula.app.model.RoomItem
import com.tabula.app.ui.theme.*

@Composable
fun RoomsScreen(
    onJoinRoom: (RoomItem) -> Unit,
    modifier: Modifier = Modifier
) {
    var showCreateDialog by remember { mutableStateOf(false) }

    LazyColumn(
        modifier = modifier
            .fillMaxSize()
            .testTag("rooms_screen"),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        // Header
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text(
                        text = "MULTIPLAYER LOBBY",
                        style = MaterialTheme.typography.labelSmall,
                        color = TabulaSecondary,
                        letterSpacing = 2.sp
                    )
                    Text(
                        text = "Active Game Rooms",
                        style = MaterialTheme.typography.headlineMedium,
                        color = TabulaOnSurface,
                        fontWeight = FontWeight.Bold
                    )
                }

                Button(
                    onClick = { showCreateDialog = true },
                    colors = ButtonDefaults.buttonColors(containerColor = TabulaPrimary),
                    shape = RoundedCornerShape(10.dp)
                ) {
                    Icon(Icons.Default.Add, contentDescription = null)
                    Spacer(modifier = Modifier.width(4.dp))
                    Text("Create Room")
                }
            }
        }

        // Room list items
        items(RoomCatalog.sampleRooms) { room ->
            RoomCard(room = room, onJoin = { onJoinRoom(room) })
        }
    }

    if (showCreateDialog) {
        AlertDialog(
            onDismissRequest = { showCreateDialog = false },
            title = { Text("Create Multiplayer Room", fontWeight = FontWeight.Bold) },
            text = {
                Text(
                    "Provision a server-authoritative match session with custom timer, seat capacities, and bot auto-fill policies.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = TabulaOnSurfaceVariant
                )
            },
            confirmButton = {
                Button(
                    onClick = { showCreateDialog = false },
                    colors = ButtonDefaults.buttonColors(containerColor = TabulaPrimary)
                ) {
                    Text("Host Room")
                }
            },
            dismissButton = {
                TextButton(onClick = { showCreateDialog = false }) {
                    Text("Cancel")
                }
            },
            containerColor = TabulaDarkSurface
        )
    }
}

@Composable
fun RoomCard(room: RoomItem, onJoin: () -> Unit, modifier: Modifier = Modifier) {
    Card(
        modifier = modifier
            .fillMaxWidth()
            .border(1.dp, TabulaOutline, RoundedCornerShape(14.dp)),
        colors = CardDefaults.cardColors(containerColor = TabulaDarkSurface),
        shape = RoundedCornerShape(14.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = room.title,
                    style = MaterialTheme.typography.titleMedium,
                    color = TabulaOnSurface,
                    fontWeight = FontWeight.Bold
                )
                if (room.isRanked) {
                    Surface(
                        shape = RoundedCornerShape(6.dp),
                        color = TabulaTertiaryContainer
                    ) {
                        Text(
                            text = "RANKED",
                            style = MaterialTheme.typography.labelSmall,
                            color = TabulaOnTertiaryContainer,
                            modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(6.dp))

            Text(
                text = "Host: ${room.host} • Clock: ${room.timeControl}",
                style = MaterialTheme.typography.bodySmall,
                color = TabulaOnSurfaceVariant
            )

            Spacer(modifier = Modifier.height(12.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "Seats: ${room.currentSeats}/${room.maxSeats} (${room.status})",
                    style = MaterialTheme.typography.labelSmall,
                    color = TabulaTurnActive
                )

                Button(
                    onClick = onJoin,
                    colors = ButtonDefaults.buttonColors(containerColor = TabulaPrimary),
                    shape = RoundedCornerShape(8.dp),
                    contentPadding = PaddingValues(horizontal = 14.dp, vertical = 6.dp)
                ) {
                    Icon(Icons.Default.MeetingRoom, contentDescription = null, modifier = Modifier.size(16.dp))
                    Spacer(modifier = Modifier.width(6.dp))
                    Text("Join Table", fontSize = 12.sp, fontWeight = FontWeight.Bold)
                }
            }
        }
    }
}
