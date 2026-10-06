package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaAccessibility
import com.loveoverflow.tabula.mobile.design.TabulaBrand
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.GameBackPort
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.voice.VoiceController
import com.loveoverflow.tabula.mobile.voice.VoiceControls

/** Visible copy of the shell; kept in one place until a localisation owner exists. */
object ShellText {
    const val HomeTitle = "Tabula"
    const val HomeStatus = "Games packaged with this app play on this device. Online play, accounts and the full catalog are not connected."
    const val NoGames = "This build has no packaged game. Run `cargo xtask stage-mobile-game` and rebuild."
    const val Back = "Back"
    const val Retry = "Try again"
    const val FailureTitle = "The game could not open"
    const val FailureNote = "Trying again starts a new game. Local games are not saved."

    fun play(name: String) = "Play $name on this device"
}

/** Screen shell: page surface, safe-area insets and a compact title (`docs/ui/screens/foundation.md`). */
@Composable
fun ShellPage(
    title: String,
    modifier: Modifier = Modifier,
    titleContent: @Composable () -> Unit = { TabulaText(title, TabulaType.headlineSm) },
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = LocalTabulaColors.current
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.surface)
            .safeDrawingPadding()
            .padding(horizontal = TabulaSpace.lg.dp, vertical = TabulaSpace.md.dp),
        verticalArrangement = Arrangement.spacedBy(TabulaSpace.lg.dp),
    ) {
        titleContent()
        content()
    }
}

/** A labelled action at least [TabulaAccessibility.minTarget] dp tall. [filled] marks the principal action. */
@Composable
fun ShellButton(label: String, filled: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val colors = LocalTabulaColors.current
    val container = if (filled) colors.primary else colors.containerHigh
    val content = if (filled) colors.onPrimary else colors.onSurface
    Box(
        modifier = modifier
            .heightIn(min = TabulaAccessibility.minTarget.dp)
            .clip(RoundedCornerShape(TabulaShape.button.dp))
            .background(container)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = TabulaSpace.xl.dp, vertical = TabulaSpace.md.dp),
        contentAlignment = Alignment.Center,
    ) {
        TabulaText(label, TabulaType.labelLg, color = content)
    }
}

@Composable
fun HomeScreen(games: List<BundledGame>, languageTag: String, onOpen: (BundledGame) -> Unit) {
    ShellPage(ShellText.HomeTitle, titleContent = { TabulaBrand() }) {
        TabulaText(
            if (games.isEmpty()) ShellText.NoGames else ShellText.HomeStatus,
            TabulaType.bodyMd,
            color = LocalTabulaColors.current.onSurfaceVariant,
        )
        for (game in games) {
            ShellButton(
                ShellText.play(game.displayName(languageTag)),
                filled = true,
                onClick = { onOpen(game) },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/**
 * Hosts the platform [GameHost] under a toolbar; the shell owns navigation, the host owns the surface.
 *
 * Back goes to the host first, so a live match shows its own leave confirmation; the shell pops
 * only when the host does not consume it. A failure the game could not explain itself replaces
 * the surface with a panel; **Try again** mounts a fresh host, which starts a new game.
 */
// The common BackHandler is experimental and deprecated in favour of NavigationEventHandler in
// Compose Multiplatform 1.12; it is the one API available on both targets without a new dependency.
@OptIn(ExperimentalComposeUiApi::class)
@Suppress("DEPRECATION")
@Composable
fun GameScreen(title: String, launch: GameLaunch, host: GameHost, onLeave: () -> Unit, voice: VoiceController? = null, vietnamese: Boolean = false) {
    val back = remember { GameBackPort() }
    var failure by remember { mutableStateOf<String?>(null) }
    val leave = { if (!back.requestBack()) onLeave() }
    BackHandler(enabled = true, onBack = leave)
    ShellPage(title) {
        Row { ShellButton(ShellText.Back, filled = false, onClick = leave) }
        if (voice != null) VoiceControls(voice, vietnamese)
        val reason = failure
        if (reason != null) {
            FailurePanel(reason, onRetry = { failure = null }, modifier = Modifier.fillMaxWidth().weight(1f))
        } else {
            // The failure panel and this branch are exclusive, so leaving the panel discards the old
            // runtime with its composition and Try again mounts a new one. Nothing else restarts it.
            host.Content(
                launch = launch,
                onEvent = { event ->
                    when {
                        event == GameHostEvent.Exited -> onLeave()
                        event is GameHostEvent.Failed && !event.shownByGame -> failure = event.reason
                    }
                },
                modifier = Modifier.fillMaxWidth().weight(1f),
                back = back,
            )
        }
    }
}

@Composable
private fun FailurePanel(reason: String, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    val colors = LocalTabulaColors.current
    Column(
        modifier = modifier
            .background(colors.container, RoundedCornerShape(TabulaShape.card.dp))
            .padding(TabulaSpace.lg.dp),
        verticalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp),
    ) {
        TabulaText(ShellText.FailureTitle, TabulaType.titleMd)
        TabulaText(reason, TabulaType.bodyMd, color = colors.onSurfaceVariant)
        TabulaText(ShellText.FailureNote, TabulaType.bodyMd, color = colors.onSurfaceVariant)
        ShellButton(ShellText.Retry, filled = true, onClick = onRetry)
    }
}
