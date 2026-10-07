package com.loveoverflow.tabula.mobile.preview

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.LocalSaveableStateRegistry
import androidx.compose.runtime.saveable.SaveableStateRegistry
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.bridge.HostMessage
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Shared CMP interactions/pixels only; no native room, replay or playable device is simulated. */
@OptIn(ExperimentalTestApi::class)
class ShellToolsUiTest {
    @BeforeTest fun reset() = SimulatedGameHost.reset()
    @AfterTest fun cleanup() = SimulatedGameHost.reset()

    @Test
    fun roomsHistoryAndSettingsAreReachableWithBackAndHonestAvailabilityAtLargeText() {
        for ((width, language, fontScale) in listOf(Triple(320, "vi", 2f), Triple(768, "en", 1f))) {
            runDesktopComposeUiTest(width = width, height = 844) {
                setContent {
                    PhoneViewport(width, 844, fontScale) {
                        TabulaApp(scheme = TabulaScheme.Light, deviceFacts = DeviceFacts(false, language))
                    }
                }
                onNodeWithTag("shell-nav-account").performClick(); waitForIdle()
                for (tool in listOf("rooms", "history", "settings")) {
                    onNodeWithTag("account-open-$tool").performScrollTo().performClick(); waitForIdle()
                    onNodeWithTag("shell-$tool").assertIsDisplayed()
                    onNodeWithTag("shell-nav-account").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
                    if (tool != "settings") onNodeWithTag("shell-$tool-unavailable").performScrollTo().assertIsDisplayed()
                    else onNodeWithTag("settings-motion-reduced").performScrollTo().assertIsDisplayed()
                    assertShellTextFitsHorizontally()
                    captureShell("tools-$width-$language-font${(fontScale * 100).toInt()}-$tool")
                    onNodeWithTag("shell-back").performClick(); waitForIdle()
                    onNodeWithTag("shell-account").assertIsDisplayed()
                }
                assertEquals(0, SimulatedGameHost.createdCount)
                onNodeWithTag("account-open-rooms").performScrollTo().performClick(); waitForIdle()
                onNodeWithTag("rooms-open-library").performScrollTo().performClick(); waitForIdle()
                onNodeWithTag("shell-games").assertIsDisplayed()
                onNodeWithTag("shell-nav-games").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
                onNodeWithTag("shell-nav-home").performClick(); waitForIdle()
                onNodeWithTag("shell-home").assertIsDisplayed()
            }
        }
    }

    @Test
    fun settingsRestoreAndReachTheNextExplicitGameLaunchWithoutMountingFromTheMenu() =
        runDesktopComposeUiTest(width = 390, height = 844) {
            fun canSave(value: Any?): Boolean = when (value) {
                null, is String, is Int, is Long, is Float, is Double, is Boolean -> true
                is List<*> -> value.all(::canSave)
                is Map<*, *> -> value.all { (key, item) -> canSave(key) && canSave(item) }
                is MutableState<*> -> canSave(value.value)
                else -> false
            }
            var registry = SaveableStateRegistry(restoredValues = null, canBeSaved = ::canSave)
            val mounted = mutableStateOf(true)
            setContent {
                if (mounted.value) CompositionLocalProvider(LocalSaveableStateRegistry provides registry) {
                    PhoneViewport(390, 844) {
                        TabulaApp(
                            gameHost = SimulatedGameHost(autoBootMillis = null), games = previewGames, catalog = previewCatalog,
                            scheme = TabulaScheme.Light, deviceFacts = DeviceFacts(false, "en"),
                        )
                    }
                }
            }
            onNodeWithTag("shell-nav-account").performClick(); waitForIdle()
            onNodeWithTag("account-open-settings").performScrollTo().performClick(); waitForIdle()
            onNodeWithTag("settings-appearance-dark").performScrollTo().performClick(); waitForIdle()
            onNodeWithTag("settings-language-vi").performScrollTo().performClick(); waitForIdle()
            onNodeWithText("Trang chủ").assertIsDisplayed()
            onNodeWithTag("settings-motion-reduced").performScrollTo().performClick(); waitForIdle()
            assertEquals(0, SimulatedGameHost.createdCount, "local preferences cannot mount a runtime")
            val saved = registry.performSave()
            assertTrue(saved.isNotEmpty())
            mounted.value = false; waitForIdle()
            registry = SaveableStateRegistry(restoredValues = saved, canBeSaved = ::canSave)
            mounted.value = true; waitForIdle()
            onNodeWithTag("shell-settings").assertIsDisplayed()
            for (tag in listOf("settings-appearance-dark", "settings-language-vi", "settings-motion-reduced")) {
                onNodeWithTag(tag).performScrollTo().assertIsSelected()
            }
            onNodeWithText("Trang chủ").assertIsDisplayed()
            assertShellTextFitsHorizontally()
            captureShell("tools-390-dark-vi-settings-restored")
            onNodeWithTag("shell-back").performClick(); waitForIdle()
            onNodeWithTag("shell-account").assertIsDisplayed()
            onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
            startPreviewGame()
            val init = SimulatedGameHost.runtimes.single().page.received.first() as HostMessage.Init
            assertEquals(ThemePreference.Dark, init.preferences.theme)
            assertEquals(LocalePreference.Vietnamese, init.preferences.locale)
            assertEquals(MotionPreference.Reduced, init.preferences.motion)
            assertEquals(1, SimulatedGameHost.createdCount)
        }
}
