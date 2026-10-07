package com.loveoverflow.tabula.mobile.account

/**
 * Immutable Tabula account identifier, not a session credential (ADR-0031 §1).
 * Parsing checks the existing HTTP account-id shape; it does not authenticate its subject.
 */
class AccountId private constructor(val value: String) {
    override fun equals(other: Any?): Boolean = other is AccountId && value == other.value
    override fun hashCode(): Int = value.hashCode()
    override fun toString(): String = "AccountId([redacted])"

    companion object {
        /** Exact nonzero lowercase hexadecimal encoding used by the current session DTOs. */
        fun parse(value: String): AccountId? = if (
            value.length == 32 && value.any { it != '0' } &&
            value.all { it in '0'..'9' || it in 'a'..'f' }
        ) AccountId(value) else null
    }
}

/** Visibility of profile details to other accounts, matching the v2 profile DTO (ADR-0044). */
enum class AccountProfileVisibility { Public, Friends, Private }

/**
 * Permitted, read-only self-profile fields (ADR-0031 §6 / ADR-0044). No provider identity,
 * avatar URL, statistics, credentials or freshness claim is invented by this presentation value.
 */
class AccountSelfProfile private constructor(
    val displayName: String,
    val handle: String,
    val visibility: AccountProfileVisibility,
) {
    override fun equals(other: Any?): Boolean = other is AccountSelfProfile &&
        displayName == other.displayName && handle == other.handle && visibility == other.visibility

    override fun hashCode(): Int = 31 * (31 * displayName.hashCode() + handle.hashCode()) + visibility.hashCode()
    override fun toString(): String = "AccountSelfProfile([redacted])"

    companion object {
        /**
         * Validates current DTO bounds without trimming, case folding or losing Unicode accents.
         * The future adapter must additionally check contract version, current authority and
         * profile/context subject correlation before constructing an [AccountIdentity].
         */
        fun create(
            displayName: String,
            handle: String,
            visibility: AccountProfileVisibility,
        ): AccountSelfProfile? {
            if (handle.length !in 3..32 || !handle.all { it in 'a'..'z' || it in '0'..'9' || it == '_' }) return null
            if (!validDisplayName(displayName)) return null
            return AccountSelfProfile(displayName, handle, visibility)
        }

        private fun validDisplayName(value: String): Boolean {
            if (value.isEmpty() || value.length > 128 || edgeWhitespace(value.first()) || edgeWhitespace(value.last())) return false
            var index = 0
            var scalars = 0
            var bytes = 0
            while (index < value.length) {
                val character = value[index].code
                val scalar = when (character) {
                    in 0xd800..0xdbff -> {
                        if (index + 1 >= value.length || value[index + 1].code !in 0xdc00..0xdfff) return false
                        index += 1
                        0x10000 + ((character - 0xd800) shl 10) + (value[index].code - 0xdc00)
                    }
                    in 0xdc00..0xdfff -> return false
                    else -> character
                }
                if (scalar in 0..0x1f || scalar in 0x7f..0x9f || scalar == 0x2028 || scalar == 0x2029) return false
                scalars += 1
                bytes += when {
                    scalar <= 0x7f -> 1
                    scalar <= 0x7ff -> 2
                    scalar <= 0xffff -> 3
                    else -> 4
                }
                if (scalars > 64 || bytes > 256) return false
                index += 1
            }
            return true
        }

        // Unicode White_Space matches Rust str::trim in the current DTO validator. Kotlin's
        // platform-dependent Char.isWhitespace/trim can admit or reject a different boundary.
        private fun edgeWhitespace(value: Char): Boolean = value.code in 0x09..0x0d || when (value.code) {
            0x20, 0x85, 0xa0, 0x1680, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000 -> true
            in 0x2000..0x200a -> true
            else -> false
        }
    }
}

/**
 * Current adapter-confirmed, read-only self identity (ADR-0031 §6). A missing permitted profile
 * stays missing. The immutable opaque account id is distinct from a mutable display name.
 * Each admitted read is a new presentation snapshot, with reference equality by design. Value
 * equality would let StateFlow/Compose conflate a fresh snapshot when Loading was not observed,
 * retaining obsolete identity-bound metadata/image/confirmation tickets.
 */
class AccountIdentity(val accountId: AccountId, val profile: AccountSelfProfile? = null) {
    override fun toString(): String = "AccountIdentity([redacted])"
}

/** Presentation operations; Refresh revalidates current session facts, never rotates a credential. */
enum class AccountOperation { Refresh, SignOut }

/** Typed unavailability without private transport, secure-store or exception diagnostics. */
enum class AccountUnavailableReason { NativeAdapterMissing, ServiceUnavailable, SecureStorageUnavailable }

/** Safe error classes for localized UI; transport failure does not establish logout (ADR-0031 §6). */
enum class AccountErrorReason { Network, Service, InvalidResponse, SignOutUnconfirmed }

/**
 * Only Authenticated carries private display data. Loading, failure, background and invalidation
 * mask it synchronously; these presentation states never establish backend authority (ADR-0031 §6).
 */
sealed interface AccountState {
    data object Unknown : AccountState
    data class Loading(val operation: AccountOperation) : AccountState
    data object SignedOut : AccountState
    data class Authenticated(val identity: AccountIdentity, val canSignOut: Boolean) : AccountState
    data object Expired : AccountState
    data class Unavailable(val reason: AccountUnavailableReason) : AccountState
    data class Error(val reason: AccountErrorReason, val retry: AccountOperation?) : AccountState
}

/** Current verified authority changes, distinct from request cancellation or route navigation. */
enum class AccountInvalidation { Expired, SignedOut, AccountChanged }
