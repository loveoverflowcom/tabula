package com.loveoverflow.tabula.mobile.preview

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.DesktopComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/**
 * Measured responsive shared-shell geometry and actual desktop pixels at the real requested font
 * scale. Account facts are explicit synthetic fixtures; these checks establish no native device,
 * TalkBack/VoiceOver, provider authentication or production session acceptance.
 */
@OptIn(ExperimentalTestApi::class)
class ResponsiveShellTest {
    private fun DesktopComposeUiTest.openAccount() {
        onNodeWithTag("shell-nav-account").performClick()
        waitForIdle()
        onNodeWithTag("shell-account").assertIsDisplayed()
    }

    private fun DesktopComposeUiTest.assertWholeTarget(tag: String, width: Int, height: Int) {
        val bounds = onNodeWithTag(tag).performScrollTo().assertIsDisplayed().getUnclippedBoundsInRoot()
        assertTrue(bounds.right - bounds.left >= 44.dp && bounds.bottom - bounds.top >= 44.dp,
            "$tag retains at least a 44 dp target: $bounds")
        assertTrue(bounds.left >= 0.dp && bounds.top >= 0.dp && bounds.right <= width.dp && bounds.bottom <= height.dp,
            "$tag remains wholly reachable in ${width}x$height: $bounds")
    }

    private fun DesktopComposeUiTest.assertUsableContentSlot(width: Int, height: Int) {
        val content = onNodeWithTag("shell-content-scroll").assertIsDisplayed()
        val bounds = content.getUnclippedBoundsInRoot()
        assertTrue(bounds.bottom - bounds.top >= (height * 0.6f).dp,
            "chrome must leave at least 60% of the viewport for task content in ${width}x$height: $bounds")
        assertTrue(bounds.left >= 0.dp && bounds.top >= 0.dp && bounds.right <= width.dp && bounds.bottom <= height.dp,
            "the content slot stays inside the viewport: $bounds")
        assertTrue(content.fetchSemanticsNode().config.contains(SemanticsProperties.VerticalScrollAxisRange),
            "remaining-height content must retain actual vertical scrolling")
    }

    private fun descendants(node: SemanticsNode): List<SemanticsNode> =
        listOf(node) + node.children.flatMap(::descendants)

