package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.session.GameSession
import com.loveoverflow.tabula.mobile.voice.*
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Controller/adapter doubles only. No SFU, microphone, native audio or WebView execution implied. */
class VoiceControllerTest {
    private class Client : VoiceClient {
        var observer: VoiceClientObserver? = null
        var attempt = 0L
        var command = 0L
        var requestedMic = false
        var connects = 0
        var disconnects = 0
        var closes = 0
        var micCalls = 0
        override fun connect(attempt: Long, grant: VoiceJoinGrant, observer: VoiceClientObserver) {
            this.attempt = attempt; this.observer = observer; connects++
        }
        override fun setMicrophoneEnabled(attempt: Long, command: Long, enabled: Boolean) {
            this.command = command; requestedMic = enabled; micCalls++
        }
        override fun disconnect() { disconnects++ }
        override fun close() { closes++ }
        fun connected() = observer!!.onConnection(attempt, VoiceConnection.Connected, null)
        fun microphone(enabled: Boolean, error: VoiceError? = null) = observer!!.onMicrophone(attempt, command, enabled, error)
    }
    private class Grants : VoiceGrantSource {
        var callback: VoiceGrantCallback? = null
        var request = 0L
        var requests = 0
        var clears = 0
        override fun request(scope: String, request: Long, callback: VoiceGrantCallback) {
            this.request = request; this.callback = callback; requests++
        }
        override fun cancel(request: Long) {}
        override fun clear() { clears++ }
        fun deliver(expiry: Long = 700, scope: String = "host-session", canPublish: Boolean = true) =
            callback!!.onGrant(request, VoiceJoinGrant(scope, "ws://127.0.0.1:7880", "synthetic.test.only", expiry, canPublish))
    }
    private class Setup {
        val client = Client()
        val grants = Grants()
        var now = 100L
        val controller = VoiceController(client, grants, VoiceClock { now })
        init { controller.enterSession("host-session") }
        fun connected(canPublish: Boolean = true) {
            controller.join(); grants.deliver(canPublish = canPublish); client.connected()
        }
    }

    @Test fun productionUnavailableNeverConnectsOrRequestsMicrophone() {
        val client = Client()
        val c = VoiceController(client, UnavailableVoiceGrantSource, VoiceClock { 100 })
        c.enterSession("host-session"); c.join(); c.setMicrophoneEnabled(true)
        assertEquals(VoiceConnection.Unavailable, c.state.connection)
        assertEquals(0, client.connects); assertEquals(0, client.micCalls)
    }

    @Test fun firstEnterPreservesTheFreshDevGrantAndConnectsReceiveOnly() {
        val source = DevVoiceGrantSource.parse(fixture(), 100)
        val client = Client()
        val c = VoiceController(client, source, VoiceClock { 100 })
        c.enterSession(DevVoiceGrantSource.SCOPE); c.join(); client.connected()
        assertEquals(1, client.connects); assertFalse(c.state.microphoneEnabled); assertEquals(0, client.micCalls)
    }

    @Test fun repeatedJoinAndMicActionsAreSingleFlight() {
        val s = Setup(); s.controller.join(); repeat(5) { s.controller.join() }
        assertEquals(1, s.grants.requests)
        s.grants.deliver(); s.client.connected(); repeat(5) { s.controller.join() }
        assertEquals(1, s.client.connects)
        repeat(5) { s.controller.setMicrophoneEnabled(true) }
        assertEquals(1, s.client.micCalls); assertTrue(s.controller.state.microphoneBusy)
        s.client.microphone(true); s.controller.setMicrophoneEnabled(true)
        assertEquals(1, s.client.micCalls); assertTrue(s.controller.state.microphoneEnabled)
    }

    @Test fun permissionDeniedKeepsListeningAndAllowsExplicitRetry() {
        val s = Setup(); s.connected(); s.controller.setMicrophoneEnabled(true)
        s.client.microphone(false, VoiceError.PermissionDenied)
        assertEquals(VoiceConnection.Connected, s.controller.state.connection)
        assertEquals(VoiceError.PermissionDenied, s.controller.state.error)
        assertFalse(s.controller.state.microphoneEnabled); assertFalse(s.controller.state.microphoneBusy)
        s.controller.setMicrophoneEnabled(true); assertEquals(2, s.client.micCalls)
    }

    @Test fun receiveOnlyGrantCannotEnableLocalMic() {
        val s = Setup(); s.connected(canPublish = false); s.controller.setMicrophoneEnabled(true)
        assertEquals(0, s.client.micCalls); assertFalse(s.controller.state.canPublish)
    }

    @Test fun staleGrantAfterLeaveOrLogoutCannotConnect() {
        for (logout in listOf(false, true)) {
            val s = Setup(); s.controller.join()
            if (logout) s.controller.onLogout() else s.controller.leaveSession()
            s.grants.deliver()
            assertEquals(0, s.client.connects); assertEquals(VoiceConnection.Idle, s.controller.state.connection)
            s.controller.join(); assertEquals(1, s.grants.requests)
        }
    }

    @Test fun staleJoinAndMicAfterLeaveCannotRestoreOutput() {
        val s = Setup(); s.connected(); s.controller.setMicrophoneEnabled(true)
        val old = s.client.attempt; val oldCommand = s.client.command
        s.controller.leaveVoice()
        s.client.observer!!.onConnection(old, VoiceConnection.Connected, null)
        s.client.observer!!.onMicrophone(old, oldCommand, true, null)
        assertEquals(VoiceConnection.Idle, s.controller.state.connection); assertFalse(s.controller.state.microphoneEnabled)
        s.connected(); assertFalse(s.controller.state.microphoneEnabled)
        s.client.observer!!.onConnection(old, VoiceConnection.Failed, VoiceError.ConnectionFailed)
        assertEquals(VoiceConnection.Connected, s.controller.state.connection)
    }

