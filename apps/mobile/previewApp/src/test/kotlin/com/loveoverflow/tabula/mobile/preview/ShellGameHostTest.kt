package com.loveoverflow.tabula.mobile.preview

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.DesktopComposeUiTest
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.bridge.HostMessage
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import java.io.File
import javax.imageio.ImageIO
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotSame
import kotlin.test.assertSame
import kotlin.test.assertTrue

/**
 * Runs the shared Compose shell on desktop against a simulated game page (testing only, ADR-0033).
 * These tests exercise navigation, the `GameHost` composition contract, `BindGameRuntime`,
 * `GameSession` and recomposition behaviour. They are not evidence about Android WebView or iOS
 * WKWebView, and the page they drive is a stand-in, not the real game.
 */
@OptIn(ExperimentalTestApi::class)
class ShellGameHostTest {
    private class TestOwner : LifecycleOwner {
        private val registry = LifecycleRegistry.createUnsafe(this)
        override val lifecycle: Lifecycle get() = registry
        val observers: Int get() = registry.observerCount
        fun event(event: Lifecycle.Event) = registry.handleLifecycleEvent(event)
    }

    @BeforeTest fun reset() = SimulatedGameHost.reset()

    @AfterTest fun clean() = SimulatedGameHost.reset()

    private val strings = ShellStrings.forLanguage("en")

    private fun runShell(
        games: List<BundledGame> = previewGames,
        autoBoot: Boolean = false,
        owner: TestOwner = TestOwner().also { it.event(Lifecycle.Event.ON_RESUME) },
        test: DesktopComposeUiTest.(size: androidx.compose.runtime.MutableState<Int>) -> Unit,
    ) = runDesktopComposeUiTest(width = 390, height = 844) {
        val host = SimulatedGameHost(autoBootMillis = if (autoBoot) 0 else null)
        val width = mutableStateOf(390)
        setContent {
            CompositionLocalProvider(LocalLifecycleOwner provides owner) {
                PhoneViewport(width.value, 844) { TabulaApp(gameHost = host, games = games, catalog = com.loveoverflow.tabula.mobile.catalog.DiscoveryCatalogState.Ready(previewCatalogGames(games)), scheme = TabulaScheme.Light, deviceFacts = DeviceFacts(false, "en")) }
            }
        }
        waitForIdle()
        test(width)
    }

    private fun shot(name: String, test: DesktopComposeUiTest) {
        val dir = System.getProperty("tabula.preview.screenshots") ?: return
        File(dir).mkdirs()
        ImageIO.write(test.onRoot().captureToImage().toAwtImage(), "png", File(dir, "$name.png"))
    }

    private val runtime get() = SimulatedGameHost.runtimes.last()

    @Test
    fun homeShowsExplicitDiscoveryFixturesWithoutMountingThePackagedHost() {
        runShell { onNodeWithTag("shell-details-${previewGames.single().id}").performScrollTo().assertIsDisplayed(); assertEquals(0, SimulatedGameHost.createdCount); shot("01-home", this) }
        runShell(games = emptyList()) {
            onNodeWithTag("discovery-empty").performScrollTo().assertIsDisplayed()
            assertEquals(0, SimulatedGameHost.createdCount)
        }
    }

    @Test
    fun openingStartsOneRuntimeWhoseFirstMessageCarriesLaunchCapabilitiesAndPreferences() = runShell {
        startPreviewGame()
        assertEquals(1, SimulatedGameHost.createdCount)
        val init = runtime.page.received.first() as HostMessage.Init
        assertEquals(1, init.generation)
        assertEquals(setOf(HostCapability.KeepAwake), init.capabilities)
        assertEquals(ThemePreference.Light, init.preferences.theme)
        assertEquals(MotionPreference.System, init.preferences.motion)
    }