    private fun DesktopComposeUiTest.assertLocalizedIconBack(language: String) {
        onAllNodesWithTag("shell-back").assertCountEquals(1)
        val back = onNodeWithTag("shell-back").assertIsDisplayed()
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.ContentDescription,
                listOf(ShellStrings.forLanguage(language)[ShellCopy.Back])))
        val bounds = back.getUnclippedBoundsInRoot()
        assertTrue(bounds.right - bounds.left in 44.dp..56.dp && bounds.bottom - bounds.top in 44.dp..56.dp,
            "the icon Back target remains compact independently of font scale: $bounds")
        val subtree = descendants(onNodeWithTag("shell-back", useUnmergedTree = true).fetchSemanticsNode())
        assertEquals(1, subtree.count { it.config.contains(SemanticsActions.OnClick) },
            "Back exposes one actionable target")
        assertTrue(subtree.none { it.config.contains(SemanticsActions.GetTextLayoutResult) },
            "the compact Back icon cannot grow a wrapping visible label")
    }

    private fun DesktopComposeUiTest.assertNavigationWordsRemainWhole() {
        onNodeWithTag("shell-navigation-rail").assertIsDisplayed()
        for (tag in listOf("home", "games", "account")) {
            val control = onNodeWithTag("shell-nav-$tag", useUnmergedTree = true).assertIsDisplayed()
            val textNodes = descendants(control.fetchSemanticsNode())
                .filter { it.config.contains(SemanticsActions.GetTextLayoutResult) }
            assertTrue(textNodes.isNotEmpty(), "rail navigation $tag retains a measured label")
            for (node in textNodes) {
                val layouts = mutableListOf<TextLayoutResult>()
                assertTrue(node.config[SemanticsActions.GetTextLayoutResult].action?.invoke(layouts) == true)
                assertTrue(layouts.isNotEmpty(), "rail navigation $tag returns an actual text layout")
                for (layout in layouts) {
                    val label = layout.layoutInput.text.text
                    for (line in 0 until layout.lineCount) {
                        assertFalse(layout.isLineEllipsized(line), "rail navigation keeps the complete label: $label")
                        if (line < layout.lineCount - 1) {
                            val end = layout.getLineEnd(line, visibleEnd = true)
                            val nextStart = layout.getLineStart(line + 1)
                            val betweenWords = label.substring(end, nextStart).any { it.isWhitespace() } ||
                                label.getOrNull(end)?.isWhitespace() == true
                            assertTrue(betweenWords, "rail navigation cannot split a word: $label, line=$line")
                        }
                    }
                }
            }
        }
    }

    @Test
    fun compactBackRetainsOneLocalizedIconTargetAndItsNavigationAtNormalAndLargeText() {
        for (width in listOf(320, 390)) for (language in listOf("en", "vi")) {
            for (fontScale in listOf(1f, 2f)) {
                val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
                try {
                    runDesktopComposeUiTest(width = width, height = 844) {
                        setContent {
                            AccountTestViewport(width, 844, fontScale) {
                                TabulaApp(account = fixture.port, scheme = TabulaScheme.Light,
                                    deviceFacts = DeviceFacts(false, language))
                            }
                        }
                        openAccount()
                        onNodeWithTag("account-open-profile").performScrollTo().performClick()
                        waitForIdle()
                        onNodeWithTag("shell-profile").assertIsDisplayed()
                        assertLocalizedIconBack(language)
                        onNodeWithTag("shell-back").performClick()
                        waitForIdle()
                        onNodeWithTag("shell-account").assertIsDisplayed()
                        assertEquals(0, fixture.adapter.signOutCalls)
                    }
                } finally { fixture.close() }
            }
        }
    }

    @Test
    fun responsiveAccountAndProfileRetainContentAndCompleteTextAcrossViewportsSchemesLocalesAndFontScales() {
        for ((width, height) in listOf(320 to 844, 390 to 844, 844 to 390)) {
            for (scheme in TabulaScheme.entries) for (language in listOf("en", "vi")) {
                for (fontScale in listOf(1f, 2f)) {
                    val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated,
                        longFields = true, vietnamese = language == "vi")
                    val case = "${width}x$height-${scheme.name.lowercase()}-$language-font${(fontScale * 100).toInt()}-synthetic"
                    try {
                        runDesktopComposeUiTest(width = width, height = height) {
                            setContent {
                                AccountTestViewport(width, height, fontScale) {
                                    TabulaApp(account = fixture.port, scheme = scheme,
                                        deviceFacts = DeviceFacts(false, language))
                                }
                            }
                            openAccount()
                            assertUsableContentSlot(width, height)
                            assertShellTextFitsHorizontally()
                            if (width > height) assertNavigationWordsRemainWhole()
                            captureShell("responsive-account-top-$case")
                            assertWholeTarget("account-open-profile", width, height)
                            onNodeWithTag("account-open-profile").performClick()
                            waitForIdle()
                            onNodeWithTag("shell-profile").assertIsDisplayed()
                            assertUsableContentSlot(width, height)
                            assertShellTextFitsHorizontally()
                            captureShell("responsive-profile-top-$case")
                            onNodeWithTag("profile-account-id").performScrollTo().assertIsDisplayed()
                            assertShellTextFitsHorizontally()
                            captureShell("responsive-profile-account-id-scrolled-$case")
                            assertWholeTarget("account-sign-out", width, height)
                            assertWholeTarget("account-browse-library", width, height)
                            assertLocalizedIconBack(language)
                            if (width > height) assertNavigationWordsRemainWhole()
                            assertEquals(0, fixture.adapter.signOutCalls,
                                "layout inspection and route navigation do not dispatch account mutations")
                        }
                    } finally { fixture.close() }
                }
            }
        }
    }
}
