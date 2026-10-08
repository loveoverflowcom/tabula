package com.loveoverflow.tabula.mobile.preview

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.DesktopComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState
import com.loveoverflow.tabula.mobile.catalog.DiscoveryGame
import com.loveoverflow.tabula.mobile.catalog.RegistryDiscoveryCatalog
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.localization.DiscoveryCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.discovery
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlin.math.abs

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
        Viewport(390, TabulaScheme.HcLight, "vi"),
        Viewport(390, TabulaScheme.HcDark, "en"),
        Viewport(768, TabulaScheme.Light, "vi"),
        Viewport(768, TabulaScheme.Dark, "en"),
    )

    @BeforeTest fun reset() = SimulatedGameHost.reset()
    @AfterTest fun cleanup() = SimulatedGameHost.reset()

    private fun DesktopComposeUiTest.reach(tag: String): SemanticsNodeInteraction {
        val node = onNodeWithTag(tag)
        // The sheet's header/footer are fixed; only controls in its body have a scroll owner.
        val ancestors = generateSequence(node.fetchSemanticsNode().parent) { it.parent }
        if (ancestors.any { it.config.contains(SemanticsActions.ScrollBy) }) node.performScrollTo()
        return node.assertIsDisplayed()
    }

    private fun DesktopComposeUiTest.assertTarget(tag: String, width: Int, height: Int = 844) {
        val node = reach(tag)
        val bounds = node.getUnclippedBoundsInRoot()
        assertTrue(bounds.right - bounds.left >= 44.dp && bounds.bottom - bounds.top >= 44.dp, "$tag retains a 44 dp target: $bounds")
        assertTrue(bounds.left >= 0.dp && bounds.right <= width.dp, "$tag retains its complete horizontal target: $bounds")
        // At 200% text a whole clickable card may be taller than the viewport. Its text still
        // wraps and scrolls; require a usable visible action area rather than shrinking its copy.
        val visible = if (tag.startsWith("shell-details-")) node.getBoundsInRoot() else bounds
        assertTrue(visible.right - visible.left >= 44.dp && visible.bottom - visible.top >= 44.dp &&
            visible.left >= 0.dp && visible.right <= width.dp && visible.top >= 0.dp && visible.bottom <= height.dp,
            "$tag exposes a complete visible action area inside the viewport: $visible (unclipped=$bounds)")
    }

    private fun DesktopComposeUiTest.detailNode(tag: String): SemanticsNodeInteraction =
        onNode(hasTestTag(tag) and hasAnyAncestor(hasTestTag("shell-detail")))

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
                val hero = onNodeWithTag("discovery-hero-art").assertIsDisplayed()
                val artBounds = hero.getUnclippedBoundsInRoot()
                val artRatio = (artBounds.right - artBounds.left).value / (artBounds.bottom - artBounds.top).value
                assertTrue(abs(artRatio - 43f / 24f) < 0.02f, "the imported scene keeps its aspect ratio: $artBounds")
                hero.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.ContentDescription))
                onAllNodesWithTag("discovery-resume-unavailable").assertCountEquals(0)
                if (case.width == 390) {
                    // Check the initial viewport, before any performScrollTo can hide the density regression.
                    val content = onNodeWithTag("shell-content-scroll").getUnclippedBoundsInRoot()
                    val firstCard = onNodeWithTag("discovery-card-${RegistryDiscoveryCatalog.games.first().id}")
                        .assertIsDisplayed().getUnclippedBoundsInRoot()
                    assertTrue(firstCard.top >= content.top && firstCard.bottom <= content.bottom,
                        "one complete game card is visible on Home without scrolling: $firstCard, content=$content")
                }
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
        BundledGame("com.example.alpha", "/play/local/", "", mapOf("en" to "Board Alpha strategy", "vi" to "Trò chơi chiến thuật Alpha")),
        BundledGame("com.example.beta", "/play/local/", "", mapOf("en" to "Board Beta strategy", "vi" to "Trò chơi chiến thuật Beta")),
        BundledGame("com.example.gamma", "/play/local/", "", mapOf("en" to "Board Gamma family", "vi" to "Trò chơi gia đình Gamma")),
        BundledGame("com.example.delta", "/play/local/", "", mapOf("en" to "Board Delta strategy", "vi" to "Trò chơi chiến thuật Delta")),
        BundledGame("com.example.epsilon", "/play/local/", "", mapOf("en" to "Board Epsilon strategy", "vi" to "Trò chơi chiến thuật Epsilon")),
    )).mapIndexed { index, game ->
        game.copy(
            categories = if (index == 2) listOf("family") else listOf("abstract"),
            categoryNames = mapOf("abstract" to mapOf("en" to "Abstract", "vi" to "Trừu tượng"),
                "family" to mapOf("en" to "Family", "vi" to "Gia đình")),
            players = if (index == 1) listOf(3) else listOf(2),
            minMinutes = 10,
            maxMinutes = if (index == 3) 40 else 20,
            complexity = if (index == 4) "heavy" else "light",
            complexityNames = if (index == 4) mapOf("en" to "Heavy", "vi" to "Nặng") else game.complexityNames,
        )
    }

    private fun DesktopComposeUiTest.openFilters() {
        onNodeWithTag("discovery-filter-toggle").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("discovery-filters").assertIsDisplayed()
    }

    private fun DesktopComposeUiTest.selectFilter(axis: String, value: String) {
        onNodeWithTag("discovery-filter-$axis").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("discovery-filter-$axis-$value").assertIsDisplayed().performClick()
        waitForIdle()
    }

    private fun DesktopComposeUiTest.closeFilters(action: String) {
        reach("discovery-filter-$action").performClick()
        waitForIdle()
        onAllNodesWithTag("discovery-filters").assertCountEquals(0)
    }

    private fun DesktopComposeUiTest.assertResults(games: List<DiscoveryGame>, expected: Set<String>) {
        for (game in games) onAllNodesWithTag("shell-details-${game.id}")
            .assertCountEquals(if (game.id in expected) 1 else 0)
    }

    @Test
    fun filterDraftCancelApplyAndResetPreserveSearchAndCombineEveryMetadataAxis() = runDesktopComposeUiTest(width = 390, height = 844) {
        val games = searchFixture()
        setContent {
            PhoneViewport(390, 844) {
                TabulaApp(catalog = DiscoveryCatalogState.Ready(games), deviceFacts = DeviceFacts(false, "en"))
            }
        }
        onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
        onAllNodesWithTag("discovery-filters").assertCountEquals(0)
        onAllNodesWithTag("discovery-filter-category-abstract").assertCountEquals(0)
        onNodeWithTag("discovery-search").performScrollTo().performTextReplacement("Board"); waitForIdle()
        assertResults(games, games.map { it.id }.toSet())
        openFilters()
        selectFilter("category", "family")
        selectFilter("player", "3")
        closeFilters("cancel")
        assertResults(games, games.map { it.id }.toSet())
        onNodeWithTag("discovery-search").assert(SemanticsMatcher.expectValue(
            SemanticsProperties.EditableText, AnnotatedString("Board")))

        openFilters()
        selectFilter("category", "abstract")
        selectFilter("player", "2")
        selectFilter("duration", "20")
        selectFilter("complexity", "light")
        closeFilters("apply")
        assertResults(games, setOf(games.first().id))
        onNodeWithTag("discovery-active-filters").performScrollTo().assertIsDisplayed()
        assertTarget("shell-details-${games.first().id}", 390)
        captureShell("redesign-390-light-en-combined-filters")
        onNodeWithTag("shell-details-${games.first().id}").performClick(); waitForIdle()
        onNodeWithTag("shell-detail").assertIsDisplayed()
        onNodeWithTag("shell-back").performClick(); waitForIdle()
        onNodeWithTag("discovery-search").performScrollTo()
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("Board")))
        assertResults(games, setOf(games.first().id))
        openFilters()
        reach("discovery-filter-reset").performClick(); waitForIdle()
        closeFilters("cancel")
        assertResults(games, setOf(games.first().id))
        onNodeWithTag("discovery-search").performTextReplacement("no such game"); waitForIdle()
        onNodeWithTag("discovery-no-results").performScrollTo().assertIsDisplayed()
        onAllNodesWithTag("discovery-empty").assertCountEquals(0)
        assertShellTextFitsHorizontally()
        captureShell("discovery-390-light-en-no-results")
        assertTarget("discovery-clear-search", 390)
        onNodeWithTag("discovery-clear-search").performClick(); waitForIdle()
        assertResults(games, setOf(games.first().id))
        onNodeWithTag("discovery-search").performTextReplacement("Board"); waitForIdle()
        openFilters()
        reach("discovery-filter-reset").performClick(); waitForIdle()
        closeFilters("apply")
        assertResults(games, games.map { it.id }.toSet())
        onNodeWithTag("discovery-search").assert(SemanticsMatcher.expectValue(
            SemanticsProperties.EditableText, AnnotatedString("Board")))
        assertEquals(0, SimulatedGameHost.createdCount, "query changes and Back do not launch a runtime")
    }

    @Test
    fun filterSheetControlsRemainReachableAtCompactWidthsAndTwoHundredPercentText() {
        val games = searchFixture()
        for ((width, language, fontScale) in listOf(Triple(320, "vi", 2f), Triple(390, "en", 1f), Triple(390, "vi", 2f))) {
            runDesktopComposeUiTest(width = width, height = 844) {
                setContent {
                    PhoneViewport(width, 844, fontScale) {
                        TabulaApp(catalog = DiscoveryCatalogState.Ready(games), deviceFacts = DeviceFacts(false, language))
                    }
                }
                onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
                openFilters()
                val strings = ShellStrings.forLanguage(language)
                for ((axis, label) in listOf("category" to DiscoveryCopy.Category, "player" to DiscoveryCopy.Players,
                    "duration" to DiscoveryCopy.Duration, "complexity" to DiscoveryCopy.Complexity)) {
                    assertTarget("discovery-filter-$axis", width)
                    onNodeWithTag("discovery-filter-$axis").assert(SemanticsMatcher.expectValue(
                        SemanticsProperties.ContentDescription,
                        listOf("${strings.discovery(label)}, ${strings.discovery(DiscoveryCopy.All)}")))
                }
                selectFilter("category", "family")
                selectFilter("player", "2")
                selectFilter("duration", "20")
                selectFilter("complexity", "light")
                assertShellTextFitsHorizontally()
                assertTarget("discovery-filter-reset", width)
                assertTarget("discovery-filter-cancel", width)
                assertTarget("discovery-filter-apply", width)
                captureShell("redesign-filter-sheet-${width}-$language-font${(fontScale * 100).toInt()}")
                closeFilters("apply")
                assertResults(games, setOf(games[2].id))
                assertEquals(0, SimulatedGameHost.createdCount, "metadata selection does not mount gameplay")
            }
        }
    }

    @Test
    fun generatedCatalogIconsDecodeBeforeCaptureAndPlannedEntriesOfferOnlyMetadata() = runDesktopComposeUiTest(width = 390, height = 844) {
        val games = RegistryDiscoveryCatalog.games
        assertTrue(games.isNotEmpty() && games.all { it.catalogIcon != null }, "this build declares actual art for every production entry")
        assertTrue(games.any { it.planned }, "the production catalog exercises explicit planned metadata")
        setContent {
            PhoneViewport(390, 844) { TabulaApp(scheme = TabulaScheme.Light, deviceFacts = DeviceFacts(false, "en")) }
        }
        onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
        for (game in games) {
            waitUntil(timeoutMillis = 5_000) {
                onAllNodesWithTag("discovery-icon-loaded-${game.catalogIcon}", useUnmergedTree = true)
                    .fetchSemanticsNodes().size == 1
            }
            onNodeWithTag("discovery-icon-loaded-${game.catalogIcon}", useUnmergedTree = true)
                .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.ContentDescription))
            onAllNodesWithTag("discovery-icon-fallback-${game.id}", useUnmergedTree = true).assertCountEquals(0)
        }
        captureShell("redesign-registry-list-loaded-icons-390")
        onNodeWithTag("discovery-view-grid").performClick(); waitForIdle()
        captureShell("redesign-registry-grid-loaded-icons-390")
        val planned = games.first { it.planned }
        onNodeWithTag("shell-details-${planned.id}").performScrollTo().performClick(); waitForIdle()
        onNodeWithTag("shell-detail").assertIsDisplayed()
        onAllNodesWithTag("shell-setup-action").assertCountEquals(0)
        onAllNodesWithTag("shell-start-local").assertCountEquals(0)
        captureShell("redesign-registry-planned-detail-loaded-icon-390")
        onNodeWithTag("shell-back").performClick(); waitForIdle()
        assertEquals(0, SimulatedGameHost.createdCount, "real discovery art and planned metadata grant no launch authority")
    }

    @Test
    fun listCardsOwnTheWholeActionAndGridAdaptsToNarrowWidthsAndLargeText() {
        val games = searchFixture()
        for ((width, fontScale, columns) in listOf(Triple(390, 1f, 2), Triple(320, 1f, 1), Triple(390, 2f, 1))) {
            runDesktopComposeUiTest(width = width, height = 844) {
                setContent {
                    PhoneViewport(width, 844, fontScale) {
                        TabulaApp(catalog = DiscoveryCatalogState.Ready(games), deviceFacts = DeviceFacts(false, "en"))
                    }
                }
                onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
                onNodeWithTag("discovery-view-list").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
                onNodeWithTag("discovery-view-grid").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, false))
                val first = onNodeWithTag("shell-details-${games.first().id}").performScrollTo()
                    .assert(SemanticsMatcher.keyIsDefined(SemanticsActions.OnClick))
                val firstBounds = first.getUnclippedBoundsInRoot()
                val art = onNodeWithTag("discovery-card-cover-${games.first().id}", useUnmergedTree = true).getUnclippedBoundsInRoot()
                assertEquals(72.dp, art.right - art.left, "list artwork retains the reference's 72 dp width")
                assertEquals(72.dp, art.bottom - art.top, "list artwork retains the reference's square shape")
                assertTrue(art.left >= firstBounds.left && art.right <= firstBounds.right &&
                    art.top >= firstBounds.top && art.bottom <= firstBounds.bottom,
                    "the artwork belongs to the clickable whole-card target: $art, target=$firstBounds")
                onNodeWithTag("discovery-view-grid").performScrollTo().performClick(); waitForIdle()
                onNodeWithTag("discovery-view-grid").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
                onNodeWithTag("discovery-view-list").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, false))
                // Alpha/Beta are the first two alphabetically; compare geometry without scrolling between reads.
                val alpha = onNodeWithTag("shell-details-${games[0].id}").getUnclippedBoundsInRoot()
                val beta = onNodeWithTag("shell-details-${games[1].id}").getUnclippedBoundsInRoot()
                if (columns == 2) {
                    assertEquals(alpha.top, beta.top, "390 dp normal text uses two cards in the first grid row")
                    assertTrue(beta.left >= alpha.right, "the second card occupies the next column")
                } else {
                    assertEquals(alpha.left, beta.left, "narrow or large-text grids retain the full reading width")
                    assertTrue(beta.top >= alpha.bottom, "narrow or large-text grids use one card per row")
                }
                assertTarget("shell-details-${games.last().id}", width)
                assertShellTextFitsHorizontally()
                captureShell("redesign-grid-${width}-font${(fontScale * 100).toInt()}")
                assertEquals(0, SimulatedGameHost.createdCount)
            }
        }
    }

    @Test
    fun detailDismissalRetainsTheLibraryQueryViewAndActualScrollPosition() = runDesktopComposeUiTest(width = 390, height = 844) {
        val games = longPreviewGames()
        setContent {
            PhoneViewport(390, 844) {
                TabulaApp(catalog = DiscoveryCatalogState.Ready(previewCatalogGames(games)), deviceFacts = DeviceFacts(false, "en"))
            }
        }
        onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
        onNodeWithTag("discovery-search").performTextReplacement("board game"); waitForIdle()
        onNodeWithTag("discovery-view-grid").performClick(); waitForIdle()
        onNodeWithTag("shell-details-${games.last().id}").performScrollTo().assertIsDisplayed()
        val before = onNodeWithTag("shell-content-scroll").fetchSemanticsNode()
            .config[SemanticsProperties.VerticalScrollAxisRange].value()
        assertTrue(before > 0f, "the detail opens after a real library scroll")
        onNodeWithTag("shell-details-${games.last().id}").performClick(); waitForIdle()
        onNodeWithTag("shell-detail").assertIsDisplayed()
        onNodeWithTag("shell-setup-action").performScrollTo().assertIsDisplayed()
        captureShell("redesign-detail-linked-390")
        onNodeWithTag("shell-back").performClick(); waitForIdle()
        onAllNodesWithTag("shell-detail").assertCountEquals(0)
        onNodeWithTag("discovery-search").assert(SemanticsMatcher.expectValue(
            SemanticsProperties.EditableText, AnnotatedString("board game")))
        onNodeWithTag("discovery-view-grid").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
        val after = onNodeWithTag("shell-content-scroll").fetchSemanticsNode()
            .config[SemanticsProperties.VerticalScrollAxisRange].value()
        assertTrue(abs(before - after) <= 1f, "modal detail preserves the existing library scroll: $before → $after")
        onNodeWithTag("shell-details-${games.last().id}").assertIsDisplayed()
        assertEquals(0, SimulatedGameHost.createdCount, "opening and dismissing detail grants no native runtime authority")
    }

    @Test
    fun plannedCatalogDetailNeverOffersSetupEvenWithAMatchingPreviewRuntime() = runDesktopComposeUiTest(width = 390, height = 844) {
        val game = previewCatalogGames(previewGames).single().copy(planned = true)
        setContent {
            PhoneViewport(390, 844) {
                TabulaApp(catalog = DiscoveryCatalogState.Ready(listOf(game)), games = previewGames,
                    gameHost = SimulatedGameHost(autoBootMillis = null), deviceFacts = DeviceFacts(false, "en"))
            }
        }
        onNodeWithTag("shell-nav-games").performClick(); waitForIdle()
        onNodeWithTag("shell-details-${game.id}").performScrollTo().performClick(); waitForIdle()
        onNodeWithTag("shell-detail").assertIsDisplayed()
        onAllNodesWithTag("shell-setup-action").assertCountEquals(0)
        onAllNodesWithTag("shell-start-local").assertCountEquals(0)
        onAllNodesWithTag("sim-page").assertCountEquals(0)
        assertShellTextFitsHorizontally()
        captureShell("redesign-detail-planned-390")
        onNodeWithTag("shell-back").performClick(); waitForIdle()
        onNodeWithTag("shell-games").assertIsDisplayed()
        assertEquals(0, SimulatedGameHost.createdCount, "a planned entry remains metadata even when a test host has the same opaque id")
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
        detailNode("discovery-catalog-loading").performScrollTo().assertIsDisplayed()
        onAllNodesWithTag("discovery-game-not-found").assertCountEquals(0)
        state.value = DiscoveryCatalogState.Error(); waitForIdle()
        detailNode("discovery-catalog-error").performScrollTo().assertIsDisplayed()
        detailNode("discovery-catalog-retry").performScrollTo().performClick(); waitForIdle()
        assertEquals(2, retries, "detail uses the same real discovery adapter retry")
        onNodeWithTag("shell-detail").assertIsDisplayed()
        assertEquals(0, SimulatedGameHost.createdCount)
    }
}
