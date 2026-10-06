package com.loveoverflow.tabula.mobile.preview

import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onRoot
import java.io.File
import javax.imageio.ImageIO
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import com.loveoverflow.tabula.mobile.voice.*
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Shared CMP interactions against labelled native-media and game-page doubles, not real audio. */
@OptIn(ExperimentalTestApi::class)
class VoiceShellTest {
    private val strings = ShellStrings.forLanguage("en")
    private class MediaDouble : VoiceClient {
        var observer: VoiceClientObserver? = null
        var attempt = 0L
        var command = 0L
        var connects = 0
        var micRequests = 0
        var disconnects = 0
        var closes = 0
        override fun connect(attempt: Long, grant: VoiceJoinGrant, observer: VoiceClientObserver) {
            this.attempt = attempt; this.observer = observer; connects++
        }
        override fun setMicrophoneEnabled(attempt: Long, command: Long, enabled: Boolean) {
            this.command = command; micRequests++
        }
        override fun disconnect() { disconnects++ }
        override fun close() { closes++ }
        fun connected() = observer!!.onConnection(attempt, VoiceConnection.CONNECTED, null)
        fun microphone(enabled: Boolean, error: VoiceError? = null) = observer!!.onMicrophone(attempt, command, enabled, error)
    }
    private fun source() = DevVoiceGrantSource.parse(
        "{\"v\":1,\"scope\":\"${DevVoiceGrantSource.SCOPE}\",\"endpoint\":\"ws://127.0.0.1:7880\",\"token\":\"synthetic.test.fixture\",\"expiresAt\":700,\"canPublish\":true}", 100,
    )
    @BeforeTest fun reset() = SimulatedGameHost.reset()
    @AfterTest fun cleanup() = SimulatedGameHost.reset()

    @Test fun productionUnavailableIsVisibleAndHasNoMediaEffect() = runDesktopComposeUiTest(width = 390, height = 844) {
        val media = MediaDouble()
        val voice = VoiceController(media, UnavailableVoiceGrantSource, VoiceClock { 100 })
        setContent { PhoneViewport(390, 844) { TabulaApp(SimulatedGameHost(), previewGames, voice, scheme = TabulaScheme.Light, deviceFacts = DeviceFacts(false, "en")) } }
        onNodeWithText("Play Preview game on this device").performClick()
        onNodeWithText("Join voice").performClick(); waitForIdle()
        onNodeWithText("Voice unavailable: backend grants are not connected").assertIsDisplayed()
        assertEquals(0, media.connects); assertEquals(0, media.micRequests)
    }

    @Test fun controlsJoinPermissionDenialRetryMicAndVoiceLeaveAreCmpOwned() = runDesktopComposeUiTest(width = 390, height = 844) {
        val media = MediaDouble()
        val voice = VoiceController(media, source(), VoiceClock { 100 })
        setContent { PhoneViewport(390, 844) { TabulaApp(SimulatedGameHost(), previewGames, voice, scheme = TabulaScheme.Light, deviceFacts = DeviceFacts(false, "en")) } }
        onNodeWithText("Play Preview game on this device").performClick()
        onNodeWithText("Join voice").performClick(); waitForIdle()
        assertEquals(1, media.connects); assertEquals(0, media.micRequests)
        media.connected(); waitForIdle()
        onNodeWithText("Turn mic on").performClick(); waitForIdle()
        media.microphone(false, VoiceError.PERMISSION_DENIED); waitForIdle()
        onNodeWithText("Voice connected · Mic off").assertIsDisplayed()
        assertFalse(voice.state.microphoneEnabled)
        onNodeWithText("Turn mic on").performClick(); waitForIdle()
        media.microphone(true); waitForIdle()
        System.getProperty("tabula.preview.screenshots")?.let { path ->
            val directory = File(path).also { it.mkdirs() }
            ImageIO.write(onRoot().captureToImage().toAwtImage(), "png", File(directory, "voice-cmp-connected-double.png"))
        }
        onNodeWithText("Turn mic off").performClick(); waitForIdle()
        media.microphone(false); waitForIdle()
        onNodeWithText("Leave voice").performClick(); waitForIdle()
        assertEquals(VoiceConnection.IDLE, voice.state.connection)
        // The game surface remains mounted and no board input was sent by mic controls.
        assertEquals(1, SimulatedGameHost.createdCount); assertEquals(0, SimulatedGameHost.disposedCount)
    }

    @Test fun gameReloadAndHostRetryPreserveVoiceButConfirmedRouteLeaveCleansUp() = runDesktopComposeUiTest(width = 390, height = 844) {
        val media = MediaDouble()
        val voice = VoiceController(media, source(), VoiceClock { 100 })
        setContent { PhoneViewport(390, 844) { TabulaApp(SimulatedGameHost(), previewGames, voice, scheme = TabulaScheme.Light, deviceFacts = DeviceFacts(false, "en")) } }
        onNodeWithText("Play Preview game on this device").performClick(); waitForIdle()
        var runtime = SimulatedGameHost.runtimes.last()
        runtime.page.completeBoot(); waitForIdle()
        onNodeWithText("Join voice").performClick(); media.connected(); waitForIdle()
        val before = media.disconnects
        onNodeWithTag("sim-reload").performClick(); waitForIdle()
        assertEquals(before, media.disconnects); assertEquals(VoiceConnection.CONNECTED, voice.state.connection)
        runtime.failHost("synthetic host failure"); waitForIdle()
        onNodeWithText(strings[ShellCopy.Retry]).performClick(); waitForIdle()
        assertEquals(before, media.disconnects)
        runtime = SimulatedGameHost.runtimes.last(); runtime.page.completeBoot(); waitForIdle()
        onNodeWithText(strings[ShellCopy.Back]).performClick(); waitForIdle()
        onNodeWithTag("sim-confirm").performClick(); waitForIdle()
        assertTrue(media.disconnects > before); assertEquals(VoiceConnection.IDLE, voice.state.connection)
        onNodeWithText("Play Preview game on this device").assertIsDisplayed()
    }
}
