package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaAccessibility
import com.loveoverflow.tabula.mobile.design.TabulaBrand
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaState
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.navigation.Destination

/** Action emphasis from the shared foundation contract; labels and hit geometry never disappear. */
enum class ShellAction { Filled, Tonal, Text }

/**
 * A shared action with token-backed state layers, a separate focus ring and a 44 dp target floor.
 * Foundation clickable owns cancellation, keyboard activation and disabled semantics (doc 04 §10).
 * Feedback is immediate, so reduced motion does not postpone activation or focus.
 */
@Composable
fun ShellButton(
    label: String,
    filled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) = ShellActionButton(label, if (filled) ShellAction.Filled else ShellAction.Tonal, onClick, modifier, enabled)

/**
 * A filled, tonal or text action. Disabled reasons stay beside this control at full contrast.
 * [compact] reduces horizontal inset for toolbar labels; fonts and minimum targets stay fixed.
 */
@Composable
fun ShellActionButton(
    label: String,
    action: ShellAction,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    compact: Boolean = false,
) {
    val colors = LocalTabulaColors.current
    val background = when (action) {
        ShellAction.Filled -> colors.primary
        ShellAction.Tonal -> colors.containerHigh
        ShellAction.Text -> colors.shellPaper.copy(alpha = 0f)
    }
    val foreground = when (action) {
        ShellAction.Filled -> colors.onPrimary
        ShellAction.Tonal -> colors.onSurface
        ShellAction.Text -> colors.primary
    }
    ShellInteractiveSurface(
        background, foreground, onClick, modifier, enabled,
        horizontalInset = if (compact) TabulaSpace.xs else TabulaSpace.md,
    ) { color ->
        TabulaText(label, TabulaType.labelLg, color = color)
    }
}

@Composable
private fun ShellInteractiveSurface(
    background: Color,
    foreground: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    isSelected: Boolean? = null,
    horizontalInset: Float = TabulaSpace.md,
    content: @Composable (Color) -> Unit,
) {
    val colors = LocalTabulaColors.current
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val hovered by source.collectIsHoveredAsState()
    val pressed by source.collectIsPressedAsState()
    val alpha = when {
        !enabled -> 0f
        pressed -> TabulaState.press
        focused -> TabulaState.focus
        hovered -> TabulaState.hover
        else -> 0f
    }
    val shape = RoundedCornerShape(TabulaShape.button.dp)
    // The ring and surface gap are reserved even when idle: focus never changes target bounds.
    val ringSpace = (TabulaAccessibility.focusRingWidth + TabulaSpace.xxs).dp
    val ring = if (focused) Modifier.border(TabulaAccessibility.focusRingWidth.dp, colors.primary, shape) else Modifier
    Box(
        modifier.widthIn(min = TabulaAccessibility.minTarget.dp).heightIn(min = TabulaAccessibility.minTarget.dp)
            .semantics { if (isSelected != null) selected = isSelected }
            .clickable(enabled = enabled, role = Role.Button, interactionSource = source, indication = null, onClick = onClick)
            .then(ring).padding(ringSpace),
        propagateMinConstraints = true,
    ) {
        Box(
            modifier = Modifier
                .widthIn(min = TabulaAccessibility.minTarget.dp)
                .heightIn(min = TabulaAccessibility.minTarget.dp)
                .clip(shape)
                .background(
                    foreground.copy(alpha = alpha).compositeOver(
                        if (enabled) background else background.copy(alpha = TabulaState.disabledContainer),
                    ),
                )
                .padding(horizontal = horizontalInset.dp, vertical = TabulaSpace.sm.dp),
            contentAlignment = Alignment.Center,
        ) { content(if (enabled) foreground else foreground.copy(alpha = TabulaState.disabledContent)) }
    }
}

/** A quiet token surface; high-contrast schemes retain a structural boundary when tones flatten. */
@Composable
fun ShellSurface(
    modifier: Modifier = Modifier,
    hero: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = LocalTabulaColors.current
    val fill = if (hero) colors.shellHero else colors.shellPaper
    val shape = RoundedCornerShape(TabulaShape.card.dp)
    val boundary = if (fill == colors.shellCanvas) Modifier.border(TabulaSpace.xxs.dp, colors.outline, shape) else Modifier
    Column(
        modifier.fillMaxWidth().background(fill, shape).then(boundary).padding(TabulaSpace.xxl.dp),
        verticalArrangement = Arrangement.spacedBy(TabulaSpace.md.dp),
        content = content,
    )
}

/** Persistent empty/unavailable/error content, with an optional real recovery action supplied by its owner. */
@Composable
fun ShellStatePanel(
    title: String,
    message: String,
    modifier: Modifier = Modifier,
    error: Boolean = false,
    action: (@Composable () -> Unit)? = null,
) {
    val colors = LocalTabulaColors.current
    ShellSurface(modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
        TabulaText(title, TabulaType.titleMd, Modifier.semantics { heading() }, if (error) colors.danger else colors.onSurface)
        TabulaText(message, TabulaType.bodyMd, color = colors.onSurfaceVariant)
        action?.invoke()
    }
}

