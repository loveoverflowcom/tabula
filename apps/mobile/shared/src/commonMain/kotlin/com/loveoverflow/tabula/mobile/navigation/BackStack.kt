package com.loveoverflow.tabula.mobile.navigation

import androidx.compose.runtime.saveable.listSaver
import com.loveoverflow.tabula.mobile.host.GameLaunch

/**
 * A place the shell can show (doc 04 §2.1, ADR-0032). Game identifiers are opaque route keys;
 * destinations never carry game state, account authority or a resumed match (I-9/I-10).
 */
sealed interface Destination {
    /** The shell's landing screen. */
    data object Home : Destination

    /** Discovery of the games packaged with this build; a remote catalog remains unavailable. */
    data object Games : Destination

    /** Display metadata for an opaque packaged-game identifier. */
    data class Detail(val gameId: String) : Destination

    /** The bounded local launch screen, corresponding to the web detail's `?setup=1` substate. */
    data class Setup(val gameId: String) : Destination

    /** The account entry point; it carries no native account session. */
    data object Account : Destination

    /** A game surface, presented through the platform's [com.loveoverflow.tabula.mobile.host.GameHost]. */
    data class Game(val launch: GameLaunch) : Destination

    /**
     * Stable shell identity aligned with the existing web routes. This is navigation identity,
     * not an OS deep-link registration or an authorizing game launch URL (ADR-0031/ADR-0033).
     */
    fun routePath(): String = when (this) {
        Home -> "/"
        Games -> "/games"
        is Detail -> "/games/$gameId"
        is Setup -> "/games/$gameId?setup=1"
        Account -> "/account"
        is Game -> "/play/local/"
    }

    companion object {
        /**
         * Parses only bounded shell paths. Unknown routes, extra queries, URL schemes and game
         * runtime paths cannot construct an active match. This checks route syntax, not game rules.
         */
        fun fromRoutePath(path: String): Destination? {
            if (path.length > MaxRouteLength) return null
            return when (path) {
                "/" -> Home
                "/games" -> Games
                "/account" -> Account
                else -> {
                    if (!path.startsWith("/games/")) return null
                    val setup = path.endsWith("?setup=1")
                    val id = path.removePrefix("/games/").let {
                        if (setup) it.removeSuffix("?setup=1") else it
                    }
                    // A route segment never decodes escapes or admits URL delimiters. The Rust
                    // registry remains the owner of game-id validity and availability (I-9).
                    if (id.isEmpty() || id == "." || id == ".." || !id.all { (it.isLetterOrDigit() && it.code < 128) || it in "._-" }) return null
                    if (setup) Setup(id) else Detail(id)
                }
            }
        }
    }
}

private const val MaxRouteLength = 256
private const val MaxHistoryEntries = 32

/**
 * Immutable navigation history; the root is always [Destination.Home].
 *
 * Navigation is pure so it is testable without a UI or a platform. The Compose layer holds
 * one value of this type and replaces it on user intent.
 */
class BackStack private constructor(private val entries: List<Destination>) {
    val current: Destination get() = entries.last()
    val canPop: Boolean get() = entries.size > 1

    /** Pushes a nested screen once and retains a bounded history with its Home root. */
    fun push(destination: Destination): BackStack = when {
        destination == current -> this
        destination == Destination.Home -> Root
        else -> BackStack(listOf(Destination.Home) + (entries.drop(1) + destination).takeLast(MaxHistoryEntries - 1))
    }

    /** Top-level navigation starts at Home; detail, setup and gameplay retain their caller. */
    fun navigate(destination: Destination): BackStack = when (destination) {
        Destination.Home -> Root
        Destination.Games, Destination.Account -> Root.push(destination)
        else -> push(destination)
    }

    /** Drops the current entry; the root cannot be popped, so this returns `this` there. */
    fun pop(): BackStack = if (canPop) BackStack(entries.dropLast(1)) else this

    /**
     * Saves public shell locations only. An active local match is represented by its setup screen;
     * neither its launch preferences/capabilities nor any runtime/session state are persisted.
     */
    fun saveRoutes(): List<String> = entries.map { destination ->
        if (destination is Destination.Game) Destination.Setup(destination.launch.gameId).routePath()
        else destination.routePath()
    }.filter { Destination.fromRoutePath(it) != null }.distinctConsecutive()

    override fun equals(other: Any?): Boolean = other is BackStack && other.entries == entries
    override fun hashCode(): Int = entries.hashCode()
    override fun toString(): String = "BackStack($entries)"

    companion object {
        val Root: BackStack = BackStack(listOf(Destination.Home))

        /** Restores an entirely valid, bounded shell history, falling back to Home if malformed. */
        fun restoreRoutes(paths: List<String>): BackStack {
            if (paths.isEmpty() || paths.size > MaxHistoryEntries || paths.first() != "/") return Root
            var restored = Root
            for (path in paths.drop(1)) {
                val destination = Destination.fromRoutePath(path) ?: return Root
                restored = restored.push(destination)
            }
            return restored
        }
    }
}

/** Compose's saved-state adapter persists only [BackStack.saveRoutes], never a local match. */
val BackStackSaver = listSaver<BackStack, String>(
    save = { it.saveRoutes() },
    restore = { BackStack.restoreRoutes(it) },
)

private fun List<String>.distinctConsecutive(): List<String> = fold(emptyList()) { paths, path ->
    if (paths.lastOrNull() == path) paths else paths + path
}
