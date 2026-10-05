package com.loveoverflow.tabula.mobile.voice

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * App/session-owned native voice lifecycle, intentionally outside GameHost composition (ADR-0037).
 * Every grant/join/mic completion is fenced by its attempt/command. All calls use the UI thread.
 * A page reload/retry is not a leave; route leave, logout, background and disposal are terminal.
 */
class VoiceController(
    private val client: VoiceClient,
    private val grants: VoiceGrantSource,
    private val clock: VoiceClock,
) : VoiceClientObserver, VoiceGrantCallback {
    var state: VoiceState by mutableStateOf(VoiceState())
        private set
    private var attempt = 0L
    private var microphoneCommand = 0L
    private var scope: String? = null
    private var grantExpiry = 0L
    private var closed = false
    private var foreground = true

    /** The native host chooses this opaque scope; it is never selected by the game document. */
    fun enterSession(hostScope: String) {
        if (closed || hostScope.isEmpty() || hostScope.length > 128) return
        if (scope == hostScope) return
        if (scope != null) leaveSession() else leaveVoice()
        scope = hostScope
    }

    fun join() {
        val currentScope = scope ?: return
        if (closed || !foreground || state.connection in setOf(
                VoiceConnection.ResolvingGrant, VoiceConnection.Connecting,
                VoiceConnection.Connected, VoiceConnection.Reconnecting,
            )) return
        retire()
        state = VoiceState(connection = VoiceConnection.ResolvingGrant)
        grants.request(currentScope, attempt, this)
    }

    override fun onGrant(request: Long, grant: VoiceJoinGrant) {
        if (closed || !foreground || request != attempt || state.connection != VoiceConnection.ResolvingGrant) return
        val now = clock.epochSeconds()
        if (grant.scope != scope || grant.expiresAtEpochSeconds <= now ||
            grant.expiresAtEpochSeconds - now > 600) {
            state = VoiceState(connection = VoiceConnection.Failed, error = VoiceError.GrantExpired)
            return
        }
        grantExpiry = grant.expiresAtEpochSeconds
        state = VoiceState(connection = VoiceConnection.Connecting, canPublish = grant.canPublish)
        client.connect(attempt, grant, this)
    }

    override fun onGrantUnavailable(request: Long, error: VoiceError) {
        if (closed || request != attempt || state.connection != VoiceConnection.ResolvingGrant) return
        state = VoiceState(connection = VoiceConnection.Unavailable, error = error)
    }

    fun setMicrophoneEnabled(enabled: Boolean) {
        if (closed || !foreground || state.connection != VoiceConnection.Connected ||
            state.microphoneBusy || (enabled && !state.canPublish)) return
        if (expireIfNeeded()) return
        if (enabled == state.microphoneEnabled) return
        microphoneCommand += 1
        state = state.copy(microphoneBusy = true, error = null)
        client.setMicrophoneEnabled(attempt, microphoneCommand, enabled)
    }

    override fun onMicrophone(attempt: Long, command: Long, enabled: Boolean, error: VoiceError?) {
        if (closed || attempt != this.attempt || command != microphoneCommand ||
            state.connection !in setOf(VoiceConnection.Connected, VoiceConnection.Reconnecting) || !state.microphoneBusy) return
        if (expireIfNeeded()) return
        state = state.copy(microphoneEnabled = enabled, microphoneBusy = false, error = error)
    }

    override fun onConnection(attempt: Long, connection: VoiceConnection, error: VoiceError?) {
        if (closed || attempt != this.attempt || scope == null || !foreground) return
        if (state.connection !in setOf(VoiceConnection.Connecting, VoiceConnection.Connected, VoiceConnection.Reconnecting)) return
        if (expireIfNeeded()) return
        if (connection == VoiceConnection.Failed || connection == VoiceConnection.Idle) {
            retire()
            state = VoiceState(connection = VoiceConnection.Failed, error = error ?: VoiceError.ConnectionFailed)
        } else if (connection in setOf(VoiceConnection.Connecting, VoiceConnection.Connected, VoiceConnection.Reconnecting)) {
            // Reconnect may finish an interrupted publication. The adapter must serialize microphone
            // work and report its actual current state; do not invent mic-on from a connection event.
            state = state.copy(connection = connection, error = error)
        }
    }

    override fun onAudioInterruption(attempt: Long) {
        if (closed || attempt != this.attempt) return
        retire()
        state = VoiceState(connection = VoiceConnection.Failed, error = VoiceError.AudioInterrupted)
    }

    /** Called on each foreground lifecycle check; the SFU remains the authority for expiry/revoke. */
    fun checkAuthorityDeadline() { expireIfNeeded() }

    fun onForegroundChanged(inForeground: Boolean) {
        if (closed || foreground == inForeground) return
        foreground = inForeground
        if (!inForeground && state.connection in setOf(
                VoiceConnection.ResolvingGrant, VoiceConnection.Connecting,
                VoiceConnection.Connected, VoiceConnection.Reconnecting,
            )) {
            retire()
            state = VoiceState(connection = VoiceConnection.Idle, error = VoiceError.BackgroundStopped)
        }
        // Deliberately no autojoin or autopublish on foreground/interruption/permission recovery.
    }

    fun leaveVoice() {
        if (closed) return
        retire()
        state = VoiceState()
    }

    fun leaveSession() {
        if (closed) return
        leaveVoice()
        scope = null
        grants.clear()
    }

    /** Future account owner must call this before publishing signed-out/new-account UI. */
    fun onLogout() = leaveSession()

    fun close() {
        if (closed) return
        leaveSession()
        closed = true
        client.close()
    }

    private fun expireIfNeeded(): Boolean {
        if (grantExpiry == 0L || clock.epochSeconds() < grantExpiry) return false
        retire()
        state = VoiceState(connection = VoiceConnection.Failed, error = VoiceError.GrantExpired)
        return true
    }

    private fun retire() {
        grants.cancel(attempt)
        attempt += 1
        microphoneCommand += 1
        grantExpiry = 0L
        client.disconnect()
    }
}
