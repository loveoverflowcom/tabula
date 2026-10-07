//! Message tables owned by the catalog boundary.
//!
//! A game's visible copy — its name, its fields, its choices — belongs to the
//! game, and the shell must not carry it: a shell that spelled out one game's
//! message keys would be branching on that game in all but name (I-9, enforced
//! by `xtask check-no-game-ids`). Each adapter therefore ships its own table,
//! and the platform keys that come from *this* crate's enums ship next to them.
//!
//! Tables are flat `(key, text)` slices: the Phase-5 shell merges them into one
//! lookup, and a real message catalog replaces this without changing the keys.

/// A locale the shell can render. Both are required copy (doc 07 §i18n).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    En,
    Vi,
}

impl Locale {
    pub const ALL: [Self; 2] = [Self::En, Self::Vi];

    /// BCP-47 tag for the document's `lang` attribute.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Vi => "vi",
        }
    }

    #[must_use]
    pub fn parse(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|locale| locale.tag() == tag)
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Vi => "Tiếng Việt",
        }
    }
}

/// A flat message table.
pub type Messages = &'static [(&'static str, &'static str)];

const PLATFORM_EN: Messages = &[
    ("rating.everyone", "Everyone"),
    ("rating.teen", "Teen"),
    ("rating.mature", "Mature"),
    ("category.abstract", "Abstract"),
    ("category.cards", "Cards"),
    ("category.social_deduction", "Social deduction"),
    ("category.tile_placement", "Tile placement"),
    ("category.party", "Party"),
    ("complexity.light", "Light"),
    ("complexity.medium", "Medium"),
    ("complexity.heavy", "Heavy"),
    ("mode.local.label", "Two players, one device"),
    (
        "mode.local.consequence",
        "Players share one device, taking turns on the same screen.",
    ),
    ("mode.bots.label", "Against the game's bot"),
    (
        "mode.bots.consequence",
        "A human takes the first seat; bots take the other seats.",
    ),
    ("mode.network.label", "Over the network"),
    (
        "mode.network.consequence",
        "Play other people online once the service exists.",
    ),
    (
        "unavailable.no_bot_factory.reason",
        "This build links no bot for this game.",
    ),
    (
        "unavailable.no_bot_factory.recovery",
        "Choose a mode that does not need a bot.",
    ),
    (
        "unavailable.no_network_service.reason",
        "There is no online service yet, so no networked match can be created.",
    ),
    (
        "unavailable.no_network_service.recovery",
        "Play on this device instead.",
    ),
    (
        "unavailable.no_gameplay_runtime.reason",
        "This build of the shell has no gameplay document to hand off to, so it cannot start a session.",
    ),
    (
        "unavailable.no_gameplay_runtime.recovery",
        "Use a build that ships the gameplay bundle; the configuration below is still validated.",
    ),
    ("unavailable.no_bot_runtime.reason", "This gameplay runtime does not support playing against a bot yet."),
    ("unavailable.no_bot_runtime.recovery", "Choose two players on this device."),
    ("unavailable.no_mode_runtime.reason", "The gameplay bundle does not implement this game and mode."),
    ("unavailable.no_mode_runtime.recovery", "Return to the catalog or use a build that supports this mode."),
    ("unavailable.navigation_failed.reason", "The browser could not open the gameplay document."),
    ("unavailable.navigation_failed.recovery", "Return to the configuration and try again."),
    ("bot.trivial", "Trivial"),
    ("bot.easy", "Easy"),
    ("bot.medium", "Medium"),
    ("bot.hard", "Hard"),
    ("setup.summary.mode", "Mode"),
    ("setup.summary.seats", "Players"),
    ("setup.summary.time", "Clock"),
    ("setup.summary.deadline", "Turn deadline"),
    ("setup.reject.missing", "Choose a value for this field."),
    (
        "setup.reject.not_a_number",
        "Enter a whole number, with no decimal point, sign, or unit.",
    ),
    (
        "setup.reject.out_of_range",
        "Enter a number between {0} and {1}.",
    ),
    (
        "setup.reject.unknown_choice",
        "That option is not offered for this game.",
    ),
    (
        "setup.reject.seat_count",
        "This game does not support that number of players.",
    ),
    (
        "setup.reject.module_field",
        "The game rejected this value. Check the field and try again.",
    ),
    (
        "setup.reject.unsupported",
        "This game does not support that combination.",
    ),
    ("summary.time.untimed", "No clock"),
    ("summary.time.increment", "{0} plus {1} added after each move"),
    ("summary.time.delay", "{0} with the first {1} of each move free"),
    ("summary.seats", "{0} players"),
    ("unit.minutes", "{0} min"),
    ("unit.seconds", "{0} s"),
];

