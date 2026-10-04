package com.loveoverflow.tabula.mobile.host

import android.content.res.AssetManager
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.webkit.WebViewFeature

/**
 * The Android [GameHost]: presents a packaged game document in a WebView (ADR-0033).
 *
 * The runtime comes from [rememberGameRuntime]: built once per composition entry and never rebuilt by
 * a recomposition, with events delivered to the latest `onEvent`. The shell's
 * "Try again" remounts this composable, which is the only way a second runtime is created.
 */
class WebViewGameHost(
    private val games: List<BundledGame>,
    private val assets: AssetManager,
) : GameHost {
    @Composable
    override fun Content(launch: GameLaunch, onEvent: (GameHostEvent) -> Unit, modifier: Modifier, back: GameBackPort) {
        val game = games.firstOrNull { it.id == launch.gameId }
        val unusable = when {
            game == null -> "unknown-game"
            launch.preferences == null -> "missing-preferences"
            // Without an origin-restricted port the page could not tell it is hosted; refuse to load it.
            !WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER) -> "bridge-unsupported"
            else -> null
        }
        if (unusable != null || game == null) {
            LaunchedEffect(launch) { onEvent(GameHostEvent.Failed(unusable ?: "unknown-game", shownByGame = false)) }
            return
        }
        val context = LocalContext.current
        val runtime = rememberGameRuntime(back, onEvent) { notify -> AndroidGameRuntime(context, game, launch, assets, notify) }
        AndroidView(factory = { runtime.view }, modifier = modifier)
    }
}
