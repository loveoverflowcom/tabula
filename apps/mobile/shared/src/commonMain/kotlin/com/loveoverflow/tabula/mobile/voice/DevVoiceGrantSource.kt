package com.loveoverflow.tabula.mobile.voice

import com.loveoverflow.tabula.mobile.bridge.Json
import com.loveoverflow.tabula.mobile.bridge.StrictJson

/**
 * Explicitly isolated localhost harness, never a production/authentication fallback (ADR-0037).
 * The native debug host loads one short-lived credential from an uncommitted fixture. There is
 * no grant signing here, no arbitrary room selector, and no game/bridge access to the fixture.
 */
class DevVoiceGrantSource private constructor(private var grant: VoiceJoinGrant?) : VoiceGrantSource {
    override fun request(scope: String, request: Long, callback: VoiceGrantCallback) {
        val value = grant
        if (scope != SCOPE || value == null) callback.onGrantUnavailable(request, VoiceError.UNAVAILABLE)
        else callback.onGrant(request, value)
    }
    override fun cancel(request: Long) {}
    override fun clear() { grant = null }

    companion object {
        const val SCOPE = "tabula-native-voice-dev"
        const val MAX_BYTES = 20_000
        private val endpoints = setOf("ws://127.0.0.1:7880", "ws://localhost:7880", "ws://10.0.2.2:7880")

        /** Invalid/missing/expired fixture means unavailable, never a weaker production path. */
        fun parse(text: String, nowEpochSeconds: Long): VoiceGrantSource {
            if (text.encodeToByteArray().size > MAX_BYTES) return UnavailableVoiceGrantSource
            val fields = (StrictJson.parse(text) as? Json.Obj)?.fields ?: return UnavailableVoiceGrantSource
            if (fields.keys != setOf("v", "scope", "endpoint", "token", "expiresAt", "canPublish")) return UnavailableVoiceGrantSource
            if ((fields["v"] as? Json.Num)?.value != 1L || (fields["scope"] as? Json.Str)?.value != SCOPE) return UnavailableVoiceGrantSource
            val endpoint = (fields["endpoint"] as? Json.Str)?.value ?: return UnavailableVoiceGrantSource
            if (endpoint !in endpoints) return UnavailableVoiceGrantSource
            val token = (fields["token"] as? Json.Str)?.value ?: return UnavailableVoiceGrantSource
            if (token.length !in 1..16_384 || token.any { it !in 'A'..'Z' && it !in 'a'..'z' && it !in '0'..'9' && it != '-' && it != '_' && it != '.' }) return UnavailableVoiceGrantSource
            val expiry = (fields["expiresAt"] as? Json.Num)?.value ?: return UnavailableVoiceGrantSource
            if (nowEpochSeconds < 0 || expiry <= nowEpochSeconds || expiry - nowEpochSeconds > 600) return UnavailableVoiceGrantSource
            val publish = (fields["canPublish"] as? Json.Bool)?.value ?: return UnavailableVoiceGrantSource
            return DevVoiceGrantSource(VoiceJoinGrant(SCOPE, endpoint, token, expiry, publish))
        }
    }
}