    @Test
    fun openCloseAndReopenBuildsAFreshRuntimeEachTimeAndDisposesTheOldOne() = runShell {
        startPreviewGame()
        val first = runtime
        runtime.page.completeBoot(); waitForIdle()
        assertTrue(first.keepAwake, "the granted keep-awake request switched the screen setting on")
        shot("02-game-ready", this)
        onNodeWithText(strings[ShellCopy.Back]).performClick(); waitForIdle()
        // Back is routed to the page, which asks to confirm; the shell does not leave yet.
        onNodeWithTag("sim-leave-dialog").assertIsDisplayed()
        shot("03-back-leave-confirmation", this)
        assertEquals(0, SimulatedGameHost.disposedCount)
        onNodeWithTag("sim-confirm").performClick(); waitForIdle()
        onNodeWithTag("shell-start-local").performScrollTo().assertIsDisplayed()
        assertEquals(1, SimulatedGameHost.disposedCount)
        assertFalse(first.keepAwake, "leaving releases the keep-awake setting")

        onNodeWithTag("shell-start-local").performScrollTo().performClick(); waitForIdle()
        assertEquals(2, SimulatedGameHost.createdCount)
        assertNotSame(first, runtime)
        assertEquals(1, runtime.session.generation, "a reopened game is a new session, not a continuation")
        assertEquals(SimulatedPage.Phase.Loading, runtime.page.phase)
    }

    @Test
    fun backWhileLoadingLeavesImmediately() = runShell {
        startPreviewGame()
        onNodeWithText(strings[ShellCopy.Back]).performClick(); waitForIdle()
        onNodeWithTag("shell-start-local").performScrollTo().assertIsDisplayed()
        assertEquals(1, SimulatedGameHost.disposedCount)
    }

    @Test
    fun recompositionResizeAndANewEventLambdaNeverRebuildTheRuntime() = runShell { width ->
        startPreviewGame()
        runtime.page.completeBoot(); waitForIdle()
        val before = runtime
        for (size in listOf(360, 412, 320, 390)) { width.value = size; waitForIdle() }
        onNodeWithTag("sim-leave").performClick(); waitForIdle()
        assertEquals(1, SimulatedGameHost.createdCount)
        assertSame(before, runtime)
        assertEquals(SimulatedPage.Phase.Ready, runtime.page.phase)
        assertEquals(0, SimulatedGameHost.disposedCount)
    }

    /** The host contract itself: fresh `onEvent`/`modifier`/`back` values on every recomposition must not restart it. */
    @Test
    fun aHostSurvivesRecompositionWithChangingParameters() = runDesktopComposeUiTest(width = 390, height = 844) {
        val host = SimulatedGameHost(autoBootMillis = null)
        val launch = com.loveoverflow.tabula.mobile.host.GameLaunch(
            "com.example.preview",
            com.loveoverflow.tabula.mobile.shell.gamePreferences(false, com.loveoverflow.tabula.mobile.shell.DeviceFacts(false, "en")),
            setOf(HostCapability.KeepAwake),
        )
        val back = com.loveoverflow.tabula.mobile.host.GameBackPort()
        var counter by mutableStateOf(0)
        val events = mutableListOf<Int>()
        setContent {
            val n = counter // read here so each change recomposes this scope with a new lambda and modifier
            host.Content(launch, { events += n }, androidx.compose.ui.Modifier.testTag("host-$n"), back)
        }
        waitForIdle()
        val first = runtime
        repeat(5) { counter += 1; waitForIdle() }
        assertEquals(1, SimulatedGameHost.createdCount)
        assertSame(first, runtime)
        assertEquals(0, SimulatedGameHost.disposedCount)
        first.page.completeBoot(); waitForIdle()
        assertTrue(events.isNotEmpty() && events.last() == 5, "events reach the latest lambda, not the one captured at creation: $events")
    }

    @Test
    fun changingBackPortsRetiresTheOldHandlerWithoutDisposingTheRuntime() = runDesktopComposeUiTest(width = 390, height = 844) {
        val host = SimulatedGameHost(autoBootMillis = null)
        val launch = com.loveoverflow.tabula.mobile.host.GameLaunch(
            "com.example.preview",
            com.loveoverflow.tabula.mobile.shell.gamePreferences(false, DeviceFacts(false, "en")),
        )
        var back by mutableStateOf(com.loveoverflow.tabula.mobile.host.GameBackPort())
        val original = back
        setContent { host.Content(launch, {}, androidx.compose.ui.Modifier, back) }
        waitForIdle()
        val first = runtime
        first.page.completeBoot(); waitForIdle()
        assertTrue(original.requestBack())
        back = com.loveoverflow.tabula.mobile.host.GameBackPort()
        waitForIdle()
        assertFalse(original.requestBack(), "a retired port cannot still control the game")
        assertTrue(back.requestBack())
        assertSame(first, runtime)
        assertEquals(1, SimulatedGameHost.createdCount)
        assertEquals(0, SimulatedGameHost.disposedCount)
    }

