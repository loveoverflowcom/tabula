package com.loveoverflow.tabula.mobile.preview

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
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.text.AnnotatedString
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.catalog.RegistryDiscoveryCatalog
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import java.io.File
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Deterministic semantic regression for the interactive MCP route (ADR-0045, I-9/I-10).
 * This runs the existing shared shell and registry catalog; it proves neither Hot Reload nor
 * an MCP connection. Native gameplay and account authority retain their unavailable defaults.
 */
@OptIn(ExperimentalTestApi::class)
class AgenticSemanticSmokeTest {
    @BeforeTest fun reset() = SimulatedGameHost.reset()
    @AfterTest fun cleanup() = SimulatedGameHost.reset()

    @Test fun englishDiscoveryLoopUsesSemanticActions() = discoveryLoop("en")
    @Test fun vietnameseDiscoveryLoopRetainsTheSameSemanticIds() = discoveryLoop("vi")

    private fun discoveryLoop(language: String) = runDesktopComposeUiTest(width = 320, height = 640) {
        val games = RegistryDiscoveryCatalog.games
        assertTrue(games.size > 1, "the registry selection exercises filtering and a trailing scroll target")
        val game = games.first()
        val query = game.displayName(language)
        setContent {
            PhoneViewport(320, 640) {
                TabulaApp(deviceFacts = DeviceFacts(reducedMotion = true, languageTag = language))
            }
        }
        waitForIdle()
        unique("shell-home").assertIsDisplayed()
        unique("shell-nav-home").assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
        onAllNodesWithTag("shell-back").assertCountEquals(0)
        snapshot(language, "home")

        click("shell-nav-games")
        unique("shell-games").assertIsDisplayed()
        val search = unique("discovery-search").assert(SemanticsMatcher.keyIsDefined(SemanticsActions.SetText))
        search.performScrollTo().performTextInput(query)
        waitForIdle()
        search.assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString(query)))
        unique("shell-details-${game.id}").performScrollTo().assertIsDisplayed()
        for (other in games.drop(1)) onAllNodesWithTag("shell-details-${other.id}").assertCountEquals(0)
        snapshot(language, "library-filtered")

        click("shell-details-${game.id}")
        unique("shell-detail").assertIsDisplayed()
        unique("discovery-native-unavailable").performScrollTo().assertIsDisplayed()
        unique("shell-setup-action").performScrollTo().assertIsDisplayed()
        assertScrolled()
        snapshot(language, "detail")

        click("shell-setup-action")
        unique("shell-setup").assertIsDisplayed()
        unique("shell-start-local").performScrollTo().assertIsNotEnabled()
        unique("shell-native-unavailable").assertIsDisplayed()
        onAllNodesWithTag("sim-page").assertCountEquals(0)
        assertEquals(0, SimulatedGameHost.createdCount, "catalog browsing must not mount a simulated or native host")
        snapshot(language, "setup-unavailable")

        click("shell-back")
        unique("shell-detail").assertIsDisplayed()
        onAllNodesWithTag("shell-setup").assertCountEquals(0)
        click("shell-back")
        unique("shell-games").assertIsDisplayed()
        unique("discovery-search").performScrollTo()
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString(query)))
        snapshot(language, "library-after-back")

        unique("discovery-clear-search").performScrollTo()
        click("discovery-clear-search")
        unique("discovery-search")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString("")))
        for (entry in games) unique("shell-details-${entry.id}")
        unique("shell-details-${games.last().id}").performScrollTo().assertIsDisplayed()
        assertScrolled()
        snapshot(language, "library-scrolled")

        click("shell-nav-home")
        unique("shell-home").assertIsDisplayed()
        onAllNodesWithTag("shell-back").assertCountEquals(0)
        assertEquals(0, SimulatedGameHost.createdCount)
    }

    /** Each reusable ID names exactly one node in the unmerged tree exposed to automation. */
    private fun DesktopComposeUiTest.unique(tag: String): SemanticsNodeInteraction {
        onAllNodesWithTag(tag, useUnmergedTree = true).assertCountEquals(1)
        return onNodeWithTag(tag)
    }

    private fun DesktopComposeUiTest.click(tag: String) {
        unique(tag).assert(SemanticsMatcher.keyIsDefined(SemanticsActions.OnClick)).performClick()
        waitForIdle()
    }

    private fun DesktopComposeUiTest.assertScrolled() {
        val node = unique("shell-content-scroll").assert(SemanticsMatcher.keyIsDefined(SemanticsActions.ScrollBy))
        val range = node.fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue(range.maxValue() > 0f && range.value() > 0f, "a semantic scroll must reach content beyond the viewport")
    }

    private fun DesktopComposeUiTest.snapshot(language: String, stage: String) {
        System.getProperty("tabula.preview.screenshots")?.let { output ->
            val directory = File(output, "agentic-semantics").also { it.mkdirs() }
            File(directory, "$language-$stage.txt").writeText(onRoot(useUnmergedTree = true).printToString())
        }
        captureShell("agentic-$language-$stage")
    }
}
