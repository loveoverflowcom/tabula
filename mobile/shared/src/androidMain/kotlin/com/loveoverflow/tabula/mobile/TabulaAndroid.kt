package com.loveoverflow.tabula.mobile

import android.content.pm.ApplicationInfo
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.BundlePaths
import com.loveoverflow.tabula.mobile.host.GameBundle
import com.loveoverflow.tabula.mobile.host.WebViewGameHost
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
    val games = packagedGames()
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
    setContent { TabulaApp(gameHost = WebViewGameHost(games, assets), games = games,
        voice = voice, voiceScope = DevVoiceGrantSource.SCOPE) }
}

/** The games staged into this build's assets by `cargo xtask stage-mobile-game`; none if absent or invalid. */
private fun ComponentActivity.packagedGames(): List<BundledGame> = try {
    assets.open("${BundlePaths.ROOT}/${BundlePaths.MANIFEST}").use { stream ->
        // One byte past the limit is enough to know the file is too large without reading it all.
        val bytes = stream.readBounded(GameBundle.MANIFEST_LIMIT_BYTES + 1)
        GameBundle.parse(bytes.decodeToString()) ?: emptyList()
    }
} catch (_: java.io.IOException) {
    emptyList()
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
