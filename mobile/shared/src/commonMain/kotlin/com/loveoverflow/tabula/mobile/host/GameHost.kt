package com.loveoverflow.tabula.mobile.host

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

/**
 * What the shell asks the host to present. The shell treats [gameId] as an opaque catalog key
 * and never branches on it: game meaning belongs to the Rust registry and rules (I-9).
 */
data class GameLaunch(val gameId: String)

/** What a [GameHost] reports back to the shell. */
sealed interface GameHostEvent {
    /** The game surface is interactive. */
    data object Ready : GameHostEvent

    /** The surface could not be presented. [reason] is for diagnostics, not for rules. */
    data class Failed(val reason: String) : GameHostEvent

    /** The player left the game surface. */
    data object Exited : GameHostEvent
}

/**
 * The seam between the Compose shell and the surface that runs a game.
 *
 * The next step in the mobile sequence supplies a WebView implementation that loads the
 * existing Rust/WASM game document. This foundation fixes only the contract: the shell
 * decides when a game is shown and reacts to [GameHostEvent]; the host owns the surface.
 * Neither side carries rules, canonical state or presentation state across this boundary.
 */
interface GameHost {
    @Composable
    fun Content(launch: GameLaunch, onEvent: (GameHostEvent) -> Unit, modifier: Modifier)
}
