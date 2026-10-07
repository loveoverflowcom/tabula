package com.loveoverflow.tabula.mobile

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import kotlinx.coroutines.delay
import com.loveoverflow.tabula.mobile.voice.DevVoiceGrantSource
import com.loveoverflow.tabula.mobile.voice.VoiceController
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.platform.LocalUriHandler
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.loveoverflow.tabula.mobile.account.AccountSessionPort
import com.loveoverflow.tabula.mobile.account.AccountState
import com.loveoverflow.tabula.mobile.account.UnavailableAccountSessionPort
import com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState
import com.loveoverflow.tabula.mobile.catalog.DiscoveryQuery
import com.loveoverflow.tabula.mobile.catalog.RegistryDiscoveryCatalog
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
import com.loveoverflow.tabula.mobile.localization.account
import com.loveoverflow.tabula.mobile.navigation.isAccountTask
import com.loveoverflow.tabula.mobile.navigation.AccountTaskBackPort
import com.loveoverflow.tabula.mobile.shell.AccountActions
import com.loveoverflow.tabula.mobile.shell.AccountAvatarImage
import com.loveoverflow.tabula.mobile.shell.AccountScreen
import com.loveoverflow.tabula.mobile.shell.LoginScreen
import com.loveoverflow.tabula.mobile.shell.RegisterScreen
import com.loveoverflow.tabula.mobile.shell.ProfileScreen
import com.loveoverflow.tabula.mobile.shell.FriendsScreen
import com.loveoverflow.tabula.mobile.shell.accountStatusCopy
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
 * The host services a first-party packaged game may use. Anything else a host asks for is denied.
 * Voice credentials/audio stay native and are deliberately absent from this bridge (ADR-0037).
 */
internal val FirstPartyCapabilities: Set<HostCapability> = setOf(HostCapability.KeepAwake)

