package com.loveoverflow.tabula.mobile.preview

import com.loveoverflow.tabula.mobile.account.AccountErrorReason
import com.loveoverflow.tabula.mobile.account.AccountId
import com.loveoverflow.tabula.mobile.account.AccountIdentity
import com.loveoverflow.tabula.mobile.account.AccountProfileVisibility
import com.loveoverflow.tabula.mobile.account.AccountRefreshResult
import com.loveoverflow.tabula.mobile.account.AccountSelfProfile
import com.loveoverflow.tabula.mobile.account.AccountSessionAdapter
import com.loveoverflow.tabula.mobile.account.AccountSessionController
import com.loveoverflow.tabula.mobile.account.AccountSessionRequest
import com.loveoverflow.tabula.mobile.account.AccountSignOutResult
import com.loveoverflow.tabula.mobile.account.AccountState
import com.loveoverflow.tabula.mobile.account.AccountUnavailableReason

/**
 * Explicit desktop-only display scenarios. These synthetic facts have no connection to a real
 * provider, stored session, credentials, native HTTP or production account composition (ADR-0036).
 */
internal enum class SyntheticPreviewAccountScenario(val option: String) {
    Unknown("unknown"),
    SignedOut("signed-out"),
    Loading("loading"),
    Authenticated("authenticated"),
    Expired("expired"),
    Unavailable("unavailable"),
    Error("error");

    companion object {
        fun parse(option: String): SyntheticPreviewAccountScenario = entries.firstOrNull { it.option == option }
            ?: error("preview.account must be unknown, signed-out, loading, authenticated, expired, unavailable or error")
    }
}

/**
 * A labelled preview adapter exercising the real account controller's single-flight and masking
 * policy. Held completions make cancellation, stale delivery and uncertain logout reproducible;
 * they are never wired into `shared/commonMain`, Android or iOS entrypoints.
 */
internal class SyntheticPreviewAccountFixture(
    scenario: SyntheticPreviewAccountScenario,
    longFields: Boolean = false,
    vietnamese: Boolean = false,
    supportsSignOut: Boolean = true,
) : AutoCloseable {
    private val suppliedIdentity = syntheticPreviewIdentity(longFields, vietnamese)
    val adapter = SyntheticPreviewAccountAdapter(suppliedIdentity, supportsSignOut)
    val port = AccountSessionController(adapter)
    val identity: AccountIdentity

    init {
        if (scenario != SyntheticPreviewAccountScenario.Unknown) {
            port.refresh()
            when (scenario) {
                SyntheticPreviewAccountScenario.Unknown, SyntheticPreviewAccountScenario.Loading -> Unit
                SyntheticPreviewAccountScenario.SignedOut -> adapter.completeRefresh(AccountRefreshResult.SignedOut)
                SyntheticPreviewAccountScenario.Authenticated -> adapter.completeRefresh(AccountRefreshResult.Authenticated(suppliedIdentity))
                SyntheticPreviewAccountScenario.Expired -> adapter.completeRefresh(AccountRefreshResult.Expired)
                SyntheticPreviewAccountScenario.Unavailable -> adapter.completeRefresh(
                    AccountRefreshResult.Unavailable(AccountUnavailableReason.NativeAdapterMissing),
                )
                SyntheticPreviewAccountScenario.Error -> adapter.completeRefresh(AccountRefreshResult.Failed(AccountErrorReason.Network))
            }
        }
        // The controller creates a fresh display snapshot, even when an adapter reuses a value.
        // Managed preview images must be bound to that admitted snapshot, never to raw input.
        identity = (port.state.value as? AccountState.Authenticated)?.identity ?: suppliedIdentity
    }

    override fun close() = port.close()
}

/** Preview-only asynchronous boundary; retained callbacks deliberately allow stale-delivery tests. */
internal class SyntheticPreviewAccountAdapter(
    val identity: AccountIdentity,
    override val supportsSignOut: Boolean,
) : AccountSessionAdapter {
    private val refreshCompletions = mutableListOf<(AccountRefreshResult) -> Unit>()
    private val signOutCompletions = mutableListOf<(AccountSignOutResult) -> Unit>()
    val cancelledRequests = mutableListOf<AccountSessionRequest>()
    var refreshCalls = 0
        private set
    var signOutCalls = 0
        private set
    var closed = false
        private set
    /** Opt-in immediate completion tests collector conflation without any fabricated UI frame. */
    var synchronousRefresh: AccountRefreshResult? = null

    override fun refresh(request: AccountSessionRequest, complete: (AccountRefreshResult) -> Unit) {
        check(!closed)
        refreshCalls++
        refreshCompletions += complete
        synchronousRefresh?.let(complete)
    }

    override fun signOut(request: AccountSessionRequest, accountId: AccountId, complete: (AccountSignOutResult) -> Unit) {
        check(!closed)
        check(accountId == identity.accountId) { "the synthetic adapter only knows its labelled fixture account" }
        signOutCalls++
        signOutCompletions += complete
    }

    override fun cancel(request: AccountSessionRequest) { cancelledRequests += request }

    override fun close() { closed = true }

    fun completeRefresh(result: AccountRefreshResult, index: Int = refreshCompletions.lastIndex) {
        check(index in refreshCompletions.indices) { "an actual refresh request must exist before completion" }
        refreshCompletions[index](result)
    }

    fun completeSignOut(result: AccountSignOutResult, index: Int = signOutCompletions.lastIndex) {
        check(index in signOutCompletions.indices) { "an actual sign-out request must exist before completion" }
        signOutCompletions[index](result)
    }
}

/** Validated, synthetic self-profile facts; long fields remain within the real DTO limits. */
internal fun syntheticPreviewIdentity(longFields: Boolean = false, vietnamese: Boolean = false): AccountIdentity {
    val displayName = if (longFields) {
        if (vietnamese) "Hồ sơ giả lập dùng để kiểm tra tên dài và khả năng đọc dễ dàng"
        else "Synthetic preview profile with a long display name for reflow"
    } else "Synthetic preview profile"
    return AccountIdentity(
        accountId = requireNotNull(AccountId.parse("11111111111111111111111111111111")),
        profile = requireNotNull(AccountSelfProfile.create(
            displayName = displayName,
            handle = if (longFields) "synthetic_preview_long_handle_12" else "synthetic_preview",
            visibility = AccountProfileVisibility.Private,
        )),
    )
}
