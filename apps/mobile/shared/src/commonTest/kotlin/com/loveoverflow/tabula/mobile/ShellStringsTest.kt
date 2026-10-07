package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ShellStringsTest {
    @Test
    fun everyStableKeyHasNonblankCopyInBothMaintainedLanguages() {
        assertEquals(ShellCopy.entries.size, ShellCopy.entries.map { it.key }.toSet().size)
        for (tag in listOf("en", "vi")) {
            val strings = ShellStrings.forLanguage(tag)
            for (copy in ShellCopy.entries) {
                assertTrue(strings[copy].isNotBlank(), "$tag ${copy.key}")
                assertFalse(strings[copy] == copy.key, "a message key is not displayed as product copy")
            }
        }
    }

    @Test
    fun regionalVietnameseUsesVietnameseAndUnsupportedLocalesUseEnglish() {
        for (tag in listOf("vi", "vi-VN", "VI-vn", "vi_VN", " vi-VN ")) {
            val strings = ShellStrings.forLanguage(tag)
            assertEquals("vi", strings.languageTag)
            assertEquals("Trang chủ", strings[ShellCopy.Home])
            assertTrue(strings.vietnamese)
        }
        for (tag in listOf("en", "en-US", "fr-FR", "", "viral", "vi".repeat(64))) {
            val strings = ShellStrings.forLanguage(tag)
            assertEquals("en", strings.languageTag)
            assertEquals("Home", strings[ShellCopy.Home])
            assertFalse(strings.vietnamese)
        }
    }

    @Test
    fun actionsUseTheSuppliedGameNameWithoutOwningGameCopy() {
        val english = ShellStrings.forLanguage("en")
        val vietnamese = ShellStrings.forLanguage("vi")
        assertEquals("Play Example on this device", english.play("Example"))
        assertEquals("View details for Example", english.details("Example"))
        assertEquals("Set up Example", english.setup("Example"))
        assertEquals("Chơi Ví dụ trên thiết bị này", vietnamese.play("Ví dụ"))
        assertEquals("Xem chi tiết Ví dụ", vietnamese.details("Ví dụ"))
        assertEquals("Thiết lập Ví dụ", vietnamese.setup("Ví dụ"))
    }
}
