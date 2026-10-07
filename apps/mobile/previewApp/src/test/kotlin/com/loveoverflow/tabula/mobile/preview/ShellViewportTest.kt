package com.loveoverflow.tabula.mobile.preview

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.DesktopComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState
import com.loveoverflow.tabula.mobile.catalog.RegistryDiscoveryCatalog
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaTheme
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.shell.GameScreen
import com.loveoverflow.tabula.mobile.shell.HomeScreen
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import com.loveoverflow.tabula.mobile.voice.DevVoiceGrantSource
import com.loveoverflow.tabula.mobile.voice.VoiceClient
import com.loveoverflow.tabula.mobile.voice.VoiceClientObserver
import com.loveoverflow.tabula.mobile.voice.VoiceClock
import com.loveoverflow.tabula.mobile.voice.VoiceConnection
import com.loveoverflow.tabula.mobile.voice.VoiceController
import com.loveoverflow.tabula.mobile.voice.VoiceControls
import com.loveoverflow.tabula.mobile.voice.VoiceJoinGrant
import java.io.File
import javax.imageio.ImageIO
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlin.test.assertFalse

/** Rendered CMP viewport checks; the game page and media are labelled doubles, never native execution. */
@OptIn(ExperimentalTestApi::class)
class ShellViewportTest {
    private val strings = ShellStrings.forLanguage("en")
    private class MediaDouble : VoiceClient {
        private var observer: VoiceClientObserver? = null
        var micRequests = 0
        override fun connect(attempt: Long, grant: VoiceJoinGrant, observer: VoiceClientObserver) {
            this.observer = observer
            observer.onConnection(attempt, VoiceConnection.CONNECTED, null)
        }
        override fun setMicrophoneEnabled(attempt: Long, command: Long, enabled: Boolean) {
            micRequests++
            observer!!.onMicrophone(attempt, command, enabled, null)
        }
        override fun disconnect() {}
        override fun close() {}
    }

    private fun connectedVoice(media: MediaDouble): VoiceController {
        val grants = DevVoiceGrantSource.parse(
            """{"v":1,"scope":"${DevVoiceGrantSource.SCOPE}","endpoint":"ws://127.0.0.1:7880","token":"synthetic.viewport.fixture","expiresAt":700,"canPublish":true}""",
            100,
        )
        return VoiceController(media, grants, VoiceClock { 100 }).also {
            it.enterSession(DevVoiceGrantSource.SCOPE)
            it.join()
        }
    }

    private val launch = GameLaunch(
        "com.example.preview",
        GamePreferences(ThemePreference.Light, MotionPreference.System, LocalePreference.English),
        emptySet(),
    )

    @BeforeTest fun reset() = SimulatedGameHost.reset()
    @AfterTest fun cleanup() = SimulatedGameHost.reset()

    private fun DesktopComposeUiTest.screenshot(name: String) {
        System.getProperty("tabula.preview.screenshots")?.let { path ->
            val directory = File(path).also { it.mkdirs() }
            ImageIO.write(onRoot().captureToImage().toAwtImage(), "png", File(directory, "$name.png"))
        }
    }

    private fun DesktopComposeUiTest.assertWholeAction(label: String, width: Int, height: Int) {
        onNodeWithText(label).assertIsDisplayed()
        val bounds = onNodeWithText(label).getUnclippedBoundsInRoot()
        assertTrue(bounds.right - bounds.left >= 44.dp && bounds.bottom - bounds.top >= 44.dp, "$label keeps a 44 dp target: $bounds")
        assertTrue(bounds.left >= 0.dp && bounds.top >= 0.dp && bounds.right <= width.dp && bounds.bottom <= height.dp,
            "$label fits the viewport: $bounds in ${width}×$height")
    }

