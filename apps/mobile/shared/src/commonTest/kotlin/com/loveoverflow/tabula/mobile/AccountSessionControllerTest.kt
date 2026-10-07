package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.account.*
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNotSame
import kotlin.test.assertTrue

/** Real coordinator with controlled adapter callbacks; no auth, HTTP, native storage or device proof. */
class AccountSessionControllerTest {
    private class Adapter(override val supportsSignOut: Boolean = true) : AccountSessionAdapter {
        data class Refresh(val request: AccountSessionRequest, val complete: (AccountRefreshResult) -> Unit)
        data class SignOut(val request: AccountSessionRequest, val accountId: AccountId, val complete: (AccountSignOutResult) -> Unit)
        val refreshes = mutableListOf<Refresh>()
        val signOuts = mutableListOf<SignOut>()
        val cancellations = mutableListOf<AccountSessionRequest>()
        var closes = 0
        var onCancel: (() -> Unit)? = null
        var throwRefresh = false
        var throwSignOut = false
        var throwCleanup = false
        var immediateRefresh: AccountRefreshResult? = null
        override fun refresh(request: AccountSessionRequest, complete: (AccountRefreshResult) -> Unit) {
            refreshes += Refresh(request, complete)
            if (throwRefresh) throw IllegalStateException("private infrastructure detail")
            immediateRefresh?.let(complete)
        }
        override fun signOut(request: AccountSessionRequest, accountId: AccountId, complete: (AccountSignOutResult) -> Unit) {
            signOuts += SignOut(request, accountId, complete)
            if (throwSignOut) throw IllegalStateException("private infrastructure detail")
        }
        override fun cancel(request: AccountSessionRequest) {
            cancellations += request
            onCancel?.invoke()
            if (throwCleanup) throw IllegalStateException("private infrastructure detail")
        }
        override fun close() {
            closes += 1
            if (throwCleanup) throw IllegalStateException("private infrastructure detail")
        }
    }

    private class Setup(supportsSignOut: Boolean = true) {
        val adapter = Adapter(supportsSignOut)
        val controller = AccountSessionController(adapter)
        val state get() = controller.state.value
        fun authenticate(identity: AccountIdentity = identity()) {
            controller.refresh()
            adapter.refreshes.last().complete(AccountRefreshResult.Authenticated(identity))
        }
    }

    @Test fun productionDefaultStaysUnavailableForEveryOperation() {
        val port = UnavailableAccountSessionPort
        port.refresh(); port.signOut(); port.cancelPending()
        for (reason in AccountInvalidation.entries) port.invalidate(reason)
        port.onForegroundChanged(false); port.onForegroundChanged(true); port.close(); port.refresh()
        assertEquals(AccountState.Unavailable(AccountUnavailableReason.NativeAdapterMissing), port.state.value)
    }

    @Test fun freshControllerStartsUnknownWithoutAutomaticWork() {
        val setup = Setup()
        assertEquals(AccountState.Unknown, setup.state)
        assertTrue(setup.adapter.refreshes.isEmpty())
        setup.controller.signOut()
        assertTrue(setup.adapter.signOuts.isEmpty())
        setup.controller.refresh()
        assertEquals(AccountState.Loading(AccountOperation.Refresh), setup.state)
        assertEquals(AccountOperation.Refresh, setup.adapter.refreshes.single().request.operation)
    }

    @Test fun typedRefreshOutcomesRemainDistinctAndMasked() {
        val cases = listOf(
            AccountRefreshResult.SignedOut to AccountState.SignedOut,
            AccountRefreshResult.Expired to AccountState.Expired,
            AccountRefreshResult.Unavailable(AccountUnavailableReason.ServiceUnavailable) to AccountState.Unavailable(AccountUnavailableReason.ServiceUnavailable),
            AccountRefreshResult.Unavailable(AccountUnavailableReason.SecureStorageUnavailable) to AccountState.Unavailable(AccountUnavailableReason.SecureStorageUnavailable),
            AccountRefreshResult.Failed(AccountErrorReason.Network) to AccountState.Error(AccountErrorReason.Network, AccountOperation.Refresh),
            AccountRefreshResult.Failed(AccountErrorReason.InvalidResponse) to AccountState.Error(AccountErrorReason.InvalidResponse, AccountOperation.Refresh),
        )
        for ((result, expected) in cases) {
            val setup = Setup(); setup.authenticate(); setup.controller.refresh()
            assertEquals(AccountState.Loading(AccountOperation.Refresh), setup.state)
            setup.adapter.refreshes.last().complete(result)
            assertEquals(expected, setup.state)
            assertFalse(setup.state is AccountState.Authenticated)
        }
    }

