package com.loveoverflow.tabula.mobile.preview

import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.semantics.SemanticsProperties
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.design.TabulaTheme
import com.loveoverflow.tabula.mobile.shell.HomeScreen
import com.loveoverflow.tabula.mobile.shell.ShellText
import java.io.File
import javax.imageio.ImageIO
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
                            TabulaTheme(scheme) {
                                HomeScreen(games = previewGames, languageTag = "en", onOpen = {})
                            }
                        }
                    }
                    waitForIdle()
                    onAllNodesWithContentDescription(ShellText.HomeTitle, useUnmergedTree = true)
                        .assertCountEquals(1)
                    onNodeWithContentDescription(ShellText.HomeTitle)
                        .assertIsDisplayed()
                        .assert(SemanticsMatcher.expectValue(SemanticsProperties.Heading, Unit))
                    // The vector wordmark supplies the name once; no duplicate live-text label.
                    onAllNodesWithText(ShellText.HomeTitle, useUnmergedTree = true).assertCountEquals(0)
                    val screenshots = System.getProperty("tabula.preview.screenshots")
                    if (screenshots != null) {
                        File(screenshots).mkdirs()
                        ImageIO.write(
                            onRoot().captureToImage().toAwtImage(),
                            "png",
                            File(screenshots, "brand-home-${scheme.name.lowercase()}-$width.png"),
                        )
                    }
                }
            }
        }
    }
}