    @Test
    fun defaultAppShowsDiscoveryWithoutCreatingANativeGameplaySurface() {
        for (width in listOf(320, 390)) {
            runDesktopComposeUiTest(width = width, height = 844) {
                setContent { PhoneViewport(width, 844) { TabulaApp(deviceFacts = DeviceFacts(false, "en")) } }
                waitForIdle()
                onNodeWithTag("shell-details-${RegistryDiscoveryCatalog.games.first().id}").performScrollTo().assertIsDisplayed()
                onAllNodesWithTag("shell-start-local").assertCountEquals(0)
                onAllNodes(hasText("Play ", substring = true)).assertCountEquals(0)
                onAllNodesWithTag("sim-page").assertCountEquals(0)
                onAllNodesWithText("Ready", substring = true).assertCountEquals(0)
                assertEquals(0, SimulatedGameHost.createdCount)
                screenshot("discovery-registry-default-app-${width}x844")
            }
        }
    }

    @Test
    fun nativeUnavailableScreenKeepsTheSamePolicyInTheDarkThemeAdapter() = runDesktopComposeUiTest(width = 390, height = 844) {
        setContent {
            PhoneViewport(390, 844) {
                TabulaApp(scheme = TabulaScheme.Dark, deviceFacts = DeviceFacts(false, "en"))
            }
        }
        waitForIdle()
        onNodeWithTag("shell-details-${RegistryDiscoveryCatalog.games.first().id}").performScrollTo().assertIsDisplayed()
        onAllNodesWithTag("shell-start-local").assertCountEquals(0)
        onAllNodes(hasText("Play ", substring = true)).assertCountEquals(0)
        screenshot("discovery-registry-dark-theme-adapter-390x844")
    }

    @Test
    fun voiceActionsWrapRatherThanSqueezingTheSecondLabelOnSmallPhones() {
        for (width in listOf(320, 390)) {
            for (vietnamese in listOf(false, true)) {
                for (fontScale in listOf(1f, 2f)) {
                    runDesktopComposeUiTest(width = width, height = 844) {
                        val media = MediaDouble()
                        val voice = connectedVoice(media)
                        setContent {
                            CompositionLocalProvider(LocalDensity provides Density(1f, fontScale)) {
                                PhoneViewport(width, 844, fontScale) {
                                    TabulaTheme {
                                        Box(Modifier.fillMaxSize().background(LocalTabulaColors.current.surface).padding(16.dp)) {
                                            VoiceControls(voice, vietnamese)
                                        }
                                    }
                                }
                            }
                        }
                        waitForIdle()
                        val leave = if (vietnamese) "Rời voice" else "Leave voice"
                        val mic = if (vietnamese) "Bật mic" else "Turn mic on"
                        assertWholeAction(leave, width, 844)
                        assertWholeAction(mic, width, 844)
                        if (width == 320 && fontScale == 2f) {
                            assertTrue(onNodeWithText(mic).getUnclippedBoundsInRoot().top >=
                                onNodeWithText(leave).getUnclippedBoundsInRoot().bottom,
                                "At 200% text on a 320 dp phone the complete actions use separate rows")
                        }
                        onNodeWithText(mic).performClick()
                        waitForIdle()
                        assertEquals(1, media.micRequests)
                        assertTrue(voice.state.microphoneEnabled)
                        screenshot("cmp-voice-${if (vietnamese) "vi" else "en"}-${width}x844-font${fontScale.toInt()}-media-double")
                        voice.close()
                    }
                }
            }
        }
    }

