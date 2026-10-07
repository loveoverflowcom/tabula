package com.loveoverflow.tabula.mobile.preview

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.LocalSaveableStateRegistry
import androidx.compose.runtime.saveable.SaveableStateRegistry
import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.DesktopComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.text.TextLayoutResult
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.bridge.HostMessage
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.design.TabulaAccessibility
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import java.io.File
import javax.imageio.ImageIO
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Saves the actual shared-shell pixels alongside existing shell preview captures. */
@OptIn(ExperimentalTestApi::class)
internal fun DesktopComposeUiTest.captureShell(name: String) {
    val path = System.getProperty("tabula.preview.screenshots") ?: return
    val directory = File(path).also { it.mkdirs() }
    ImageIO.write(onRoot().captureToImage().toAwtImage(), "png", File(directory, "$name.png"))
}

/**
 * Checks measured, unclipped text width and its layout result, including offscreen scroll content.
 * A clipped viewport alone cannot pass this check: the text's original x/width must fit, and text
 * may not silently overflow or ellipsize horizontally. The check rejects an empty semantics tree.
 */
@OptIn(ExperimentalTestApi::class)
internal fun DesktopComposeUiTest.assertShellTextFitsHorizontally() {
    val viewport = onNodeWithTag("phone-viewport").fetchSemanticsNode()
    val left = viewport.positionInRoot.x
    val right = left + viewport.size.width
    val textNodes = onAllNodes(
        SemanticsMatcher.keyIsDefined(SemanticsActions.GetTextLayoutResult),
        useUnmergedTree = true,
    ).fetchSemanticsNodes()
    assertTrue(textNodes.isNotEmpty(), "the shell must expose text layout evidence")
    for (node in textNodes) {
        val text = node.config.getOrElse(SemanticsProperties.Text) { emptyList() }.joinToString()
        assertTrue(node.positionInRoot.x >= left - 1f, "text starts beyond the viewport: $text")
        assertTrue(node.positionInRoot.x + node.size.width <= right + 1f, "text extends beyond the viewport: $text")
        val layouts = mutableListOf<TextLayoutResult>()
        assertTrue(node.config[SemanticsActions.GetTextLayoutResult].action?.invoke(layouts) == true,
            "text layout callback must execute: $text")
        assertTrue(layouts.isNotEmpty(), "text layout callback must return an actual layout: $text")
        for (layout in layouts) {
            // BasicText can shrink to its glyph width after paragraph layout used the offered
            // max width. didOverflowWidth compares those different widths; actual glyph edges
            // are the oracle here. One pixel permits integer layout rounding, not clipped words.
            assertTrue(layout.lineCount > 0, "the layout must contain an actual text line: $text")
            for (line in 0 until layout.lineCount) {
                assertTrue(layout.getLineLeft(line) >= -1f, "text glyphs extend left of their node: $text")
                assertTrue(layout.getLineRight(line) <= layout.size.width + 1f,
                    "text glyphs extend beyond their node: $text; lineRight=${layout.getLineRight(line)}, width=${layout.size.width}")
                assertFalse(layout.isLineEllipsized(line), "long shell text must wrap rather than lose glyphs: $text")
            }
        }
    }
}

/**
 * Exercises shared shell routes, localization, responsive layout and simulated host handoff.
 * Theme/locale pairs cover each supported dimension without implying a full Cartesian matrix.
 * The host is labelled simulation evidence; none of these interactions execute a device WebView.
 */
@OptIn(ExperimentalTestApi::class)
class ShellNavigationTest {
    private data class PreviewCase(val width: Int, val scheme: TabulaScheme, val language: String) {
        val name get() = "$width-${scheme.name.lowercase()}-$language"
    }
    private val cases = listOf(
        PreviewCase(320, TabulaScheme.Light, "en"),
        PreviewCase(390, TabulaScheme.Dark, "vi"),
        PreviewCase(768, TabulaScheme.Light, "vi"),
        PreviewCase(768, TabulaScheme.Dark, "en"),
    )

    @BeforeTest fun reset() = SimulatedGameHost.reset()
    @AfterTest fun cleanup() = SimulatedGameHost.reset()