/**
 * The Compose Multiplatform app root shared by Android and iOS (ADR-0032, ADR-0043).
 *
 * It owns screens and navigation only. [gameHost] is where the platform presents a game surface and
 * [games] is the exact runtime inventory supplied by that host. [catalog] contains independent
 * public registry discovery facts and cannot establish native launch availability (ADR-0045).
 * [scheme] and [deviceFacts] allow the same shell to be exercised with explicit presentation
 * settings in previews. Production callers omit them to read the host's settings.
 * [account] is the app-owned current-session presentation port, closed on app disposal. Its
 * production default stays unavailable; explicit preview adapters belong only to `previewApp`.
 * [accountAvatar] is an already-loaded managed image bound to the exact current identity snapshot,
 * not a remote image source or account credential. Neither value enters saved shell state.
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
    catalog: DiscoveryCatalogState = DiscoveryCatalogState.Ready(RegistryDiscoveryCatalog.games),
    onRetryCatalog: (() -> Unit)? = null,
    account: AccountSessionPort = UnavailableAccountSessionPort,
    accountAvatar: AccountAvatarImage? = null,
) {
    // Account display is app-owned, never saved with navigation or held by a game runtime.
    // Stop/background retires pending work and private facts; Start cannot silently sign in.
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(account) { onDispose { account.close() } }
    DisposableEffect(account, lifecycleOwner) {
        val lifecycle = lifecycleOwner.lifecycle
        account.onForegroundChanged(lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED))
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> account.onForegroundChanged(true)
                Lifecycle.Event.ON_STOP -> account.onForegroundChanged(false)
                else -> Unit
            }
        }
        lifecycle.addObserver(observer)
        onDispose {
            lifecycle.removeObserver(observer)
            account.onForegroundChanged(false)
        }
    }
    // Replacing a port must not retain the previous port's initial collected presentation.
    val accountState = key(account) { account.state.collectAsState().value }
    // Voice lifetime belongs to the app/session owner, not the replaceable game runtime.
    DisposableEffect(voice) { onDispose { voice?.close() } }
    LaunchedEffect(voice) {
        if (voice != null) while (true) {
            delay(1_000)
            voice.checkAuthorityDeadline()
        }
    }
    // A recreated host returns to a shell location; an active local game is never resumed.
    var history by rememberSaveable(stateSaver = BackStackSaver) { mutableStateOf(BackStack.Root) }
    var query by rememberSaveable(stateSaver = DiscoveryQuerySaver) { mutableStateOf(DiscoveryQuery()) }
    val screenState = rememberSaveableStateHolder()
    val uriHandler = LocalUriHandler.current
    val selectedScheme = scheme ?: if (isSystemInDarkTheme()) TabulaScheme.Dark else TabulaScheme.Light
    val dark = selectedScheme == TabulaScheme.Dark || selectedScheme == TabulaScheme.HcDark
    val device = deviceFacts ?: rememberDeviceFacts()
    val strings = ShellStrings.forLanguage(device.languageTag)
    val accountBack = remember { AccountTaskBackPort() }
    val navigateShell: (Destination) -> Unit = { target ->
        val next = history.navigate(target)
        if (next != history && history.current.isAccountTask) account.cancelPending()
        history = next
    }
    val popShell: () -> Unit = {
        if (!accountBack.requestBack() && history.canPop) {
            if (history.current.isAccountTask) account.cancelPending()
            history = history.pop()
        }
    }
    val accountActions = AccountActions(
        navigate = { target ->
            val next = if (target == Destination.Home || target == Destination.Games) history.navigate(target) else history.push(target)
            if (next != history && history.current.isAccountTask) account.cancelPending()
            history = next
        },
        refresh = account::refresh,
        signOut = account::signOut,
        cancel = account::cancelPending,
        back = accountBack,
    )
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
            // Gameplay keeps host-first Back; account confirmation dismisses before route Back.
            BackHandler(enabled = history.canPop, onBack = popShell)
            ShellChrome(
                destination = destination,
                strings = strings,
                onNavigate = navigateShell,
                onBack = popShell,
                accountIdentity = (accountState as? AccountState.Authenticated)?.identity,
                accountAvatar = accountAvatar,
                accountDescription = strings.account(accountStatusCopy(accountState)),
            ) {
                screenState.SaveableStateProvider(destination.routePath()) {
                    val discoveryGames = (catalog as? DiscoveryCatalogState.Ready)?.games.orEmpty()
                    when (destination) {
                        Destination.Home -> HomeScreen(
                            catalog = catalog,
                            strings = strings,
                            onBrowse = { history = history.navigate(Destination.Games) },
                            onDetail = { history = history.push(Destination.Detail(it.id)) },
                            onRetry = onRetryCatalog,
                        )
                        Destination.Games -> GamesScreen(
                            catalog = catalog,
                            query = query,
                            strings = strings,
                            onQueryChange = { query = it },
                            onDetail = { history = history.push(Destination.Detail(it.id)) },
                            onRetry = onRetryCatalog,
                        )
                        is Destination.Detail -> DetailScreen(
                            game = discoveryGames.firstOrNull { it.id == destination.gameId },
                            catalog = catalog,
                            strings = strings,
                            canLaunch = gameHost !== PlaceholderGameHost && games.any { it.id == destination.gameId },
                            onSetup = { history = history.push(Destination.Setup(destination.gameId)) },
                            onOpenRules = { uri -> uriHandler.openUri(uri) },
                            onRetry = onRetryCatalog,
                        )
                        is Destination.Setup -> SetupScreen(
                            game = discoveryGames.firstOrNull { it.id == destination.gameId },
                            catalog = catalog,
                            strings = strings,
                            canLaunch = gameHost !== PlaceholderGameHost && games.any { it.id == destination.gameId },
                            onPlay = {
                                if (gameHost !== PlaceholderGameHost && discoveryGames.any { it.id == destination.gameId }) {
                                    games.firstOrNull { it.id == destination.gameId }?.let(openLocalGame)
                                }
                            },
                            onRetry = onRetryCatalog,
                        )
                        Destination.Account -> AccountScreen(accountState, strings, accountActions, accountAvatar)
                        Destination.Login -> LoginScreen(accountState, strings, accountActions)
                        Destination.Register -> RegisterScreen(accountState, strings, accountActions)
                        Destination.Profile -> ProfileScreen(accountState, strings, accountActions, accountAvatar)
                        Destination.Friends -> FriendsScreen(accountState, strings, accountActions)
                        is Destination.Game -> Unit
                    }
                }
            }
        }
    }
}

/** Saves only public discovery preferences; these values grant no launch or match authority. */
private val DiscoveryQuerySaver = listSaver<DiscoveryQuery, String>(
    save = { listOf(it.text, it.category.orEmpty(), it.players?.toString().orEmpty(), it.maxMinutes?.toString().orEmpty(), it.complexity.orEmpty()) },
    restore = { values ->
        if (values.size != 5) DiscoveryQuery() else DiscoveryQuery(
            text = values[0],
            category = values[1].ifEmpty { null },
            players = values[2].toIntOrNull()?.takeIf { it > 0 },
            maxMinutes = values[3].toIntOrNull()?.takeIf { it > 0 },
            complexity = values[4].ifEmpty { null },
        )
    },
)
