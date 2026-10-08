package com.loveoverflow.tabula.mobile.catalog

/** Public discovery copy generated from the Rust registry; this value cannot launch a game (I-9, ADR-0043). */
data class DiscoveryGame(
    val id: String,
    val names: Map<String, String>,
    val taglines: Map<String, String> = emptyMap(),
    val descriptions: Map<String, String> = emptyMap(),
    val categories: List<String> = emptyList(),
    val categoryNames: Map<String, Map<String, String>> = emptyMap(),
    val tags: List<String> = emptyList(),
    val players: List<Int> = listOf(2),
    val minMinutes: Int = 0,
    val maxMinutes: Int = 0,
    val complexity: String = "light",
    val complexityNames: Map<String, String> = emptyMap(),
    val rulesVersion: Int = 0,
    val rulesUrls: Map<String, String> = emptyMap(),
    val modes: List<DiscoveryMode> = emptyList(),
    val fields: List<DiscoveryField> = emptyList(),
    val cover: DiscoveryCover? = null,
    val version: String = "",
    val contentRating: String = "everyone",
    val contentRatingNames: Map<String, String> = emptyMap(),
    val hiddenInformation: Boolean = false,
    /** Opaque generated discovery-file stem; not a gameplay pack or game-specific dispatch key. */
    val catalogIcon: String? = null,
    /** Explicit public planned information has no setup or native launch authority. */
    val planned: Boolean = false,
) {
    fun displayName(languageTag: String): String = localized(names, languageTag, id)
    fun tagline(languageTag: String): String = localized(taglines, languageTag)
    fun description(languageTag: String): String = localized(descriptions, languageTag)
    fun categoryLabel(category: String, languageTag: String): String =
        localized(categoryNames[category].orEmpty(), languageTag, category)
    fun complexityLabel(languageTag: String): String = localized(complexityNames, languageTag, complexity)
    fun contentRatingLabel(languageTag: String): String = localized(contentRatingNames, languageTag, contentRating)
    fun rulesUrl(languageTag: String): String? = localized(rulesUrls, languageTag).takeIf(String::isNotBlank)

    /** Unvalidated presentation draft only; the native Rust adapter must validate any future launch. */
    fun defaultDraft(): Map<String, String> = fields.associate { it.key to it.default }

    /** Descriptor visibility follows registry ConfigForm; hidden values remain draft data, never rules facts. */
    fun visibleFields(draft: Map<String, String>): List<DiscoveryField> {
        val revealed = mutableSetOf<String>()
        val hidden = mutableSetOf<String>()
        fields.forEach { field ->
            val selected = draft[field.key] ?: field.default
            field.choices.forEach { choice ->
                if (choice.value == selected) revealed.addAll(choice.reveals) else hidden.addAll(choice.reveals)
            }
        }
        return fields.filter { it.key in revealed || it.key !in hidden }
    }
}

/** Registry mode descriptions concern its current host; [registryAvailable] does not establish native availability. */
data class DiscoveryMode(
    val id: String,
    val labels: Map<String, String>,
    val consequences: Map<String, String> = emptyMap(),
    val registryAvailable: Boolean = false,
    val reasons: Map<String, String> = emptyMap(),
    val recoveries: Map<String, String> = emptyMap(),
) {
    fun label(languageTag: String): String = localized(labels, languageTag, id)
    fun consequence(languageTag: String): String = localized(consequences, languageTag)
    fun reason(languageTag: String): String = localized(reasons, languageTag)
    fun recovery(languageTag: String): String = localized(recoveries, languageTag)
}

/** Module-owned field copy and bounds for shell controls; bounds never substitute for Rust config validation. */
data class DiscoveryField(
    val key: String,
    val labels: Map<String, String>,
    val hints: Map<String, String> = emptyMap(),
    val choices: List<DiscoveryChoice> = emptyList(),
    val min: String? = null,
    val max: String? = null,
    val default: String = "",
) {
    fun label(languageTag: String): String = localized(labels, languageTag, key)
    fun hint(languageTag: String): String = localized(hints, languageTag)
}

/** One declared choice and the fields it reveals; opaque keys carry no game-specific CMP meaning. */
data class DiscoveryChoice(
    val value: String,
    val labels: Map<String, String>,
    val reveals: List<String> = emptyList(),
) {
    fun label(languageTag: String): String = localized(labels, languageTag, value)
}

/** Small inert game-owned SVG, generated without a runtime asset pack or GPU preload (doc 04 §3.2). */
data class DiscoveryCover(val svg: String, val width: Float = 364f, val height: Float = 160f)

/** Loading/failure states concern discovery only; Ready entries grant no gameplay authority. */
sealed interface DiscoveryCatalogState {
    data object Loading : DiscoveryCatalogState
    data class Ready(val games: List<DiscoveryGame>) : DiscoveryCatalogState
    data object Unavailable : DiscoveryCatalogState
    data class Error(val message: String = "") : DiscoveryCatalogState
}

/** Independent discovery axes combine with AND, matching the registry's CatalogQuery. */
data class DiscoveryQuery(
    val text: String = "",
    val category: String? = null,
    val players: Int? = null,
    val maxMinutes: Int? = null,
    val complexity: String? = null,
) {
    val isEmpty: Boolean get() = this == DiscoveryQuery()
}

/** Localized title/tagline/tag search and exact metadata filters; ordering is title then opaque id. */
fun queryDiscovery(games: List<DiscoveryGame>, query: DiscoveryQuery, languageTag: String): List<DiscoveryGame> {
    val needle = foldDiscovery(query.text)
    return games.filter { game ->
        (query.category == null || query.category in game.categories) &&
            (query.players == null || query.players in game.players) &&
            (query.maxMinutes == null || game.maxMinutes <= query.maxMinutes) &&
            (query.complexity == null || query.complexity == game.complexity) &&
            (needle.isEmpty() || (listOf(game.displayName(languageTag), game.tagline(languageTag)) + game.tags)
                .any { needle in foldDiscovery(it) })
    }.sortedWith(compareBy<DiscoveryGame> { foldDiscovery(it.displayName(languageTag)) }.thenBy { it.id })
}

/** English fallback and regional locale normalization, as used by the shell's exhaustive vi/en copy. */
private fun localized(values: Map<String, String>, languageTag: String, fallback: String = ""): String {
    val language = languageTag.lowercase().replace('_', '-').substringBefore('-')
    return values[language] ?: values["en"] ?: fallback
}

/** The same Latin/Vietnamese fold as registry catalog::fold; no game-specific search aliases. */
internal fun foldDiscovery(text: String): String = text.trim().lowercase().map { character ->
    when (character) {
        in "àáâãäåăạảấầẩẫậắằẳẵặ" -> 'a'
        in "èéêëẹẻẽếềểễệ" -> 'e'
        in "ìíîïịỉĩ" -> 'i'
        in "òóôõöơọỏốồổỗộớờởỡợ" -> 'o'
        in "ùúûüưụủũứừửữự" -> 'u'
        in "ỳýỵỷỹ" -> 'y'
        'đ' -> 'd'
        else -> character
    }
}.joinToString("")
