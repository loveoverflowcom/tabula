package com.loveoverflow.tabula.mobile.design

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.sp

/**
 * Applies a scheme of the generated Tabula tokens (`TabulaTokens.kt`, from `tokens.toml`).
 *
 * This is a platform adapter, not a second theme: it adds no colours, sizes or roles. A
 * missing role is a change to `tokens.toml` and `cargo xtask gen-tokens`, never a literal
 * here. High-contrast schemes are selectable through [scheme]; reading the OS accessibility
 * setting belongs to the host and is not wired in this foundation.
 */
@Composable
fun TabulaTheme(
    scheme: TabulaScheme = if (isSystemInDarkTheme()) TabulaScheme.Dark else TabulaScheme.Light,
    content: @Composable () -> Unit,
) {
    CompositionLocalProvider(LocalTabulaColors provides TabulaSchemes.of(scheme), content = content)
}

val LocalTabulaColors = staticCompositionLocalOf { TabulaSchemes.light }

/** Maps a generated text style onto Compose; the family role resolves to a platform typeface. */
fun TabulaTextStyle.toTextStyle(color: Color): TextStyle = TextStyle(
    color = color,
    fontFamily = when (role) {
        TabulaFontRole.Display -> FontFamily.Serif
        TabulaFontRole.Text -> FontFamily.SansSerif
        TabulaFontRole.Mono -> FontFamily.Monospace
    },
    fontSize = size.sp,
    lineHeight = lineHeight.sp,
    fontWeight = FontWeight(weight),
    letterSpacing = letterSpacing.sp,
)

/** Text drawn from a generated style, with optional paragraph alignment (doc 04 §10). */
@Composable
fun TabulaText(
    text: String,
    style: TabulaTextStyle,
    modifier: Modifier = Modifier,
    color: Color = LocalTabulaColors.current.onSurface,
    textAlign: TextAlign = TextAlign.Unspecified,
) {
    BasicText(text = text, modifier = modifier, style = style.toTextStyle(color).copy(textAlign = textAlign))
}
