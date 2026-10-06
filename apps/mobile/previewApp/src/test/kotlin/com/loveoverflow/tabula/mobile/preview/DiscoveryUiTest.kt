package com.loveoverflow.tabula.mobile.preview

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.DesktopComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState
import com.loveoverflow.tabula.mobile.catalog.DiscoveryGame
import com.loveoverflow.tabula.mobile.catalog.RegistryDiscoveryCatalog
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Genuine shared CMP discovery pixels/interactions; no browser or native gameplay is simulated by these claims. */
@OptIn(ExperimentalTestApi::class)
class DiscoveryUiTest {
    private data class Viewport(val width: Int, val scheme: TabulaScheme, val language: String) {
        val name get() = "$width-${scheme.name.lowercase()}-$language"
    }
    private val viewports = listOf(
        Viewport(320, TabulaScheme.Light, "en"),
        Viewport(390, TabulaScheme.Light, "en"),
        Viewport(390, TabulaScheme.Dark, "vi"),
        Viewport(768, TabulaScheme.Light, "vi"),
        Viewport(768, TabulaScheme.Dark, "en"),
    )

    @BeforeTest fun reset() = SimulatedGameHost.reset()
    @AfterTest fun cleanup() = SimulatedGameHost.reset()

    private fun DesktopComposeUiTest.assertTarget(tag: String, width: Int, height: Int = 844) {
        val node = onNodeWithTag(tag).performScrollTo().assertIsDisplayed()
        val bounds = node.getUnclippedBoundsInRoot()
        assertTrue(bounds.right - bounds.left >= 44.dp && bounds.bottom - bounds.top >= 44.dp, "$tag retains a 44 dp target: $bounds")
        assertTrue(bounds.left >= 0.dp && bounds.right <= width.dp && bounds.top >= 0.dp && bounds.bottom <= height.dp,
            "$tag fits the viewport: $bounds")
    }

    @Test
    fun registryHomeLibraryAndDetailRenderAcrossPhoneTabletThemeAndLocalePartitions() {
        assertTrue(RegistryDiscoveryCatalog.games.isNotEmpty(), "the generated registry fixture selection must be nonempty")
        for (case in viewports) {
            runDesktopComposeUiTest(width = case.width, height = 844) {
                setContent {
                    PhoneViewport(case.width, 844) {
                        TabulaApp(scheme = case.scheme, deviceFacts = DeviceFacts(false, case.language))
                    }
                }
                waitForIdle()
                onNodeWithTag("shell-home").assertIsDisplayed()
                onAllNodesWithTag("discovery-resume-unavailable").assertCountEquals(0)
                assertShellTextFitsHorizontally()
                captureShell("discovery-${case.name}-registry-home")
                onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
                onNodeWithTag("discovery-search").assertIsDisplayed()
                assertShellTextFitsHorizontally()
                captureShell("discovery-${case.name}-registry-library")
                val id = RegistryDiscoveryCatalog.games.first().id
                assertTarget("shell-details-$id", case.width)
                onNodeWithTag("shell-details-$id").performClick(); waitForIdle()
                onNodeWithTag("shell-detail").assertIsDisplayed()
                assertShellTextFitsHorizontally()
                captureShell("discovery-${case.name}-registry-detail")
                onNodeWithTag("shell-setup-action").performScrollTo().performClick(); waitForIdle()
                onNodeWithTag("shell-setup").assertIsDisplayed()
                onNodeWithTag("shell-start-local").performScrollTo().assertIsNotEnabled()
                onNodeWithTag("shell-native-unavailable").assertIsDisplayed()
                assertEquals(0, SimulatedGameHost.createdCount, "registry metadata grants no native launch authority")
                assertShellTextFitsHorizontally()
                captureShell("discovery-${case.name}-native-unavailable-setup")
            }
        }
    }

