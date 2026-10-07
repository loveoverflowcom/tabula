package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.localization.AccountCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.account
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class AccountStringsTest {
    @Test
    fun everyAccountStateTaskAndRecoveryHasCopyInBothLanguages() {
        for (language in listOf("en", "vi")) {
            val strings = ShellStrings.forLanguage(language)
            for (copy in AccountCopy.entries) {
                assertTrue(strings.account(copy).isNotBlank(), "$language: $copy")
                assertFalse(strings.account(copy).contains("AccountCopy."), "an internal enum reference cannot become displayed copy")
            }
        }
    }

    @Test
    fun regionalLocalesReuseAccountVocabularyAndUnsupportedLocalesUseEnglish() {
        val english = ShellStrings.forLanguage("en")
        val vietnamese = ShellStrings.forLanguage("vi")
        for (copy in AccountCopy.entries) {
            assertEquals(vietnamese.account(copy), ShellStrings.forLanguage("VI-vn").account(copy))
            assertEquals(english.account(copy), ShellStrings.forLanguage("fr-FR").account(copy))
        }
        assertEquals("Đăng nhập", vietnamese.account(AccountCopy.Login))
        assertEquals("Tạo tài khoản", vietnamese.account(AccountCopy.Register))
        assertEquals("Bạn bè", vietnamese.account(AccountCopy.Friends))
    }

    @Test
    fun unknownAndLoadingHaveDistinctDescriptionsAndMobileUnavailabilityIsScoped() {
        for (language in listOf("en", "vi")) {
            val strings = ShellStrings.forLanguage(language)
            assertFalse(strings.account(AccountCopy.Checking) == strings.account(AccountCopy.Loading))
            assertFalse(strings.account(AccountCopy.SignOutUnconfirmedBody) == strings.account(AccountCopy.SignedOutBody))
            for (copy in listOf(AccountCopy.NativeUnavailableBody, AccountCopy.LoginUnavailableBody,
                AccountCopy.RegisterUnavailableBody, AccountCopy.FriendsUnavailableBody, AccountCopy.OtherProfilesUnavailable)) {
                assertTrue(strings.account(copy).lowercase().contains(if (language == "vi") "bản dựng" else "build"), "$language: $copy")
            }
        }
    }
}
