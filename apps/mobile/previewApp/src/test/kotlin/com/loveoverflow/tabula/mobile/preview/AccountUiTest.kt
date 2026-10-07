package com.loveoverflow.tabula.mobile.preview

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.DesktopComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performMouseInput
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.v2.runDesktopComposeUiTest
import androidx.compose.ui.unit.dp
import androidx.lifecycle.LifecycleOwner
import com.loveoverflow.tabula.mobile.TabulaApp
import com.loveoverflow.tabula.mobile.account.AccountErrorReason
import com.loveoverflow.tabula.mobile.account.AccountIdentity
import com.loveoverflow.tabula.mobile.account.AccountInvalidation
import com.loveoverflow.tabula.mobile.account.AccountOperation
import com.loveoverflow.tabula.mobile.account.AccountRefreshResult
import com.loveoverflow.tabula.mobile.account.AccountSignOutResult
import com.loveoverflow.tabula.mobile.account.AccountState
import com.loveoverflow.tabula.mobile.account.AccountUnavailableReason
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.localization.AccountCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.account
import com.loveoverflow.tabula.mobile.shell.DeviceFacts
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Actual shared CMP routes, semantics, pixels and input over the real account controller and an
 * explicitly synthetic desktop adapter. These tests prove no provider, HTTP, native device or
 * durable sign-out acceptance; they cannot convert the production unavailable port into auth.
 */
@OptIn(ExperimentalTestApi::class)
class AccountUiTest {
    private fun DesktopComposeUiTest.openAccount() {
        onNodeWithTag("shell-nav-account").performClick()
        waitForIdle()
        onNodeWithTag("shell-account").assertIsDisplayed()
    }

