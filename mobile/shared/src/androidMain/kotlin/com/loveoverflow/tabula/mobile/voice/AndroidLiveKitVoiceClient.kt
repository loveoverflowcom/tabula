package com.loveoverflow.tabula.mobile.voice

import android.content.Context
import android.media.AudioManager
import android.os.Looper
import io.livekit.android.AudioOptions
import io.livekit.android.ConnectOptions
import io.livekit.android.LiveKit
import io.livekit.android.LiveKitOverrides
import io.livekit.android.events.RoomEvent
import io.livekit.android.events.collect
import io.livekit.android.room.Room
import io.livekit.android.room.participant.Participant
import io.livekit.android.room.track.Track
import io.livekit.android.util.LoggingLevel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/**
 * Android media-plane adapter for pinned LiveKit 2.29.0 (ADR-0037). No room is reused. UI-thread
 * ownership fences callbacks before cancellation; serialized SDK work drains before final release.
 * AudioSwitch owns communication mode, focus and wired/Bluetooth routing, including their cleanup.
 */
internal class AndroidLiveKitVoiceClient(
    context: Context,
    private val permission: AndroidMicrophonePermission,
) : VoiceClient {
    private val context = context.applicationContext
    private val owner = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val sdkCommands = Mutex()
    private var current: Attempt? = null
    private var cleanupTail: Job? = null
    private var closed = false
    private var cleanupFailed = false

    private class Attempt(
        val id: Long,
        val canPublish: Boolean,
        val expiry: Long,
        val observer: VoiceClientObserver,
        val job: Job,
    ) {
        var room: Room? = null
        var focusListener: AudioManager.OnAudioFocusChangeListener? = null
        var connected = false
        var microphoneCommand = -1L
        var microphoneEnabled = false
        var microphoneMutationPending = false
    }

    override fun connect(attempt: Long, grant: VoiceJoinGrant, observer: VoiceClientObserver) {
        requireUiThread()
        if (closed) return
        disconnect()
        val retiring = cleanupTail
        val next = Attempt(attempt, grant.canPublish, grant.expiresAtEpochSeconds, observer,
            SupervisorJob(owner.coroutineContext[Job]))
        current = next
        CoroutineScope(owner.coroutineContext + next.job).launch {
            retiring?.join()
            if (!isCurrent(next)) return@launch
            if (cleanupFailed) {
                fail(next, VoiceError.ConnectionFailed)
                return@launch
            }
            try {
                sdkCommands.withLock {
                    if (!isCurrent(next)) return@withLock
                    if (System.currentTimeMillis() / 1_000 >= next.expiry) {
                        fail(next, VoiceError.GrantExpired)
                        return@withLock
                    }
                    // The SDK's diagnostics can include URLs/credentials. Keep all provider logs off.
                    LiveKit.loggingLevel = LoggingLevel.OFF
                    LiveKit.enableWebRTCLogging = false
                    val room = LiveKit.create(context, overrides = LiveKitOverrides(
                        audioOptions = AudioOptions(disableAudioPrewarming = true),
                    ))
                    next.room = room
                    room.audioSwitchHandler?.apply {
                        loggingEnabled = false
                        val listener = AudioManager.OnAudioFocusChangeListener { focus ->
                            if (focus == AudioManager.AUDIOFOCUS_LOSS ||
                                focus == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT ||
                                focus == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK) {
                                owner.launch {
                                    if (isCurrent(next)) next.observer.onAudioInterruption(next.id)
                                }
                            }
                        }
                        next.focusListener = listener
                        registerOnAudioFocusChangeListener(listener)
                    }
                    // Subscribe before joining so no disconnect/reconnect event can be missed.
                    CoroutineScope(owner.coroutineContext + next.job).launch(start = CoroutineStart.UNDISPATCHED) {
                        room.events.collect { event ->
                            if (!isCurrent(next)) return@collect
                            when (event) {
                                is RoomEvent.Reconnecting -> if (next.connected) {
                                    next.observer.onConnection(next.id, VoiceConnection.Reconnecting, null)
                                }
                                is RoomEvent.Reconnected -> if (next.connected) {
                                    if (microphoneEnabled(room) != next.microphoneEnabled) {
                                        fail(next, VoiceError.PublicationFailed)
                                    } else next.observer.onConnection(next.id, VoiceConnection.Connected, null)
                                }
                                is RoomEvent.Disconnected -> fail(next, VoiceError.ConnectionFailed)
                                is RoomEvent.TrackMuted -> onMicrophonePublicationChanged(
                                    next, room, event.participant, event.publication.source)
                                is RoomEvent.TrackUnmuted -> onMicrophonePublicationChanged(
                                    next, room, event.participant, event.publication.source)
                                is RoomEvent.TrackUnpublished -> onMicrophonePublicationChanged(
                                    next, room, event.participant, event.publication.source)
                                is RoomEvent.TrackPublished -> onMicrophonePublicationChanged(
                                    next, room, event.participant, event.publication.source)
                                // Fail closed on an authority change after initial participant setup.
                                is RoomEvent.ParticipantPermissionsChanged -> if (
                                    next.connected && event.participant === room.localParticipant &&
                                    event.oldPermissions != null
                                ) {
                                    // The null -> initial permission event can be queued until after
                                    // connect() returns. It is setup, not a later authority change.
                                    fail(next, VoiceError.PublicationFailed)
                                }
                                else -> Unit
                            }
                        }
                    }
                    // Joining is strictly receive-only. No OS capture permission is requested here.
                    room.connect(grant.endpoint, grant.token, ConnectOptions(audio = false, video = false))
                    if (!isCurrent(next)) return@withLock
                    if (room.state != Room.State.CONNECTED) {
                        fail(next, VoiceError.ConnectionFailed)
                    } else {
                        next.connected = true
                        next.observer.onConnection(next.id, VoiceConnection.Connected, null)
                    }
                }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: Exception) {
                // No provider exception, URL, token, room identity or stack trace leaves this boundary.
                if (isCurrent(next)) fail(next, VoiceError.ConnectionFailed)
            }
        }
    }

    override fun setMicrophoneEnabled(attempt: Long, command: Long, enabled: Boolean) {
        requireUiThread()
        val active = current ?: return
        if (closed || active.id != attempt || !active.connected || command <= active.microphoneCommand) return
        active.microphoneCommand = command
        CoroutineScope(owner.coroutineContext + active.job).launch {
            if (enabled && (!active.canPublish || !permission.request())) {
                if (isCurrent(active) && command == active.microphoneCommand) {
                    active.observer.onMicrophone(attempt, command, active.microphoneEnabled, VoiceError.PermissionDenied)
                }
                return@launch
            }
            // Permission may finish after leaving, logging out, backgrounding or a newer command.
            if (!isCurrent(active) || command != active.microphoneCommand) return@launch
            if (System.currentTimeMillis() / 1_000 >= active.expiry) {
                active.observer.onMicrophone(attempt, command, false, VoiceError.GrantExpired)
                return@launch
            }
            sdkCommands.withLock {
                if (!isCurrent(active) || command != active.microphoneCommand) return@withLock
                if (System.currentTimeMillis() / 1_000 >= active.expiry) {
                    active.observer.onMicrophone(attempt, command, false, VoiceError.GrantExpired)
                    return@withLock
                }
                val room = active.room ?: return@withLock
                if (room.state != Room.State.CONNECTED) {
                    fail(active, VoiceError.ConnectionFailed)
                    return@withLock
                }
                active.microphoneMutationPending = true
                val success = try {
                    room.localParticipant.setMicrophoneEnabled(enabled)
                } catch (cancelled: CancellationException) {
                    throw cancelled
                } catch (_: SecurityException) {
                    if (isCurrent(active) && command == active.microphoneCommand) {
                        val actual = microphoneEnabled(room)
                        active.microphoneEnabled = actual
                        active.observer.onMicrophone(attempt, command, actual, VoiceError.PermissionDenied)
                    }
                    return@withLock
                } catch (_: Exception) {
                    false
                } finally {
                    active.microphoneMutationPending = false
                }
                if (!isCurrent(active) || command != active.microphoneCommand) return@withLock
                // isMicrophoneEnabled is a derived asynchronous flow in this SDK. Read the actual
                // publication directly to avoid acknowledging a lagging flow as a completed toggle.
                val actual = microphoneEnabled(room)
                active.microphoneEnabled = actual
                active.observer.onMicrophone(attempt, command, actual,
                    if (success && actual == enabled) null else VoiceError.PublicationFailed)
            }
        }
    }

    override fun disconnect() {
        requireUiThread()
        val old = current ?: return
        current = null // Synchronous retirement: even an already queued callback has no authority.
        old.job.cancel()
        val earlier = cleanupTail
        cleanupTail = owner.launch {
            withContext(NonCancellable) {
                earlier?.join()
                old.job.cancelAndJoin()
                sdkCommands.withLock {
                    val room = old.room
                    if (room != null) {
                        old.focusListener?.let { room.audioSwitchHandler?.unregisterOnAudioFocusChangeListener(it) }
                        // SDK disconnect uses runBlocking internally; never block the UI during release.
                        val released = withContext(Dispatchers.IO) {
                            var success = true
                            try { room.disconnect() } catch (_: Exception) { success = false }
                            try { room.release() } catch (_: Exception) { success = false }
                            // Ensure the SDK route/focus manager stops even if another SDK resource
                            // throws during release. A failed cleanup prevents another room join.
                            try { room.audioHandler.stop() } catch (_: Exception) { success = false }
                            success
                        }
                        if (!released) cleanupFailed = true
                    }
                    old.room = null
                    old.focusListener = null
                }
            }
        }
    }

    override fun close() {
        requireUiThread()
        if (closed) return
        disconnect()
        closed = true
        permission.close()
        val retiring = cleanupTail
        owner.launch {
            retiring?.join()
            owner.cancel()
        }
    }

    private fun fail(attempt: Attempt, error: VoiceError) {
        if (!isCurrent(attempt)) return
        attempt.observer.onConnection(attempt.id, VoiceConnection.Failed, error)
        // Usually the controller disconnects inside its callback; release even if another observer
        // does not, and never let that callback accidentally retire a newly created attempt.
        if (current === attempt) disconnect()
    }

    private fun isCurrent(attempt: Attempt): Boolean = !closed && current === attempt

    private fun onMicrophonePublicationChanged(
        attempt: Attempt,
        room: Room,
        participant: Participant,
        source: Track.Source,
    ) {
        if (!isCurrent(attempt) || !attempt.connected || attempt.microphoneMutationPending ||
            participant !== room.localParticipant || source != Track.Source.MICROPHONE ||
            room.state != Room.State.CONNECTED) return
        // Read the current publication, not the queued event's old muted/unpublished payload.
        // Own toggle events may arrive after the command completed; then current state already
        // matches its acknowledgement. An unsolicited difference cannot leave stale "Mic on" UI
        // or silently restore capture after "Mic off", so retire the room and require a new join.
        if (microphoneEnabled(room) != attempt.microphoneEnabled) fail(attempt, VoiceError.PublicationFailed)
    }

    private fun microphoneEnabled(room: Room): Boolean =
        room.localParticipant.getTrackPublication(Track.Source.MICROPHONE)?.let {
            it.track != null && !it.muted
        } ?: false

    private fun requireUiThread() {
        check(Looper.myLooper() == Looper.getMainLooper()) { "VoiceClient requires the UI thread" }
    }
}
