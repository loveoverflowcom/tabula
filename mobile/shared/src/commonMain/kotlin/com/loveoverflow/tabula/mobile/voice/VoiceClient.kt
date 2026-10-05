package com.loveoverflow.tabula.mobile.voice

/** Native media plane, separate from backend `tabula-voice::VoiceService` (ADR-0037). */
interface VoiceClient {
    /** All callbacks must arrive on the host UI thread. Never forward SDK diagnostics or tokens. */
    fun connect(attempt: Long, grant: VoiceJoinGrant, observer: VoiceClientObserver)
    /** A positive request asks the OS for microphone permission before publishing. */
    fun setMicrophoneEnabled(attempt: Long, command: Long, enabled: Boolean)
    /** Retire callbacks synchronously; stop/release the native room even during an in-flight join. */
    fun disconnect()
    /** Final, idempotent owner cleanup. A closed client cannot reconnect. */
    fun close()
}

/** Only bounded public facts leave the SDK adapter. */
interface VoiceClientObserver {
    fun onConnection(attempt: Long, connection: VoiceConnection, error: VoiceError?)
    fun onMicrophone(attempt: Long, command: Long, enabled: Boolean, error: VoiceError?)
    fun onAudioInterruption(attempt: Long)
}

/** Upper-snake entries preserve word boundaries in Kotlin/Native's lowerCamelCase Swift export. */
enum class VoiceConnection { IDLE, RESOLVING_GRANT, CONNECTING, CONNECTED, RECONNECTING, UNAVAILABLE, FAILED }

/** Typed bounded failures; underscore-normalized entry names keep the native Swift selectors stable. */
enum class VoiceError { UNAVAILABLE, GRANT_EXPIRED, PERMISSION_DENIED, CONNECTION_FAILED, PUBLICATION_FAILED, AUDIO_INTERRUPTED, BACKGROUND_STOPPED }

/**
 * Memory-only provider credential. No data-class/serialization/diagnostic representation; never
 * put this object in GameLaunch, bridge messages, game WS, navigation or persistent preferences.
 * Only a host grant source constructs one. The SFU, not [canPublish], enforces actual authority.
 */
class VoiceJoinGrant internal constructor(
    val scope: String,
    val endpoint: String,
    val token: String,
    val expiresAtEpochSeconds: Long,
    val canPublish: Boolean,
) {
    override fun toString(): String = "VoiceJoinGrant([redacted])"
}

/** Host authority seam. A production implementation must bind current account/session/membership. */
interface VoiceGrantSource {
    fun request(scope: String, request: Long, callback: VoiceGrantCallback)
    fun cancel(request: Long)
    fun clear()
}

interface VoiceGrantCallback {
    fun onGrant(request: Long, grant: VoiceJoinGrant)
    fun onGrantUnavailable(request: Long, error: VoiceError)
}

/** Production fails closed until backend grant issuance and session revocation fences exist. */
object UnavailableVoiceGrantSource : VoiceGrantSource {
    override fun request(scope: String, request: Long, callback: VoiceGrantCallback) =
        callback.onGrantUnavailable(request, VoiceError.UNAVAILABLE)
    override fun cancel(request: Long) {}
    override fun clear() {}
}

/** Platform wall clock is used only by the imperative host, never by Rust game rules. */
fun interface VoiceClock { fun epochSeconds(): Long }

/** Public UI state contains no endpoint, room, identity or credential. Mic means local publication. */
data class VoiceState(
    val connection: VoiceConnection = VoiceConnection.IDLE,
    val microphoneEnabled: Boolean = false,
    val microphoneBusy: Boolean = false,
    val canPublish: Boolean = false,
    val error: VoiceError? = null,
)