    @Test fun repeatedRefreshAndRetryAreSingleFlightUntilCompletion() {
        val setup = Setup()
        repeat(5) { setup.controller.refresh() }
        assertEquals(1, setup.adapter.refreshes.size)
        setup.adapter.refreshes.single().complete(AccountRefreshResult.Failed(AccountErrorReason.Network))
        repeat(5) { setup.controller.refresh() }
        assertEquals(2, setup.adapter.refreshes.size)
        setup.adapter.refreshes.last().complete(AccountRefreshResult.SignedOut)
        assertEquals(AccountState.SignedOut, setup.state)
    }

    @Test fun everyFreshReadCreatesItsOwnImmutablePresentationIdentity() {
        val setup = Setup(); val source = identity(); setup.authenticate(source)
        val first = (setup.state as AccountState.Authenticated).identity
        assertNotSame(source, first)
        setup.authenticate(source)
        val second = (setup.state as AccountState.Authenticated).identity
        assertEquals(first.accountId, second.accountId)
        assertEquals(first.profile, second.profile)
        assertFalse(first == second)
        assertNotSame(first, second)
        assertNotSame(source, second)
    }

    @Test fun sameValueSynchronousRefreshIsObservableWhenSlowCollectorMissesLoading() = runBlocking {
        val setup = Setup(); val source = identity(); setup.authenticate(source)
        val initial = setup.state as AccountState.Authenticated
        val observed = mutableListOf<AccountState>()
        val resume = CompletableDeferred<Unit>()
        val observer = launch(start = CoroutineStart.UNDISPATCHED) {
            setup.controller.state.collect {
                observed += it
                if (observed.size == 1) resume.await()
            }
        }
        // Deliberately miss Loading: the adapter completes before this collector can resume.
        setup.adapter.immediateRefresh = AccountRefreshResult.Authenticated(source)
        setup.controller.refresh()
        val refreshed = setup.state as AccountState.Authenticated
        assertEquals(initial.identity.accountId, refreshed.identity.accountId)
        assertEquals(initial.identity.profile, refreshed.identity.profile)
        resume.complete(Unit)
        yield()
        assertEquals(2, observed.size)
        assertFalse(initial == refreshed)
        assertTrue(observed.none { it is AccountState.Loading })
        assertTrue(observed.last() === refreshed)
        assertNotSame(initial.identity, (observed.last() as AccountState.Authenticated).identity)
        observer.cancel()
    }

    @Test fun duplicateCompletionCannotOverwriteAcceptedStateOrRetireNewWork() {
        val setup = Setup(); setup.controller.refresh()
        val old = setup.adapter.refreshes.single()
        old.complete(AccountRefreshResult.SignedOut)
        old.complete(AccountRefreshResult.Authenticated(identity()))
        assertEquals(AccountState.SignedOut, setup.state)
        setup.controller.refresh()
        old.complete(AccountRefreshResult.Expired)
        assertEquals(AccountState.Loading(AccountOperation.Refresh), setup.state)
        setup.adapter.refreshes.last().complete(AccountRefreshResult.Authenticated(identity()))
        assertTrue(setup.state is AccountState.Authenticated)
    }

    @Test fun expiryRetiresPendingWorkAndLatePrivateReplyCannotRestoreIdentity() {
        val setup = Setup(); setup.authenticate(); setup.controller.refresh()
        val old = setup.adapter.refreshes.last()
        setup.controller.invalidate(AccountInvalidation.Expired)
        assertEquals(AccountState.Expired, setup.state)
        assertEquals(old.request, setup.adapter.cancellations.single())
        old.complete(AccountRefreshResult.Authenticated(identity()))
        assertEquals(AccountState.Expired, setup.state)
    }

