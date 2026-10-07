package com.loveoverflow.tabula.mobile.preview

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.semantics.SemanticsProperties
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import kotlin.test.Test

/** Shared shell identity checks; this preview is not Android/iOS device or launcher evidence. */
@OptIn(ExperimentalTestApi::class)
class BrandIdentityTest {
    @Test
    fun canonicalIdentityFitsSmallPhonesAndHasOneAccessibleHeadingInEveryScheme() {
        for (scheme in TabulaScheme.entries) {
            for (width in listOf(320, 390)) {
                runDesktopComposeUiTest(width = width, height = 844) {
                    setContent {
                        PhoneViewport(width, 844) {
                            TabulaApp(
                                games = previewGames,
                                catalog = previewCatalog,
                                scheme = scheme,
                                deviceFacts = DeviceFacts(false, "en"),
                            )
                        }
                    }
                    waitForIdle()
                    onAllNodesWithContentDescription("Tabula", useUnmergedTree = true).assertCountEquals(1)
                    onNodeWithContentDescription("Tabula")
                        .assertIsDisplayed()
                        .assert(SemanticsMatcher.expectValue(SemanticsProperties.Heading, Unit))
                    // The vector wordmark supplies the name once; no duplicate live-text label.
                    onAllNodesWithText("Tabula", useUnmergedTree = true).assertCountEquals(0)
                    assertShellTextFitsHorizontally()
                    captureShell("brand-home-${scheme.name.lowercase()}-$width")
                }
            }
        }
    }
}
