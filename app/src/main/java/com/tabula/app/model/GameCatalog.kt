package com.tabula.app.model

enum class GameCategory(val title: String) {
    ALL("All Games"),
    ABSTRACT("Abstract & Board"),
    CARDS("Card Games"),
    SOCIAL("Social Deduction"),
    TILES("Tile Placement")
}

enum class GameComplexity(val label: String) {
    LIGHT("Light"),
    MEDIUM("Medium"),
    COMPLEX("Complex")
}

data class GameSummary(
    val id: String,
    val title: String,
    val subtitle: String,
    val category: GameCategory,
    val players: String,
    val estMinutes: String,
    val complexity: GameComplexity,
    val description: String,
    val capabilities: List<String>,
    val bannerIcon: String,
    val isPlayableInApp: Boolean = true
)

object GameCatalog {
    val games = listOf(
        GameSummary(
            id = "chess",
            title = "Chess (Cờ Vua)",
            subtitle = "Phase 1 Control Group • Correctness Benchmark",
            category = GameCategory.ABSTRACT,
            players = "2 Players",
            estMinutes = "5–15m",
            complexity = GameComplexity.MEDIUM,
            description = "The classic deterministic benchmark with zero hidden information. Validates strict legality, castling, en passant, promotion, and countdown clocks.",
            capabilities = listOf("Strict Sequential", "Clocks & Timers", "Deterministic Replay", "Spectators"),
            bannerIcon = "♟️",
            isPlayableInApp = true
        ),
        GameSummary(
            id = "tictactoe",
            title = "Tic-Tac-Toe & Caro",
            subtitle = "Phase 0 Smoke Test • Pure State Machine",
            category = GameCategory.ABSTRACT,
            players = "2 Players",
            estMinutes = "1–3m",
            complexity = GameComplexity.LIGHT,
            description = "The baseline pure-function engine. Play 3x3 Tic-Tac-Toe or expandable 5-in-a-row Caro Gomoku with deterministic Bot policies and move timers.",
            capabilities = listOf("Tiny State (9 cells)", "Full Move Enumeration", "Smart AI Bot", "Zero Randomness"),
            bannerIcon = "⭕",
            isPlayableInApp = true
        ),
        GameSummary(
            id = "tiles",
            title = "Tiles (Carcassonne)",
            subtitle = "Phase 3 Benchmark • Dynamic Grid & Camera",
            category = GameCategory.TILES,
            players = "2–5 Players",
            estMinutes = "15–30m",
            complexity = GameComplexity.MEDIUM,
            description = "Dynamic tile-placement strategy. Draw terrain tiles, match road and city boundaries, place meeples, and score completed features on an expanding board.",
            capabilities = listOf("Dynamic Topology", "Follower Placement", "Incremental Scoring", "Secret Bag Order"),
            bannerIcon = "🏰",
            isPlayableInApp = true
        ),
        GameSummary(
            id = "cards",
            title = "Tiến Lên (Cards)",
            subtitle = "Phase 3 Benchmark • Hidden Information",
            category = GameCategory.CARDS,
            players = "4 Players",
            estMinutes = "10–20m",
            complexity = GameComplexity.MEDIUM,
            description = "Hidden hand trick-taking with low-card start, doubles, straights, and bomb chops. Proves the cryptographic projection boundary and delayed spectator feeds.",
            capabilities = listOf("Per-Seat Projection", "Cryptographic Salt", "Trick Shedding", "Delayed Spectators"),
            bannerIcon = "🃏",
            isPlayableInApp = true
        ),
        GameSummary(
            id = "werewolf",
            title = "Werewolf (Ma Sói)",
            subtitle = "Phase 3/7 Benchmark • Phased Social Deduction",
            category = GameCategory.SOCIAL,
            players = "6–20 Players",
            estMinutes = "20–45m",
            complexity = GameComplexity.COMPLEX,
            description = "Multi-seat night and day phases with hidden roles (Villager, Werewolf, Seer, Doctor, Hunter). Scoped chat and secret action resolution.",
            capabilities = listOf("Event Non-existence", "Scoped Chat & Voice", "Night Resolution", "Plurality Voting"),
            bannerIcon = "🐺",
            isPlayableInApp = true
        )
    )

    fun getById(id: String): GameSummary? = games.find { it.id == id }
}
