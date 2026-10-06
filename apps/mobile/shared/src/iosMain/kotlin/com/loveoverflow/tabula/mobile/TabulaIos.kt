@file:OptIn(ExperimentalForeignApi::class)

package com.loveoverflow.tabula.mobile

import androidx.compose.ui.window.ComposeUIViewController
import androidx.compose.runtime.DisposableEffect
import com.loveoverflow.tabula.mobile.host.BundlePaths
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.GameBundle
import com.loveoverflow.tabula.mobile.host.WKWebViewGameHost
import com.loveoverflow.tabula.mobile.voice.DevVoiceGrantSource
import com.loveoverflow.tabula.mobile.voice.UnavailableVoiceGrantSource
import com.loveoverflow.tabula.mobile.voice.VoiceClient
import com.loveoverflow.tabula.mobile.voice.VoiceClock
import com.loveoverflow.tabula.mobile.voice.VoiceController
import kotlinx.cinterop.ExperimentalForeignApi
import platform.Foundation.NSBundle
import platform.Foundation.NSData
import platform.Foundation.NSDate
import platform.Foundation.timeIntervalSince1970
import platform.Foundation.NSNotificationCenter
import platform.Foundation.NSOperationQueue
import platform.Foundation.NSString
import platform.Foundation.NSUTF8StringEncoding
import platform.Foundation.dataWithContentsOfFile
import platform.Foundation.stringWithContentsOfFile
import platform.UIKit.UIViewController
import platform.UIKit.UIApplication
import platform.UIKit.UIApplicationState
import platform.UIKit.UIApplicationDidBecomeActiveNotification
import platform.UIKit.UIApplicationDidEnterBackgroundNotification

/**
 * Native media is injected by the Swift host (ADR-0037). Release always supplies null fixture text;
 * no production grant source, SDK object, microphone request or WebView voice route is fabricated.
 */
fun TabulaViewController(voiceClient: VoiceClient, developmentGrantText: String?): UIViewController {
    val games = packagedGames()
    val clock = VoiceClock { NSDate().timeIntervalSince1970.toLong() }
    val grants = developmentGrantText?.let { DevVoiceGrantSource.parse(it, clock.epochSeconds()) }
        ?: UnavailableVoiceGrantSource
    val voice = VoiceController(voiceClient, grants, clock)
    return ComposeUIViewController {
        DisposableEffect(voice) {
            val center = NSNotificationCenter.defaultCenter
            // A microphone permission dialog makes UIApplication inactive, but is not backgrounding.
            // Real background entry is terminal. Becoming active only permits a new explicit join.
            voice.onForegroundChanged(UIApplication.sharedApplication.applicationState != UIApplicationState.UIApplicationStateBackground)
            val background = center.addObserverForName(
                UIApplicationDidEnterBackgroundNotification, null, NSOperationQueue.mainQueue,
            ) { voice.onForegroundChanged(false) }
            val active = center.addObserverForName(
                UIApplicationDidBecomeActiveNotification, null, NSOperationQueue.mainQueue,
            ) {
                voice.onForegroundChanged(true)
                voice.checkAuthorityDeadline()
            }
            onDispose {
                center.removeObserver(background)
                center.removeObserver(active)
                voice.close()
            }
        }
        TabulaApp(gameHost = WKWebViewGameHost(games), games = games, voice = voice, voiceScope = DevVoiceGrantSource.SCOPE)
    }
}

/** The games the Xcode build copied into the app bundle; none if absent, oversized or invalid. */
private fun packagedGames(): List<BundledGame> {
    val path = "${NSBundle.mainBundle.resourcePath}/${BundlePaths.ROOT}/${BundlePaths.MANIFEST}"
    val data = NSData.dataWithContentsOfFile(path) ?: return emptyList()
    if (data.length > GameBundle.MANIFEST_LIMIT_BYTES.toULong()) return emptyList()
    val text = NSString.stringWithContentsOfFile(path, NSUTF8StringEncoding, null) ?: return emptyList()
    return GameBundle.parse(text) ?: emptyList()
}