/** Named indeterminate status; no fabricated fraction and no motion requirement. */
@Composable
fun ShellLoading(strings: ShellStrings, modifier: Modifier = Modifier) {
    TabulaText(
        strings[ShellCopy.Loading],
        TabulaType.bodyMd,
        modifier.semantics {
            liveRegion = LiveRegionMode.Polite
            progressBarRangeInfo = ProgressBarRangeInfo.Indeterminate
        },
        LocalTabulaColors.current.onSurfaceVariant,
    )
}

/**
 * Scrollable task content below the shell toolbar. The viewport owns safe-area insets; gutters
 * supplement them. Large text grows naturally and every trailing action remains reachable.
 */
@Composable
fun ShellPage(
    title: String,
    modifier: Modifier = Modifier,
    displayTitle: Boolean = false,
    scrollable: Boolean = true,
    titleContent: @Composable () -> Unit = {
        TabulaText(title, if (displayTitle) TabulaType.displaySm else TabulaType.headlineSm, Modifier.semantics { heading() })
    },
    content: @Composable ColumnScope.() -> Unit,
) {
    BoxWithConstraints(modifier.fillMaxSize()) {
        val gutter = if (maxWidth < 600.dp) TabulaSpace.lg.dp else TabulaSpace.xxl.dp
        val scroll = if (scrollable) Modifier.verticalScroll(rememberScrollState()).testTag("shell-content-scroll") else Modifier
        Column(
            modifier = Modifier.fillMaxSize().then(scroll).padding(horizontal = gutter, vertical = TabulaSpace.xxl.dp),
            verticalArrangement = Arrangement.spacedBy(TabulaSpace.xxl.dp),
        ) {
            titleContent()
            content()
        }
    }
}

/**
 * Adaptive shell chrome: compact topbar and bottom navigation; rail from 600 logical dp.
 * Its content is a real remaining-height slot, so fixed navigation cannot cover scrollable tasks.
 * Only supported destinations appear; nested discovery screens select the Games destination.
 */
@Composable
fun ShellChrome(
    destination: Destination,
    strings: ShellStrings,
    onNavigate: (Destination) -> Unit,
    onBack: () -> Unit,
    content: @Composable () -> Unit,
) {
    val colors = LocalTabulaColors.current
    BoxWithConstraints(Modifier.fillMaxSize().background(colors.shellCanvas).safeDrawingPadding().testTag("shell-chrome")) {
        val wide = maxWidth >= 600.dp
        Row(Modifier.fillMaxSize()) {
            if (wide) {
                Column(
                    Modifier.width((TabulaSpace.xxxxxxl + TabulaSpace.xxxxxl).dp).fillMaxHeight()
                        .padding(horizontal = TabulaSpace.xxs.dp, vertical = TabulaSpace.lg.dp).testTag("shell-navigation-rail"),
                    verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp),
                ) { ShellNavigation(destination, strings, onNavigate, vertical = true) }
            }
            Column(Modifier.weight(1f).fillMaxHeight()) {
                ShellTopbar(destination, strings, onNavigate, onBack)
                Box(Modifier.weight(1f).fillMaxWidth()) { content() }
                if (!wide) {
                    Row(
                        Modifier.fillMaxWidth().background(colors.shellCanvas)
                            .padding(horizontal = TabulaSpace.xxs.dp, vertical = TabulaSpace.sm.dp).testTag("shell-bottom-navigation"),
                        horizontalArrangement = Arrangement.spacedBy(TabulaSpace.xxs.dp),
                    ) { ShellNavigation(destination, strings, onNavigate, vertical = false) }
                }
            }
        }
    }
}

@Composable
private fun ShellTopbar(destination: Destination, strings: ShellStrings, onNavigate: (Destination) -> Unit, onBack: () -> Unit) {
    val colors = LocalTabulaColors.current
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val backLimit = maxWidth / 3
        Row(
            Modifier.fillMaxWidth().heightIn(min = TabulaSpace.xxxxxxl.dp).padding(horizontal = TabulaSpace.sm.dp, vertical = TabulaSpace.xxs.dp),
            horizontalArrangement = Arrangement.spacedBy(TabulaSpace.xxs.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (destination is Destination.Detail || destination is Destination.Setup) {
                ShellActionButton(strings[ShellCopy.Back], ShellAction.Text, onBack,
                    Modifier.widthIn(max = backLimit).testTag("shell-back"), compact = true)
            }
            Box(Modifier.weight(1f)) { TabulaBrand(height = TabulaSpace.xxxl.dp) }
            ShellInteractiveSurface(
                colors.shellPaper.copy(alpha = 0f), colors.primary,
                onClick = { onNavigate(Destination.Account) },
                modifier = Modifier.testTag("shell-account-entry").semantics {
                    contentDescription = "${strings[ShellCopy.Account]}, ${strings[ShellCopy.Anonymous]}"
                },
            ) { color -> NeutralAvatar(color, Modifier.size(TabulaSpace.xxxl.dp)) }
        }
    }
}

