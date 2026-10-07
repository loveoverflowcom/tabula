package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.design.TabulaAccessibility
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.design.TabulaSchemes
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaType
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

/** Checks the generated adapter keeps the contract the shell relies on; values stay in tokens.toml. */
class TokenAdapterTest {
    @Test
    fun everyAuthoredSchemeResolvesAndSchemesDiffer() {
        val surfaces = TabulaScheme.entries.map { TabulaSchemes.of(it).surface }
        assertEquals(4, surfaces.size)
        assertNotEquals(TabulaSchemes.light.surface, TabulaSchemes.dark.surface)
        assertNotEquals(TabulaSchemes.light.primary, TabulaSchemes.light.surface)
    }

    @Test
    fun teamAndSeatPalettesKeepTheAuthoredWidth() {
        for (scheme in TabulaScheme.entries) {
            assertEquals(8, TabulaSchemes.of(scheme).team.size)
            assertEquals(8, TabulaSchemes.of(scheme).seatMarker.size)
        }
    }

    @Test
    fun touchTargetKeepsTheAccessibilityFloor() {
        assertTrue(TabulaAccessibility.minTarget >= 44f)
        assertTrue(TabulaSpace.lg > TabulaSpace.sm)
        assertTrue(TabulaType.headlineSm.lineHeight >= TabulaType.headlineSm.size)
    }
}
