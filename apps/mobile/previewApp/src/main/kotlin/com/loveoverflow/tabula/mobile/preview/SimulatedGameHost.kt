package com.loveoverflow.tabula.mobile.preview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.bridge.BridgeCodec
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.host.GameBackPort
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.host.GameRuntimeControls
import com.loveoverflow.tabula.mobile.host.rememberGameRuntime
import com.loveoverflow.tabula.mobile.session.GameSession
import com.loveoverflow.tabula.mobile.session.SessionEffect
import com.loveoverflow.tabula.mobile.shell.ShellButton
import kotlinx.coroutines.delay

/**
 * One simulated runtime. It is the desktop twin of `AndroidGameRuntime`: the same [GameSession]
 * decides everything, and this class only performs the effects it returns. Every instance is
 * recorded in [SimulatedGameHost.runtimes] so tests can prove how many were built and disposed.
 */
internal class SimulatedRuntime(
    launch: GameLaunch,
    private val notify: (GameHostEvent) -> Unit,
) : GameRuntimeControls {
    val session = GameSession(launch.capabilities, requireNotNull(launch.preferences))
    val page = SimulatedPage(::fromPage)
    val dropped = mutableListOf<String>()
    var keepAwake = false
        private set
    var disposed = false
        private set

    init {
        page.load()
    }

    private fun fromPage(text: String) {
        if (disposed) return
        run(session.onPageText(text))
    }

    /** Page messages that reach the host from a retired runtime must change nothing. */
    fun injectLatePageText(text: String) = fromPage(text)

    fun failHost(reason: String) = run(session.onHostFailure(reason))

    fun helloTimeout() = run(session.onHelloTimeout())

    override fun onBack(): Boolean {
        if (disposed) return false
        val outcome = session.onBack()
        run(outcome.effects)
        return outcome.consumed
    }

    override fun suspend() { if (!disposed) run(session.onHostSuspend()) }

    override fun resume() { if (!disposed) run(session.onHostResume()) }

    override fun dispose() {
        if (disposed) return
        run(session.onDispose())
        disposed = true
        SimulatedGameHost.disposedCount += 1
    }

    private fun run(effects: List<SessionEffect>) {
        for (effect in effects) {
            when (effect) {
                is SessionEffect.Send -> {
                    BridgeCodec.encode(effect.message) // the real wire encoder must accept every host message
                    page.onHost(effect.message)
                }
                is SessionEffect.KeepAwake -> keepAwake = effect.on
                is SessionEffect.Notify -> notify(effect.event)
                is SessionEffect.Dropped -> dropped += effect.reason
            }
        }
    }
}

/**
 * A [GameHost] for desktop preview and tests. It builds its runtime through the same
 * `rememberGameRuntime` the WebView hosts use, so the tests of that rule apply to them too.
 */
internal class SimulatedGameHost(private val autoBootMillis: Long? = 600) : GameHost {
    @Composable
    override fun Content(launch: GameLaunch, onEvent: (GameHostEvent) -> Unit, modifier: Modifier, back: GameBackPort) {
        val runtime = rememberGameRuntime(back, onEvent) { notify ->
            SimulatedRuntime(launch, notify).also {
                runtimes += it
                createdCount += 1
            }
        }
        if (autoBootMillis != null) LaunchedEffect(runtime) {
            delay(autoBootMillis)
            runtime.page.completeBoot()
        }
        val colors = LocalTabulaColors.current
        Column(
            modifier = modifier
                .testTag("sim-page")
                .background(colors.container, RoundedCornerShape(TabulaShape.card.dp))
                .padding(TabulaSpace.lg.dp),
            verticalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp),
        ) {
            TabulaText("SIMULATED GAME PAGE", TabulaType.titleMd)
            TabulaText("Desktop preview stand-in. Not the real game and not a WebView.", TabulaType.bodyMd, color = colors.onSurfaceVariant)
            TabulaText("runtime#${runtime.hashCode()} " + runtime.page.summary(), TabulaType.bodyMd, modifier = Modifier.testTag("sim-state"))
            if (runtime.page.leaveDialogOpen) {
                TabulaText("Leave this game? Local games are not saved.", TabulaType.titleMd, modifier = Modifier.testTag("sim-leave-dialog"))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp), modifier = Modifier.fillMaxWidth()) {
                ShellButton("Leave", filled = false, onClick = { runtime.page.openLeaveDialog() }, modifier = Modifier.testTag("sim-leave"))
                ShellButton("Confirm", filled = true, onClick = { runtime.page.confirmLeave() }, modifier = Modifier.testTag("sim-confirm"))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp), modifier = Modifier.fillMaxWidth()) {
                ShellButton("Fail page", filled = false, onClick = { runtime.page.fail() }, modifier = Modifier.testTag("sim-fail"))
                ShellButton("Reload", filled = false, onClick = { runtime.page.reload(); runtime.page.completeBoot() }, modifier = Modifier.testTag("sim-reload"))
            }
        }
    }

    companion object {
        /** Every runtime ever built in this process, in order. */
        val runtimes = mutableListOf<SimulatedRuntime>()
        var createdCount = 0
        var disposedCount = 0

        fun reset() {
            runtimes.clear()
            createdCount = 0
            disposedCount = 0
        }
    }
}
