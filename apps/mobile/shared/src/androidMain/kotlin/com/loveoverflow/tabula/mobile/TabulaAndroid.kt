package com.loveoverflow.tabula.mobile

import android.content.pm.ApplicationInfo
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import com.loveoverflow.tabula.mobile.voice.AndroidLiveKitVoiceClient
import com.loveoverflow.tabula.mobile.voice.AndroidMicrophonePermission
import com.loveoverflow.tabula.mobile.voice.DevVoiceGrantSource
import com.loveoverflow.tabula.mobile.voice.UnavailableVoiceGrantSource
import com.loveoverflow.tabula.mobile.voice.VoiceClock
import com.loveoverflow.tabula.mobile.voice.VoiceController
import com.loveoverflow.tabula.mobile.voice.VoiceGrantSource

/** Installs the shared Tabula UI into an Android activity; the activity stays a thin host. */
fun ComponentActivity.installTabulaContent() {
    enableEdgeToEdge()
    // Register the ActivityResult launcher before setContent and before STARTED. A permission
    // dialog only pauses the Activity; onStop is the conservative background boundary.
    val permission = AndroidMicrophonePermission(this)
    val clock = VoiceClock { System.currentTimeMillis() / 1_000 }
    val voice = VoiceController(AndroidLiveKitVoiceClient(this, permission), voiceGrants(clock), clock)
    lifecycle.addObserver(object : DefaultLifecycleObserver {
        override fun onStart(owner: LifecycleOwner) { voice.onForegroundChanged(true) }
        override fun onStop(owner: LifecycleOwner) { voice.onForegroundChanged(false) }
        override fun onDestroy(owner: LifecycleOwner) {
            voice.close()
            owner.lifecycle.removeObserver(this)
        }
    })
    // ADR-0043: the native adapter is not implemented. The default host and empty catalog
    // expose the unavailable state; no web document or alternate runtime is launched.
    setContent { TabulaApp(voice = voice, voiceScope = DevVoiceGrantSource.SCOPE) }
}

/** Production has no grant issuer. Only debuggable builds can read the native-only local fixture. */
private fun ComponentActivity.voiceGrants(clock: VoiceClock): VoiceGrantSource {
    if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE == 0) return UnavailableVoiceGrantSource
    return try {
        assets.open("voice-dev-grant.json").use { stream ->
            val bytes = stream.readBounded(DevVoiceGrantSource.MAX_BYTES + 1)
            if (bytes.size > DevVoiceGrantSource.MAX_BYTES) UnavailableVoiceGrantSource
            else DevVoiceGrantSource.parse(bytes.decodeToString(), clock.epochSeconds())
        }
    } catch (_: java.io.IOException) {
        UnavailableVoiceGrantSource
    }
}

/** InputStream.readNBytes is API 33; the host supports API 24 without core-library desugaring. */
private fun java.io.InputStream.readBounded(limit: Int): ByteArray {
    val bytes = ByteArray(limit)
    var count = 0
    while (count < limit) {
        val read = read(bytes, count, limit - count)
        if (read < 0) break
        if (read == 0) {
            val single = read()
            if (single < 0) break
            bytes[count++] = single.toByte()
        } else count += read
    }
    return bytes.copyOf(count)
}
