package com.loveoverflow.tabula.mobile.host

import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.SurfaceView
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner

/**
 * Android GameHost consumer skeleton inside the existing CMP GameScreen/Activity (ADR-0043).
 * Production has an empty inventory and typed failed preflight: no SurfaceView is mounted and
 * no native launch or success is claimed. TODO(phase 6): enable only after real backend, approved
 * ABI, verified selected-game packaging and Android input/lifecycle/device acceptance.
 */
internal object AndroidNativeGameHost : GameHost {
    // Process-wide, never replaced on navigation/recomposition to bypass a pending worker join.
    private val factory by lazy {
        val owner = Handler(Looper.getMainLooper())
        AndroidNativeRuntimeFactory(
            ::assertAndroidNativeOwnerThread,
            SystemClock::uptimeMillis,
            AndroidNativeRuntimePort(::assertAndroidNativeOwnerThread, { action ->
                check(owner.post { action() }) { "native-owner-callback-unavailable" }
            }),
        )
    }

    @Composable
    override fun Content(launch: GameLaunch, onEvent: (GameHostEvent) -> Unit, modifier: Modifier, back: GameBackPort) {
        val latestEvent by rememberUpdatedState(onEvent)
        val lifecycle = LocalLifecycleOwner.current.lifecycle
        // Launch facts are a once-per-entry snapshot, not a recomposition restart key.
        val initialLaunch = remember { launch.copy(capabilities = launch.capabilities.toSet()) }
        var opened by remember { mutableStateOf<NativeRuntimeOpen?>(null) }
        var keepAwake by remember { mutableStateOf(false) }
        DisposableEffect(Unit) {
            val result = factory.open(
                initialLaunch,
                lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED),
                { latestEvent(it) },
                { keepAwake = it },
            )
            opened = result
            if (result is NativeRuntimeOpen.Unavailable) latestEvent(GameHostEvent.Failed(result.reason, shownByGame = false))
            // Covers disposal before a subsequent composition mounts BindGameRuntime.
            // Its existing dispose is idempotent if the mounted binding also retires it.
            onDispose { (result as? NativeRuntimeOpen.Opened)?.runtime?.dispose() }
        }
        when (val result = opened) {
            is NativeRuntimeOpen.Opened -> AndroidEmbeddedSurfaceContent(result.runtime, keepAwake, modifier, back)
            else -> Box(modifier) // GameScreen owns localized error/Back recovery, not another app root.
        }
    }
}

/** Owns a view binding only; NativeGameRuntime owns the existing lifecycle/match separation. */
private class AndroidSurfaceSlot {
    var binding: AndroidNativeSurfaceBinding? = null
    fun close() { binding?.close(); binding = null }
}

/**
 * Embedded SurfaceView consumer seam. This branch is unreachable while preflight is unavailable.
 * No ticker/drawing/rules live here. View size/input are local physical pixels; outer GameScreen
 * owns insets and AndroidNativeSurfaceBinding owns DPI, touches and synchronous retirement.
 * TODO(phase 6): real AndroidView recomposition/release, density/surface-loss and re-entry checks.
 */
@Composable
private fun AndroidEmbeddedSurfaceContent(runtime: NativeGameRuntime, keepAwake: Boolean, modifier: Modifier, back: GameBackPort) {
    BindGameRuntime(runtime, back)
    val slot = remember(runtime) { AndroidSurfaceSlot() }
    DisposableEffect(runtime) { onDispose { slot.close() } }
    key(runtime) {
        AndroidView(
            modifier = modifier,
            factory = { context -> SurfaceView(context).also { view ->
                slot.binding = AndroidNativeSurfaceBinding(view, runtime)
            } },
            update = { it.keepScreenOn = keepAwake },
            onRelease = { view -> view.keepScreenOn = false; slot.close() },
        )
    }
}
