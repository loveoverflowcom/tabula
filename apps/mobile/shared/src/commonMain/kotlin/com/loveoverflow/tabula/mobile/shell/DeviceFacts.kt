package com.loveoverflow.tabula.mobile.shell

import androidx.compose.runtime.Composable
import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ThemePreference

/** Device settings the shell reads from the OS so the game can honour them at launch. */
data class DeviceFacts(val reducedMotion: Boolean, val languageTag: String)

/** Reads the OS reduced-motion and language settings. The platform owns how. */
@Composable
expect fun rememberDeviceFacts(): DeviceFacts

/**
 * The preferences handed to a game at start. They are a snapshot: the board reads them once at
 * launch, so a later OS change applies to the next game, not to the one on screen.
 */
fun gamePreferences(dark: Boolean, facts: DeviceFacts): GamePreferences = GamePreferences(
    theme = if (dark) ThemePreference.Dark else ThemePreference.Light,
    motion = if (facts.reducedMotion) MotionPreference.Reduced else MotionPreference.System,
    locale = if (facts.languageTag.lowercase().startsWith("vi")) LocalePreference.Vietnamese else LocalePreference.English,
)