    private fun DesktopComposeUiTest.assertNoEditableCredentials() {
        onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsActions.SetText), useUnmergedTree = true).assertCountEquals(0)
        onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsProperties.EditableText), useUnmergedTree = true).assertCountEquals(0)
    }

    private fun DesktopComposeUiTest.assertPrivateFactsAbsent(identity: AccountIdentity) {
        val facts = listOfNotNull(identity.accountId.value, identity.profile?.displayName, identity.profile?.handle)
        onAllNodes(SemanticsMatcher("contains an old private identity fact") { node ->
            val text = node.config.getOrElse(SemanticsProperties.Text) { emptyList() }.joinToString { it.text }
            val descriptions = node.config.getOrElse(SemanticsProperties.ContentDescription) { emptyList() }.joinToString()
            facts.any { it.isNotEmpty() && (text.contains(it) || descriptions.contains(it)) }
        }, useUnmergedTree = true).assertCountEquals(0)
        for (tag in listOf("profile-account-id", "profile-display-name", "profile-handle", "profile-visibility")) {
            onAllNodesWithTag(tag).assertCountEquals(0)
        }
        onAllNodesWithTag("account-avatar-image").assertCountEquals(0)
    }

    private fun DesktopComposeUiTest.assertAccountNavigationSelected() {
        onNodeWithTag("shell-nav-account")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
    }

    private fun DesktopComposeUiTest.assertTargetFits(tag: String, width: Int, height: Int = 844) {
        val target = onNodeWithTag(tag).performScrollTo().assertIsDisplayed()
        val bounds = target.getUnclippedBoundsInRoot()
        assertTrue(bounds.width >= 44.dp && bounds.height >= 44.dp, "$tag retains a 44 dp target: $bounds")
        assertTrue(bounds.left >= 0.dp && bounds.right <= width.dp && bounds.top >= 0.dp && bounds.bottom <= height.dp,
            "$tag remains wholly reachable within the viewport: $bounds")
    }

    @Test
    fun productionDefaultRemainsUnavailableAndBrowsingDoesNotInventAnIdentity() = runDesktopComposeUiTest(width = 320, height = 844) {
        setContent { AccountTestViewport(320, 844) { TabulaApp(deviceFacts = DeviceFacts(false, "en")) } }
        openAccount()
        onNodeWithTag("shell-account-unavailable").performScrollTo().assertIsDisplayed()
        assertPrivateFactsAbsent(syntheticPreviewIdentity())
        assertNoEditableCredentials()
        onNodeWithTag("account-open-profile").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("shell-profile").assertIsDisplayed()
        assertPrivateFactsAbsent(syntheticPreviewIdentity())
        assertNoEditableCredentials()
        onNodeWithTag("shell-back").performClick()
        waitForIdle()
        onNodeWithTag("account-open-friends").performScrollTo().performClick()
        waitForIdle()
        onNodeWithTag("shell-friends").assertIsDisplayed()
        onNodeWithTag("account-friends-unavailable").performScrollTo().assertIsDisplayed()
        assertPrivateFactsAbsent(syntheticPreviewIdentity())
        onNodeWithTag("shell-back").performClick()
        waitForIdle()
        assertTargetFits("account-browse-library", 320)
        captureShell("account-default-unavailable-320-light-en")
        onNodeWithTag("account-browse-library").performClick()
        waitForIdle()
        onNodeWithTag("shell-games").assertIsDisplayed()
        openAccount()
        onNodeWithTag("shell-account-unavailable").performScrollTo().assertIsDisplayed()
        assertPrivateFactsAbsent(syntheticPreviewIdentity())
    }

    @Test
    fun explicitFixturesRenderEveryDistinctAccountStateWithoutCredentialFields() {
        for (scenario in SyntheticPreviewAccountScenario.entries) {
            val fixture = SyntheticPreviewAccountFixture(scenario)
            try {
                runDesktopComposeUiTest(width = 390, height = 844) {
                    setContent {
                        AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) }
                    }
                    openAccount()
                    onNodeWithTag("account-state-${scenario.option}").performScrollTo().assertIsDisplayed()
                    assertAccountNavigationSelected()
                    assertNoEditableCredentials()
                    if (scenario != SyntheticPreviewAccountScenario.Authenticated) assertPrivateFactsAbsent(fixture.identity)
                    assertShellTextFitsHorizontally()
                    captureShell("account-fixture-${scenario.option}-390-light-en")
                }
            } finally { fixture.close() }
        }
    }

    @Test
    fun loginRegistrationProfileAndFriendsRetainTheirCallerOnBack() {
        for (scenario in listOf(SyntheticPreviewAccountScenario.SignedOut, SyntheticPreviewAccountScenario.Authenticated)) {
            val fixture = SyntheticPreviewAccountFixture(scenario)
            try {
                runDesktopComposeUiTest(width = 390, height = 844) {
                    setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
                    openAccount()
                    val routes = if (scenario == SyntheticPreviewAccountScenario.SignedOut) {
                        listOf("login", "register")
                    } else listOf("profile", "friends")
                    for (route in routes) {
                        onNodeWithTag("account-open-$route").performScrollTo().performClick()
                        waitForIdle()
                        onNodeWithTag("shell-$route").assertIsDisplayed()
                        assertAccountNavigationSelected()
                        assertNoEditableCredentials()
                        captureShell("account-route-$route-390-light-en")
                        onNodeWithTag("shell-back").performClick()
                        waitForIdle()
                        onNodeWithTag("shell-account").assertIsDisplayed()
                    }
                    assertEquals(1, fixture.adapter.refreshCalls, "route navigation starts no implicit session request")
                    assertEquals(0, fixture.adapter.signOutCalls)
                }
            } finally { fixture.close() }
        }
    }

    @Test
    fun loginAndRegistrationExplainTheProviderBoundaryAndNeverClaimCompletion() = runDesktopComposeUiTest(width = 320, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.SignedOut)
        val strings = ShellStrings.forLanguage("vi")
        try {
            setContent {
                AccountTestViewport(320, 844, fontScale = 2f) {
                    TabulaApp(account = fixture.port, scheme = TabulaScheme.Dark, deviceFacts = DeviceFacts(true, "vi"))
                }
            }
            openAccount()
            onNodeWithTag("account-open-login").performScrollTo().performClick()
            waitForIdle()
            onNodeWithText(strings.account(AccountCopy.LoginUnavailableBody)).performScrollTo().assertIsDisplayed()
            onNodeWithText(strings.account(AccountCopy.ProviderBoundary)).performScrollTo().assertIsDisplayed()
            assertNoEditableCredentials()
            assertShellTextFitsHorizontally()
            captureShell("account-login-provider-unavailable-320-dark-vi-font200")
            onNodeWithTag("shell-back").performClick()
            waitForIdle()
            onNodeWithTag("account-open-register").performScrollTo().performClick()
            waitForIdle()
            onNodeWithText(strings.account(AccountCopy.RegisterUnavailableBody)).performScrollTo().assertIsDisplayed()
            onNodeWithText(strings.account(AccountCopy.RegistrationDisposition)).performScrollTo().assertIsDisplayed()
            assertNoEditableCredentials()
            assertEquals(AccountState.SignedOut, fixture.port.state.value, "enrollment copy does not create a local account")
            assertShellTextFitsHorizontally()
            captureShell("account-register-provider-unavailable-320-dark-vi-font200")
        } finally { fixture.close() }
    }

    @Test
    fun profileShowsOnlyTheCurrentValidatedFactsAndFriendsDoesNotInventSocialData() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
        val strings = ShellStrings.forLanguage("en")
        try {
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            onNodeWithTag("account-open-profile").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("profile-account-id").performScrollTo().assertIsDisplayed()
            onNodeWithText(fixture.identity.accountId.value).assertIsDisplayed()
            onNodeWithTag("profile-display-name").performScrollTo().assertIsDisplayed()
            onAllNodesWithText(requireNotNull(fixture.identity.profile).displayName).assertCountEquals(2)
            onNodeWithTag("profile-handle").performScrollTo().assertIsDisplayed()
            onNodeWithTag("profile-visibility").performScrollTo().assertIsDisplayed()
            assertNoEditableCredentials()
            onNodeWithTag("shell-back").performClick()
            waitForIdle()
            onNodeWithTag("account-open-friends").performScrollTo().performClick()
            waitForIdle()
            onNodeWithText(strings.account(AccountCopy.FriendsUnavailableBody)).performScrollTo().assertIsDisplayed()
            assertNoEditableCredentials()
            onAllNodesWithText("Online", substring = false).assertCountEquals(0)
            onAllNodesWithText("Offline", substring = false).assertCountEquals(0)
            captureShell("account-friends-unavailable-authenticated-390-light-en")
        } finally { fixture.close() }
    }

    @Test
    fun cancellingSessionCheckRetiresLateIdentityAndRapidRetriesAreSingleFlight() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Error)
        try {
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            val retry = onNodeWithTag("account-retry").performScrollTo()
            val dispatchRetry = requireNotNull(retry.fetchSemanticsNode().config[SemanticsActions.OnClick].action)
            // Retain the actual UI callback to exercise taps queued before the loading frame.
            repeat(6) { assertTrue(dispatchRetry()) }
            waitForIdle()
            assertEquals(2, fixture.adapter.refreshCalls)
            repeat(6) { fixture.port.refresh() }
            assertEquals(2, fixture.adapter.refreshCalls, "the real controller rejects duplicate dispatch while loading")
            onNodeWithTag("account-state-loading").performScrollTo().assertIsDisplayed()
            onAllNodesWithTag("account-retry").fetchSemanticsNodes().forEach { node ->
                assertTrue(node.config.contains(SemanticsProperties.Disabled), "a retained retry control must be disabled while busy")
            }
            assertPrivateFactsAbsent(fixture.identity)
            onNodeWithTag("account-cancel").performScrollTo().performClick()
            waitForIdle()
            assertEquals(AccountState.Unknown, fixture.port.state.value)
            assertEquals(1, fixture.adapter.cancelledRequests.size)
            fixture.adapter.completeRefresh(AccountRefreshResult.Authenticated(fixture.identity), index = 1)
            waitForIdle()
            assertEquals(AccountState.Unknown, fixture.port.state.value, "retired completion cannot restore stale identity")
            assertPrivateFactsAbsent(fixture.identity)
            captureShell("account-cancelled-refresh-no-private-facts-390-light-en")
        } finally { fixture.close() }
    }

    @Test
    fun signOutConfirmationCanBeCancelledAndUnconfirmedLogoutLocksOutSessionRefresh() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
        try {
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            onNodeWithTag("account-sign-out").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-confirm-sign-out").performKeyInput { pressKey(Key.Escape) }
            waitForIdle()
            onAllNodesWithTag("account-sign-out-confirmation").assertCountEquals(0)
            assertEquals(0, fixture.adapter.signOutCalls, "Escape cancels local confirmation without a logout request")
            onNodeWithTag("account-sign-out").assert(SemanticsMatcher.expectValue(SemanticsProperties.Focused, true))
            onNodeWithTag("account-sign-out").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-cancel-sign-out").performScrollTo().performClick()
            waitForIdle()
            assertEquals(0, fixture.adapter.signOutCalls, "cancelling confirmation dispatches nothing")
            assertTrue(fixture.port.state.value is AccountState.Authenticated)
            onNodeWithTag("account-sign-out").assert(SemanticsMatcher.expectValue(SemanticsProperties.Focused, true))
            onNodeWithTag("account-sign-out").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-confirm-sign-out").performScrollTo().performClick()
            waitForIdle()
            assertEquals(AccountState.Loading(AccountOperation.SignOut), fixture.port.state.value)
            assertEquals(1, fixture.adapter.signOutCalls)
            assertPrivateFactsAbsent(fixture.identity)
            repeat(6) { fixture.port.signOut(); fixture.port.refresh() }
            assertEquals(1, fixture.adapter.signOutCalls)
            assertEquals(1, fixture.adapter.refreshCalls)
            fixture.adapter.completeSignOut(AccountSignOutResult.Unconfirmed)
            waitForIdle()
            assertEquals(AccountState.Error(AccountErrorReason.SignOutUnconfirmed, AccountOperation.SignOut), fixture.port.state.value)
            assertPrivateFactsAbsent(fixture.identity)
            fixture.port.refresh()
            assertEquals(1, fixture.adapter.refreshCalls, "an unresolved logout cannot restore a refreshed private snapshot")
            onNodeWithTag("account-retry").performScrollTo().performClick()
            waitForIdle()
            assertEquals(2, fixture.adapter.signOutCalls, "retry uses the original sign-out operation")
            assertEquals(1, fixture.adapter.refreshCalls)
            fixture.adapter.completeSignOut(AccountSignOutResult.Confirmed)
            waitForIdle()
            assertEquals(AccountState.SignedOut, fixture.port.state.value)
            onNodeWithTag("account-state-signed-out").performScrollTo().assertIsDisplayed()
            assertPrivateFactsAbsent(fixture.identity)
            captureShell("account-confirmed-synthetic-sign-out-390-light-en")
        } finally { fixture.close() }
    }

    @Test
    fun leavingAccountRoutesCancelsPendingWorkAndCannotRestorePrivateFacts() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Loading)
        try {
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            onNodeWithTag("shell-nav-games").performClick()
            waitForIdle()
            assertEquals(1, fixture.adapter.cancelledRequests.size)
            fixture.adapter.completeRefresh(AccountRefreshResult.Authenticated(fixture.identity))
            waitForIdle()
            assertPrivateFactsAbsent(fixture.identity)
            openAccount()
            assertEquals(AccountState.Unknown, fixture.port.state.value)
            assertPrivateFactsAbsent(fixture.identity)
        } finally { fixture.close() }
    }

    @Test
    fun stoppingWaitForDispatchedSignOutDoesNotClaimLogoutOrAcceptItsRetiredReply() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
        try {
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            onNodeWithTag("account-sign-out").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-confirm-sign-out").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-cancel").performScrollTo().performClick()
            waitForIdle()
            assertEquals(AccountState.Error(AccountErrorReason.SignOutUnconfirmed, AccountOperation.SignOut), fixture.port.state.value)
            assertEquals(1, fixture.adapter.cancelledRequests.size)
            assertPrivateFactsAbsent(fixture.identity)
            fixture.adapter.completeSignOut(AccountSignOutResult.Confirmed, index = 0)
            waitForIdle()
            assertTrue(fixture.port.state.value is AccountState.Error, "a retired reply does not publish a logout-complete claim")
            onAllNodesWithTag("account-state-signed-out").assertCountEquals(0)
            onNodeWithTag("account-retry").performScrollTo().performClick()
            waitForIdle()
            assertEquals(2, fixture.adapter.signOutCalls)
            fixture.adapter.completeSignOut(AccountSignOutResult.Confirmed, index = 1)
            waitForIdle()
            assertEquals(AccountState.SignedOut, fixture.port.state.value)
            assertPrivateFactsAbsent(fixture.identity)
        } finally { fixture.close() }
    }

    @Test
    fun everyIdentityLosingTransitionRemovesFactsFromNestedProfileAndManagedAvatar() {
        val terminalResults = listOf(
            AccountRefreshResult.SignedOut,
            AccountRefreshResult.Expired,
            AccountRefreshResult.Unavailable(AccountUnavailableReason.ServiceUnavailable),
            AccountRefreshResult.Failed(AccountErrorReason.Network),
        )
        for (result in terminalResults) {
            val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
            try {
                runDesktopComposeUiTest(width = 390, height = 844) {
                    val avatar = syntheticManagedAvatar(fixture.identity, TabulaScheme.Light)
                    setContent {
                        AccountTestViewport(390, 844) {
                            TabulaApp(account = fixture.port, accountAvatar = avatar, deviceFacts = DeviceFacts(false, "en"))
                        }
                    }
                    openAccount()
                    onNodeWithTag("account-open-profile").performScrollTo().performClick()
                    waitForIdle()
                    assertTrue(onAllNodesWithTag("account-avatar-image").fetchSemanticsNodes().isNotEmpty())
                    fixture.port.refresh()
                    waitForIdle()
                    assertPrivateFactsAbsent(fixture.identity)
                    fixture.adapter.completeRefresh(result)
                    waitForIdle()
                    onNodeWithTag("shell-profile").assertIsDisplayed()
                    assertPrivateFactsAbsent(fixture.identity)
                    assertNoEditableCredentials()
                }
            } finally { fixture.close() }
        }
    }

    @Test
    fun managedImageRequiresTheExactCurrentIdentitySnapshotAndBackgroundMasksIt() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
        try {
            val avatar = syntheticManagedAvatar(fixture.identity, TabulaScheme.Dark)
            setContent {
                AccountTestViewport(390, 844) {
                    TabulaApp(account = fixture.port, accountAvatar = avatar, scheme = TabulaScheme.Dark, deviceFacts = DeviceFacts(false, "en"))
                }
            }
            openAccount()
            assertTrue(onAllNodesWithTag("account-avatar-image").fetchSemanticsNodes().isNotEmpty())
            fixture.port.onForegroundChanged(false)
            waitForIdle()
            assertPrivateFactsAbsent(fixture.identity)
            fixture.port.onForegroundChanged(true)
            waitForIdle()
            assertPrivateFactsAbsent(fixture.identity)
            fixture.port.refresh()
            val newSnapshot = syntheticPreviewIdentity()
            fixture.adapter.completeRefresh(AccountRefreshResult.Authenticated(newSnapshot))
            waitForIdle()
            onNodeWithTag("account-state-authenticated").performScrollTo().assertIsDisplayed()
            onAllNodesWithTag("account-avatar-image").assertCountEquals(0)
            assertTrue(onAllNodesWithTag("account-avatar-neutral").fetchSemanticsNodes().isNotEmpty(),
                "an equal account ID does not authorize an image from the previous session snapshot")
            fixture.port.invalidate(AccountInvalidation.AccountChanged)
            waitForIdle()
            assertPrivateFactsAbsent(newSnapshot)
            captureShell("account-managed-avatar-stale-snapshot-fallback-390-dark-en")
        } finally { fixture.close() }
    }

    @Test
    fun actualAppLifecycleObserverMasksIdentityOnStopAndDoesNotRestoreItOnStart() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
        val lifecycle = SyntheticAccountTestLifecycleOwner()
        try {
            setContent {
                AccountTestViewport(390, 844, lifecycleOwner = lifecycle) {
                    TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en"))
                }
            }
            openAccount()
            onNodeWithTag("account-state-authenticated").performScrollTo().assertIsDisplayed()
            runOnIdle { lifecycle.stop() }
            waitForIdle()
            assertEquals(AccountState.Unknown, fixture.port.state.value)
            assertPrivateFactsAbsent(fixture.identity)
            runOnIdle { lifecycle.start() }
            waitForIdle()
            assertEquals(AccountState.Unknown, fixture.port.state.value)
            assertPrivateFactsAbsent(fixture.identity)
            assertEquals(1, fixture.adapter.refreshCalls, "foreground is not automatic login or account revalidation")
            onNodeWithTag("account-retry").performScrollTo().performClick()
            waitForIdle()
            assertEquals(2, fixture.adapter.refreshCalls)
        } finally { fixture.close() }
    }

    @Test
    fun profileToolbarBackCancelsLocalSignOutConfirmationBeforeLeavingTheRoute() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
        try {
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            onNodeWithTag("account-open-profile").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-sign-out").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-sign-out-confirmation").performScrollTo().assertIsDisplayed()
            onNodeWithTag("shell-back").performClick()
            waitForIdle()
            onNodeWithTag("shell-profile").assertIsDisplayed()
            onAllNodesWithTag("account-sign-out-confirmation").assertCountEquals(0)
            assertEquals(0, fixture.adapter.signOutCalls, "first toolbar Back cancels confirmation only")
            onNodeWithTag("shell-back").performClick()
            waitForIdle()
            onNodeWithTag("shell-account").assertIsDisplayed()
            assertEquals(0, fixture.adapter.signOutCalls)
        } finally { fixture.close() }
    }

    @Test
    fun immediateSameFactRefreshRetiresTheOldAvatarAndConfirmationWithoutALoadingFrame() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
        try {
            val avatar = syntheticManagedAvatar(fixture.identity, TabulaScheme.Light)
            setContent {
                AccountTestViewport(390, 844) {
                    TabulaApp(account = fixture.port, accountAvatar = avatar, deviceFacts = DeviceFacts(false, "en"))
                }
            }
            openAccount()
            onNodeWithTag("account-open-profile").performScrollTo().performClick()
            waitForIdle()
            assertTrue(onAllNodesWithTag("account-avatar-image").fetchSemanticsNodes().isNotEmpty())
            onNodeWithTag("account-sign-out").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-sign-out-confirmation").performScrollTo().assertIsDisplayed()
            fixture.adapter.synchronousRefresh = AccountRefreshResult.Authenticated(fixture.adapter.identity)
            runOnIdle { fixture.port.refresh() }
            // The synthetic adapter completes inside refresh, before another Compose frame.
            val current = fixture.port.state.value as AccountState.Authenticated
            assertTrue(current.identity !== fixture.identity)
            assertEquals(fixture.identity.accountId, current.identity.accountId)
            assertEquals(fixture.identity.profile, current.identity.profile)
            waitForIdle()
            onAllNodesWithTag("account-avatar-image").assertCountEquals(0)
            onAllNodesWithTag("account-sign-out-confirmation").assertCountEquals(0)
            onNodeWithTag("account-sign-out").performScrollTo().assertIsDisplayed()
            assertTrue(onAllNodesWithTag("account-avatar-neutral").fetchSemanticsNodes().isNotEmpty())
            assertEquals(0, fixture.adapter.signOutCalls)
            captureShell("account-immediate-refresh-retires-image-confirmation-390-light-en")
        } finally { fixture.close() }
    }

    @Test
    fun replacingLifecycleOwnerRetiresOldDisplayWithoutClosingTheCurrentPort() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated)
        val first = SyntheticAccountTestLifecycleOwner()
        val second = SyntheticAccountTestLifecycleOwner()
        val owner = mutableStateOf<LifecycleOwner>(first)
        val visible = mutableStateOf(true)
        try {
            setContent {
                AccountTestViewport(390, 844, lifecycleOwner = owner.value) {
                    if (visible.value) TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en"))
                }
            }
            openAccount()
            runOnIdle { owner.value = second }
            waitForIdle()
            assertTrue(!fixture.adapter.closed, "owner replacement removes its observer but does not terminally close the app-owned port")
            assertEquals(AccountState.Unknown, fixture.port.state.value)
            assertPrivateFactsAbsent(fixture.identity)
            onNodeWithTag("account-retry").performScrollTo().performClick()
            waitForIdle()
            assertEquals(2, fixture.adapter.refreshCalls, "the same live port can explicitly revalidate under its new owner")
            fixture.adapter.completeRefresh(AccountRefreshResult.Authenticated(fixture.adapter.identity))
            waitForIdle()
            assertTrue(fixture.port.state.value is AccountState.Authenticated)
            runOnIdle { first.stop() }
            waitForIdle()
            assertTrue(fixture.port.state.value is AccountState.Authenticated, "the retired owner's observer cannot mask the new owner")
            runOnIdle { visible.value = false }
            waitForIdle()
            assertTrue(fixture.adapter.closed, "removing the app finally closes its account adapter")
            assertEquals(AccountState.Unknown, fixture.port.state.value)
        } finally { fixture.close() }
    }

    @Test
    fun idOnlySessionDoesNotInventProfileFieldsOrDisableReadOnlyAccountRoutes() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Loading)
        val identity = AccountIdentity(fixture.identity.accountId)
        try {
            fixture.adapter.completeRefresh(AccountRefreshResult.Authenticated(identity))
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            onNodeWithTag("account-open-profile").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("profile-details-unavailable").performScrollTo().assertIsDisplayed()
            onNodeWithTag("profile-account-id").performScrollTo().assertIsDisplayed()
            for (tag in listOf("profile-display-name", "profile-handle", "profile-visibility")) onAllNodesWithTag(tag).assertCountEquals(0)
            assertNoEditableCredentials()
            onNodeWithTag("shell-back").performClick()
            waitForIdle()
            onNodeWithTag("account-open-friends").performScrollTo().performClick()
            waitForIdle()
            onNodeWithTag("account-friends-unavailable").performScrollTo().assertIsDisplayed()
        } finally { fixture.close() }
    }

    @Test
    fun longValidatedProfilesWrapAndActionsRemainReachableAcrossCompactThemesLocalesAndLargeText() {
        for (width in listOf(320, 390)) for (scheme in listOf(TabulaScheme.Light, TabulaScheme.Dark)) {
            for (language in listOf("en", "vi")) for (fontScale in listOf(1f, 2f)) {
                val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated,
                    longFields = true, vietnamese = language == "vi")
                try {
                    runDesktopComposeUiTest(width = width, height = 844) {
                        setContent {
                            AccountTestViewport(width, 844, fontScale) {
                                TabulaApp(account = fixture.port, scheme = scheme, deviceFacts = DeviceFacts(true, language))
                            }
                        }
                        openAccount()
                        assertShellTextFitsHorizontally()
                        assertTargetFits("account-open-profile", width)
                        onNodeWithTag("account-open-profile").performClick()
                        waitForIdle()
                        assertShellTextFitsHorizontally()
                        onNodeWithTag("profile-visibility").performScrollTo().assertIsDisplayed()
                        assertShellTextFitsHorizontally()
                        assertNoEditableCredentials()
                        captureShell("account-profile-long-$width-${scheme.name.lowercase()}-$language-font${(fontScale * 100).toInt()}")
                        onNodeWithTag("shell-back").performClick()
                        waitForIdle()
                        assertTargetFits("account-sign-out", width)
                        assertTargetFits("account-browse-library", width)
                        assertShellTextFitsHorizontally()
                    }
                } finally { fixture.close() }
            }
        }
    }

    @Test
    fun cancelledPointerDoesNotDispatchRetryAndKeyboardActivationDispatchesOnce() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Error)
        try {
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            val retry = onNodeWithTag("account-retry").performScrollTo().assertIsDisplayed()
            retry.performMouseInput {
                moveTo(center)
                press()
                moveTo(Offset(-10f, -10f))
                release()
            }
            waitForIdle()
            assertEquals(1, fixture.adapter.refreshCalls, "pointer release outside the target cancels activation")
            retry.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            retry.performKeyInput { pressKey(Key.Enter) }
            waitForIdle()
            assertEquals(2, fixture.adapter.refreshCalls, "focused Enter dispatches one session check")
            assertEquals(AccountState.Loading(AccountOperation.Refresh), fixture.port.state.value)
            fixture.port.cancelPending()
            waitForIdle()
            val next = onNodeWithTag("account-retry").performScrollTo()
            next.performSemanticsAction(SemanticsActions.RequestFocus) { assertTrue(it()) }
            next.performKeyInput { pressKey(Key.Spacebar) }
            waitForIdle()
            assertEquals(3, fixture.adapter.refreshCalls, "focused Space dispatches one session check")
        } finally { fixture.close() }
    }

    @Test
    fun adapterWithoutSignOutCapabilityDoesNotOfferAFalseLogoutAction() = runDesktopComposeUiTest(width = 390, height = 844) {
        val fixture = SyntheticPreviewAccountFixture(SyntheticPreviewAccountScenario.Authenticated, supportsSignOut = false)
        try {
            setContent { AccountTestViewport(390, 844) { TabulaApp(account = fixture.port, deviceFacts = DeviceFacts(false, "en")) } }
            openAccount()
            onAllNodesWithTag("account-sign-out").fetchSemanticsNodes().forEach { node ->
                assertTrue(node.config.contains(SemanticsProperties.Disabled), "unsupported sign-out must be disabled if retained")
            }
            fixture.port.signOut()
            assertEquals(0, fixture.adapter.signOutCalls)
            assertTrue(fixture.port.state.value is AccountState.Authenticated)
        } finally { fixture.close() }
    }
}
