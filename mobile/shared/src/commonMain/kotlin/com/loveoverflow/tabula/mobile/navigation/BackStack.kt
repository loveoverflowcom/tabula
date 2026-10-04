package com.loveoverflow.tabula.mobile.navigation

import com.loveoverflow.tabula.mobile.host.GameLaunch

/** A place the shell can show. Destinations carry data to display, never game state. */
sealed interface Destination {
    /** The shell's landing screen. */
    data object Home : Destination

    /** A game surface, presented through the platform's [com.loveoverflow.tabula.mobile.host.GameHost]. */
    data class Game(val launch: GameLaunch) : Destination
}

/**
 * Immutable navigation history; the root is always [Destination.Home].
 *
 * Navigation is pure so it is testable without a UI or a platform. The Compose layer holds
 * one value of this type and replaces it on user intent.
 */
class BackStack private constructor(private val entries: List<Destination>) {
    val current: Destination get() = entries.last()
    val canPop: Boolean get() = entries.size > 1

    fun push(destination: Destination): BackStack = BackStack(entries + destination)

    /** Drops the current entry; the root cannot be popped, so this returns `this` there. */
    fun pop(): BackStack = if (canPop) BackStack(entries.dropLast(1)) else this

    override fun equals(other: Any?): Boolean = other is BackStack && other.entries == entries
    override fun hashCode(): Int = entries.hashCode()
    override fun toString(): String = "BackStack($entries)"

    companion object {
        val Root: BackStack = BackStack(listOf(Destination.Home))
    }
}