    @Test
    fun homeActionsRemainReachableOnShortViewportsAndAtTwoHundredPercentText() {
        for ((width, height, fontScale) in listOf(Triple(320, 640, 2f), Triple(390, 844, 1f), Triple(640, 320, 2f))) {
            runDesktopComposeUiTest(width = width, height = height) {
                val games = (1..6).map { number ->
                    BundledGame("com.example.viewport$number", "/play/local/", "locale=en", mapOf("en" to "Board game $number"))
                }
                var opened: String? = null
                setContent {
                    CompositionLocalProvider(LocalDensity provides Density(1f, fontScale)) {
                        PhoneViewport(width, height, fontScale) {
                            TabulaTheme { HomeScreen(DiscoveryCatalogState.Ready(previewCatalogGames(games)), strings,
                                onDetail = { opened = it.id }, onBrowse = {}) }
                        }
                    }
                }
                waitForIdle()
                val last = "shell-details-${games.last().id}"
                onNodeWithTag(last).performScrollTo()
                waitForIdle()
                val bounds = onNodeWithTag(last).getUnclippedBoundsInRoot()
                assertTrue(bounds.right - bounds.left >= 44.dp && bounds.bottom - bounds.top >= 44.dp, "discovery card retains a complete target")
                onNodeWithText("Board game 6").assertIsDisplayed()
                screenshot("cmp-home-${width}x$height-font${fontScale.toInt()}-fixture-catalog")
                onNodeWithTag(last).performClick()
                assertEquals("com.example.viewport6", opened)
            }
        }
    }

    @Test
    fun longHostFailureScrollsToRetryAndTheCompactToolbarKeepsBackReachable() {
        val title = "A long packaged game name that must leave the native Back action reachable"
        val reason = "Synthetic host failure with a detailed explanation. ".repeat(12)
        for ((width, height) in listOf(320 to 640, 390 to 844, 640 to 320)) {
            runDesktopComposeUiTest(width = width, height = height) {
                SimulatedGameHost.reset()
                setContent {
                    CompositionLocalProvider(LocalDensity provides Density(1f, 2f)) {
                        PhoneViewport(width, height, fontScale = 2f) {
                            TabulaTheme { GameScreen(title, launch, SimulatedGameHost(autoBootMillis = null), onLeave = {}) }
                        }
                    }
                }
                waitForIdle()
                assertWholeAction(strings[ShellCopy.Back], width, height)
                SimulatedGameHost.runtimes.last().failHost(reason)
                waitForIdle()
                onNodeWithText(strings[ShellCopy.Retry]).performScrollTo()
                waitForIdle()
                assertWholeAction(strings[ShellCopy.Retry], width, height)
                assertWholeAction(strings[ShellCopy.Back], width, height)
                screenshot("cmp-host-failure-${width}x$height-font2-simulated-host")
                onNodeWithText(strings[ShellCopy.Retry]).performClick()
                waitForIdle()
                assertEquals(2, SimulatedGameHost.createdCount, "Retry creates one replacement runtime")
                assertEquals(1, SimulatedGameHost.disposedCount, "The failed runtime was released")
            }
        }
    }

    @Test
    fun nativeVoiceTextDoesNotTakeTheWholeGameViewportInShortLandscape() = runDesktopComposeUiTest(width = 640, height = 320) {
        val voice = connectedVoice(MediaDouble())
        setContent {
            CompositionLocalProvider(LocalDensity provides Density(1f, 2f)) {
                PhoneViewport(640, 320, fontScale = 2f) {
                    TabulaTheme(TabulaScheme.Dark) {
                        GameScreen("A long packaged game name with native voice", launch, SimulatedGameHost(autoBootMillis = null), onLeave = {}, voice = voice)
                    }
                }
            }
        }
        waitForIdle()
        assertWholeAction(strings[ShellCopy.Back], 640, 320)
        val surface = onNodeWithTag("sim-page").getUnclippedBoundsInRoot()
        assertTrue(surface.bottom - surface.top >= 44.dp,
            "The native voice panel leaves a nonzero, usable surface viewport")
        onNodeWithText("Turn mic on").performScrollTo()
        waitForIdle()
        assertWholeAction("Turn mic on", 640, 320)
        onNodeWithText("Turn mic on").performClick()
        assertTrue(voice.state.microphoneEnabled)
        assertEquals(1, SimulatedGameHost.createdCount, "Scrolling native controls retains the game runtime")
        voice.close()
    }
}