    /** Short navigation/Back labels may wrap between words; breaking a word harms 200% readability. */
    private fun DesktopComposeUiTest.assertShortLabelWrapsAtWordBoundaries(tag: String) {
        val nodes = mutableListOf<SemanticsNode>()
        fun collect(node: SemanticsNode) {
            if (node.config.contains(SemanticsActions.GetTextLayoutResult)) nodes += node
            node.children.forEach(::collect)
        }
        collect(onNodeWithTag(tag, useUnmergedTree = true).fetchSemanticsNode())
        assertTrue(nodes.isNotEmpty(), "the action $tag must expose a real label layout")
        for (node in nodes) {
            val layouts = mutableListOf<TextLayoutResult>()
            assertTrue(node.config[SemanticsActions.GetTextLayoutResult].action?.invoke(layouts) == true)
            assertTrue(layouts.isNotEmpty())
            for (layout in layouts) {
                val label = layout.layoutInput.text.text
                for (line in 0 until layout.lineCount - 1) {
                    val end = layout.getLineEnd(line, visibleEnd = true)
                    val nextStart = layout.getLineStart(line + 1)
                    val wrapsBetweenWords = label.substring(end, nextStart).any { it.isWhitespace() } ||
                        label.getOrNull(end)?.isWhitespace() == true
                    assertTrue(wrapsBetweenWords,
                        "short label must retain whole words: $tag, label=$label, line=$line, end=$end, nextStart=$nextStart")
                }
            }
        }
    }

