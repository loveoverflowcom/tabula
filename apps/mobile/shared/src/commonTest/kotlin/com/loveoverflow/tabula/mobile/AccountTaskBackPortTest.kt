package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.navigation.AccountTaskBackPort
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class AccountTaskBackPortTest {
    @Test
    fun aLocalConfirmationCanDismissBeforeRouteBack() {
        val back = AccountTaskBackPort()
        assertFalse(back.requestBack())
        var dismissals = 0
        val release = back.register { dismissals++; true }
        assertTrue(back.requestBack())
        assertEquals(1, dismissals)
        release()
        assertFalse(back.requestBack())
    }

    @Test
    fun oldTaskDisposalCannotClearANewerTaskHandler() {
        val back = AccountTaskBackPort()
        val oldRelease = back.register { false }
        val nextRelease = back.register { true }
        oldRelease()
        assertTrue(back.requestBack())
        nextRelease()
        nextRelease()
        assertFalse(back.requestBack())
    }
}
