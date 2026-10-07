package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.ShellToolsCopy
import com.loveoverflow.tabula.mobile.localization.tools
import com.loveoverflow.tabula.mobile.navigation.BackStack
import com.loveoverflow.tabula.mobile.navigation.Destination
import com.loveoverflow.tabula.mobile.navigation.isAccountSection
import com.loveoverflow.tabula.mobile.navigation.isAccountTask
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import com.loveoverflow.tabula.mobile.shell.ShellAppearance
import com.loveoverflow.tabula.mobile.shell.ShellLanguage
import com.loveoverflow.tabula.mobile.shell.ShellMotion
import com.loveoverflow.tabula.mobile.shell.ShellPreferences
import com.loveoverflow.tabula.mobile.shell.gamePreferences
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue

class ShellToolsTest {
    @Test
    fun publicToolsReturnToAccountAndRestoreWithoutAccountTaskAuthority() {
        val account = BackStack.Root.navigate(Destination.Account)
        for ((destination, path) in listOf(
            Destination.Rooms to "/rooms", Destination.History to "/history", Destination.Settings to "/settings",
        )) {
            assertEquals(path, destination.routePath())
            assertEquals(destination, Destination.fromRoutePath(path))
            assertTrue(destination.isAccountSection)
            assertFalse(destination.isAccountTask, "a public tool is not an account operation")
            val nested = account.push(destination)
            assertEquals(account, nested.pop())
            assertSame(nested, nested.push(destination))
            assertEquals(nested, BackStack.restoreRoutes(nested.saveRoutes()))
            assertEquals(BackStack.Root.navigate(Destination.Games), nested.navigate(Destination.Games))
            for (suffix in listOf("?token=private", "?match=private", "#private", "/another")) {
                assertNull(Destination.fromRoutePath(path + suffix))
            }
        }
    }

    @Test
    fun defaultPreferencesFollowHostSchemeLanguageAndAccessibility() {
        val preferences = ShellPreferences()
        for (scheme in TabulaScheme.entries) assertEquals(scheme, preferences.resolveScheme(scheme))
        assertEquals(DeviceFacts(true, "vi"), preferences.resolveDevice(DeviceFacts(true, "vi-VN")))
        assertEquals(DeviceFacts(false, "en"), preferences.resolveDevice(DeviceFacts(false, "fr-FR")))
    }

    @Test
    fun explicitOverridesResolveToOneConsistentNextLaunchSnapshot() {
        val preferences = ShellPreferences(ShellAppearance.Dark, ShellLanguage.Vietnamese, ShellMotion.Reduced)
        val resolved = preferences.resolveDevice(DeviceFacts(false, "en"))
        assertEquals(TabulaScheme.Dark, preferences.resolveScheme(TabulaScheme.Light))
        assertEquals(DeviceFacts(true, "vi"), resolved)
        val launch = gamePreferences(true, resolved)
        assertEquals(ThemePreference.Dark, launch.theme)
        assertEquals(LocalePreference.Vietnamese, launch.locale)
        assertEquals(MotionPreference.Reduced, launch.motion)
        val changed = preferences.copy(appearance = ShellAppearance.Light, language = ShellLanguage.English)
        assertEquals(TabulaScheme.Light, changed.resolveScheme(TabulaScheme.Dark))
        assertEquals("en", changed.resolveDevice(DeviceFacts(false, "vi")).languageTag)
        assertEquals(ThemePreference.Dark, launch.theme, "later choices cannot mutate an active launch snapshot")
        assertTrue(ShellPreferences(motion = ShellMotion.System).resolveDevice(DeviceFacts(true, "en")).reducedMotion)
    }

    @Test
    fun everyFinitePreferenceChoiceRoundTripsThroughTheBoundedSaveFormat() {
        for (appearance in ShellAppearance.entries) for (language in ShellLanguage.entries) for (motion in ShellMotion.entries) {
            val preferences = ShellPreferences(appearance, language, motion)
            assertEquals(3, preferences.saveCodes().size)
            assertEquals(preferences, ShellPreferences.restoreCodes(preferences.saveCodes()))
        }
    }

    @Test
    fun malformedSavedPreferencesFallBackAsOneUnit() {
        for (values in listOf(
            emptyList(), listOf("dark", "vi"), listOf("dark", "vi", "reduced", "private"),
            listOf("private", "vi", "reduced"), listOf("dark", "private", "reduced"),
            listOf("dark", "vi", "private"), listOf("DARK", "vi", "reduced"),
            listOf("dark", "vi".repeat(1000), "reduced"),
        )) assertEquals(ShellPreferences(), ShellPreferences.restoreCodes(values))
    }

    @Test
    fun everyToolKeyHasDistinctNonblankEnglishAndVietnameseCopy() {
        assertEquals(ShellToolsCopy.entries.size, ShellToolsCopy.entries.map { it.key }.toSet().size)
        for (language in listOf("en", "vi")) for (copy in ShellToolsCopy.entries) {
            val value = ShellStrings.forLanguage(language).tools(copy)
            assertTrue(value.isNotBlank())
            assertFalse(value == copy.key)
        }
    }
}
