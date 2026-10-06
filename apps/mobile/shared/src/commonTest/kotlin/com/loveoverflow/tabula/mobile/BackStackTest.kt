package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.navigation.BackStack
import com.loveoverflow.tabula.mobile.navigation.Destination
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue

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
}
