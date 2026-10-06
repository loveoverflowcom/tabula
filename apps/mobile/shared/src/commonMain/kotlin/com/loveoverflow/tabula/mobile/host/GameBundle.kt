package com.loveoverflow.tabula.mobile.host

import com.loveoverflow.tabula.mobile.bridge.Json
import com.loveoverflow.tabula.mobile.bridge.StrictJson

/**
 * A first-party game packaged inside the app (ADR-0033). The list comes from `tabula-games.json`,
 * written by `cargo xtask stage-mobile-game` from the Rust registry. [query] is the registry's
 * validated launch configuration: the shell appends it to the document URL and never reads it.
 */
data class BundledGame(
    val id: String,
    val entry: String,
    val query: String,
    val names: Map<String, String>,
) {
    /** The display name for [languageTag], falling back to English, then to the opaque id. */
    fun displayName(languageTag: String): String = names[languageTag] ?: names["en"] ?: id
}

object GameBundle {
    const val MANIFEST_LIMIT_BYTES = 16 * 1024
    private const val MAX_GAMES = 8
    private val idPattern = Regex("^[a-z0-9]+(\\.[a-z0-9-]+)+$")
    private val entryPattern = Regex("^/play/[a-z0-9-]+/$")
    private val queryPattern = Regex("^[A-Za-z0-9._~%=&-]{0,2048}$")

    /** Parses and validates the manifest. Returns `null` for anything outside the schema. */
    fun parse(text: String): List<BundledGame>? {
        if (text.encodeToByteArray().size > MANIFEST_LIMIT_BYTES) return null
        val root = (StrictJson.parse(text) as? Json.Obj)?.fields ?: return null
        if (root.keys != setOf("schema", "games") || (root["schema"] as? Json.Num)?.value != 1L) return null
        val items = (root["games"] as? Json.Arr)?.items ?: return null
        if (items.size > MAX_GAMES) return null
        val games = items.map { item ->
            val fields = (item as? Json.Obj)?.fields ?: return null
            if (fields.keys != setOf("id", "entry", "query", "names")) return null
            val id = (fields["id"] as? Json.Str)?.value?.takeIf(idPattern::matches) ?: return null
            val entry = (fields["entry"] as? Json.Str)?.value?.takeIf(entryPattern::matches) ?: return null
            val query = (fields["query"] as? Json.Str)?.value?.takeIf(queryPattern::matches) ?: return null
            val names = (fields["names"] as? Json.Obj)?.fields ?: return null
            if (names.isEmpty() || names.size > 8) return null
            BundledGame(
                id = id,
                entry = entry,
                query = query,
                names = names.entries.associate { (tag, name) ->
                    if (!Regex("^[a-z]{2,3}$").matches(tag)) return null
                    tag to ((name as? Json.Str)?.value?.takeIf { it.isNotBlank() && it.length <= 80 } ?: return null)
                },
            )
        }
        if (games.map { it.id }.toSet().size != games.size) return null
        return games
    }
}

/** The document URL for [game] on a host's virtual [origin] (for example `https://appassets.example`). */
fun BundledGame.documentUrl(origin: String): String =
    if (query.isEmpty()) "$origin$entry" else "$origin$entry?$query"
