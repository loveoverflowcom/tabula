package com.loveoverflow.tabula.mobile.localization

/** Copy for local settings and honest native service availability (doc 04 §3.3). */
enum class ShellToolsCopy(val key: String) {
    Rooms("mobile.rooms.title"), RoomsIntro("mobile.rooms.intro"), RoomsUnavailable("mobile.rooms.unavailable.title"),
    RoomsUnavailableBody("mobile.rooms.unavailable.body"), RoomsMenuBody("mobile.rooms.menu.body"),
    History("mobile.history.title"), HistoryIntro("mobile.history.intro"), HistoryUnavailable("mobile.history.unavailable.title"),
    HistoryUnavailableBody("mobile.history.unavailable.body"), HistoryMenuBody("mobile.history.menu.body"),
    Settings("mobile.settings.title"), SettingsIntro("mobile.settings.intro"), SettingsMenuBody("mobile.settings.menu.body"),
    SettingsScope("mobile.settings.scope"), Appearance("mobile.settings.appearance"), Language("mobile.settings.language"),
    Motion("mobile.settings.motion"), MotionBody("mobile.settings.motion.body"),
    System("mobile.settings.system"), Light("mobile.settings.light"), Dark("mobile.settings.dark"),
    English("mobile.settings.english"), Vietnamese("mobile.settings.vietnamese"), Reduced("mobile.settings.reduced"),
}

/** Exhaustive English/Vietnamese copy; local tools never synthesize identity or gameplay facts. */
fun ShellStrings.tools(copy: ShellToolsCopy): String = if (vietnamese) when (copy) {
    ShellToolsCopy.Rooms -> "Phòng chơi"
    ShellToolsCopy.RoomsIntro -> "Cùng bạn bè chọn một trò chơi."
    ShellToolsCopy.RoomsUnavailable -> "Phòng chơi online chưa khả dụng"
    ShellToolsCopy.RoomsUnavailableBody -> "Bản dựng di động này chưa kết nối dịch vụ phòng chơi. Bạn chưa thể tạo hoặc tham gia phòng; hãy khám phá trò chơi trong thư viện."
    ShellToolsCopy.RoomsMenuBody -> "Xem trạng thái tạo và tham gia phòng online."
    ShellToolsCopy.History -> "Lịch sử trận đấu"
    ShellToolsCopy.HistoryIntro -> "Nơi xem lại những ván đã chơi."
    ShellToolsCopy.HistoryUnavailable -> "Lịch sử chưa khả dụng"
    ShellToolsCopy.HistoryUnavailableBody -> "Bản dựng di động này chưa lưu ván local hoặc kết nối lịch sử tài khoản. Chưa có trận đấu hay bản replay để xem ở đây."
    ShellToolsCopy.HistoryMenuBody -> "Xem trạng thái lịch sử trận đấu và replay."
    ShellToolsCopy.Settings -> "Cài đặt ứng dụng"
    ShellToolsCopy.SettingsIntro -> "Theo cài đặt thiết bị hoặc chọn giao diện, ngôn ngữ và chuyển động cho ứng dụng này."
    ShellToolsCopy.SettingsMenuBody -> "Chọn giao diện, ngôn ngữ và giảm chuyển động."
    ShellToolsCopy.SettingsScope -> "Lựa chọn áp dụng cho ứng dụng và ván local tiếp theo, độc lập với tài khoản. Ván đang chơi giữ nguyên cài đặt lúc bắt đầu."
    ShellToolsCopy.Appearance -> "Giao diện"
    ShellToolsCopy.Language -> "Ngôn ngữ"
    ShellToolsCopy.Motion -> "Chuyển động"
    ShellToolsCopy.MotionBody -> "Theo thiết bị luôn tôn trọng tùy chọn giảm chuyển động của hệ điều hành."
    ShellToolsCopy.System -> "Theo thiết bị"
    ShellToolsCopy.Light -> "Sáng"
    ShellToolsCopy.Dark -> "Tối"
    ShellToolsCopy.English -> "English"
    ShellToolsCopy.Vietnamese -> "Tiếng Việt"
    ShellToolsCopy.Reduced -> "Giảm chuyển động"
} else when (copy) {
    ShellToolsCopy.Rooms -> "Rooms"
    ShellToolsCopy.RoomsIntro -> "Choose a game to enjoy with friends."
    ShellToolsCopy.RoomsUnavailable -> "Online rooms unavailable"
    ShellToolsCopy.RoomsUnavailableBody -> "Room services are not connected in this mobile build. You cannot host or join a room yet; explore games in the library."
    ShellToolsCopy.RoomsMenuBody -> "Check availability of hosting and joining online rooms."
    ShellToolsCopy.History -> "Match history"
    ShellToolsCopy.HistoryIntro -> "A place to revisit the games you have played."
    ShellToolsCopy.HistoryUnavailable -> "History unavailable"
    ShellToolsCopy.HistoryUnavailableBody -> "This mobile build does not save local games or connect to account history. There are no matches or replays to view here yet."
    ShellToolsCopy.HistoryMenuBody -> "Check availability of match history and replays."
    ShellToolsCopy.Settings -> "App settings"
    ShellToolsCopy.SettingsIntro -> "Follow your device or choose appearance, language and motion for this app."
    ShellToolsCopy.SettingsMenuBody -> "Choose appearance, language and reduced motion."
    ShellToolsCopy.SettingsScope -> "Choices apply to the app and your next local game, independently of your account. A game already in progress keeps its launch settings."
    ShellToolsCopy.Appearance -> "Appearance"
    ShellToolsCopy.Language -> "Language"
    ShellToolsCopy.Motion -> "Motion"
    ShellToolsCopy.MotionBody -> "Following your device always respects its reduced-motion accessibility setting."
    ShellToolsCopy.System -> "Follow device"
    ShellToolsCopy.Light -> "Light"
    ShellToolsCopy.Dark -> "Dark"
    ShellToolsCopy.English -> "English"
    ShellToolsCopy.Vietnamese -> "Tiếng Việt"
    ShellToolsCopy.Reduced -> "Reduced motion"
}
