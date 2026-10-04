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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaAccessibility
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.host.GameHost
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch

/** Visible copy of the shell; kept in one place until a localisation owner exists. */
object ShellText {
    const val HomeTitle = "Tabula"
    const val HomeStatus = "Mobile foundation. The game catalog and match flows are not connected yet."
    const val OpenGameSlot = "Open game slot"
    const val GameTitle = "Game"
    const val Back = "Back"
}

/** Screen shell: page surface, safe-area insets and a compact title (`docs/ui/screens/foundation.md`). */
@Composable
fun ShellPage(title: String, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val colors = LocalTabulaColors.current
    Column(
        modifier = modifier
            .fillMaxSize()
            .background(colors.surface)
            .safeDrawingPadding()
            .padding(horizontal = TabulaSpace.lg.dp, vertical = TabulaSpace.md.dp),
        verticalArrangement = Arrangement.spacedBy(TabulaSpace.lg.dp),
    ) {
        TabulaText(title, TabulaType.headlineSm)
        content()
    }
}

/** A labelled action at least [TabulaAccessibility.minTarget] dp tall. [filled] marks the principal action. */
@Composable
fun ShellButton(label: String, filled: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val colors = LocalTabulaColors.current
    val container = if (filled) colors.primary else colors.containerHigh
    val content = if (filled) colors.onPrimary else colors.onSurface
    Box(
        modifier = modifier
            .heightIn(min = TabulaAccessibility.minTarget.dp)
            .clip(RoundedCornerShape(TabulaShape.button.dp))
            .background(container)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = TabulaSpace.xl.dp, vertical = TabulaSpace.md.dp),
        contentAlignment = Alignment.Center,
    ) {
        TabulaText(label, TabulaType.labelLg, color = content)
    }
}

@Composable
fun HomeScreen(onOpenGameSlot: () -> Unit) {
    ShellPage(ShellText.HomeTitle) {
        TabulaText(
            ShellText.HomeStatus,
            TabulaType.bodyMd,
            color = LocalTabulaColors.current.onSurfaceVariant,
        )
        ShellButton(ShellText.OpenGameSlot, filled = true, onClick = onOpenGameSlot, modifier = Modifier.fillMaxWidth())
    }
}

/** Hosts the platform [GameHost] under a toolbar; the shell owns navigation, the host owns the surface. */
@Composable
fun GameScreen(launch: GameLaunch, host: GameHost, onEvent: (GameHostEvent) -> Unit, onBack: () -> Unit) {
    ShellPage(ShellText.GameTitle) {
        Row { ShellButton(ShellText.Back, filled = false, onClick = onBack) }
        host.Content(launch, onEvent, Modifier.fillMaxWidth().weight(1f))
    }
}
