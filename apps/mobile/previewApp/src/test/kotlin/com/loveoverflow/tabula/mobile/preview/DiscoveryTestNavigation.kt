package com.loveoverflow.tabula.mobile.preview

import androidx.compose.ui.test.DesktopComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import kotlin.test.assertEquals

/** The discovery route reaches setup before any explicit simulated-host launch. */
@OptIn(ExperimentalTestApi::class)
internal fun DesktopComposeUiTest.openPreviewSetup(id: String = previewGames.single().id) {
    onNodeWithTag("shell-details-$id").performScrollTo().performClick()
    waitForIdle()
    onNodeWithTag("shell-detail").assertIsDisplayed()
    onNodeWithTag("shell-setup-action").performScrollTo().performClick()
    waitForIdle()
    onNodeWithTag("shell-setup").assertIsDisplayed()
}

/** Keeps the old lifecycle assertions while exercising the new Home → detail → setup handoff. */
@OptIn(ExperimentalTestApi::class)
internal fun DesktopComposeUiTest.startPreviewGame(id: String = previewGames.single().id) {
    val before = SimulatedGameHost.createdCount
    openPreviewSetup(id)
    assertEquals(before, SimulatedGameHost.createdCount, "browsing detail and setup does not mount gameplay")
    onNodeWithTag("shell-start-local").performScrollTo().performClick()
    waitForIdle()
}
