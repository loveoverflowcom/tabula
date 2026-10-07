package com.loveoverflow.tabula.mobile.account

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * App-owned account presentation seam (ADR-0031 §6). No password, credential, raw URL, callback
 * payload, persistence or social operation crosses it. Native auth/network adapters remain gated.
 * Calls and callbacks are serialized on the platform owner thread, outside recomposition.
 */
interface AccountSessionPort {
    val state: StateFlow<AccountState>

    /** Explicit current-authority revalidation; no credential rotation or automatic sign-in. */
    fun refresh()

    /** Supported only for an adapter-confirmed account and its matching sign-out capability. */
    fun signOut()

    /** Retires route-owned work. Cancelling a dispatched logout cannot undo server effects. */
    fun cancelPending()

    /** The authority/lifecycle owner clears old-account display before publishing its new state. */
    fun invalidate(reason: AccountInvalidation)

    /** Background masks identity immediately; foreground requires fresh explicit revalidation. */
    fun onForegroundChanged(inForeground: Boolean)

    /** Idempotent terminal disposal; late replies can never restore private display. */
    fun close()
}

/** Production default: no adapter, request, credential or fabricated successful sign-in. */
object UnavailableAccountSessionPort : AccountSessionPort {
    private val unavailable = MutableStateFlow<AccountState>(AccountState.Unavailable(AccountUnavailableReason.NativeAdapterMissing))
    override val state: StateFlow<AccountState> = unavailable.asStateFlow()
    override fun refresh() = Unit
    override fun signOut() = Unit
    override fun cancelPending() = Unit
    override fun invalidate(reason: AccountInvalidation) = Unit
    override fun onForegroundChanged(inForeground: Boolean) = Unit
    override fun close() = Unit
}

/** Non-authorizing, process-local correlation ticket for one request. Identity is never logged. */
class AccountSessionRequest internal constructor(val operation: AccountOperation) {
    override fun toString(): String = "AccountSessionRequest(operation=$operation)"
}

/** Current verified presentation facts supplied by an eventual adapter; no transport DTOs. */
sealed interface AccountRefreshResult {
    data class Authenticated(val identity: AccountIdentity) : AccountRefreshResult
    data object SignedOut : AccountRefreshResult
    data object Expired : AccountRefreshResult
    data class Unavailable(val reason: AccountUnavailableReason) : AccountRefreshResult
    data class Failed(val reason: AccountErrorReason) : AccountRefreshResult
}

/** Confirmed means current-device durable revocation, not global logout (ADR-0031 §5/§6). */
sealed interface AccountSignOutResult {
    data object Confirmed : AccountSignOutResult
    data object Unconfirmed : AccountSignOutResult
}

/**
 * Narrow future native adapter seam. There is no shipping implementation in this UI draft.
 * The adapter owns trusted origin, secure-store/credential handling, current authority and
 * self-profile subject/version validation. System-browser authorization and verified deep-link
 * callback handling must be implemented in that platform boundary, never accepted as UI URLs.
 * Before any later bootstrap exposes an account, that adapter must reconcile unresolved logout
 * suppression/revocation across its own lifetime. This draft's in-memory controller fence is not
 * persistent suppression, secure-store deletion or a process-restart recovery implementation.
 * All completions are marshalled to the controller's owner thread. Cancel is best-effort resource
 * cleanup, never proof that a dispatched mutation was undone. Close releases adapter resources.
 */
interface AccountSessionAdapter {
    val supportsSignOut: Boolean
    fun refresh(request: AccountSessionRequest, complete: (AccountRefreshResult) -> Unit)
    fun signOut(request: AccountSessionRequest, accountId: AccountId, complete: (AccountSignOutResult) -> Unit)
    fun cancel(request: AccountSessionRequest)
    fun close()
}
