package com.loveoverflow.tabula.mobile

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.design.TabulaTheme
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.navigation.BackStack
import com.loveoverflow.tabula.mobile.navigation.Destination
import com.loveoverflow.tabula.mobile.shell.GameScreen
import com.loveoverflow.tabula.mobile.shell.HomeScreen
import com.loveoverflow.tabula.mobile.shell.PlaceholderGameHost
import com.loveoverflow.tabula.mobile.shell.gamePreferences
import com.loveoverflow.tabula.mobile.shell.rememberDeviceFacts

/**
 * The host services a first-party packaged game may use. Anything else a page asks for is denied.
 * Online play, voice and credentials are deliberately absent (ADR-0032 "Not opened").
 */
internal val FirstPartyCapabilities: Set<HostCapability> = setOf(HostCapability.KeepAwake)

/**
 * The Compose Multiplatform app root shared by Android and iOS (ADR-0032, ADR-0033).
 *
 * It owns screens and navigation only. [gameHost] is where the platform presents a game surface and
 * [games] is the list of games packaged with this build; with neither, the shell says so.
 */
@Composable
fun TabulaApp(gameHost: GameHost = PlaceholderGameHost, games: List<BundledGame> = emptyList()) {
    var history by remember { mutableStateOf(BackStack.Root) }
    val dark = isSystemInDarkTheme()
    val device = rememberDeviceFacts()
    TabulaTheme {
        when (val destination = history.current) {
            Destination.Home -> HomeScreen(
                games = games,
                languageTag = device.languageTag,
                onOpen = { game ->
                    // Preferences are read at the moment the player opens the game.
                    val launch = GameLaunch(game.id, gamePreferences(dark, device), FirstPartyCapabilities)
                    history = history.push(Destination.Game(launch))
                },
            )
            is Destination.Game -> GameScreen(
                title = games.firstOrNull { it.id == destination.launch.gameId }?.displayName(device.languageTag)
                    ?: destination.launch.gameId,
                launch = destination.launch,
                host = gameHost,
                onLeave = { history = history.pop() },
            )
        }
    }
}