    @Test fun olderMicCompletionCannotOverrideNewerCommand() {
        val s = Setup(); s.connected(); s.controller.setMicrophoneEnabled(true)
        val old = s.client.command
        s.client.microphone(true); s.controller.setMicrophoneEnabled(false)
        s.client.observer!!.onMicrophone(s.client.attempt, old, true, null)
        assertTrue(s.controller.state.microphoneBusy)
        s.client.microphone(false); assertFalse(s.controller.state.microphoneEnabled)
    }

    @Test fun reconnectReportsNetworkLossAndPendingMicCompletionDoesNotStick() {
        val s = Setup(); s.connected(); s.controller.setMicrophoneEnabled(true)
        s.client.observer!!.onConnection(s.client.attempt, VoiceConnection.Reconnecting, null)
        s.client.microphone(false, VoiceError.PublicationFailed)
        assertFalse(s.controller.state.microphoneBusy)
        s.controller.setMicrophoneEnabled(true); assertEquals(1, s.client.micCalls)
        s.client.connected(); assertEquals(VoiceConnection.Connected, s.controller.state.connection)
        s.controller.setMicrophoneEnabled(true); assertEquals(2, s.client.micCalls)
    }

    @Test fun backgroundStopsResourcesAndDoesNotAutoJoinOrUnmute() {
        val s = Setup(); s.connected(); s.controller.setMicrophoneEnabled(true); s.client.microphone(true)
        val before = s.client.disconnects
        s.controller.onForegroundChanged(false)
        assertTrue(s.client.disconnects > before); assertFalse(s.controller.state.microphoneEnabled)
        assertEquals(VoiceError.BackgroundStopped, s.controller.state.error)
        s.controller.join(); assertEquals(1, s.grants.requests)
        s.controller.onForegroundChanged(true)
        assertEquals(1, s.grants.requests)
        s.connected(); assertFalse(s.controller.state.microphoneEnabled)
    }

    @Test fun interruptionStopsRoomAndLateNativeEventsCannotResumeIt() {
        val s = Setup(); s.connected(); val old = s.client.attempt
        s.client.observer!!.onAudioInterruption(old); s.client.connected()
        assertEquals(VoiceError.AudioInterrupted, s.controller.state.error)
        assertEquals(VoiceConnection.Failed, s.controller.state.connection)
    }

    @Test fun grantWrongScopeExpiredBoundaryAndTooLongLifetimeFailClosed() {
        for ((expiry, scope) in listOf(100L to "host-session", 701L to "host-session", 700L to "foreign")) {
            val s = Setup(); s.controller.join(); s.grants.deliver(expiry = expiry, scope = scope)
            assertEquals(0, s.client.connects); assertEquals(VoiceError.GrantExpired, s.controller.state.error)
        }
    }

    @Test fun deadlineDisconnectsAtEqualityAndNeverResumesFromOldCallbacks() {
        val s = Setup(); s.connected(); s.now = 700; s.controller.checkAuthorityDeadline(); s.client.connected()
        assertEquals(VoiceError.GrantExpired, s.controller.state.error)
        assertEquals(VoiceConnection.Failed, s.controller.state.connection)
    }

    @Test fun closeIsIdempotentAndTerminalAndClearsGrantSource() {
        val s = Setup(); s.connected(); s.controller.close(); s.controller.close(); s.controller.join(); s.client.connected()
        assertEquals(1, s.client.closes); assertEquals(1, s.grants.clears)
        assertEquals(1, s.client.connects); assertEquals(VoiceConnection.Idle, s.controller.state.connection)
    }

    @Test fun gameDocumentHelloReloadDoesNotDisconnectNativeVoice() {
        val s = Setup(); s.connected(); val before = s.client.disconnects
        val prefs = GamePreferences(ThemePreference.Light, MotionPreference.Reduced, LocalePreference.English)
        val game = GameSession(emptySet(), prefs)
        game.onPageText("{\"v\":1,\"type\":\"hello\"}")
        game.onPageText("{\"v\":1,\"type\":\"hello\"}")
        assertEquals(2, game.generation)
        assertEquals(before, s.client.disconnects); assertEquals(VoiceConnection.Connected, s.controller.state.connection)
    }

    @Test fun devFixtureGrammarEndpointLifetimeAndDiagnosticsAreBounded() {
        for (bad in listOf(
            fixture(endpoint = "wss://production.example"), fixture(expiry = 100), fixture(expiry = 701),
            fixture().replace("\"v\":1", "\"v\":1,\"v\":1"), fixture().replace("\"token\":\"test.token.fixture\"", "\"token\":\"bad secret\""),
        )) assertEquals(UnavailableVoiceGrantSource, DevVoiceGrantSource.parse(bad, 100))
        assertEquals("VoiceJoinGrant([redacted])", VoiceJoinGrant("scope", "endpoint", "secret", 500, true).toString())
    }

    private fun fixture(endpoint: String = "ws://127.0.0.1:7880", expiry: Long = 700) =
        "{\"v\":1,\"scope\":\"${DevVoiceGrantSource.SCOPE}\",\"endpoint\":\"$endpoint\",\"token\":\"test.token.fixture\",\"expiresAt\":$expiry,\"canPublish\":true}"
}
