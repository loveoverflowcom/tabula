package com.loveoverflow.tabula.mobile.preview

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.compose.LocalLifecycleOwner

/** Explicit test owner: shared account lifecycle tests must not depend on a desktop Window. */
internal class SyntheticAccountTestLifecycleOwner : LifecycleOwner {
    private val registry = LifecycleRegistry.createUnsafe(this)
    override val lifecycle: Lifecycle get() = registry

    init { registry.currentState = Lifecycle.State.RESUMED }

    fun stop() { registry.handleLifecycleEvent(Lifecycle.Event.ON_STOP) }
    fun start() { registry.handleLifecycleEvent(Lifecycle.Event.ON_START) }
}

/** Tests actual app lifecycle observation; production and the interactive preview use their host. */
@Composable
internal fun AccountTestViewport(
    width: Int,
    height: Int,
    fontScale: Float = 1f,
    lifecycleOwner: LifecycleOwner? = null,
    content: @Composable () -> Unit,
) {
    val owner = lifecycleOwner ?: remember { SyntheticAccountTestLifecycleOwner() }
    CompositionLocalProvider(LocalLifecycleOwner provides owner) {
        PhoneViewport(width, height, fontScale, content)
    }
}