    private fun DesktopComposeUiTest.assertSelected(destination: String) {
        val viewport = onNodeWithTag("phone-viewport").fetchSemanticsNode()
        val pixelsPerDp = viewport.size.width.toFloat() / currentWidth
        for (name in listOf("home", "games", "account")) {
            val node = onNodeWithTag("shell-nav-$name")
                .assertIsDisplayed()
                .assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, name == destination))
                .fetchSemanticsNode()
            assertTrue(node.size.height / pixelsPerDp >= TabulaAccessibility.minTarget - 0.5f,
                "navigation target $name must remain at least ${TabulaAccessibility.minTarget}dp tall")
            assertTrue(node.size.width / pixelsPerDp >= TabulaAccessibility.minTarget - 0.5f,
                "navigation target $name must remain at least ${TabulaAccessibility.minTarget}dp wide")
            assertShortLabelWrapsAtWordBoundaries("shell-nav-$name")
        }
    }
    private var currentWidth = 390

    @Test
    fun fullShellRoutesAndLocalLaunchReturnAcrossPhoneTabletThemesAndLanguages() {
        for (case in cases) {
            SimulatedGameHost.reset()
            currentWidth = case.width
            val strings = ShellStrings.forLanguage(case.language)
            runDesktopComposeUiTest(width = case.width, height = 844) {
                setContent {
                    PhoneViewport(case.width, 844) {
                        TabulaApp(
                            gameHost = SimulatedGameHost(autoBootMillis = null),
                            games = previewGames,
                            catalog = previewCatalog,
                            scheme = case.scheme,
                            deviceFacts = DeviceFacts(false, case.language),
                        )
                    }
                }
                waitForIdle()
                onNodeWithTag("shell-home").assertIsDisplayed()
                assertSelected("home")
                assertShellTextFitsHorizontally()
                captureShell("parity-${case.name}-home")

                onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
                onNodeWithTag("shell-games").assertIsDisplayed()
                assertSelected("games")
                onNodeWithTag("discovery-results-count").performScrollTo().assertIsDisplayed()
                assertEquals(0, SimulatedGameHost.createdCount, "browsing does not mount gameplay")
                assertShellTextFitsHorizontally()
                captureShell("parity-${case.name}-games")

                onNodeWithTag("shell-details-${previewGames.first().id}").performScrollTo().performClick()
                waitForIdle()
                onNodeWithTag("shell-detail").assertIsDisplayed()
                assertSelected("games")
                assertShellTextFitsHorizontally()
                captureShell("parity-${case.name}-detail")

                onNodeWithTag("shell-setup-action").performScrollTo().performClick(); waitForIdle()
                onNodeWithTag("shell-setup").assertIsDisplayed()
                assertSelected("games")
                onNodeWithTag("shell-online-unavailable").performScrollTo().assertIsDisplayed()
                assertEquals(0, SimulatedGameHost.createdCount, "setup itself does not mount gameplay")
                assertShellTextFitsHorizontally()
                captureShell("parity-${case.name}-setup")

                onNodeWithTag("shell-start-local").performScrollTo().performClick(); waitForIdle()
                val runtime = SimulatedGameHost.runtimes.single()
                val init = runtime.page.received.first() as HostMessage.Init
                assertEquals(if (case.scheme == TabulaScheme.Dark) ThemePreference.Dark else ThemePreference.Light,
                    init.preferences.theme)
                assertEquals(if (case.language == "vi") LocalePreference.Vietnamese else LocalePreference.English,
                    init.preferences.locale)
                runtime.page.completeBoot(); waitForIdle()
                onNodeWithText(strings[ShellCopy.Back]).performClick(); waitForIdle()
                onNodeWithTag("sim-leave-dialog").assertIsDisplayed()
                assertEquals(0, SimulatedGameHost.disposedCount, "the game handles confirmation before shell return")
                onNodeWithTag("sim-confirm").performClick(); waitForIdle()
                onNodeWithTag("shell-setup").assertIsDisplayed()
                assertSelected("games")
                assertEquals(1, SimulatedGameHost.disposedCount, "confirmed return releases the runtime")

                onNodeWithTag("shell-back").performClick(); waitForIdle()
                onNodeWithTag("shell-detail").assertIsDisplayed()
                onNodeWithTag("shell-back").performClick(); waitForIdle()
                onNodeWithTag("shell-games").assertIsDisplayed()
                onNodeWithTag("shell-nav-account").performClick(); waitForIdle()
                onNodeWithTag("shell-account").assertIsDisplayed()
                onNodeWithTag("shell-account-unavailable").assertIsDisplayed()
                assertSelected("account")
                assertShellTextFitsHorizontally()
                captureShell("parity-${case.name}-account")
                assertEquals(1, SimulatedGameHost.createdCount, "gated account navigation creates no additional game")
                onNodeWithTag("shell-nav-home").performClick(); waitForIdle()
                onNodeWithTag("shell-home").assertIsDisplayed()
                assertSelected("home")
            }
        }
    }

    /** Compose saveable restoration only; this does not simulate OS process death or resume a game. */
    @Test
    fun saveableRestorationKeepsSetupAndReturnsALiveGameToSetupWithoutStartingAnotherRuntime() =
        runDesktopComposeUiTest(width = 390, height = 844) {
            currentWidth = 390
            // Desktop's StateRestorationTester platform encoder is not implemented in Skiko.
            // Exercise actual Compose registry save/dispose/restore with its standard saver values.
            fun shellValueCanBeSaved(value: Any?): Boolean = when (value) {
                null, is String, is Int, is Long, is Float, is Double, is Boolean -> true
                is List<*> -> value.all(::shellValueCanBeSaved)
                is Map<*, *> -> value.all { (key, item) -> shellValueCanBeSaved(key) && shellValueCanBeSaved(item) }
                is MutableState<*> -> shellValueCanBeSaved(value.value)
                else -> false
            }
            var registry = SaveableStateRegistry(restoredValues = null, canBeSaved = ::shellValueCanBeSaved)
            val mounted = mutableStateOf(true)
            fun restoreShell() {
                val saved = registry.performSave()
                assertTrue(saved.isNotEmpty(), "the mounted app must register a saveable history")
                mounted.value = false
                waitForIdle()
                registry = SaveableStateRegistry(restoredValues = saved, canBeSaved = ::shellValueCanBeSaved)
                mounted.value = true
                waitForIdle()
            }
            setContent {
                if (mounted.value) CompositionLocalProvider(LocalSaveableStateRegistry provides registry) {
                    PhoneViewport(390, 844) {
                        TabulaApp(
                            gameHost = SimulatedGameHost(autoBootMillis = null),
                            games = previewGames,
                            catalog = previewCatalog,
                            scheme = TabulaScheme.Light,
                            deviceFacts = DeviceFacts(false, "en"),
                        )
                    }
                }
            }
            waitForIdle()
            onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
            onNodeWithTag("shell-details-${previewGames.first().id}").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("shell-setup-action").performScrollTo().performClick(); waitForIdle()
            restoreShell()
            onNodeWithTag("shell-setup").assertIsDisplayed()
            assertSelected("games")
            assertEquals(0, SimulatedGameHost.createdCount, "restoring setup has no gameplay effect")

            onNodeWithTag("shell-start-local").performScrollTo().performClick(); waitForIdle()
            val old = SimulatedGameHost.runtimes.single()
            old.page.completeBoot(); waitForIdle()
            restoreShell()
            onNodeWithTag("shell-setup").assertIsDisplayed()
            assertSelected("games")
            assertEquals(1, SimulatedGameHost.createdCount, "restoring a live route cannot relaunch a local game")
            assertEquals(1, SimulatedGameHost.disposedCount, "restoration releases the old game composition")
            assertTrue(old.disposed)
            captureShell("parity-390-light-en-restored-setup")
            onNodeWithTag("shell-back").performClick(); waitForIdle()
            onNodeWithTag("shell-detail").assertIsDisplayed()
        }

    @Test
    fun largeVietnameseTextAndLongCatalogNamesWrapAndRemainReachableByVerticalScrolling() {
        currentWidth = 320
        val strings = ShellStrings.forLanguage("vi")
        val games = longPreviewGames()
        runDesktopComposeUiTest(width = 320, height = 844) {
            setContent {
                PhoneViewport(320, 844, fontScale = 2f) {
                    TabulaApp(
                        gameHost = SimulatedGameHost(autoBootMillis = null),
                        games = games,
                        catalog = com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState.Ready(previewCatalogGames(games)),
                        scheme = TabulaScheme.Dark,
                        deviceFacts = DeviceFacts(true, "vi"),
                    )
                }
            }
            waitForIdle()
            assertSelected("home")
            assertShellTextFitsHorizontally()
            captureShell("parity-320-dark-vi-font200-home")
            val lastPlay = onNodeWithTag("shell-details-${games.last().id}")
            lastPlay.performScrollTo().assertIsDisplayed()
            onNodeWithText(games.last().displayName("vi")).assertIsDisplayed()
            assertShellTextFitsHorizontally()
            val scroll = onNodeWithTag("shell-content-scroll").fetchSemanticsNode()
                .config[SemanticsProperties.VerticalScrollAxisRange]
            assertTrue(scroll.maxValue() > 0f && scroll.value() > 0f,
                "the last of eight long games must be reached by actual vertical scrolling")
            captureShell("parity-320-dark-vi-font200-home-scrolled")
            assertEquals(0, SimulatedGameHost.createdCount)

            onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
            onNodeWithTag("shell-details-${games.last().id}").performScrollTo().assertIsDisplayed()
            assertShellTextFitsHorizontally()
            captureShell("parity-320-dark-vi-font200-games-scrolled")
            onNodeWithTag("shell-details-${games.last().id}").performClick(); waitForIdle()
            onNodeWithTag("shell-setup-action").performScrollTo().assertIsDisplayed()
            assertShortLabelWrapsAtWordBoundaries("shell-back")
            assertShellTextFitsHorizontally()
            onNodeWithTag("shell-setup-action").performClick(); waitForIdle()
            onNodeWithTag("shell-online-unavailable").performScrollTo().assertIsDisplayed()
            assertShortLabelWrapsAtWordBoundaries("shell-back")
            assertShellTextFitsHorizontally()
            captureShell("parity-320-dark-vi-font200-setup")
            onNodeWithTag("shell-start-local").performScrollTo().assertIsDisplayed().performClick(); waitForIdle()
            assertEquals(1, SimulatedGameHost.createdCount, "the local action remains executable after scrolling")
            assertShortLabelWrapsAtWordBoundaries("shell-back")
            onNodeWithText(strings[ShellCopy.Back]).performClick(); waitForIdle()
            onNodeWithTag("shell-setup").assertIsDisplayed()
            assertEquals(1, SimulatedGameHost.disposedCount)
        }
    }
}
