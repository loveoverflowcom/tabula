package com.loveoverflow.tabula.mobile.localization

/**
 * Stable keys for copy owned by the mobile shell (doc 04 §3.3, ADR-0032). Shared navigation and
 * task vocabulary follows the web shell; game names still arrive from the Rust registry (I-9).
 */
enum class ShellCopy(val key: String) {
    Home("nav.home"),
    Games("nav.library"),
    Account("nav.account"),
    Game("shell.context.game"),
    Back("common.back"),
    HomeTitle("app.title"),
    HomeHeading("home.eyebrow"),
    HomeIntro("home.lead"),
    HomeStatus("mobile.home.scope"),
    BrowseGames("home.hero.action"),
    PackagedGames("mobile.library.packaged"),
    LocalOnly("mobile.mode.local"),
    NoGamesTitle("mobile.library.empty.heading"),
    NoGames("mobile.library.empty.body"),
    CatalogUnavailableTitle("mobile.catalog.unavailable.heading"),
    CatalogUnavailable("mobile.catalog.unavailable.body"),
    Details("card.details.action"),
    DetailUnavailableTitle("detail.notfound.heading"),
    DetailUnavailable("detail.notfound.body"),
    Setup("detail.setup"),
    SetupTitle("setup.heading"),
    SetupIntro("mobile.setup.intro"),
    LocalMode("mobile.setup.local"),
    OnlineUnavailable("mobile.setup.online.unavailable"),
    StartLocalGame("mobile.setup.start"),
    AccountTitle("accounts.title"),
    Anonymous("mobile.account.guest"),
    AccountUnavailableTitle("accounts.service.unavailable.heading"),
    AccountUnavailable("mobile.account.unavailable.body"),
    PreferencesUnavailable("mobile.account.preferences.unavailable"),
    FailureTitle("mobile.game.failed.heading"),
    FailureNote("mobile.game.failed.body"),
    Retry("common.retry"),
    Loading("common.loading"),
}

/**
 * Complete English/Vietnamese shell copy. Unsupported OS locales use English; regional tags
 * select their language before packaged names are looked up. Locale affects presentation only.
 */
class ShellStrings private constructor(val languageTag: String) {
    val vietnamese: Boolean get() = languageTag == "vi"

    operator fun get(copy: ShellCopy): String = if (vietnamese) vietnamese(copy) else english(copy)

    /** A launch action naming registry-supplied display data, with no game-specific branch. */
    fun play(name: String): String = if (vietnamese) "Chơi $name trên thiết bị này" else "Play $name on this device"

    fun details(name: String): String = if (vietnamese) "Xem chi tiết $name" else "View details for $name"

    fun setup(name: String): String = if (vietnamese) "Thiết lập $name" else "Set up $name"

    companion object {
        /** Normalizes a bounded host locale tag; arbitrary unsupported input safely uses English. */
        fun forLanguage(languageTag: String): ShellStrings {
            val language = if (languageTag.length <= 64) languageTag.trim().lowercase().substringBefore('-').substringBefore('_') else ""
            return ShellStrings(if (language == "vi") "vi" else "en")
        }
    }
}

// Exhaustive switches make a new key require copy in both maintained languages.
private fun english(copy: ShellCopy): String = when (copy) {
    ShellCopy.Home -> "Home"
    ShellCopy.Games -> "Library"
    ShellCopy.Account -> "Account"
    ShellCopy.Game -> "Game"
    ShellCopy.Back -> "Back"
    ShellCopy.HomeTitle -> "Tabula"
    ShellCopy.HomeHeading -> "Your play space"
    ShellCopy.HomeIntro -> "A little pause. A new move."
    ShellCopy.HomeStatus -> "Games packaged with this app play on this device. Online play and account services are unavailable."
    ShellCopy.BrowseGames -> "Explore games"
    ShellCopy.PackagedGames -> "Games on this device"
    ShellCopy.LocalOnly -> "Local play"
    ShellCopy.NoGamesTitle -> "Native gameplay unavailable"
    ShellCopy.NoGames -> "Native gameplay is not available in this build yet. You can still explore the app."
    ShellCopy.CatalogUnavailableTitle -> "Full catalog unavailable"
    ShellCopy.CatalogUnavailable -> "The full game catalog is not connected. This library shows only games packaged with this app."
    ShellCopy.Details -> "View details"
    ShellCopy.DetailUnavailableTitle -> "Game unavailable"
    ShellCopy.DetailUnavailable -> "This game is not packaged with this build. Return to the library to choose an available game."
    ShellCopy.Setup -> "Set up a match"
    ShellCopy.SetupTitle -> "New local game"
    ShellCopy.SetupIntro -> "Start a new game on this device with the packaged default settings."
    ShellCopy.LocalMode -> "Play on this device"
    ShellCopy.OnlineUnavailable -> "Online play is unavailable in this mobile build."
    ShellCopy.StartLocalGame -> "Start local game"
    ShellCopy.AccountTitle -> "Account"
    ShellCopy.Anonymous -> "Guest"
    ShellCopy.AccountUnavailableTitle -> "Account services unavailable"
    ShellCopy.AccountUnavailable -> "Sign-in, account profiles and friends are not connected in this mobile build. Local games can be played without an account."
    ShellCopy.PreferencesUnavailable -> "Account preferences are not connected. You can change appearance, language and motion in app settings."
    ShellCopy.FailureTitle -> "The game could not open"
    ShellCopy.FailureNote -> "Trying again starts a new game. Local games are not saved."
    ShellCopy.Retry -> "Try again"
    ShellCopy.Loading -> "Loading…"
}