    @Test
    fun libraryDistinguishesZeroOneAndManyCatalogsAtSmallAndWideWidths() {
        for ((width, count) in listOf(320 to 0, 390 to 1, 768 to 8)) {
            val games = longPreviewGames(count)
            runDesktopComposeUiTest(width = width, height = 844) {
                setContent {
                    PhoneViewport(width, 844) {
                        TabulaApp(
                            catalog = DiscoveryCatalogState.Ready(previewCatalogGames(games)),
                            scheme = if (width == 390) TabulaScheme.Dark else TabulaScheme.Light,
                            deviceFacts = DeviceFacts(false, if (width == 390) "vi" else "en"),
                        )
                    }
                }
                onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
                if (count == 0) {
                    onNodeWithTag("discovery-empty").performScrollTo().assertIsDisplayed()
                    onAllNodesWithTag("discovery-no-results").assertCountEquals(0)
                } else {
                    for (game in games) onAllNodesWithTag("shell-details-${game.id}").assertCountEquals(1)
                    assertTarget("shell-details-${games.last().id}", width)
                }
                assertShellTextFitsHorizontally()
                captureShell("discovery-library-$width-count$count-long-names")
                assertEquals(0, SimulatedGameHost.createdCount)
            }
        }
    }

    @Test
    fun longEnglishAndVietnameseTitlesAtTwoHundredPercentTextScrollThroughLibraryAndDetail() {
        for ((width, language, scheme) in listOf(Triple(320, "vi", TabulaScheme.Dark), Triple(390, "en", TabulaScheme.Light))) {
            val games = longPreviewGames()
            runDesktopComposeUiTest(width = width, height = 844) {
                setContent {
                    PhoneViewport(width, 844, fontScale = 2f) {
                        TabulaApp(
                            catalog = DiscoveryCatalogState.Ready(previewCatalogGames(games)),
                            scheme = scheme,
                            deviceFacts = DeviceFacts(true, language),
                        )
                    }
                }
                waitForIdle()
                assertShellTextFitsHorizontally()
                captureShell("discovery-$width-${scheme.name.lowercase()}-$language-font200-home")
                onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
                assertTarget("shell-details-${games.last().id}", width)
                assertShellTextFitsHorizontally()
                val scroll = onNodeWithTag("shell-content-scroll").fetchSemanticsNode()
                    .config[SemanticsProperties.VerticalScrollAxisRange]
                assertTrue(scroll.maxValue() > 0f && scroll.value() > 0f, "the final long title is reached by actual vertical scrolling")
                captureShell("discovery-$width-${scheme.name.lowercase()}-$language-font200-library-scrolled")
                onNodeWithTag("shell-details-${games.last().id}").performClick(); waitForIdle()
                assertShellTextFitsHorizontally()
                captureShell("discovery-$width-${scheme.name.lowercase()}-$language-font200-detail")
                assertTarget("shell-setup-action", width)
                assertEquals(0, SimulatedGameHost.createdCount)
            }
        }
    }

    private fun searchFixture(): List<DiscoveryGame> = previewCatalogGames(listOf(
        BundledGame("com.example.alpha", "/play/local/", "", mapOf("en" to "Alpha strategy", "vi" to "Chiến thuật Alpha")),
        BundledGame("com.example.beta", "/play/local/", "", mapOf("en" to "Beta family", "vi" to "Gia đình Beta")),
    )).mapIndexed { index, game ->
        if (index == 0) game else game.copy(categories = listOf("family"),
            categoryNames = mapOf("family" to mapOf("en" to "Family", "vi" to "Gia đình")), players = listOf(3, 4))
    }