    @Test fun verifiedSignOutAndAccountChangeRetireEveryOldCompletion() {
        for ((reason, expected) in listOf(AccountInvalidation.SignedOut to AccountState.SignedOut, AccountInvalidation.AccountChanged to AccountState.Unknown)) {
            val setup = Setup(); setup.authenticate(); setup.controller.refresh()
            val old = setup.adapter.refreshes.last()
            setup.controller.invalidate(reason)
            old.complete(AccountRefreshResult.Authenticated(identity()))
            assertEquals(expected, setup.state)
            setup.controller.refresh()
            old.complete(AccountRefreshResult.SignedOut)
            assertEquals(AccountState.Loading(AccountOperation.Refresh), setup.state)
            setup.adapter.refreshes.last().complete(AccountRefreshResult.Authenticated(identity(2)))
            assertEquals(identity(2).accountId, (setup.state as AccountState.Authenticated).identity.accountId)
        }
    }

    @Test fun wrongSubjectInSameAccountRecheckFailsClosedUntilNewContextIsRead() {
        val setup = Setup(); setup.authenticate(); setup.controller.refresh()
        setup.adapter.refreshes.last().complete(AccountRefreshResult.Authenticated(identity(2)))
        assertEquals(AccountState.Error(AccountErrorReason.InvalidResponse, AccountOperation.Refresh), setup.state)
        assertFalse(setup.state is AccountState.Authenticated)
    }

    @Test fun backgroundMasksCurrentProfileAndRequiresExplicitFreshRead() {
        val setup = Setup(); setup.authenticate(); setup.controller.refresh()
        val old = setup.adapter.refreshes.last(); val before = setup.adapter.refreshes.size
        setup.controller.onForegroundChanged(false)
        assertEquals(AccountState.Unknown, setup.state)
        setup.controller.refresh(); setup.controller.signOut()
        old.complete(AccountRefreshResult.Authenticated(identity()))
        assertEquals(before, setup.adapter.refreshes.size)
        assertTrue(setup.adapter.signOuts.isEmpty())
        setup.controller.onForegroundChanged(true)
        assertEquals(AccountState.Unknown, setup.state)
        assertEquals(before, setup.adapter.refreshes.size)
        setup.controller.refresh()
        old.complete(AccountRefreshResult.Authenticated(identity()))
        assertEquals(AccountState.Loading(AccountOperation.Refresh), setup.state)
        setup.adapter.refreshes.last().complete(AccountRefreshResult.Authenticated(identity(2)))
        assertEquals(identity(2).accountId, (setup.state as AccountState.Authenticated).identity.accountId)
    }

    @Test fun unsupportedSignOutHasNoEffectOrFabricatedSuccess() {
        val setup = Setup(supportsSignOut = false); setup.authenticate()
        val before = setup.state
        assertFalse((before as AccountState.Authenticated).canSignOut)
        repeat(5) { setup.controller.signOut() }
        assertEquals(before, setup.state)
        assertTrue(setup.adapter.signOuts.isEmpty())
    }

    @Test fun signOutMasksAndSupersedesRefreshThenOnlyConfirmedResultPublishesSignedOut() {
        val setup = Setup(); setup.authenticate(); setup.controller.refresh()
        val old = setup.adapter.refreshes.last()
        setup.controller.signOut()
        assertEquals(AccountState.Loading(AccountOperation.SignOut), setup.state)
        assertEquals(old.request, setup.adapter.cancellations.single())
        assertEquals(identity().accountId, setup.adapter.signOuts.single().accountId)
        old.complete(AccountRefreshResult.Authenticated(identity()))
        assertEquals(AccountState.Loading(AccountOperation.SignOut), setup.state)
        repeat(5) { setup.controller.signOut(); setup.controller.refresh() }
        assertEquals(1, setup.adapter.signOuts.size)
        setup.adapter.signOuts.single().complete(AccountSignOutResult.Confirmed)
        assertEquals(AccountState.SignedOut, setup.state)
        old.complete(AccountRefreshResult.Authenticated(identity()))
        assertEquals(AccountState.SignedOut, setup.state)
    }

