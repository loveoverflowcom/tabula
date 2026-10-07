package com.tabula.app.model

data class RoomItem(
    val id: String,
    val gameId: String,
    val title: String,
    val host: String,
    val currentSeats: Int,
    val maxSeats: Int,
    val timeControl: String,
    val isRanked: Boolean = false,
    val status: String = "Waiting for players"
)

object RoomCatalog {
    val sampleRooms = listOf(
        RoomItem("room-101", "chess", "Rapid 5+3 Blitz Arena", "Grandmaster_V", 1, 2, "5m + 3s", true, "Waiting for Player 2"),
        RoomItem("room-102", "chess", "Casual Chess Tactics", "Alex_VN", 1, 2, "10m", false, "Ready to start"),
        RoomItem("room-103", "tictactoe", "Caro Gomoku 5-in-a-Row", "HanoiGamer", 1, 2, "3m", false, "Open"),
        RoomItem("room-104", "cards", "Tiến Lên Miền Nam 4-Seats", "ThaoNguyen", 3, 4, "20s/turn", true, "1 seat left"),
        RoomItem("room-105", "werewolf", "Village of Shadows (Night 1)", "Moderator_Alpha", 8, 12, "Phase 120s", false, "Lobby filling (8/12)")
    )
}

data class MatchHistoryItem(
    val id: String,
    val gameTitle: String,
    val opponent: String,
    val result: String, // "Victory", "Defeat", "Draw"
    val scoreChange: String,
    val movesCount: Int,
    val duration: String,
    val playedAt: String
)

object MatchHistoryCatalog {
    val sampleHistory = listOf(
        MatchHistoryItem("match-901", "Chess (Cờ Vua)", "Stockfish Lv.2", "Victory", "+18 Elo", 24, "4m 12s", "Today, 14:20"),
        MatchHistoryItem("match-902", "Tic-Tac-Toe", "TabulaBot", "Victory", "+10 Elo", 7, "45s", "Today, 11:05"),
        MatchHistoryItem("match-903", "Chess (Cờ Vua)", "Master_Thang", "Defeat", "-14 Elo", 38, "8m 50s", "Yesterday, 20:15"),
        MatchHistoryItem("match-904", "Tiến Lên (Cards)", "Table #4", "Victory", "+25 Elo", 16, "12m 30s", "Oct 05, 18:40"),
        MatchHistoryItem("match-905", "Tic-Tac-Toe", "Local Friend", "Draw", "+0 Elo", 9, "1m 10s", "Oct 04, 16:10")
    )
}
