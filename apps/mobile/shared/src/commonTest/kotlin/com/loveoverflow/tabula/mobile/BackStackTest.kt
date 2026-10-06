package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.navigation.BackStack
import com.loveoverflow.tabula.mobile.navigation.Destination
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue
import kotlin.test.assertNull

class BackStackTest {
    private val game = Destination.Game(GameLaunch("opaque-id"))

    @Test
    fun rootIsHomeAndCannotBePopped() {
        assertEquals(Destination.Home, BackStack.Root.current)
        assertFalse(BackStack.Root.canPop)
        assertSame(BackStack.Root, BackStack.Root.pop())
    }

    @Test
    fun pushShowsDestinationAndPopRestoresThePreviousOne() {
        val pushed = BackStack.Root.push(game)
        assertEquals(game, pushed.current)
        assertTrue(pushed.canPop)
        assertEquals(BackStack.Root, pushed.pop())
    }

    @Test
    fun popUnwindsInReverseOrder() {
        val other = Destination.Game(GameLaunch("another"))
        val stack = BackStack.Root.push(game).push(other)
        assertEquals(other, stack.current)
        assertEquals(game, stack.pop().current)
        assertEquals(Destination.Home, stack.pop().pop().current)
    }

    @Test
    fun pushDoesNotMutateTheReceiver() {
        val before = BackStack.Root
        before.push(game)
        assertEquals(Destination.Home, before.current)
    }

    @Test
    fun shellRoutesUseTheExistingWebIdentity() {
        val routes = listOf(
            Destination.Home to "/",
            Destination.Games to "/games",
            Destination.Detail("com.example.packaged") to "/games/com.example.packaged",
            Destination.Setup("com.example.packaged") to "/games/com.example.packaged?setup=1",
            Destination.Account to "/account",
        )
        for ((destination, path) in routes) {
            assertEquals(path, destination.routePath())
            assertEquals(destination, Destination.fromRoutePath(path))
        }
        assertEquals("/play/local/", game.routePath())
        assertNull(Destination.fromRoutePath(game.routePath()), "runtime identity cannot restore or launch a match")
    }

    @Test
    fun discoverySetupAndGameReturnToTheirCallerInOrder() {
        val detail = Destination.Detail("opaque-id")
        val setup = Destination.Setup("opaque-id")
        val stack = BackStack.Root.navigate(Destination.Games).push(detail).push(setup).push(game)
        assertEquals(game, stack.current)
        assertEquals(setup, stack.pop().current)
        assertEquals(detail, stack.pop().pop().current)
        assertEquals(Destination.Games, stack.pop().pop().pop().current)
        assertEquals(BackStack.Root, stack.pop().pop().pop().pop())
    }

    @Test
    fun topLevelNavigationResetsTheNestedHistoryAndRepeatedIntentIsANoop() {
        val nested = BackStack.Root.navigate(Destination.Games).push(Destination.Detail("opaque-id"))
        val account = nested.navigate(Destination.Account)
        assertEquals(Destination.Account, account.current)
        assertEquals(Destination.Home, account.pop().current)
        assertEquals(BackStack.Root, nested.navigate(Destination.Home))
        assertSame(nested, nested.push(nested.current))
    }

    @Test
    fun savedShellHistoryRestoresTheSameRouteAndBackOrder() {
        val stack = BackStack.Root.navigate(Destination.Games)
            .push(Destination.Detail("opaque-id"))
            .push(Destination.Setup("opaque-id"))
        val restored = BackStack.restoreRoutes(stack.saveRoutes())
        assertEquals(stack, restored)
        assertEquals(Destination.Detail("opaque-id"), restored.pop().current)
    }

    @Test
    fun savingAnActiveLocalMatchRestoresSetupWithoutLaunchOrSessionState() {
        val setup = BackStack.Root.navigate(Destination.Games)
            .push(Destination.Detail("opaque-id"))
            .push(Destination.Setup("opaque-id"))
        val saved = setup.push(game).saveRoutes()
        assertEquals(listOf("/", "/games", "/games/opaque-id", "/games/opaque-id?setup=1"), saved)
        assertEquals(setup, BackStack.restoreRoutes(saved), "launch restoration is a shell location, never gameplay")
        assertEquals(Destination.Setup("opaque-id"), BackStack.restoreRoutes(BackStack.Root.push(game).saveRoutes()).current)
    }

    @Test
    fun malformedOrOversizedSavedHistoryFallsBackToHome() {
        val malformed = listOf(
            emptyList(),
            listOf("/account"),
            listOf("/", "/friends"),
            listOf("/", "/games", "/play/local/"),
            List(33) { "/" },
            listOf("/", "/games/${"a".repeat(256)}"),
        )
        for (paths in malformed) assertEquals(BackStack.Root, BackStack.restoreRoutes(paths))
    }

    @Test
    fun routeParsingRejectsUrlDelimitersEscapesAndUnimplementedScreens() {
        val rejected = listOf(
            "https://example.test/games/opaque-id", "/games/", "/games/.", "/games/..",
            "/games/one/two", "/games/%2e%2e", "/games/opaque-id#secret", "/games/opaque-id?setup=2",
            "/games/opaque-id?setup=1&token=secret", "/games/opaque-id?token=secret", "/games/opaque-id\\other",
            "/login", "/me", "/friends", "/games/việt",
        )
        for (path in rejected) assertNull(Destination.fromRoutePath(path), path)
    }

    @Test
    fun navigationHistoryIsBoundedAndAlwaysRetainsItsHomeRoot() {
        var stack = BackStack.Root
        repeat(100) { stack = stack.push(Destination.Detail("opaque-$it")) }
        assertEquals(32, stack.saveRoutes().size)
        assertEquals("/", stack.saveRoutes().first())
        assertEquals(Destination.Detail("opaque-99"), stack.current)
        repeat(31) { stack = stack.pop() }
        assertEquals(BackStack.Root, stack)
    }
}
