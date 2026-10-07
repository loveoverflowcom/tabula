package com.loveoverflow.tabula.mobile.preview

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState
import com.loveoverflow.tabula.mobile.catalog.DiscoveryGame
import com.loveoverflow.tabula.mobile.catalog.RegistryDiscoveryCatalog
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.shell.DeviceFacts

/** A phone-sized content box; the shell's own safe-area handling is the only inset owner. */
@Composable
internal fun PhoneViewport(width: Int, height: Int, fontScale: Float = 1f, content: @Composable () -> Unit) {
    val density = LocalDensity.current
    CompositionLocalProvider(LocalDensity provides Density(density.density, fontScale)) {
        Box(Modifier.requiredSize(width.dp, height.dp).clipToBounds().testTag("phone-viewport")) { content() }
    }
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

/** Explicit display fixtures, separate from the simulated host's packaged launch authority. */
internal fun previewCatalogGames(games: List<BundledGame>): List<DiscoveryGame> = games.map { game ->
    DiscoveryGame(
        id = game.id,
        names = game.names,
        taglines = mapOf("en" to "A board game for a thoughtful break", "vi" to "Một trò chơi bàn cho phút nghỉ thảnh thơi"),
        descriptions = mapOf("en" to "Explicit desktop discovery fixture; gameplay uses a labelled simulated host.",
            "vi" to "Dữ liệu khám phá mẫu trên máy tính; trang chơi là mô phỏng có nhãn."),
        categories = listOf("abstract"),
        categoryNames = mapOf("abstract" to mapOf("en" to "Abstract", "vi" to "Trừu tượng")),
        players = listOf(2),
        minMinutes = 15,
        maxMinutes = 30,
        complexity = "light",
        complexityNames = mapOf("en" to "Light", "vi" to "Nhẹ"),
        rulesVersion = 1,
    )
}

internal val previewCatalog: DiscoveryCatalogState get() = DiscoveryCatalogState.Ready(previewCatalogGames(previewGames))

/** Long localized names test real text reflow rather than a truncated thumbnail label. */
internal fun longPreviewGames(count: Int = 8) = (1..count).map { index ->
    BundledGame(
        id = "com.example.long-$index",
        entry = "/play/local/",
        query = "locale=vi",
        names = mapOf(
            "en" to "A long board game title for an accessible and thoughtful family gathering $index",
            "vi" to "Trò chơi bàn dành cho gia đình với tên dài để kiểm tra khả năng đọc và xuống dòng $index",
        ),
    )
}

/** `./gradlew :previewApp:run` (testing only; the page is simulated, see SimulatedPage). */
fun main() {
    val width = System.getProperty("tabula.preview.width", "390").toInt()
    val height = System.getProperty("tabula.preview.height", "844").toInt()
    val fontScale = System.getProperty("tabula.preview.fontScale", "1").toFloat()
    val scheme = if (System.getProperty("tabula.preview.dark") == "true") TabulaScheme.Dark else TabulaScheme.Light
    val facts = DeviceFacts(
        reducedMotion = System.getProperty("tabula.preview.reducedMotion") == "true",
        languageTag = System.getProperty("tabula.preview.language", "en"),
    )
    val accountScenario = SyntheticPreviewAccountScenario.parse(System.getProperty("tabula.preview.account", "unavailable"))
    val accountFixture = SyntheticPreviewAccountFixture(
        scenario = accountScenario,
        longFields = System.getProperty("tabula.preview.accountLongFields") == "true",
        vietnamese = facts.languageTag.lowercase().startsWith("vi"),
    )
    val accountAvatar = when (System.getProperty("tabula.preview.accountAvatar", "neutral")) {
        "neutral" -> null
        "managed" -> syntheticManagedAvatar(accountFixture.identity, scheme)
        else -> error("preview.accountAvatar must be neutral or managed")
    }
    val catalog = when (System.getProperty("tabula.preview.catalog", "registry")) {
        "zero" -> DiscoveryCatalogState.Ready(emptyList())
        "one", "simulated" -> previewCatalog
        "many" -> DiscoveryCatalogState.Ready(previewCatalogGames(longPreviewGames()))
        "loading" -> DiscoveryCatalogState.Loading
        "error" -> DiscoveryCatalogState.Error("Explicit desktop catalog fixture")
        "unavailable" -> DiscoveryCatalogState.Unavailable
        "registry" -> DiscoveryCatalogState.Ready(RegistryDiscoveryCatalog.games)
        else -> error("preview.catalog must be registry, zero, one, many, simulated, loading, error or unavailable")
    }
    System.setProperty("compose.layers.type", "ON_SAME_CANVAS")
    SimulatedGameHost.reset()
    application {
        Window(
            onCloseRequest = ::exitApplication,
            title = "Tabula shell — synthetic account: ${accountScenario.option} — simulated game — $width×$height dp",
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
            PhoneViewport(width, height, fontScale) {
                TabulaApp(
                    gameHost = SimulatedGameHost(), games = previewGames, catalog = catalog,
                    scheme = scheme, deviceFacts = facts, account = accountFixture.port, accountAvatar = accountAvatar,
                )
            }
        }
    }
    accountFixture.close()
}
