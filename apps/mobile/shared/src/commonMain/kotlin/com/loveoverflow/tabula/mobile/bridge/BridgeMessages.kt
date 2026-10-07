package com.loveoverflow.tabula.mobile.bridge

/**
 * The typed control channel between the Compose shell's [com.loveoverflow.tabula.mobile.host.GameHost]
 * and the game document (ADR-0033). It carries lifecycle, preferences and capability-gated service
 * requests, never canonical state, projections, render commands or credentials (I-5/I-6/I-10).
 *
 * Wire form: one JSON object per message, at most [BridgeCodec.MAX_BYTES] UTF-8 bytes, protocol
 * version 1. The authoritative vectors are `apps/game-client/web/tests/bridge-vectors.json`; the
 * page's `host-bridge.js` and this codec both execute them.
 */

/** A service the host may perform for the page. A capability absent from a launch is denied. */
enum class HostCapability(val wire: String) {
    /** Keep the screen on while a match is in progress. Needs no OS permission. */
    KeepAwake("keep-awake"),
}

/** Why the page reports it cannot continue. Free text is a bounded diagnostic, never rules. */
enum class FailureCode(val wire: String) {
    Runtime("runtime"),
    GraphicsContext("graphics-context"),
    Timeout("timeout"),
    Bridge("bridge"),
}

enum class ThemePreference(val wire: String) {
    System("system"), Light("light"), Dark("dark"), HighContrastLight("hc-light"), HighContrastDark("hc-dark"),
}

enum class MotionPreference(val wire: String) { System("system"), Reduced("reduced") }

enum class LocalePreference(val wire: String) { Vietnamese("vi"), English("en") }

/** Host preferences handed to the game once, at start. The board reads them at launch only. */
data class GamePreferences(
    val theme: ThemePreference,
    val motion: MotionPreference,
    val locale: LocalePreference,
)

enum class ReplyCode(val wire: String) { Ok("ok"), Denied("denied"), Unsupported("unsupported") }

/** Page → host. */
sealed interface PageMessage {
    /** First message of every document load; answers with a fresh generation. */
    data object Hello : PageMessage

    data class Ready(val generation: Int, val bootMs: Int) : PageMessage
    data class Failed(val generation: Int, val code: FailureCode, val detail: String) : PageMessage
    data class Exit(val generation: Int) : PageMessage
    data class Service(val generation: Int, val id: Int, val capability: HostCapability, val enabled: Boolean) : PageMessage
}

/** Host → page. */
sealed interface HostMessage {
    val generation: Int

    data class Init(
        override val generation: Int,
        val capabilities: Set<HostCapability>,
        val preferences: GamePreferences,
    ) : HostMessage

    data class Suspend(override val generation: Int) : HostMessage
    data class Resume(override val generation: Int) : HostMessage
    data class Dispose(override val generation: Int) : HostMessage
    data class BackRequested(override val generation: Int) : HostMessage
    data class Reply(override val generation: Int, val id: Int, val ok: Boolean, val code: ReplyCode) : HostMessage
}

object BridgeCodec {
    const val PROTOCOL = 1
    const val MAX_BYTES = 4096
    const val MAX_DETAIL_CODE_POINTS = 200
    private const val MAX_GENERATION = 2_147_483_647L
    private const val MAX_BOOT_MS = 3_600_000L

    /** Decodes untrusted page text. Anything outside the grammar yields `null`; this never throws. */
    fun decodePage(text: String): PageMessage? {
        if (text.encodeToByteArray().size > MAX_BYTES) return null
        val root = (StrictJson.parse(text) as? Json.Obj)?.fields ?: return null
        if ((root["v"] as? Json.Num)?.value != PROTOCOL.toLong()) return null
        val type = (root["type"] as? Json.Str)?.value ?: return null
        fun exactly(vararg keys: String) = root.keys == setOf("v", "type", *keys)
        fun generation(): Int? = (root["gen"] as? Json.Num)?.value?.takeIf { it in 1..MAX_GENERATION }?.toInt()
        return when (type) {
            "hello" -> if (exactly()) PageMessage.Hello else null
            "ready" -> {
                if (!exactly("gen", "bootMs")) return null
                val boot = (root["bootMs"] as? Json.Num)?.value?.takeIf { it in 0..MAX_BOOT_MS } ?: return null
                PageMessage.Ready(generation() ?: return null, boot.toInt())
            }
            "failed" -> {
                if (!exactly("gen", "code", "detail")) return null
                val code = FailureCode.entries.firstOrNull { it.wire == (root["code"] as? Json.Str)?.value } ?: return null
                val detail = (root["detail"] as? Json.Str)?.value ?: return null
                if (detail.codePointCount() > MAX_DETAIL_CODE_POINTS) return null
                PageMessage.Failed(generation() ?: return null, code, detail)
            }
            "exit" -> if (exactly("gen")) generation()?.let { PageMessage.Exit(it) } else null
            "service" -> {
                if (!exactly("gen", "id", "name", "enabled")) return null
                val id = (root["id"] as? Json.Num)?.value?.takeIf { it in 1..MAX_GENERATION } ?: return null
                val capability = HostCapability.entries.firstOrNull { it.wire == (root["name"] as? Json.Str)?.value } ?: return null
                val enabled = (root["enabled"] as? Json.Bool)?.value ?: return null
                PageMessage.Service(generation() ?: return null, id.toInt(), capability, enabled)
            }
            else -> null
        }
    }

    /** Encodes a host message. Every field is an enum or an integer, so no text needs escaping. */
    fun encode(message: HostMessage): String = when (message) {
        is HostMessage.Init -> {
            val capabilities = message.capabilities.sortedBy { it.wire }.joinToString(",") { "\"${it.wire}\"" }
            val p = message.preferences
            "{\"v\":1,\"type\":\"init\",\"gen\":${message.generation},\"capabilities\":[$capabilities]," +
                "\"preferences\":{\"theme\":\"${p.theme.wire}\",\"motion\":\"${p.motion.wire}\",\"locale\":\"${p.locale.wire}\"}}"
        }
        is HostMessage.Suspend -> simple("suspend", message.generation)
        is HostMessage.Resume -> simple("resume", message.generation)
        is HostMessage.Dispose -> simple("dispose", message.generation)
        is HostMessage.BackRequested -> simple("back-requested", message.generation)
        is HostMessage.Reply ->
            "{\"v\":1,\"type\":\"reply\",\"gen\":${message.generation},\"id\":${message.id},\"ok\":${message.ok},\"code\":\"${message.code.wire}\"}"
    }.also { check(it.encodeToByteArray().size <= MAX_BYTES) }

    private fun simple(type: String, generation: Int) = "{\"v\":1,\"type\":\"$type\",\"gen\":$generation}"

    private fun String.codePointCount(): Int {
        var count = 0
        var index = 0
        while (index < length) {
            index += if (this[index].isHighSurrogate() && index + 1 < length) 2 else 1
            count++
        }
        return count
    }
}