const PLATFORM_VI: Messages = &[
    ("rating.everyone", "Mọi lứa tuổi"),
    ("rating.teen", "Thiếu niên"),
    ("rating.mature", "Người lớn"),
    ("category.abstract", "Trừu tượng"),
    ("category.cards", "Bài"),
    ("category.social_deduction", "Suy luận xã hội"),
    ("category.tile_placement", "Xếp mảnh"),
    ("category.party", "Vui nhộn"),
    ("complexity.light", "Nhẹ"),
    ("complexity.medium", "Vừa"),
    ("complexity.heavy", "Nặng"),
    ("mode.local.label", "Hai người, một máy"),
    (
        "mode.local.consequence",
        "Mọi người dùng chung một thiết bị, lần lượt chơi trên cùng màn hình.",
    ),
    ("mode.bots.label", "Đấu với máy của trò chơi"),
    (
        "mode.bots.consequence",
        "Người chơi giữ ghế đầu tiên; máy giữ các ghế còn lại.",
    ),
    ("mode.network.label", "Qua mạng"),
    (
        "mode.network.consequence",
        "Chơi với người khác trực tuyến khi dịch vụ sẵn sàng.",
    ),
    (
        "unavailable.no_bot_factory.reason",
        "Bản dựng này không kèm máy chơi cho trò này.",
    ),
    (
        "unavailable.no_bot_factory.recovery",
        "Hãy chọn chế độ không cần máy chơi.",
    ),
    (
        "unavailable.no_network_service.reason",
        "Chưa có dịch vụ trực tuyến nên không thể tạo ván qua mạng.",
    ),
    (
        "unavailable.no_network_service.recovery",
        "Hãy chơi ngay trên máy này.",
    ),
    (
        "unavailable.no_gameplay_runtime.reason",
        "Bản dựng này của shell không kèm tài liệu gameplay để bàn giao nên chưa thể bắt đầu ván.",
    ),
    (
        "unavailable.no_gameplay_runtime.recovery",
        "Hãy dùng bản dựng có kèm gói gameplay; cấu hình bên dưới vẫn được kiểm tra.",
    ),
    (
        "unavailable.no_bot_runtime.reason",
        "Bản gameplay này chưa hỗ trợ đấu với máy.",
    ),
    (
        "unavailable.no_bot_runtime.recovery",
        "Hãy chọn hai người chơi trên máy này.",
    ),
    (
        "unavailable.no_mode_runtime.reason",
        "Gói gameplay chưa hỗ trợ trò chơi và chế độ này.",
    ),
    (
        "unavailable.no_mode_runtime.recovery",
        "Hãy về danh mục hoặc dùng bản dựng hỗ trợ chế độ này.",
    ),
    (
        "unavailable.navigation_failed.reason",
        "Trình duyệt chưa mở được trang gameplay.",
    ),
    (
        "unavailable.navigation_failed.recovery",
        "Hãy quay lại cấu hình và thử lại.",
    ),
    ("bot.trivial", "Ngẫu nhiên"),
    ("bot.easy", "Dễ"),
    ("bot.medium", "Vừa"),
    ("bot.hard", "Khó"),
    ("setup.summary.mode", "Chế độ"),
    ("setup.summary.seats", "Số người chơi"),
    ("setup.summary.time", "Đồng hồ"),
    ("setup.summary.deadline", "Hạn mỗi lượt"),
    ("setup.reject.missing", "Hãy chọn một giá trị cho ô này."),
    (
        "setup.reject.not_a_number",
        "Hãy nhập số nguyên, không dấu thập phân, dấu âm hay đơn vị.",
    ),
    (
        "setup.reject.out_of_range",
        "Hãy nhập số trong khoảng {0}–{1}.",
    ),
    (
        "setup.reject.unknown_choice",
        "Trò chơi này không có lựa chọn đó.",
    ),
    (
        "setup.reject.seat_count",
        "Trò chơi này không hỗ trợ số người chơi đó.",
    ),
    (
        "setup.reject.module_field",
        "Trò chơi từ chối giá trị này. Hãy kiểm tra lại ô nhập.",
    ),
    (
        "setup.reject.unsupported",
        "Trò chơi này không hỗ trợ tổ hợp đó.",
    ),
    ("summary.time.untimed", "Không tính giờ"),
    ("summary.time.increment", "{0}, cộng thêm {1} sau mỗi nước"),
    ("summary.time.delay", "{0}, mỗi nước được hoãn {1} đầu tiên"),
    ("summary.seats", "{0} người chơi"),
    ("unit.minutes", "{0} phút"),
    ("unit.seconds", "{0} giây"),
];

/// Copy for the enums this crate defines: modes, reasons, bot levels, summary
/// labels and rejection messages.
#[must_use]
pub const fn platform_messages(locale: Locale) -> Messages {
    match locale {
        Locale::En => PLATFORM_EN,
        Locale::Vi => PLATFORM_VI,
    }
}
