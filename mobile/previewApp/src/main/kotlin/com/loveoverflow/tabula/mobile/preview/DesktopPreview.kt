package com.loveoverflow.tabula.mobile.preview

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.host.BundledGame

/** A phone-sized content box; the shell's own safe-area handling is the only inset owner. */
@Composable
internal fun PhoneViewport(width: Int, height: Int, content: @Composable () -> Unit) {
    Box(Modifier.requiredSize(width.dp, height.dp).clipToBounds().testTag("phone-viewport")) { content() }
}

/** Fixture catalog: the same shape `tabula-games.json` has, with an opaque id and no game logic. */
internal val previewGames = listOf(
    BundledGame(
        id = "com.example.preview",
        entry = "/play/local/",
        query = "locale=en",
        names = mapOf("en" to "Preview game", "vi" to "Trò chơi mẫu"),
    ),
)

/** `./gradlew :previewApp:run` (testing only; the page is simulated, see SimulatedPage). */
fun main() {
    val width = System.getProperty("tabula.preview.width", "390").toInt()
    val height = System.getProperty("tabula.preview.height", "844").toInt()
    System.setProperty("compose.layers.type", "ON_SAME_CANVAS")
    SimulatedGameHost.reset()
    application {
        Window(
            onCloseRequest = ::exitApplication,
            title = "Tabula shell — desktop preview (simulated game page) — $width×$height dp",
            state = rememberWindowState(width = Dp.Unspecified, height = Dp.Unspecified),
            resizable = false,
        ) {
            LaunchedEffect(Unit) {
                withFrameNanos { }
                withFrameNanos { }
                val actual = window.contentPane.size
                println("Tabula preview content: ${actual.width}x${actual.height} (requested ${width}x$height) density=${window.graphicsConfiguration?.defaultTransform?.scaleX}")
                if (System.getProperty("tabula.preview.smokeWindow") == "true") exitApplication()
            }
            PhoneViewport(width, height) { TabulaApp(gameHost = SimulatedGameHost(), games = previewGames) }
        }
    }
}
