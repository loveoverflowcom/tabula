package com.loveoverflow.tabula.mobile.localization

/** Copy for metadata-only mobile discovery; never supplies game rules (doc 04 §3.3, I-9). */
enum class DiscoveryCopy {
    HomeHeading, HeroEyebrow, HeroHeading, HeroBody, CatalogHeading, BrowseAll,
    Search, SearchHint, ClearSearch, Filters, HideFilters, All, Category, Players, Duration, Complexity,
    ResetFilters, Results, EmptyTitle, EmptyBody, NoResultsTitle, NoResultsBody, CatalogErrorTitle,
    CatalogErrorBody, CatalogUnavailableTitle, CatalogUnavailableBody, Overview, Modes, Rules,
    RulesUnavailable, NativeUnavailable, OnlineUnavailable, Version, Rating, HiddenInformation, Yes, No, Configuration, ConfigurationDefaults,
    ConfigurationUnavailable, ViewSetup, SetupUnavailable, Untimed, NoConfiguration, ArtUnavailable,
}

/** Both maintained languages are exhaustive; game-owned labels are resolved separately. */
fun ShellStrings.discovery(copy: DiscoveryCopy): String = if (vietnamese) when (copy) {
    DiscoveryCopy.HomeHeading -> "Chơi một ván nhé?"
    DiscoveryCopy.HeroEyebrow -> "Góc khám phá mới"
    DiscoveryCopy.HeroHeading -> "Mỗi nước đi,\nmột điều để học."
    DiscoveryCopy.HeroBody -> "Tìm game hợp với bạn. Chọn cách chơi được hỗ trợ."
    DiscoveryCopy.CatalogHeading -> "Khám phá trò chơi"
    DiscoveryCopy.BrowseAll -> "Xem thư viện"
    DiscoveryCopy.Search -> "Tìm trò chơi"
    DiscoveryCopy.SearchHint -> "Tên trò chơi hoặc từ khóa"
    DiscoveryCopy.ClearSearch -> "Xóa tìm kiếm"
    DiscoveryCopy.Filters -> "Thêm bộ lọc"
    DiscoveryCopy.HideFilters -> "Thu gọn bộ lọc"
    DiscoveryCopy.All -> "Tất cả"
    DiscoveryCopy.Category -> "Thể loại"
    DiscoveryCopy.Players -> "Số người chơi"
    DiscoveryCopy.Duration -> "Thời gian dự kiến"
    DiscoveryCopy.Complexity -> "Độ phức tạp"
    DiscoveryCopy.ResetFilters -> "Đặt lại bộ lọc"
    DiscoveryCopy.Results -> "Trò chơi trong thư viện"
    DiscoveryCopy.EmptyTitle -> "Chưa có trò chơi"
    DiscoveryCopy.EmptyBody -> "Danh mục này chưa có trò chơi để khám phá."
    DiscoveryCopy.NoResultsTitle -> "Không có trò chơi phù hợp"
    DiscoveryCopy.NoResultsBody -> "Thử từ khóa khác hoặc đặt lại bộ lọc."
    DiscoveryCopy.CatalogErrorTitle -> "Chưa thể tải danh mục"
    DiscoveryCopy.CatalogErrorBody -> "Danh mục trò chơi chưa tải được. Lựa chọn tìm kiếm và bộ lọc của bạn vẫn được giữ."
    DiscoveryCopy.CatalogUnavailableTitle -> "Danh mục chưa khả dụng"
    DiscoveryCopy.CatalogUnavailableBody -> "Danh mục trò chơi chưa khả dụng trên thiết bị này."
    DiscoveryCopy.Overview -> "Giới thiệu"
    DiscoveryCopy.Modes -> "Cách chơi"
    DiscoveryCopy.Rules -> "Luật và tài liệu"
    DiscoveryCopy.RulesUnavailable -> "Bản trò chơi này chưa cung cấp liên kết đến tài liệu luật."
    DiscoveryCopy.NativeUnavailable -> "Trò chơi native chưa khả dụng trong bản dựng di động này. Bạn có thể đọc thông tin trò chơi."
    DiscoveryCopy.OnlineUnavailable -> "Chơi online chưa khả dụng trong bản dựng di động này."
    DiscoveryCopy.Version -> "Phiên bản"
    DiscoveryCopy.Rating -> "Độ tuổi"
    DiscoveryCopy.HiddenInformation -> "Thông tin ẩn"
    DiscoveryCopy.Yes -> "Có"
    DiscoveryCopy.No -> "Không"
    DiscoveryCopy.Configuration -> "Thiết lập trò chơi"
    DiscoveryCopy.ConfigurationDefaults -> "Khởi chạy local sử dụng thiết lập mặc định của bản trò chơi đi kèm."
    DiscoveryCopy.ConfigurationUnavailable -> "Chưa thể thay đổi thiết lập trên thiết bị này. Các giá trị bên dưới là mặc định từ trò chơi."
    DiscoveryCopy.ViewSetup -> "Xem thiết lập"
    DiscoveryCopy.SetupUnavailable -> "Chưa thể bắt đầu ván chơi"
    DiscoveryCopy.Untimed -> "Không giới hạn thời gian"
    DiscoveryCopy.NoConfiguration -> "Trò chơi sử dụng thiết lập mặc định."
    DiscoveryCopy.ArtUnavailable -> "Ảnh minh họa chưa khả dụng; thông tin trò chơi vẫn đầy đủ."
} else when (copy) {
    DiscoveryCopy.HomeHeading -> "Shall we play?"
    DiscoveryCopy.HeroEyebrow -> "A little discovery"
    DiscoveryCopy.HeroHeading -> "Every move,\na little discovery."
    DiscoveryCopy.HeroBody -> "Find a game that feels right. Explore the ways you can play."
    DiscoveryCopy.CatalogHeading -> "Discover games"
    DiscoveryCopy.BrowseAll -> "Browse the library"
    DiscoveryCopy.Search -> "Search games"
    DiscoveryCopy.SearchHint -> "Game name or keyword"
    DiscoveryCopy.ClearSearch -> "Clear search"
    DiscoveryCopy.Filters -> "More filters"
    DiscoveryCopy.HideFilters -> "Fewer filters"
    DiscoveryCopy.All -> "All"
    DiscoveryCopy.Category -> "Category"
    DiscoveryCopy.Players -> "Players"
    DiscoveryCopy.Duration -> "Estimated duration"
    DiscoveryCopy.Complexity -> "Complexity"
    DiscoveryCopy.ResetFilters -> "Reset filters"
    DiscoveryCopy.Results -> "Games in the library"
    DiscoveryCopy.EmptyTitle -> "No games yet"
    DiscoveryCopy.EmptyBody -> "There are no games to explore in this catalog yet."
    DiscoveryCopy.NoResultsTitle -> "No games match"
    DiscoveryCopy.NoResultsBody -> "Try another keyword or reset the filters."
    DiscoveryCopy.CatalogErrorTitle -> "The catalog could not load"
    DiscoveryCopy.CatalogErrorBody -> "The game catalog could not load. Your search and filters have been kept."
    DiscoveryCopy.CatalogUnavailableTitle -> "Catalog unavailable"
    DiscoveryCopy.CatalogUnavailableBody -> "The game catalog is not available on this device."
    DiscoveryCopy.Overview -> "About the game"
    DiscoveryCopy.Modes -> "Ways to play"
    DiscoveryCopy.Rules -> "Rules and resources"
    DiscoveryCopy.RulesUnavailable -> "This game version does not provide a rules document link yet."
    DiscoveryCopy.NativeUnavailable -> "Native gameplay is not available in this mobile build yet. You can still explore the game."
    DiscoveryCopy.OnlineUnavailable -> "Online play is not available in this mobile build."
    DiscoveryCopy.Version -> "Version"
    DiscoveryCopy.Rating -> "Content rating"
    DiscoveryCopy.HiddenInformation -> "Hidden information"
    DiscoveryCopy.Yes -> "Yes"
    DiscoveryCopy.No -> "No"
    DiscoveryCopy.Configuration -> "Game settings"
    DiscoveryCopy.ConfigurationDefaults -> "Local launch uses the packaged game's default settings."
    DiscoveryCopy.ConfigurationUnavailable -> "Settings cannot be changed on this device yet. Values below are the game's defaults."
    DiscoveryCopy.ViewSetup -> "View setup"
    DiscoveryCopy.SetupUnavailable -> "A game cannot start yet"
    DiscoveryCopy.Untimed -> "Untimed"
    DiscoveryCopy.NoConfiguration -> "This game uses its default settings."
    DiscoveryCopy.ArtUnavailable -> "Artwork is unavailable; all game information remains readable."
}

fun ShellStrings.discoveryCount(count: Int, total: Int): String =
    if (vietnamese) "$count / $total trò chơi" else "$count of $total games"

fun ShellStrings.discoveryPlayers(counts: List<Int>): String {
    val value = if (counts.size > 1 && counts.zipWithNext().all { (a, b) -> b == a + 1 }) "${counts.first()}–${counts.last()}" else counts.joinToString(", ")
    return if (vietnamese) "$value người chơi" else "$value players"
}

fun ShellStrings.discoveryMinutes(min: Int, max: Int): String =
    if (min == max) { if (vietnamese) "$min phút" else "$min min" }
    else { if (vietnamese) "$min–$max phút" else "$min–$max min" }

fun ShellStrings.discoveryDurationLimit(max: Int): String =
    if (vietnamese) "Tối đa $max phút" else "Up to $max min"

fun ShellStrings.discoveryRulesVersion(version: Int): String =
    if (vietnamese) "Luật phiên bản $version" else "Rules version $version"
