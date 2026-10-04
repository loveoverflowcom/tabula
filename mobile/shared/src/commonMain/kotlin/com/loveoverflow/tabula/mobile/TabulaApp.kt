package com.loveoverflow.tabula.mobile

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.loveoverflow.tabula.mobile.design.TabulaTheme
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.navigation.BackStack
import com.loveoverflow.tabula.mobile.navigation.Destination
import com.loveoverflow.tabula.mobile.shell.GameScreen
import com.loveoverflow.tabula.mobile.shell.HomeScreen
import com.loveoverflow.tabula.mobile.shell.PlaceholderGameHost

/** Opaque key for the foundation's reserved slot; no game is selected or implied by it. */
internal const val SlotPreviewGameId = "slot-preview"

/**
 * The Compose Multiplatform app root shared by Android and iOS (ADR-0032).
 *
 * It owns screens and navigation only. [gameHost] is the seam where the platform presents a
 * game surface; the default draws a placeholder.
 */
@Composable
fun TabulaApp(gameHost: GameHost = PlaceholderGameHost) {
    var history by remember { mutableStateOf(BackStack.Root) }
    TabulaTheme {
        when (val destination = history.current) {
            Destination.Home -> HomeScreen(
                onOpenGameSlot = { history = history.push(Destination.Game(GameLaunch(SlotPreviewGameId))) },
            )
            is Destination.Game -> GameScreen(
                launch = destination.launch,
                host = gameHost,
                // Failed keeps the player on the screen that explains it; only Exited leaves.
                onEvent = { event -> if (event == GameHostEvent.Exited) history = history.pop() },
                onBack = { history = history.pop() },
            )
        }
    }
}
