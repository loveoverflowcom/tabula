package com.loveoverflow.tabula.mobile

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import kotlinx.coroutines.delay
import com.loveoverflow.tabula.mobile.voice.DevVoiceGrantSource
import com.loveoverflow.tabula.mobile.voice.VoiceController
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.backhandler.BackHandler
import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.design.TabulaTheme
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.navigation.BackStack
import com.loveoverflow.tabula.mobile.navigation.BackStackSaver
import com.loveoverflow.tabula.mobile.navigation.Destination
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.shell.AccountScreen
import com.loveoverflow.tabula.mobile.shell.DetailScreen
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import com.loveoverflow.tabula.mobile.shell.GameScreen
import com.loveoverflow.tabula.mobile.shell.GamesScreen
import com.loveoverflow.tabula.mobile.shell.HomeScreen
import com.loveoverflow.tabula.mobile.shell.PlaceholderGameHost
import com.loveoverflow.tabula.mobile.shell.SetupScreen
import com.loveoverflow.tabula.mobile.shell.ShellChrome
import com.loveoverflow.tabula.mobile.shell.gamePreferences
import com.loveoverflow.tabula.mobile.shell.rememberDeviceFacts

/**
 * The host services a first-party packaged game may use. Anything else a page asks for is denied.
 * Voice credentials/audio stay native and are deliberately absent from this bridge (ADR-0037).
 */
internal val FirstPartyCapabilities: Set<HostCapability> = setOf(HostCapability.KeepAwake)

/**
 * The Compose Multiplatform app root shared by Android and iOS (ADR-0032, ADR-0033).
 *
 * It owns screens and navigation only. [gameHost] is where the platform presents a game surface and
 * [games] is the list of games packaged with this build; with neither, the shell says so.
 * [scheme] and [deviceFacts] allow the same shell to be exercised with explicit presentation
 * settings in previews. Production callers omit them to read the host's settings.
 */
@OptIn(ExperimentalComposeUiApi::class)
@Suppress("DEPRECATION")
@Composable
fun TabulaApp(
    gameHost: GameHost = PlaceholderGameHost,
    games: List<BundledGame> = emptyList(),
    voice: VoiceController? = null,
    voiceScope: String = DevVoiceGrantSource.SCOPE,
    scheme: TabulaScheme? = null,
    deviceFacts: DeviceFacts? = null,
) {
    // Voice lifetime belongs to the app/session owner, not the replaceable WebView runtime.
    DisposableEffect(voice) { onDispose { voice?.close() } }
    LaunchedEffect(voice) {
        if (voice != null) while (true) {
            delay(1_000)
            voice.checkAuthorityDeadline()
        }
    }
    // A recreated host returns to a shell location; an active local game is never resumed.
    var history by rememberSaveable(stateSaver = BackStackSaver) { mutableStateOf(BackStack.Root) }
    val selectedScheme = scheme ?: if (isSystemInDarkTheme()) TabulaScheme.Dark else TabulaScheme.Light
    val dark = selectedScheme == TabulaScheme.Dark || selectedScheme == TabulaScheme.HcDark
    val device = deviceFacts ?: rememberDeviceFacts()
    val strings = ShellStrings.forLanguage(device.languageTag)
    val openLocalGame: (BundledGame) -> Unit = { game ->
        // Preferences are read at explicit launch, never recovered as match/session authority.
        val launch = GameLaunch(game.id, gamePreferences(dark, device.copy(languageTag = strings.languageTag)), FirstPartyCapabilities)
        voice?.enterSession(voiceScope)
        history = history.push(Destination.Game(launch))
    }
    TabulaTheme(scheme = selectedScheme) {
        val destination = history.current
        if (destination is Destination.Game) {
            GameScreen(
                title = games.firstOrNull { it.id == destination.launch.gameId }?.displayName(strings.languageTag)
                    ?: destination.launch.gameId,
                launch = destination.launch,
                host = gameHost,
                onLeave = { voice?.leaveSession(); history = history.pop() },
                voice = voice,
                vietnamese = strings.vietnamese,
                strings = strings,
            )
        } else {
            // Gameplay keeps its existing host-first Back handling; shell routes pop directly.
            BackHandler(enabled = history.canPop, onBack = { history = history.pop() })
            ShellChrome(
                destination = destination,
                strings = strings,
                onNavigate = { history = history.navigate(it) },
                onBack = { history = history.pop() },
            ) {
                when (destination) {
                    Destination.Home -> HomeScreen(
                        games = games,
                        strings = strings,
                        onOpen = openLocalGame,
                        onBrowse = { history = history.navigate(Destination.Games) },
                    )
                    Destination.Games -> GamesScreen(
                        games = games,
                        strings = strings,
                        onDetail = { history = history.push(Destination.Detail(it.id)) },
                    )
                    is Destination.Detail -> DetailScreen(
                        game = games.firstOrNull { it.id == destination.gameId },
                        strings = strings,
                        onSetup = { history = history.push(Destination.Setup(destination.gameId)) },
                    )
                    is Destination.Setup -> SetupScreen(
                        game = games.firstOrNull { it.id == destination.gameId },
                        strings = strings,
                        onPlay = { games.firstOrNull { it.id == destination.gameId }?.let(openLocalGame) },
                    )
                    Destination.Account -> AccountScreen(strings = strings)
                    is Destination.Game -> Unit
                }
            }
        }
    }
}