@Composable
private fun ShellNavigation(destination: Destination, strings: ShellStrings, onNavigate: (Destination) -> Unit, vertical: Boolean) {
    val entries = listOf(
        Triple(Destination.Home, ShellCopy.Home, "home"),
        Triple(Destination.Games, ShellCopy.Games, "games"),
        Triple(Destination.Account, ShellCopy.Account, "account"),
    )
    if (vertical) {
        Column(verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp)) {
            for ((target, copy, tag) in entries) ShellNavigationItem(destination, strings, onNavigate, target, copy, tag)
        }
    } else {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(TabulaSpace.xxs.dp)) {
            for ((target, copy, tag) in entries) {
                Box(Modifier.weight(1f)) { ShellNavigationItem(destination, strings, onNavigate, target, copy, tag) }
            }
        }
    }
}

@Composable
private fun ShellNavigationItem(
    destination: Destination,
    strings: ShellStrings,
    onNavigate: (Destination) -> Unit,
    target: Destination,
    copy: ShellCopy,
    tag: String,
) {
    val selected = when (target) {
        Destination.Home -> destination == Destination.Home
        Destination.Games -> destination == Destination.Games || destination is Destination.Detail || destination is Destination.Setup
        Destination.Account -> destination == Destination.Account
        else -> false
    }
    val colors = LocalTabulaColors.current
    ShellInteractiveSurface(
        if (selected) colors.primary.copy(alpha = TabulaState.focus).compositeOver(colors.shellPaper) else colors.shellCanvas,
        if (selected) colors.primary else colors.onSurfaceVariant,
        onClick = { onNavigate(target) },
        isSelected = selected,
        // Leave room for whole words at 200% text with the host's fallback typeface.
        horizontalInset = TabulaSpace.xxs,
        modifier = Modifier.fillMaxWidth().testTag("shell-nav-$tag"),
    ) { color ->
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(TabulaSpace.xs.dp)) {
            NavigationIcon(tag, color)
            TabulaText(strings[copy], TabulaType.labelMd, color = color)
        }
    }
}

/** Same neutral human silhouette as the web fallback; it carries no inferred name or profile. */
@Composable
fun ShellAnonymousAvatar(strings: ShellStrings, modifier: Modifier = Modifier) {
    NeutralAvatar(LocalTabulaColors.current.primary, modifier.size(TabulaSpace.xxxxxl.dp).testTag("shell-anonymous-avatar")
        .semantics { contentDescription = strings[ShellCopy.Anonymous] })
}

@Composable
private fun NeutralAvatar(color: Color, modifier: Modifier) {
    Canvas(modifier) {
        val scale = minOf(size.width, size.height) / 40f
        val stroke = Stroke(2f * scale)
        drawCircle(color, 6f * scale, Offset(20f * scale, 14f * scale), style = stroke)
        val shoulders = Path().apply {
            moveTo(8f * scale, 33f * scale)
            lineTo(8f * scale, 31f * scale)
            cubicTo(8f * scale, 15f * scale, 32f * scale, 15f * scale, 32f * scale, 31f * scale)
            lineTo(32f * scale, 33f * scale)
        }
        drawPath(shoulders, color, style = stroke)
    }
}

@Composable
private fun NavigationIcon(kind: String, color: Color) {
    Canvas(Modifier.size(TabulaSpace.xxl.dp).clearAndSetSemantics { }) {
        val unit = size.width / 24f
        val stroke = Stroke(2f * unit)
        if (kind == "account") {
            drawCircle(color, 3f * unit, Offset(12f * unit, 7f * unit), style = stroke)
            drawPath(Path().apply {
                moveTo(4f * unit, 21f * unit)
                cubicTo(4f * unit, 8f * unit, 20f * unit, 8f * unit, 20f * unit, 21f * unit)
            }, color, style = stroke)
        } else if (kind == "home") {
            drawPath(Path().apply {
                moveTo(3f * unit, 10f * unit)
                lineTo(12f * unit, 3f * unit)
                lineTo(21f * unit, 10f * unit)
                lineTo(21f * unit, 21f * unit)
                lineTo(3f * unit, 21f * unit)
                close()
            }, color, style = stroke)
        } else {
            for (x in listOf(3f, 14f)) for (y in listOf(3f, 14f)) {
                drawRect(color, Offset(x * unit, y * unit), androidx.compose.ui.geometry.Size(7f * unit, 7f * unit), style = stroke)
            }
        }
    }
}
