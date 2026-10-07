package com.loveoverflow.tabula.mobile.host

/** What a platform runtime exposes so the shared composition glue can drive it. */
interface GameRuntimeControls {
    /** `true` when the game will handle Back itself (it shows its own leave confirmation). */
    fun onBack(): Boolean

    /** The app is leaving the foreground: stop drawing, keep the match and its clocks. */
    fun suspend()

    /** The app is back in the foreground. */
    fun resume()

    /** Release the surface. Called exactly once when the composable leaves; must be idempotent. */
    fun dispose()
}