    @Test
    fun suspendAndResumeReachThePageOnceEachAndStopAfterTheGameIsLeft() {
        val owner = TestOwner().also { it.event(Lifecycle.Event.ON_RESUME) }
        runShell(owner = owner) {
            val baseline = owner.observers
            startPreviewGame()
            runtime.page.completeBoot(); waitForIdle()
            val game = runtime
            assertEquals(baseline + 1, owner.observers, "one lifecycle observer while the game is on screen")
            owner.event(Lifecycle.Event.ON_PAUSE); owner.event(Lifecycle.Event.ON_STOP); waitForIdle()
            assertTrue(game.page.suspended)
            owner.event(Lifecycle.Event.ON_START); owner.event(Lifecycle.Event.ON_RESUME); waitForIdle()
            assertFalse(game.page.suspended)
            assertEquals(1, game.page.received.count { it is HostMessage.Suspend })
            assertEquals(1, game.page.received.count { it is HostMessage.Resume })

            onNodeWithText(strings[ShellCopy.Back]).performClick(); waitForIdle()
            onNodeWithTag("sim-confirm").performClick(); waitForIdle()
            assertEquals(baseline, owner.observers, "leaving the game removes its lifecycle observer")
            val seen = game.page.received.size
            owner.event(Lifecycle.Event.ON_PAUSE); owner.event(Lifecycle.Event.ON_RESUME); waitForIdle()
            assertEquals(seen, game.page.received.size, "a left game's observer is removed")
        }
    }

    @Test
    fun aHostFailureReplacesTheSurfaceAndTryAgainBuildsExactlyOneNewGame() = runShell {
        startPreviewGame()
        runtime.page.completeBoot(); waitForIdle()
        val failed = runtime
        failed.failHost("renderer-killed"); waitForIdle()
        onNodeWithText(strings[ShellCopy.FailureTitle]).assertIsDisplayed()
        onNodeWithText("renderer-killed").assertIsDisplayed()
        shot("04-host-failure-panel", this)
        assertEquals(1, SimulatedGameHost.disposedCount, "the failed surface is released, not left behind the panel")
        onNodeWithText(strings[ShellCopy.Retry]).performClick(); waitForIdle()
        assertEquals(2, SimulatedGameHost.createdCount)
        assertEquals(1, runtime.session.generation)
        assertEquals(SimulatedPage.Phase.Loading, runtime.page.phase)
    }

    @Test
    fun aPageFailureKeepsTheGamesOwnOverlayAndRetryIsANewGeneration() = runShell {
        startPreviewGame()
        runtime.page.completeBoot(); waitForIdle()
        onNodeWithTag("sim-fail").performClick(); waitForIdle()
        onNodeWithTag("sim-page").assertIsDisplayed()
        assertTrue(runCatching { onNodeWithText(strings[ShellCopy.FailureTitle]).assertIsDisplayed() }.isFailure, "no shell panel for a failure the game explains itself")
        onNodeWithTag("sim-reload").performClick(); waitForIdle()
        assertEquals(2, runtime.session.generation)
        assertEquals(SimulatedPage.Phase.Ready, runtime.page.phase)
        assertEquals(1, SimulatedGameHost.createdCount, "the page's own reload is not a new runtime")
    }

    @Test
    fun lateMessagesFromALeftRuntimeChangeNothing() = runShell {
        startPreviewGame()
        runtime.page.completeBoot(); waitForIdle()
        val old = runtime
        onNodeWithText(strings[ShellCopy.Back]).performClick(); waitForIdle()
        onNodeWithTag("sim-confirm").performClick(); waitForIdle()
        onNodeWithTag("shell-start-local").performScrollTo().assertIsDisplayed()
        old.injectLatePageText("""{"v":1,"type":"ready","gen":1,"bootMs":1}""")
        old.injectLatePageText("""{"v":1,"type":"exit","gen":1}""")
        old.failHost("late-renderer-event")
        waitForIdle()
        onNodeWithTag("shell-start-local").performScrollTo().assertIsDisplayed()
        assertEquals(1, SimulatedGameHost.createdCount)
        assertEquals(1, SimulatedGameHost.disposedCount)
        assertTrue(old.session.phase == com.loveoverflow.tabula.mobile.session.GameSession.Phase.Closed)
    }
}
