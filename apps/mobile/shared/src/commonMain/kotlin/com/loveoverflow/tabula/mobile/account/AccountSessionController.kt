package com.loveoverflow.tabula.mobile.account

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Generation-fenced account presentation coordinator (ADR-0031 §6). Opaque request identity
 * prevents old/duplicate completions from publishing data without an overflowing sequence counter.
 * No Compose, clock, platform I/O, persistence or credential is owned here. The app/native adapter
 * must serialize every entry and callback on its owner thread; StateFlow alone is not that lock.
 */
class AccountSessionController(private val adapter: AccountSessionAdapter) : AccountSessionPort {
    private val currentState = MutableStateFlow<AccountState>(AccountState.Unknown)
    override val state: StateFlow<AccountState> = currentState.asStateFlow()
    private var pending: AccountSessionRequest? = null
    private var subject: AccountId? = null
    private var signOutIntent: AccountId? = null
    private var foreground = true
    private var closed = false

    override fun refresh() {
        if (closed || !foreground || pending != null || signOutIntent != null) return
        val expectedSubject = subject
        val request = AccountSessionRequest(AccountOperation.Refresh)
        pending = request
        currentState.value = AccountState.Loading(AccountOperation.Refresh)
        // A synchronous collector may invalidate/cancel while the state is published.
        if (!accepts(request)) return
        try {
            adapter.refresh(request) { result -> completeRefresh(request, expectedSubject, result) }
        } catch (_: Exception) {
            completeRefresh(request, expectedSubject, AccountRefreshResult.Failed(AccountErrorReason.Service))
        }
    }

    override fun signOut() {
        if (closed || !foreground || !adapter.supportsSignOut) return
        if (pending?.operation == AccountOperation.SignOut) return
        val target = signOutIntent ?: subject ?: return
        // Sign-out may supersede a profile/context recheck, fencing that reply before dispatch.
        val retired = pending
        subject = null
        signOutIntent = target
        val request = AccountSessionRequest(AccountOperation.SignOut)
        pending = request
        currentState.value = AccountState.Loading(AccountOperation.SignOut)
        cancelAdapter(retired)
        if (!accepts(request)) return
        try {
            adapter.signOut(request, target) { result -> completeSignOut(request, result) }
        } catch (_: Exception) {
            completeSignOut(request, AccountSignOutResult.Unconfirmed)
        }
    }

    override fun cancelPending() {
        if (closed || pending == null) return
        retire(maskedState())
    }

    override fun invalidate(reason: AccountInvalidation) {
        if (closed) return
        subject = null
        when (reason) {
            AccountInvalidation.Expired -> {
                signOutIntent = null
                retire(AccountState.Expired)
            }
            AccountInvalidation.SignedOut -> {
                signOutIntent = null
                retire(AccountState.SignedOut)
            }
            AccountInvalidation.AccountChanged -> retire(maskedState())
        }
    }

    override fun onForegroundChanged(inForeground: Boolean) {
        if (closed || foreground == inForeground) return
        foreground = inForeground
        if (!inForeground) retire(maskedState())
        // No automatic restore, login, request or operation retry on foreground.
    }

    override fun close() {
        if (closed) return
        closed = true
        foreground = false
        retire(maskedState())
        // A throwing adapter cannot disclose an exception or reopen this local display fence.
        try { adapter.close() } catch (_: Exception) { }
    }

    private fun completeRefresh(
        request: AccountSessionRequest,
        expectedSubject: AccountId?,
        result: AccountRefreshResult,
    ) {
        if (!accepts(request)) return
        pending = null
        subject = null
        currentState.value = when (result) {
            is AccountRefreshResult.Authenticated -> {
                if (expectedSubject != null && result.identity.accountId != expectedSubject) {
                    AccountState.Error(AccountErrorReason.InvalidResponse, AccountOperation.Refresh)
                } else {
                    subject = result.identity.accountId
                    // A freshly admitted read gets its own presentation identity even when an
                    // adapter reuses an immutable value. Metadata/image owners can bind this
                    // exact snapshot and retire tickets from the previous display activation.
                    val identity = AccountIdentity(result.identity.accountId, result.identity.profile)
                    AccountState.Authenticated(identity, adapter.supportsSignOut)
                }
            }
            AccountRefreshResult.SignedOut -> AccountState.SignedOut
            AccountRefreshResult.Expired -> AccountState.Expired
            is AccountRefreshResult.Unavailable -> AccountState.Unavailable(result.reason)
            is AccountRefreshResult.Failed -> AccountState.Error(result.reason, AccountOperation.Refresh)
        }
    }

    private fun completeSignOut(request: AccountSessionRequest, result: AccountSignOutResult) {
        if (!accepts(request)) return
        pending = null
        subject = null
        currentState.value = when (result) {
            AccountSignOutResult.Confirmed -> {
                signOutIntent = null
                AccountState.SignedOut
            }
            AccountSignOutResult.Unconfirmed -> maskedState()
        }
    }

    private fun accepts(request: AccountSessionRequest): Boolean = !closed && foreground && pending === request

    private fun maskedState(): AccountState = if (signOutIntent != null) {
        AccountState.Error(AccountErrorReason.SignOutUnconfirmed, AccountOperation.SignOut)
    } else AccountState.Unknown

    private fun retire(next: AccountState) {
        val retired = pending
        pending = null
        subject = null
        currentState.value = next
        // Retire before invoking cancel: even a synchronous cancellation completion is stale.
        cancelAdapter(retired)
    }

    private fun cancelAdapter(request: AccountSessionRequest?) {
        if (request == null) return
        try { adapter.cancel(request) } catch (_: Exception) { }
    }
}
