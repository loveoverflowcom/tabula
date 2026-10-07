package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.account.*
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Presentation boundary tests only; construction does not establish live session authority. */
class AccountStateTest {
    @Test fun accountIdRejectsNoncanonicalEmptyZeroAndMalformedValues() {
        for (value in listOf("", "0".repeat(32), "1".repeat(31), "1".repeat(33), "A".repeat(32), "g".repeat(32), " 1".repeat(16))) {
            assertNull(AccountId.parse(value))
        }
        val value = "0123456789abcdef0123456789abcdef"
        assertEquals(value, assertNotNull(AccountId.parse(value)).value)
        assertEquals(AccountId.parse(value), AccountId.parse(value))
        assertEquals(AccountId.parse(value).hashCode(), AccountId.parse(value).hashCode())
    }

    @Test fun immutableOpaqueIdentityIsSeparateFromPermittedDisplayFields() {
        val id = assertNotNull(AccountId.parse("00000000000000000000000000000001"))
        val profile = assertNotNull(AccountSelfProfile.create("Nguyễn An", "nguyen_an", AccountProfileVisibility.Friends))
        val identity = AccountIdentity(id, profile)
        assertEquals(id, identity.accountId)
        assertEquals("Nguyễn An", identity.profile?.displayName)
        assertEquals("nguyen_an", identity.profile?.handle)
        assertEquals(AccountProfileVisibility.Friends, identity.profile?.visibility)
        assertNull(AccountIdentity(id).profile)
        assertFalse(identity == AccountIdentity(id, profile))
    }

    @Test fun handleBoundsAreExactLowercaseAsciiAndNeverNormalized() {
        for (handle in listOf("abc", "_12", "a".repeat(32))) assertNotNull(profile(handle = handle))
        for (handle in listOf("", "ab", "Alice", " an ", "nguyễn", "a".repeat(33), "abc-def", "abc\n")) {
            assertNull(profile(handle = handle))
        }
    }

    @Test fun displayNameUnicodeScalarAndUtf8BoundariesMatchExistingDto() {
        for (name in listOf("Nguyễn An", "e\u0301", "🦊".repeat(64), "a".repeat(64), "\u200dA\u200d", "\ufeffA\ufeff")) {
            assertEquals(name, assertNotNull(profile(name)).displayName)
        }
        for (name in listOf("", "a".repeat(65), "🦊".repeat(65), "\ud800", "\udc00", "a\ud800b", "a\udc00b")) {
            assertNull(profile(name))
        }
    }

    @Test fun displayNameControlsAndExactUnicodeEdgeWhitespaceAreRejected() {
        for (control in listOf('\u0000', '\u001f', '\u007f', '\u0085', '\u009f', '\u2028', '\u2029')) {
            assertNull(profile("a${control}b"))
        }
        for (space in listOf('\u0009', '\u0020', '\u00a0', '\u1680', '\u2000', '\u200a', '\u202f', '\u205f', '\u3000')) {
            assertNull(profile("${space}An"))
            assertNull(profile("An${space}"))
        }
        assertNotNull(profile("An\u00a0Nguyễn"))
    }

    @Test fun allPrivatePresentationValuesRedactDiagnostics() {
        val id = assertNotNull(AccountId.parse("0123456789abcdef0123456789abcdef"))
        val profile = assertNotNull(profile("Private Person", "private_handle"))
        val identity = AccountIdentity(id, profile)
        for (value in listOf(id, profile, identity, AccountState.Authenticated(identity, true), AccountRefreshResult.Authenticated(identity))) {
            val diagnostic = value.toString()
            assertFalse(diagnostic.contains(id.value))
            assertFalse(diagnostic.contains(profile.displayName))
            assertFalse(diagnostic.contains(profile.handle))
            assertTrue(diagnostic.contains("redacted"))
        }
    }

    private fun profile(name: String = "Nguyễn An", handle: String = "nguyen_an") =
        AccountSelfProfile.create(name, handle, AccountProfileVisibility.Private)
}
