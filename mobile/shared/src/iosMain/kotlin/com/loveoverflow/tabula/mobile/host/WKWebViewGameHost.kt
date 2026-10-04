package com.loveoverflow.tabula.mobile.host

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.UIKitView

/**
 * The iOS [GameHost]: presents a packaged game document in a WKWebView (ADR-0033).
 *
 * Same contract as the Android host (both use [rememberGameRuntime]): the runtime is built once per
 * composition entry and only "Try again" remounts it. Compiled for iOS (klib) but not executed in this change.
 */
class WKWebViewGameHost(private val games: List<BundledGame>) : GameHost {
    @Composable
    override fun Content(launch: GameLaunch, onEvent: (GameHostEvent) -> Unit, modifier: Modifier, back: GameBackPort) {
        val game = games.firstOrNull { it.id == launch.gameId }
        if (game == null || launch.preferences == null) {
            LaunchedEffect(launch) {
                onEvent(GameHostEvent.Failed(if (game == null) "unknown-game" else "missing-preferences", shownByGame = false))
            }
            return
        }
        val runtime = rememberGameRuntime(back, onEvent) { notify -> IosGameRuntime(game, launch, notify) }
        UIKitView(factory = { runtime.view }, modifier = modifier)
    }
}
