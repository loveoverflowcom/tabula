package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.host.GameBackPort
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch

/**
 * The host used when a build attaches no platform surface. It draws no game and reports
 * [GameHostEvent.Failed], so the shell never mistakes the placeholder for a playable surface.
 */
object PlaceholderGameHost : GameHost {
    const val Message = "No game host is attached to this build."
    const val Reason = "no-game-host"

    @Composable
    override fun Content(launch: GameLaunch, onEvent: (GameHostEvent) -> Unit, modifier: Modifier, back: GameBackPort) {
        val colors = LocalTabulaColors.current
        LaunchedEffect(launch) { onEvent(GameHostEvent.Failed(Reason, shownByGame = false)) }
        Box(
            modifier = modifier
                .background(colors.container, RoundedCornerShape(TabulaShape.card.dp))
                .border(1.dp, colors.outline, RoundedCornerShape(TabulaShape.card.dp))
                .padding(TabulaSpace.lg.dp),
            contentAlignment = Alignment.Center,
        ) {
            TabulaText(Message, TabulaType.bodyMd, color = colors.onSurfaceVariant)
        }
    }
}