    @Test fun unconfirmedSignOutKeepsPrivateDataMaskedAndAllowsOnlySingleFlightSignOutRetry() {
        val setup = Setup(); setup.authenticate(); setup.controller.signOut()
        setup.adapter.signOuts.single().complete(AccountSignOutResult.Unconfirmed)
        assertEquals(unconfirmed(), setup.state)
        val before = setup.adapter.refreshes.size
        setup.controller.refresh()
        setup.controller.invalidate(AccountInvalidation.AccountChanged)
        setup.controller.onForegroundChanged(false); setup.controller.onForegroundChanged(true)
        setup.controller.refresh()
        assertEquals(before, setup.adapter.refreshes.size)
        assertEquals(unconfirmed(), setup.state)
        repeat(5) { setup.controller.signOut() }
        assertEquals(2, setup.adapter.signOuts.size)
        setup.adapter.signOuts.last().complete(AccountSignOutResult.Confirmed)
        assertEquals(AccountState.SignedOut, setup.state)
        setup.controller.refresh()
        assertEquals(before + 1, setup.adapter.refreshes.size)
    }

    @Test fun verifiedExpiredOrSignedOutAuthorityCanResolveSignOutSuppression() {
        for ((reason, expected) in listOf(AccountInvalidation.Expired to AccountState.Expired, AccountInvalidation.SignedOut to AccountState.SignedOut)) {
            val setup = Setup(); setup.authenticate(); setup.controller.signOut()
            val old = setup.adapter.signOuts.last()
            setup.controller.invalidate(reason)
            old.complete(AccountSignOutResult.Unconfirmed)
            assertEquals(expected, setup.state)
            setup.controller.refresh()
            assertEquals(2, setup.adapter.refreshes.size)
        }
    }

    @Test fun cancelledRefreshRetiresItsCallbackAndDoesNotRestorePreviousProfile() {
        val setup = Setup(); setup.authenticate(); setup.controller.refresh()
        val old = setup.adapter.refreshes.last()
        setup.controller.cancelPending(); setup.controller.cancelPending()
        assertEquals(AccountState.Unknown, setup.state)
        assertEquals(listOf(old.request), setup.adapter.cancellations)
        old.complete(AccountRefreshResult.Authenticated(identity()))
        assertEquals(AccountState.Unknown, setup.state)
        setup.controller.refresh()
        assertEquals(3, setup.adapter.refreshes.size)
    }

    @Test fun cancelDoesNotClaimDispatchedSignOutWasUndoneOrConfirmed() {
        val setup = Setup(); setup.authenticate(); setup.controller.signOut()
        val old = setup.adapter.signOuts.last()
        setup.controller.cancelPending()
        assertEquals(unconfirmed(), setup.state)
        old.complete(AccountSignOutResult.Confirmed)
        assertEquals(unconfirmed(), setup.state)
        setup.controller.refresh()
        assertEquals(1, setup.adapter.refreshes.size)
        setup.controller.signOut()
        assertEquals(old.accountId, setup.adapter.signOuts.last().accountId)
        old.complete(AccountSignOutResult.Confirmed)
        assertEquals(AccountState.Loading(AccountOperation.SignOut), setup.state)
        setup.adapter.signOuts.last().complete(AccountSignOutResult.Confirmed)
        assertEquals(AccountState.SignedOut, setup.state)
    }

    @Test fun signOutReplyAfterBackgroundOrAccountChangeCannotOverrideMaskedState() {
        for (background in listOf(true, false)) {
            val setup = Setup(); setup.authenticate(); setup.controller.signOut()
            val old = setup.adapter.signOuts.last()
            if (background) setup.controller.onForegroundChanged(false) else setup.controller.invalidate(AccountInvalidation.AccountChanged)
            old.complete(AccountSignOutResult.Confirmed)
            assertEquals(unconfirmed(), setup.state)
            if (background) setup.controller.onForegroundChanged(true)
            setup.controller.signOut()
            old.complete(AccountSignOutResult.Unconfirmed)
            assertEquals(AccountState.Loading(AccountOperation.SignOut), setup.state)
            setup.adapter.signOuts.last().complete(AccountSignOutResult.Confirmed)
            assertEquals(AccountState.SignedOut, setup.state)
        }
    }

