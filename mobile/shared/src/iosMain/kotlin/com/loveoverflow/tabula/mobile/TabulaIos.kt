@file:OptIn(ExperimentalForeignApi::class)

package com.loveoverflow.tabula.mobile

import androidx.compose.ui.window.ComposeUIViewController
import com.loveoverflow.tabula.mobile.host.BundlePaths
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.GameBundle
import com.loveoverflow.tabula.mobile.host.WKWebViewGameHost
import kotlinx.cinterop.ExperimentalForeignApi
import platform.Foundation.NSBundle
import platform.Foundation.NSData
import platform.Foundation.NSString
import platform.Foundation.NSUTF8StringEncoding
import platform.Foundation.dataWithContentsOfFile
import platform.Foundation.stringWithContentsOfFile
import platform.UIKit.UIViewController

/** The view controller the Swift host presents; the host stays a thin container. */
fun TabulaViewController(): UIViewController {
    val games = packagedGames()
    return ComposeUIViewController { TabulaApp(gameHost = WKWebViewGameHost(games), games = games) }
}

/** The games the Xcode build copied into the app bundle; none if absent, oversized or invalid. */
private fun packagedGames(): List<BundledGame> {
    val path = "${NSBundle.mainBundle.resourcePath}/${BundlePaths.ROOT}/${BundlePaths.MANIFEST}"
    val data = NSData.dataWithContentsOfFile(path) ?: return emptyList()
    if (data.length > GameBundle.MANIFEST_LIMIT_BYTES.toULong()) return emptyList()
    val text = NSString.stringWithContentsOfFile(path, NSUTF8StringEncoding, null) ?: return emptyList()
    return GameBundle.parse(text) ?: emptyList()
}
