package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.design.toTextStyle
import com.loveoverflow.tabula.mobile.host.GameBackPort
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.voice.VoiceController
import com.loveoverflow.tabula.mobile.voice.VoiceControls

/**
 * Hosts the platform [GameHost] under a toolbar; the shell owns navigation, the host owns the surface.
 * Back goes to the host first. Unexplained host failure releases that surface; retry mounts one
 * fresh host and starts a new game. Recomposition and resize never change runtime identity.
 */
// This is the shared Back API available on both targets without a new navigation dependency.
@OptIn(ExperimentalComposeUiApi::class)
@Suppress("DEPRECATION")
@Composable
fun GameScreen(
    title: String,
    launch: GameLaunch,
    host: GameHost,
    onLeave: () -> Unit,
    voice: VoiceController? = null,
    vietnamese: Boolean = false,
    strings: ShellStrings = ShellStrings.forLanguage(if (vietnamese) "vi" else "en"),
) {
    val back = remember { GameBackPort() }
    var failure by remember { mutableStateOf<String?>(null) }
    val leave = { if (!back.requestBack()) onLeave() }
    BackHandler(enabled = true, onBack = leave)
    ShellPage(
        title,
        Modifier.background(LocalTabulaColors.current.shellCanvas).safeDrawingPadding(),
        scrollable = false,
        titleContent = { GameToolbar(title, strings, leave) },
    ) {
        BoxWithConstraints(Modifier.fillMaxWidth().weight(1f)) {
            val voiceHeight = maxHeight / 2
            Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(TabulaSpace.lg.dp)) {
                if (voice != null) {
                    // Large text and permissions scroll without consuming the game viewport (I-10).
                    VoiceControls(voice, strings.vietnamese,
                        Modifier.heightIn(max = voiceHeight).verticalScroll(rememberScrollState()))
                }
                val reason = failure
                if (reason != null) {
                    FailurePanel(reason, strings, onRetry = { failure = null }, modifier = Modifier.fillMaxWidth().weight(1f))
                } else {
                    // Exclusive branches discard the failed runtime; retry mounts a fresh one.
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
    }
}

/** Long game names keep their semantic text while the compact toolbar reserves room for Back. */
@Composable
private fun GameToolbar(title: String, strings: ShellStrings, onBack: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ShellActionButton(strings[ShellCopy.Back], ShellAction.Tonal, onBack,
            Modifier.testTag("shell-back"), compact = true)
        BasicText(
            title,
            modifier = Modifier.weight(1f).semantics { heading() },
            style = TabulaType.headlineSm.toTextStyle(LocalTabulaColors.current.onSurface),
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun FailurePanel(reason: String, strings: ShellStrings, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.verticalScroll(rememberScrollState())) {
        ShellStatePanel(
            strings[ShellCopy.FailureTitle], reason,
            Modifier.fillMaxWidth().testTag("shell-host-failure"), error = true,
        ) {
            TabulaText(strings[ShellCopy.FailureNote], TabulaType.bodyMd, color = LocalTabulaColors.current.onSurfaceVariant)
            ShellActionButton(strings[ShellCopy.Retry], ShellAction.Filled, onRetry, Modifier.testTag("shell-retry"))
        }
    }
}
