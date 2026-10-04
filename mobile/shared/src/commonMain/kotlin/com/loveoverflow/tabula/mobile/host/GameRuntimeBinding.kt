package com.loveoverflow.tabula.mobile.host

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner

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

/**
 * The glue every platform host needs and must not duplicate: route Back to the runtime, forward
 * foreground/background to it, and dispose it when this composable leaves the composition.
 *
 * [runtime] is built by the caller with `remember` so a recomposition reuses it; this function keys
 * only on that instance, so nothing here can cause a second runtime to be created.
 */
@Composable
fun BindGameRuntime(runtime: GameRuntimeControls, back: GameBackPort) {
    DisposableEffect(runtime) {
        back.register { runtime.onBack() }
        onDispose {
            back.register(null)
            runtime.dispose()
        }
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle, runtime) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_PAUSE -> runtime.suspend()
                Lifecycle.Event.ON_RESUME -> runtime.resume()
                else -> Unit
            }
        }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }
}

/**
 * Builds the platform runtime for a game screen **once per composition entry** and binds it.
 *
 * Every host (Android, iOS and the desktop preview) goes through this one function, so the rule the
 * shell depends on lives, and is tested, in a single place: a recomposition, a new [onEvent] lambda or
 * any other changed parameter never creates a second runtime, while events always reach the latest
 * lambda. The only way to get another runtime is to leave the composition and enter it again.
 */
@Composable
fun <R : GameRuntimeControls> rememberGameRuntime(
    back: GameBackPort,
    onEvent: (GameHostEvent) -> Unit,
    create: (notify: (GameHostEvent) -> Unit) -> R,
): R {
    val latestEvent = rememberUpdatedState(onEvent)
    val runtime = remember { create { latestEvent.value(it) } }
    BindGameRuntime(runtime, back)
    return runtime
}
