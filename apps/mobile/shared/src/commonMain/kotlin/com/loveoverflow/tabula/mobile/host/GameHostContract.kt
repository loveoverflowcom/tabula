package com.loveoverflow.tabula.mobile.host

import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.HostCapability

/**
 * What the shell asks the host to present. The shell treats [gameId] as an opaque catalog key
 * and never branches on it: game meaning belongs to the Rust registry and rules (I-9).
 *
 * [capabilities] is the complete set of host services this launch may use; a request for any other
 * service is denied. [preferences] are the shell's view of the device (theme, reduced motion,
 * language) and are handed to the game once, at start.
 */
data class GameLaunch(
    val gameId: String,
    val preferences: GamePreferences? = null,
    val capabilities: Set<HostCapability> = emptySet(),
)

/** What a [GameHost] reports back to the shell. */
sealed interface GameHostEvent {
    /** The game surface is interactive. [bootMs] is runtime start to first usable interactive frame, measured by the host. */
    data class Ready(val bootMs: Int) : GameHostEvent

    /**
     * The surface could not continue. [reason] is for diagnostics, not for rules. When
     * [shownByGame] the game surface already shows its own recovery overlay; otherwise the
     * shell must explain the failure itself because no game UI is available to do so.
     */
    data class Failed(val reason: String, val shownByGame: Boolean) : GameHostEvent

    /** The player left the game surface. */
    data object Exited : GameHostEvent
}

/**
 * Lets the shell ask the active host to handle a Back request first. While a match is live the
 * game owns the leave confirmation; the shell leaves the screen itself only when the host does
 * not consume the request. A host registers its handler for as long as it is composed.
 */
class GameBackPort {
    private var handler: (() -> Boolean)? = null

    fun register(handler: (() -> Boolean)?) {
        this.handler = handler
    }

    /** `true` when the host handled the request; `false` when the shell should pop the screen. */
    fun requestBack(): Boolean = handler?.invoke() ?: false
}
