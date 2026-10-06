package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.bridge.Json
import com.loveoverflow.tabula.mobile.bridge.StrictJson
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class StrictJsonTest {
    private fun accepts(text: String) = StrictJson.parse(text)

    @Test
    fun readsTheShapesTheBridgeUses() {
        assertEquals(Json.Num(-12), accepts("-12"))
        assertEquals(Json.Num(0), accepts(" 0 "))
        assertEquals(Json.Str("a\"b\\c\u00e9"), accepts("\"a\\\"b\\\\c\\u00e9\""))
        assertEquals(Json.Str("😀"), accepts("\"😀\""))
        assertEquals(Json.Arr(listOf(Json.Bool(true), Json.Null)), accepts("[true,null]"))
        assertEquals(Json.Obj(mapOf("k" to Json.Num(1))), accepts("{\"k\":1}"))
    }

    @Test
    fun rejectsEverythingTheGrammarForbids() {
        val rejected = listOf(
            "", "{", "{\"a\":1,}", "[1,]", "{\"a\":1}{", "{\"a\":1} x", "nul", "True",
            "01", "-", "1.5", "1e3", "1E3", "+1", ".5", "9999999999999999",
            "\"unterminated", "\"tab\tinside\"", "\"bad\\q\"", "\"\\u12\"", "\"\\ud800\"", "\"\\udc00x\"",
            "{\"a\":1,\"a\":2}", "{1:2}", "{\"a\" 1}",
            "\"\ud800\"", "\"\udc00\"",
        )
        for (text in rejected) assertNull(accepts(text), "should reject <$text>")
    }

    @Test
    fun nestingIsBounded() {
        val ok = "[".repeat(StrictJson.MAX_DEPTH) + "]".repeat(StrictJson.MAX_DEPTH)
        val deep = "[".repeat(StrictJson.MAX_DEPTH + 2) + "]".repeat(StrictJson.MAX_DEPTH + 2)
        assertEquals(true, accepts(ok) != null)
        assertNull(accepts(deep))
        // A pathological depth must be rejected, not overflow the stack.
        assertNull(accepts("[".repeat(100_000)))
    }
}
