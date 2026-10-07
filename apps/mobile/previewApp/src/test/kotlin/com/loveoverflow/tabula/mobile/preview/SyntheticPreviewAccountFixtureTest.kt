package com.loveoverflow.tabula.mobile.preview

import com.loveoverflow.tabula.mobile.account.AccountErrorReason
import com.loveoverflow.tabula.mobile.account.AccountOperation
import com.loveoverflow.tabula.mobile.account.AccountRefreshResult
import com.loveoverflow.tabula.mobile.account.AccountState
import com.loveoverflow.tabula.mobile.account.AccountUnavailableReason
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotSame
import kotlin.test.assertSame
import kotlin.test.assertTrue

/** Pure preview-fixture checks; this target deliberately makes no Compose-pixel claim. */
class SyntheticPreviewAccountFixtureTest {
    @Test
    fun everySelectableScenarioSeedsItsNamedStateThroughTheRealController() {
        for (scenario in SyntheticPreviewAccountScenario.entries) {
            val fixture = SyntheticPreviewAccountFixture(scenario)
            try {
                val state = fixture.port.state.value
                when (scenario) {
                    SyntheticPreviewAccountScenario.Unknown -> assertEquals(AccountState.Unknown, state)
                    SyntheticPreviewAccountScenario.SignedOut -> assertEquals(AccountState.SignedOut, state)
                    SyntheticPreviewAccountScenario.Loading -> assertEquals(AccountState.Loading(AccountOperation.Refresh), state)
                    SyntheticPreviewAccountScenario.Authenticated -> {
                        assertTrue(state is AccountState.Authenticated)
                        assertTrue(state.canSignOut)
                        assertSame(state.identity, fixture.identity, "bitmap fixture binds to the admitted display snapshot")
                        assertNotSame(fixture.adapter.identity, fixture.identity, "controller retires the adapter's input snapshot")
                    }
                    SyntheticPreviewAccountScenario.Expired -> assertEquals(AccountState.Expired, state)
                    SyntheticPreviewAccountScenario.Unavailable -> assertEquals(
                        AccountState.Unavailable(AccountUnavailableReason.NativeAdapterMissing), state,
                    )
                    SyntheticPreviewAccountScenario.Error -> assertEquals(
                        AccountState.Error(AccountErrorReason.Network, AccountOperation.Refresh), state,
                    )
                }
                assertEquals(if (scenario == SyntheticPreviewAccountScenario.Unknown) 0 else 1, fixture.adapter.refreshCalls)
                assertEquals(0, fixture.adapter.signOutCalls)
            } finally { fixture.close() }
        }
    }

    @Test
    fun longLocalizedDisplayFactsAreSyntheticAndStayInsideTheCurrentDtoBounds() {
        for (vietnamese in listOf(false, true)) {
            val identity = syntheticPreviewIdentity(longFields = true, vietnamese = vietnamese)
            val profile = requireNotNull(identity.profile)
            assertTrue(profile.displayName.length <= 64)
            assertTrue(profile.displayName.encodeToByteArray().size <= 256)
            assertEquals(32, profile.handle.length, "the long fixture exercises the full legal handle limit")
            assertTrue(profile.displayName.contains(if (vietnamese) "giả lập" else "Synthetic preview"))
            assertTrue(profile.handle.startsWith("synthetic_preview"))
        }
    }

    @Test
    fun scenarioParsingIsExplicitAndUnknownOptionsCannotCreateASilentAuthFixture() {
        for (scenario in SyntheticPreviewAccountScenario.entries) {
            assertEquals(scenario, SyntheticPreviewAccountScenario.parse(scenario.option))
        }
        for (invalid in listOf("", "signed-in", "Authenticated", "authenticated ", "production")) {
            assertFailsWith<IllegalStateException> { SyntheticPreviewAccountScenario.parse(invalid) }
        }
    }

    @Test
    fun heldRefreshCanBeCancelledAndItsLateCompletionCannotPublishIdentity() {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Loading)
        try {
            repeat(10) { fixture.port.refresh() }
            assertEquals(1, fixture.adapter.refreshCalls)
            fixture.port.cancelPending()
            assertEquals(1, fixture.adapter.cancelledRequests.size)
            fixture.adapter.completeRefresh(AccountRefreshResult.Authenticated(fixture.identity))
            assertEquals(AccountState.Unknown, fixture.port.state.value)
        } finally { fixture.close() }
    }

    @Test
    fun closeReleasesTheSyntheticAdapterAndRetiresAllLaterDisplayCompletions() {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Loading)
        fixture.close()
        fixture.close()
        assertTrue(fixture.adapter.closed)
        fixture.adapter.completeRefresh(AccountRefreshResult.Authenticated(fixture.identity))
        assertEquals(AccountState.Unknown, fixture.port.state.value)
    }
}