    @Test fun synchronousCancelCallbackIsAlreadyRetiredBeforeAdapterCleanup() {
        val setup = Setup(); setup.controller.refresh()
        val old = setup.adapter.refreshes.last()
        setup.adapter.onCancel = { old.complete(AccountRefreshResult.Authenticated(identity())) }
        setup.controller.invalidate(AccountInvalidation.Expired)
        assertEquals(AccountState.Expired, setup.state)
    }

    @Test fun stateCollectorCancellationCanRetireRefreshBeforeItIsDispatched() = runBlocking {
        val setup = Setup()
        val observer = launch(Dispatchers.Unconfined) {
            setup.controller.state.collect { if (it is AccountState.Loading) setup.controller.cancelPending() }
        }
        setup.controller.refresh()
        assertEquals(AccountState.Unknown, setup.state)
        assertTrue(setup.adapter.refreshes.isEmpty())
        observer.cancel()
    }

    @Test fun stateCollectorRepeatedSignOutCannotReenterAndDispatchTwoMutations() = runBlocking {
        val setup = Setup(); setup.authenticate()
        val observer = launch(Dispatchers.Unconfined) {
            setup.controller.state.collect { if (it == AccountState.Loading(AccountOperation.SignOut)) setup.controller.signOut() }
        }
        setup.controller.signOut()
        assertEquals(1, setup.adapter.signOuts.size)
        setup.adapter.signOuts.single().complete(AccountSignOutResult.Confirmed)
        assertEquals(AccountState.SignedOut, setup.state)
        observer.cancel()
    }

    @Test fun throwingAdapterMapsToTypedErrorsWithoutPrivateExceptionText() {
        val setup = Setup(); setup.adapter.throwRefresh = true; setup.controller.refresh()
        assertEquals(AccountState.Error(AccountErrorReason.Service, AccountOperation.Refresh), setup.state)
        assertFalse(setup.state.toString().contains("private infrastructure"))
        setup.adapter.throwRefresh = false; setup.authenticate()
        setup.adapter.throwSignOut = true; setup.controller.signOut()
        assertEquals(unconfirmed(), setup.state)
        assertFalse(setup.state.toString().contains("private infrastructure"))
    }

    @Test fun closeIsTerminalIdempotentAndLateRepliesCannotRestorePrivateDisplay() {
        val setup = Setup(); setup.authenticate(); setup.controller.refresh()
        val old = setup.adapter.refreshes.last()
        setup.adapter.throwCleanup = true
        setup.controller.close(); setup.controller.close()
        old.complete(AccountRefreshResult.Authenticated(identity()))
        setup.controller.refresh(); setup.controller.signOut(); setup.controller.onForegroundChanged(true)
        setup.controller.invalidate(AccountInvalidation.SignedOut)
        assertEquals(AccountState.Unknown, setup.state)
        assertEquals(1, setup.adapter.closes)
        assertEquals(2, setup.adapter.refreshes.size)
        assertTrue(setup.adapter.signOuts.isEmpty())
        assertFalse(setup.state.toString().contains("private infrastructure"))
    }

    @Test fun closingPendingSignOutPreservesUncertaintyWithoutRestoringOldIdentity() {
        val setup = Setup(); setup.authenticate(); setup.controller.signOut()
        val old = setup.adapter.signOuts.last()
        setup.controller.close()
        old.complete(AccountSignOutResult.Confirmed)
        assertEquals(unconfirmed(), setup.state)
        assertEquals(1, setup.adapter.closes)
        assertFalse(setup.state is AccountState.Authenticated)
    }

    companion object {
        private fun identity(number: Int = 1): AccountIdentity {
            val id = assertNotNull(AccountId.parse(number.toString(16).padStart(32, '0')))
            val profile = assertNotNull(AccountSelfProfile.create("Synthetic Person $number", "synthetic_$number", AccountProfileVisibility.Friends))
            return AccountIdentity(id, profile)
        }
        private fun unconfirmed() = AccountState.Error(AccountErrorReason.SignOutUnconfirmed, AccountOperation.SignOut)
    }
}
