package com.loveoverflow.tabula.mobile

import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.BundlePaths
import com.loveoverflow.tabula.mobile.host.GameBundle
import com.loveoverflow.tabula.mobile.host.WebViewGameHost

/** Installs the shared Tabula UI into an Android activity; the activity stays a thin host. */
fun ComponentActivity.installTabulaContent() {
    enableEdgeToEdge()
    val games = packagedGames()
    setContent { TabulaApp(gameHost = WebViewGameHost(games, assets), games = games) }
}

/** The games staged into this build's assets by `cargo xtask stage-mobile-game`; none if absent or invalid. */
private fun ComponentActivity.packagedGames(): List<BundledGame> = try {
    assets.open("${BundlePaths.ROOT}/${BundlePaths.MANIFEST}").use { stream ->
        // One byte past the limit is enough to know the file is too large without reading it all.
        val bytes = stream.readNBytes(GameBundle.MANIFEST_LIMIT_BYTES + 1)
        GameBundle.parse(bytes.decodeToString()) ?: emptyList()
    }
} catch (_: java.io.IOException) {
    emptyList()
}
