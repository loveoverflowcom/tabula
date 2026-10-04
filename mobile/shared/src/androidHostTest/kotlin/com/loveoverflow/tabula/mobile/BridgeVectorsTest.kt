package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.bridge.BridgeCodec
import com.loveoverflow.tabula.mobile.bridge.FailureCode
import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.bridge.HostMessage
import com.loveoverflow.tabula.mobile.bridge.Json
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.PageMessage
import com.loveoverflow.tabula.mobile.bridge.ReplyCode
import com.loveoverflow.tabula.mobile.bridge.StrictJson
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * Runs the same wire vectors the page's `host-bridge.js` runs (`tests/bridge-vectors.json`), so the
 * two implementations of the ADR-0033 grammar cannot drift apart without a test failing.
 */
class BridgeVectorsTest {
    private val vectors: Map<String, Json> = run {
        // Gradle runs host tests with the module directory (mobile/shared) as the working directory.
        val file = File("../../apps/game-client/web/tests/bridge-vectors.json")
        assertTrue(file.isFile, "bridge vectors not found at ${file.absoluteFile}")
        // The fixture nests deeper than a wire message may, so it is read with a raised bound.
        (StrictJson.parse(file.readText(), maxDepth = 16) as Json.Obj).fields
    }

    private fun entries(direction: String) = (vectors.getValue(direction) as Json.Arr).items.map { (it as Json.Obj).fields }

    private fun Map<String, Json>.text() = (getValue("text") as Json.Str).value
    private fun Map<String, Json>.name() = (getValue("name") as Json.Str).value
    private fun Map<String, Json>.accept() = (getValue("accept") as Json.Bool).value
    private fun Map<String, Json>.message() = (getValue("message") as Json.Obj).fields

    private fun Map<String, Json>.str(key: String) = (getValue(key) as Json.Str).value
    private fun Map<String, Json>.int(key: String) = (getValue(key) as Json.Num).value.toInt()

    private fun expectedPage(m: Map<String, Json>): PageMessage = when (m.str("type")) {
        "hello" -> PageMessage.Hello
        "ready" -> PageMessage.Ready(m.int("gen"), m.int("bootMs"))
        "failed" -> PageMessage.Failed(m.int("gen"), FailureCode.entries.first { it.wire == m.str("code") }, m.str("detail"))
        "exit" -> PageMessage.Exit(m.int("gen"))
        "service" -> PageMessage.Service(m.int("gen"), m.int("id"), HostCapability.KeepAwake, (m.getValue("enabled") as Json.Bool).value)
        else -> error("unknown vector type")
    }

    private fun expectedHost(m: Map<String, Json>): HostMessage = when (m.str("type")) {
        "init" -> {
            val p = (m.getValue("preferences") as Json.Obj).fields
            HostMessage.Init(
                m.int("gen"),
                (m.getValue("capabilities") as Json.Arr).items.map { c -> HostCapability.entries.first { it.wire == (c as Json.Str).value } }.toSet(),
                GamePreferences(
                    ThemePreference.entries.first { it.wire == p.str("theme") },
                    MotionPreference.entries.first { it.wire == p.str("motion") },
                    LocalePreference.entries.first { it.wire == p.str("locale") },
                ),
            )
        }
        "suspend" -> HostMessage.Suspend(m.int("gen"))
        "resume" -> HostMessage.Resume(m.int("gen"))
        "dispose" -> HostMessage.Dispose(m.int("gen"))
        "back-requested" -> HostMessage.BackRequested(m.int("gen"))
        "reply" -> HostMessage.Reply(
            m.int("gen"), m.int("id"), (m.getValue("ok") as Json.Bool).value,
            ReplyCode.entries.first { it.wire == m.str("code") },
        )
        else -> error("unknown vector type")
    }

    @Test
    fun everyPageToHostVectorDecodesToItsTypedMessageOrIsRejected() {
        val all = entries("pageToHost")
        assertTrue(all.size >= 25, "vector file unexpectedly small")
        for (vector in all) {
            val decoded = BridgeCodec.decodePage(vector.text())
            if (vector.accept()) assertEquals(expectedPage(vector.message()), decoded, vector.name())
            else assertNull(decoded, vector.name())
        }
    }

    @Test
    fun everyHostToPageVectorIsProducedByteForByteByTheEncoder() {
        val accepted = entries("hostToPage").filter { it.accept() }
        assertTrue(accepted.size >= 8)
        for (vector in accepted) assertEquals(vector.text(), BridgeCodec.encode(expectedHost(vector.message())), vector.name())
    }

    @Test
    fun oversizedPageTextIsRejected() {
        val padded = "{\"v\":1,\"type\":\"failed\",\"gen\":1,\"code\":\"runtime\",\"detail\":\"${"x".repeat(BridgeCodec.MAX_BYTES)}\"}"
        assertNull(BridgeCodec.decodePage(padded))
        assertNotNull(BridgeCodec.decodePage("{\"v\":1,\"type\":\"hello\"}"))
    }
}
