package com.tabula.app.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color

// Semantic tokens from tokens.toml & tabula-design
val TabulaPrimary = Color(0xFF7B4DFF)
val TabulaOnPrimary = Color(0xFFFFFFFF)
val TabulaPrimaryContainer = Color(0xFF26194D)
val TabulaOnPrimaryContainer = Color(0xFFD6C7FF)

val TabulaSecondary = Color(0xFF00E5FF) // Electric cyan accent
val TabulaOnSecondary = Color(0xFF00363D)
val TabulaSecondaryContainer = Color(0xFF004F59)
val TabulaOnSecondaryContainer = Color(0xFF9CF0FF)

val TabulaTertiary = Color(0xFFFFB547) // Amber accent for timers & ratings
val TabulaOnTertiary = Color(0xFF452B00)
val TabulaTertiaryContainer = Color(0xFF634000)
val TabulaOnTertiaryContainer = Color(0xFFFFDEAC)

// Background and Surface (Deep Obsidian / Slate luxury palette)
val TabulaDarkBackground = Color(0xFF0D0B14)
val TabulaDarkSurface = Color(0xFF14121F)
val TabulaDarkSurfaceVariant = Color(0xFF1E1B2E)
val TabulaDarkSurfaceHighlight = Color(0xFF2A2740)

val TabulaOnBackground = Color(0xFFEDEBF7)
val TabulaOnSurface = Color(0xFFE4E1F0)
val TabulaOnSurfaceVariant = Color(0xFFA5A1BA)

val TabulaOutline = Color(0xFF3B3754)
val TabulaOutlineVariant = Color(0xFF26233B)

// Game-Semantic colors (from tokens.toml [sys.color])
val TabulaTurnActive = Color(0xFF00E676)
val TabulaTurnWaiting = Color(0xFF88869E)
val TabulaLegalTarget = Color(0xFF7B4DFF)
val TabulaIllegalTarget = Color(0xFFFF5252)
val TabulaSelected = Color(0xFFFFD54F)
val TabulaLastAction = Color(0xFF448AFF)
val TabulaThreat = Color(0xFFFF1744)

val TabulaDarkColorScheme = darkColorScheme(
    primary = TabulaPrimary,
    onPrimary = TabulaOnPrimary,
    primaryContainer = TabulaPrimaryContainer,
    onPrimaryContainer = TabulaOnPrimaryContainer,
    secondary = TabulaSecondary,
    onSecondary = TabulaOnSecondary,
    secondaryContainer = TabulaSecondaryContainer,
    onSecondaryContainer = TabulaOnSecondaryContainer,
    tertiary = TabulaTertiary,
    onTertiary = TabulaOnTertiary,
    tertiaryContainer = TabulaTertiaryContainer,
    onTertiaryContainer = TabulaOnTertiaryContainer,
    background = TabulaDarkBackground,
    onBackground = TabulaOnBackground,
    surface = TabulaDarkSurface,
    onSurface = TabulaOnSurface,
    surfaceVariant = TabulaDarkSurfaceVariant,
    onSurfaceVariant = TabulaOnSurfaceVariant,
    outline = TabulaOutline,
    outlineVariant = TabulaOutlineVariant
)