private fun vietnamese(copy: ShellCopy): String = when (copy) {
    ShellCopy.Home -> "Trang chủ"
    ShellCopy.Games -> "Thư viện"
    ShellCopy.Account -> "Tài khoản"
    ShellCopy.Game -> "Trò chơi"
    ShellCopy.Back -> "Quay lại"
    ShellCopy.HomeTitle -> "Tabula"
    ShellCopy.HomeHeading -> "Không gian chơi"
    ShellCopy.HomeIntro -> "Một chút nghỉ ngơi. Một nước đi mới."
    ShellCopy.HomeStatus -> "Trò chơi đi kèm ứng dụng có thể chơi trên thiết bị này. Chơi online và dịch vụ tài khoản chưa khả dụng."
    ShellCopy.BrowseGames -> "Khám phá trò chơi"
    ShellCopy.PackagedGames -> "Trò chơi trên thiết bị này"
    ShellCopy.LocalOnly -> "Chơi local"
    ShellCopy.NoGamesTitle -> "Trò chơi native chưa khả dụng"
    ShellCopy.NoGames -> "Bản dựng này chưa hỗ trợ chơi native. Bạn vẫn có thể khám phá ứng dụng."
    ShellCopy.CatalogUnavailableTitle -> "Chưa có danh mục đầy đủ"
    ShellCopy.CatalogUnavailable -> "Danh mục trò chơi đầy đủ chưa được kết nối. Thư viện chỉ hiển thị trò chơi đi kèm ứng dụng."
    ShellCopy.Details -> "Xem chi tiết"
    ShellCopy.DetailUnavailableTitle -> "Trò chơi chưa khả dụng"
    ShellCopy.DetailUnavailable -> "Trò chơi này không đi kèm bản dựng. Quay lại thư viện để chọn trò chơi khả dụng."
    ShellCopy.Setup -> "Thiết lập trận đấu"
    ShellCopy.SetupTitle -> "Ván local mới"
    ShellCopy.SetupIntro -> "Bắt đầu ván mới trên thiết bị này với thiết lập mặc định của bản trò chơi đi kèm."
    ShellCopy.LocalMode -> "Chơi trên thiết bị này"
    ShellCopy.OnlineUnavailable -> "Bản dựng di động này chưa hỗ trợ chơi online."
    ShellCopy.StartLocalGame -> "Bắt đầu ván local"
    ShellCopy.AccountTitle -> "Tài khoản"
    ShellCopy.Anonymous -> "Khách"
    ShellCopy.AccountUnavailableTitle -> "Dịch vụ tài khoản chưa khả dụng"
    ShellCopy.AccountUnavailable -> "Đăng nhập, hồ sơ tài khoản và bạn bè chưa được kết nối trong bản dựng di động này. Bạn có thể chơi local mà không cần tài khoản."
    ShellCopy.PreferencesUnavailable -> "Tùy chọn tài khoản chưa được kết nối. Bạn có thể đổi giao diện, ngôn ngữ và chuyển động trong cài đặt ứng dụng."
    ShellCopy.FailureTitle -> "Chưa thể mở trò chơi"
    ShellCopy.FailureNote -> "Thử lại sẽ bắt đầu ván mới. Ván local không được lưu."
    ShellCopy.Retry -> "Thử lại"
    ShellCopy.Loading -> "Đang tải…"
}
