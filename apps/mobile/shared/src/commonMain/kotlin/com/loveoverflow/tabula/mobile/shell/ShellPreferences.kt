package com.loveoverflow.tabula.mobile.shell

import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.localization.ShellStrings

/** Local appearance choice; System retains the host's scheme, including explicit previews. */
enum class ShellAppearance(val savedCode: String) { System("system"), Light("light"), Dark("dark") }

/** Local shell language choice; arbitrary host locales retain the English fallback. */
enum class ShellLanguage(val savedCode: String) { System("system"), English("en"), Vietnamese("vi") }

/** Reduced motion can be requested explicitly; System continues to respect OS accessibility. */
enum class ShellMotion(val savedCode: String) { System("system"), Reduced("reduced") }

/**
 * Bounded public presentation choices saved with shell state (doc 04 §10, I-10).
 * No identity, credentials, game state or match authority enters this value. The effective values
 * are copied into GameLaunch only at explicit launch; an active game's preferences stay fixed.
 */
data class ShellPreferences(
    val appearance: ShellAppearance = ShellAppearance.System,
    val language: ShellLanguage = ShellLanguage.System,
    val motion: ShellMotion = ShellMotion.System,
) {
    /** Resolves local appearance against the current host setting without inventing a palette. */
    fun resolveScheme(hostScheme: TabulaScheme): TabulaScheme = when (appearance) {
        ShellAppearance.System -> hostScheme
        ShellAppearance.Light -> TabulaScheme.Light
        ShellAppearance.Dark -> TabulaScheme.Dark
    }

    /** Resolves the same normalized language and motion facts used by shell copy and game launch. */
    fun resolveDevice(host: DeviceFacts): DeviceFacts = DeviceFacts(
        reducedMotion = motion == ShellMotion.Reduced || host.reducedMotion,
        languageTag = ShellStrings.forLanguage(when (language) {
            ShellLanguage.System -> host.languageTag
            ShellLanguage.English -> "en"
            ShellLanguage.Vietnamese -> "vi"
        }).languageTag,
    )

    /** Stable, fixed-size save format containing only allow-listed public choices. */
    fun saveCodes(): List<String> = listOf(appearance.savedCode, language.savedCode, motion.savedCode)

    companion object {
        /** Rejects malformed/unknown saved data as one unit rather than retaining partial choices. */
        fun restoreCodes(values: List<String>): ShellPreferences {
            if (values.size != 3) return ShellPreferences()
            val appearance = ShellAppearance.entries.firstOrNull { it.savedCode == values[0] } ?: return ShellPreferences()
            val language = ShellLanguage.entries.firstOrNull { it.savedCode == values[1] } ?: return ShellPreferences()
            val motion = ShellMotion.entries.firstOrNull { it.savedCode == values[2] } ?: return ShellPreferences()
            return ShellPreferences(appearance, language, motion)
        }
    }
}
