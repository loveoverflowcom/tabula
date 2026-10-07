package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.catalog.DiscoveryChoice
import com.loveoverflow.tabula.mobile.catalog.DiscoveryField
import com.loveoverflow.tabula.mobile.catalog.DiscoveryGame
import com.loveoverflow.tabula.mobile.catalog.DiscoveryQuery
import com.loveoverflow.tabula.mobile.catalog.RegistryDiscoveryCatalog
import com.loveoverflow.tabula.mobile.catalog.queryDiscovery
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class DiscoveryCatalogTest {
    private val mapGame = DiscoveryGame(
        id = "com.example.map",
        names = mapOf("en" to "Map", "vi" to "Bản đồ"),
        taglines = mapOf("en" to "Claim connected regions", "vi" to "Giành vùng nối liền"),
        categories = listOf("tile_placement"),
        tags = listOf("roads", "cities"),
        players = listOf(2, 4),
        minMinutes = 15,
        maxMinutes = 45,
        complexity = "medium",
    )
    private val duel = DiscoveryGame(
        id = "com.example.duel",
        names = mapOf("en" to "Duel", "vi" to "Đấu đôi"),
        categories = listOf("abstract"),
        players = listOf(2),
        minMinutes = 10,
        maxMinutes = 20,
        complexity = "light",
    )

    @Test
    fun localizedSearchFoldsToneMarksAndReadsOnlyDeclaredNameTaglineTags() {
        val games = listOf(mapGame, duel)
        assertEquals(listOf(mapGame), queryDiscovery(games, DiscoveryQuery(text = " BAN DO "), "vi-VN"))
        assertEquals(listOf(duel), queryDiscovery(games, DiscoveryQuery(text = "dau doi"), "vi"))
        assertEquals(listOf(mapGame), queryDiscovery(games, DiscoveryQuery(text = "connected"), "en"))
        assertEquals(listOf(mapGame), queryDiscovery(games, DiscoveryQuery(text = "ROADS"), "vi"))
        assertEquals(emptyList(), queryDiscovery(games, DiscoveryQuery(text = "com.example"), "en"))
        assertEquals(listOf(duel, mapGame), queryDiscovery(games, DiscoveryQuery(text = " \t "), "en"))
    }

    @Test
    fun filtersCombineExactSeatSetsAndMaximumEstimatedDuration() {
        val games = listOf(mapGame, duel)
        assertEquals(emptyList(), queryDiscovery(games, DiscoveryQuery(players = 3), "en"))
        assertEquals(listOf(mapGame), queryDiscovery(games, DiscoveryQuery(players = 4), "en"))
        assertEquals(listOf(duel), queryDiscovery(games, DiscoveryQuery(maxMinutes = 44), "en"))
        assertEquals(listOf(duel, mapGame), queryDiscovery(games, DiscoveryQuery(maxMinutes = 45), "en"))
        assertEquals(listOf(mapGame), queryDiscovery(games, DiscoveryQuery(category = "tile_placement", players = 2, maxMinutes = 45, complexity = "medium"), "en"))
        assertEquals(emptyList(), queryDiscovery(games, DiscoveryQuery(category = "tile_placement", complexity = "light"), "en"))
        assertTrue(DiscoveryQuery().isEmpty)
        assertFalse(DiscoveryQuery(players = 2).isEmpty)
    }

    @Test
    fun orderingUsesLocalizedTitleThenOpaqueIdAndUnknownLocalesFallBackToEnglish() {
        val earlierId = duel.copy(id = "com.example.a")
        assertEquals(listOf(earlierId, duel, mapGame), queryDiscovery(listOf(mapGame, duel, earlierId), DiscoveryQuery(), "en"))
        assertEquals(listOf(mapGame, duel), queryDiscovery(listOf(duel, mapGame), DiscoveryQuery(), "vi"))
        assertEquals("Bản đồ", mapGame.displayName("VI_vn"))
        assertEquals("Map", mapGame.displayName("fr-FR"))
        assertEquals("com.example.empty", DiscoveryGame("com.example.empty", emptyMap()).displayName("en"))
    }

    @Test
    fun setupShowsOnlyDeclaredChoiceFieldsWithoutApprovingRuleValidity() {
        val choice = DiscoveryField(
            key = "limit",
            labels = mapOf("en" to "Limit"),
            choices = listOf(
                DiscoveryChoice("none", mapOf("en" to "None")),
                DiscoveryChoice("timed", mapOf("en" to "Timed"), reveals = listOf("seconds")),
            ),
            default = "none",
        )
        val seconds = DiscoveryField("seconds", mapOf("en" to "Seconds"), min = "0", max = "120", default = "30")
        val game = duel.copy(fields = listOf(choice, seconds))
        assertEquals(mapOf("limit" to "none", "seconds" to "30"), game.defaultDraft())
        assertEquals(listOf(choice), game.visibleFields(game.defaultDraft()))
        assertEquals(listOf(choice, seconds), game.visibleFields(mapOf("limit" to "timed", "seconds" to "0")))
        assertEquals(listOf(choice), game.visibleFields(mapOf("limit" to "hostile-choice")))
    }

    @Test
    fun generatedRegistryCatalogContainsCompleteLightweightBilingualDiscoveryData() {
        val games = RegistryDiscoveryCatalog.games
        assertTrue(games.isNotEmpty(), "this build links discovery modules")
        assertEquals(games.size, games.map { it.id }.toSet().size)
        games.forEach { game ->
            assertTrue(game.players.isNotEmpty())
            assertTrue(game.minMinutes <= game.maxMinutes)
            assertTrue(game.version.isNotBlank())
            listOf("vi", "en").forEach { language ->
                assertTrue(game.names[language].orEmpty().isNotBlank())
                assertTrue(game.descriptions[language].orEmpty().isNotBlank())
                assertTrue(game.complexityNames[language].orEmpty().isNotBlank())
                assertTrue(game.contentRatingNames[language].orEmpty().isNotBlank())
                game.fields.forEach { assertTrue(it.labels[language].orEmpty().isNotBlank()) }
            }
            val cover = assertNotNull(game.cover)
            assertTrue(cover.svg.encodeToByteArray().size < 8192)
            assertFalse(cover.svg.contains("<image"))
            assertFalse(cover.svg.contains("href="))
        }
    }
}