    @Test
    fun searchCategoryNoResultsClearAndBackRetainTheCurrentCatalogConstraints() = runDesktopComposeUiTest(width = 390, height = 844) {
        val games = searchFixture()
        setContent {
            PhoneViewport(390, 844) {
                TabulaApp(catalog = DiscoveryCatalogState.Ready(games), deviceFacts = DeviceFacts(false, "en"))
            }
        }
        onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
        onNodeWithTag("discovery-filter-toggle").performScrollTo().performClick(); waitForIdle()
        assertTarget("discovery-filter-category-abstract", 390)
        onNodeWithTag("discovery-filter-category-abstract").performClick()
        onNodeWithTag("discovery-search").performScrollTo().performTextReplacement("Alpha"); waitForIdle()
        onNodeWithTag("discovery-filter-category-abstract")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
        onAllNodesWithTag("shell-details-${games.first().id}").assertCountEquals(1)
        onAllNodesWithTag("shell-details-${games.last().id}").assertCountEquals(0)
        assertTarget("shell-details-${games.first().id}", 390)
        captureShell("discovery-390-light-en-filtered-alpha")
        onNodeWithTag("shell-details-${games.first().id}").performClick(); waitForIdle()
        onNodeWithTag("shell-detail").assertIsDisplayed()
        onNodeWithTag("shell-back").performClick(); waitForIdle()
        onNodeWithTag("discovery-search").performScrollTo()
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, androidx.compose.ui.text.AnnotatedString("Alpha")))
        onNodeWithTag("discovery-filter-category-abstract")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
        onNodeWithTag("discovery-search").performTextReplacement("no such game"); waitForIdle()
        onNodeWithTag("discovery-no-results").performScrollTo().assertIsDisplayed()
        onAllNodesWithTag("discovery-empty").assertCountEquals(0)
        assertShellTextFitsHorizontally()
        captureShell("discovery-390-light-en-no-results")
        assertTarget("discovery-clear-search", 390)
        onNodeWithTag("discovery-clear-search").performClick(); waitForIdle()
        onAllNodesWithTag("shell-details-${games.first().id}").assertCountEquals(1)
        onAllNodesWithTag("shell-details-${games.last().id}").assertCountEquals(0)
        onNodeWithTag("discovery-reset-filters").performScrollTo().performClick(); waitForIdle()
        for (game in games) onAllNodesWithTag("shell-details-${game.id}").assertCountEquals(1)
        assertEquals(0, SimulatedGameHost.createdCount, "query changes and Back do not launch a runtime")
    }

    @Test
    fun loadingUnavailableAndFailureAreDistinctAndRetryHasARealCaller() = runDesktopComposeUiTest(width = 320, height = 844) {
        val state = mutableStateOf<DiscoveryCatalogState>(DiscoveryCatalogState.Loading)
        var retries = 0
        setContent {
            PhoneViewport(320, 844) {
                TabulaApp(catalog = state.value, deviceFacts = DeviceFacts(false, "vi"), onRetryCatalog = {
                    retries++
                    state.value = previewCatalog
                })
            }
        }
        onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
        onNodeWithTag("discovery-catalog-loading").performScrollTo().assertIsDisplayed()
        captureShell("discovery-320-light-vi-loading")
        state.value = DiscoveryCatalogState.Unavailable; waitForIdle()
        onNodeWithTag("shell-catalog-unavailable").performScrollTo().assertIsDisplayed()
        captureShell("discovery-320-light-vi-unavailable")
        state.value = DiscoveryCatalogState.Error("synthetic upstream failure"); waitForIdle()
        onNodeWithTag("discovery-catalog-error").performScrollTo().assertIsDisplayed()
        onAllNodesWithText("synthetic upstream failure").assertCountEquals(0)
        assertShellTextFitsHorizontally()
        captureShell("discovery-320-light-vi-error")
        onNodeWithTag("discovery-catalog-retry").performScrollTo().performClick(); waitForIdle()
        assertEquals(1, retries)
        onNodeWithTag("shell-details-${previewGames.single().id}").performScrollTo().assertIsDisplayed()
        onNodeWithTag("shell-details-${previewGames.single().id}").performClick(); waitForIdle()
        state.value = DiscoveryCatalogState.Loading; waitForIdle()
        onNodeWithTag("discovery-catalog-loading").performScrollTo().assertIsDisplayed()
        onAllNodesWithTag("discovery-game-not-found").assertCountEquals(0)
        state.value = DiscoveryCatalogState.Error(); waitForIdle()
        onNodeWithTag("discovery-catalog-error").performScrollTo().assertIsDisplayed()
        onNodeWithTag("discovery-catalog-retry").performScrollTo().performClick(); waitForIdle()
        assertEquals(2, retries, "detail uses the same real discovery adapter retry")
        onNodeWithTag("shell-detail").assertIsDisplayed()
        assertEquals(0, SimulatedGameHost.createdCount)
    }
}
