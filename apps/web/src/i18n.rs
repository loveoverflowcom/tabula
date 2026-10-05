//! The shell's message lookup.
//!
//! Two tables are merged: this crate's navigation and task copy, and the
//! catalog's own tables (platform enums plus one per linked game). The split is
//! not stylistic — a shell that spelled out a game's message keys would name
//! that game in its source, which I-9 forbids and `xtask check-no-game-ids`
//! rejects. Game copy therefore arrives as data, through the registry.
//!
//! A key with no translation renders as a visibly labeled fallback rather than
//! as product copy (docs/ui/screens/discovery.md, "Typed data mapping").

use tabula_registry::{I18nKey, Locale, Localizer};

/// Copy owned by the shell: navigation, task headings, states and controls.
///
/// Keys here describe the *shell's* vocabulary only.
const SHELL_EN: &[(&str, &str)] = &[
    ("nav.account", "Account"),
    ("accounts.title", "Account"),
    ("accounts.scope", "This isolated account view can check a session and read your account ID. Sign-in, registration and social features are unavailable."),
    ("accounts.profile.title", "Profile"),
    ("accounts.profile.self", "Your profile"),
    ("accounts.profile.read_only", "Read-only self profile"),
    ("accounts.profile.id", "Account ID"),
    ("accounts.profile.only_id", "Only your immutable account ID is available here. Names, handles, profile editing, history and statistics are not provided."),
    ("accounts.action.check", "Check session"),
    ("accounts.action.refresh", "Refresh session"),
    ("accounts.action.logout", "Sign out"),
    ("accounts.action.logout_retry", "Retry sign-out"),
    ("accounts.action.cancel", "Cancel"),
    ("accounts.action.library", "Browse games"),
    ("accounts.action.back_account", "Back to account"),
    ("accounts.local.explanation", "Supported local games can be played without an account."),
    ("accounts.session.checking", "Checking your session…"),
    ("accounts.session.refreshing", "Refreshing and checking your session…"),
    ("accounts.session.signed_out", "You're signed out."),
    ("accounts.session.confirmed", "The latest check confirmed your session and self profile."),
    ("accounts.session.ended", "Your previous session is no longer confirmed. Check the session again to continue with account features."),
    ("accounts.session.unconfirmed", "Your session couldn't be confirmed. Account data is hidden."),
    ("accounts.service.unavailable", "Account services aren't available. You can still browse games."),
    ("accounts.error.generic", "Couldn't check this account request. Try checking the session again."),
    ("accounts.connection.disconnected", "Can't connect right now. This doesn't confirm sign-out. Check the session when your connection is available."),
    ("accounts.operation.cancelled", "Account checking was cancelled and account data is hidden. You can check the session again."),
    ("accounts.logout.title", "Sign out of this browser?"),
    ("accounts.logout.explanation", "Other tabs sharing this session may lose account access. Local games remain available."),
    ("accounts.logout.pending", "Signing out. Account data is hidden while the server result is checked…"),
    ("accounts.logout.unknown", "Account data is hidden, but server sign-out isn't confirmed. Check the session or retry sign-out."),
    ("accounts.logout.context_changed", "The session context no longer matches this sign-out request. Earlier server sign-out is unconfirmed and account data stays hidden. You can check again or browse games."),
    ("accounts.features.title", "Unavailable account features"),
    ("accounts.login.title", "Sign in"),
    ("accounts.login.unavailable", "Sign-in isn't available in this build. No email or password is collected here."),
    ("accounts.login.link_unavailable", "Sign in · unavailable"),
    ("accounts.register.title", "Create account"),
    ("accounts.register.unavailable", "Account registration isn't available in this build. No account or sign-in session can be created here."),
    ("accounts.register.link_unavailable", "Create account · unavailable"),
    ("accounts.friends.title", "Friends"),
    ("accounts.friends.unavailable", "Friends services aren't available. Friend lists, requests and presence cannot be checked here."),
    ("accounts.friends.link_unavailable", "Friends · unavailable"),
    ("accounts.profile.other_unavailable", "Other-profile lookup isn't available. This address doesn't select your self profile or confirm that an account exists."),
    ("app.title", "Tabula"),
    ("app.skip", "Skip to the main task"),
    ("app.locale", "Language"),
    ("nav.home", "Home"),
    ("nav.library", "Games"),
    ("home.heading", "Continue or start a game"),
    ("home.resume.none", "No match is waiting to be continued."),
    (
        "home.resume.unavailable",
        "This build cannot check for a match to continue: there is no session service yet.",
    ),
    ("home.browse", "Browse all games"),
    ("home.featured", "Available here"),
    ("library.heading", "Games"),
    ("library.search.label", "Search games"),
    ("library.search.clear", "Clear search"),
    ("library.filter.heading", "Filters"),
    ("library.filter.category", "Category"),
    ("library.filter.players", "Players"),
    ("library.filter.duration", "Up to"),
    ("library.filter.complexity", "Complexity"),
    ("library.filter.mode", "Mode"),
    ("library.filter.any", "Any"),
    ("library.filter.reset", "Reset filters"),
    ("library.results.heading", "Results"),
    ("library.results.count", "{0} of {1} games"),
    (
        "library.empty.catalog",
        "No games are available to you in this build.",
    ),
    ("library.empty.filtered", "No games match these filters."),
    (
        "library.filter.invalid",
        "The {0} filter in the address was not a value this catalog offers, so it was not applied.",
    ),
    ("card.details", "View details for {0}"),
    ("card.unavailable", "No mode can start a match here yet."),
    ("detail.back", "Back to games"),
    ("detail.capabilities", "What this game supports"),
    ("detail.modes", "How you can play"),
    ("detail.resources", "Rules and resources"),
    (
        "detail.rules.unavailable",
        "No rules document is published for this game.",
    ),
    ("detail.setup", "Set up a match"),
    ("detail.notfound.heading", "No such game"),
    (
        "detail.notfound.body",
        "This address does not name a game in this catalog.",
    ),
    (
        "detail.invalid.heading",
        "That address is not a game identifier",
    ),
    ("detail.version", "Package {0}, rules version {1}"),
    ("detail.seats", "Players"),
    ("detail.duration", "Estimated length"),
    ("detail.duration.range", "{0}–{1} min"),
    ("detail.complexity", "Complexity"),
    ("detail.rating", "Content rating"),
    ("detail.hidden_information", "Hidden information"),
    ("detail.spectators", "Spectators"),
    ("detail.ranked", "Ranked rules"),
    ("detail.async", "Asynchronous turns"),
    ("detail.declared", "Declared by the rules"),
    (
        "detail.declared.hint",
        "A rules declaration is not a working online feature; see the modes above.",
    ),
    ("value.yes", "Yes"),
    ("value.no", "No"),
    ("seats.exact", "{0}"),
    ("seats.range", "{0}–{1}"),
    ("setup.heading", "Set up a match"),
    ("setup.back", "Back to the game"),
    ("setup.mode", "Mode"),
    ("setup.seats", "Players"),
    ("setup.bot", "Bot level"),
    ("setup.options", "Game options"),
    ("setup.summary.heading", "Summary"),
    ("setup.summary.game", "Game"),
    ("setup.summary.version", "Version"),
    ("setup.state.editing", "Not validated yet"),
    ("setup.state.validating", "Checking this configuration"),
    ("setup.state.opening", "Opening the local game"),
    ("setup.state.ready", "Ready to start"),
    ("setup.state.rejected", "This configuration was rejected"),
    ("setup.state.unavailable", "A match cannot be started here"),
    ("setup.validate", "Check this configuration"),
    ("setup.start", "Start this match"),
    ("setup.resume", "Back to the configuration"),
    ("setup.notvalidated", "Entered values, not yet validated"),
    (
        "setup.nomode",
        "No mode available for this game can start a match.",
    ),
    ("category.abstract", "Abstract"),
    ("category.cards", "Cards"),
    ("category.social_deduction", "Social deduction"),
    ("category.tile_placement", "Tile placement"),
    ("category.party", "Party"),
    ("complexity.light", "Light"),
    ("complexity.medium", "Medium"),
    ("complexity.heavy", "Heavy"),
    ("rating.everyone", "Everyone"),
    ("rating.teen", "Teen"),
    ("rating.mature", "Mature"),
    ("missing", "[no translation: {0}]"),
];

const SHELL_VI: &[(&str, &str)] = &[
    ("nav.account", "Tài khoản"),
    ("accounts.title", "Tài khoản"),
    ("accounts.scope", "Màn hình tài khoản biệt lập này có thể kiểm tra phiên đăng nhập và đọc ID tài khoản của bạn. Đăng nhập, đăng ký và tính năng xã hội chưa khả dụng."),
    ("accounts.profile.title", "Hồ sơ"),
    ("accounts.profile.self", "Hồ sơ của bạn"),
    ("accounts.profile.read_only", "Hồ sơ của bạn, chỉ đọc"),
    ("accounts.profile.id", "ID tài khoản"),
    ("accounts.profile.only_id", "Tại đây chỉ có ID tài khoản cố định của bạn. Tên, tên tài khoản, chỉnh sửa hồ sơ, lịch sử và số liệu chưa được cung cấp."),
    ("accounts.action.check", "Kiểm tra phiên đăng nhập"),
    ("accounts.action.refresh", "Làm mới phiên đăng nhập"),
    ("accounts.action.logout", "Đăng xuất"),
    ("accounts.action.logout_retry", "Thử đăng xuất lại"),
    ("accounts.action.cancel", "Hủy"),
    ("accounts.action.library", "Xem thư viện trò chơi"),
    ("accounts.action.back_account", "Quay lại tài khoản"),
    ("accounts.local.explanation", "Những trò chơi hỗ trợ chơi local không cần tài khoản."),
    ("accounts.session.checking", "Đang kiểm tra phiên đăng nhập…"),
    ("accounts.session.refreshing", "Đang làm mới và kiểm tra phiên đăng nhập…"),
    ("accounts.session.signed_out", "Bạn chưa đăng nhập."),
    ("accounts.session.confirmed", "Lần kiểm tra gần nhất đã xác nhận phiên đăng nhập và hồ sơ của bạn."),
    ("accounts.session.ended", "Phiên đăng nhập trước của bạn không còn được xác nhận. Kiểm tra lại phiên để tiếp tục dùng tính năng tài khoản."),
    ("accounts.session.unconfirmed", "Chưa thể xác nhận phiên đăng nhập. Dữ liệu tài khoản đã được ẩn."),
    ("accounts.service.unavailable", "Dịch vụ tài khoản chưa khả dụng. Bạn vẫn có thể xem thư viện trò chơi."),
    ("accounts.error.generic", "Chưa thể kiểm tra yêu cầu tài khoản này. Hãy thử kiểm tra lại phiên đăng nhập."),
    ("accounts.connection.disconnected", "Hiện chưa thể kết nối. Điều này không xác nhận việc đăng xuất. Kiểm tra phiên đăng nhập khi có kết nối."),
    ("accounts.operation.cancelled", "Đã hủy việc kiểm tra tài khoản và ẩn dữ liệu tài khoản. Bạn có thể kiểm tra lại phiên đăng nhập."),
    ("accounts.logout.title", "Đăng xuất khỏi trình duyệt này?"),
    ("accounts.logout.explanation", "Các thẻ khác dùng chung phiên này có thể mất quyền truy cập tài khoản. Bạn vẫn có thể chơi local."),
    ("accounts.logout.pending", "Đang đăng xuất. Dữ liệu tài khoản được ẩn trong khi kiểm tra kết quả từ máy chủ…"),
    ("accounts.logout.unknown", "Dữ liệu tài khoản đã được ẩn, nhưng chưa xác nhận đăng xuất trên máy chủ. Kiểm tra phiên đăng nhập hoặc thử đăng xuất lại."),
    ("accounts.logout.context_changed", "Ngữ cảnh phiên không còn khớp với yêu cầu đăng xuất này. Chưa xác nhận đăng xuất trước đó trên máy chủ và dữ liệu tài khoản vẫn được ẩn. Bạn có thể kiểm tra lại hoặc xem thư viện trò chơi."),
    ("accounts.features.title", "Tính năng tài khoản chưa khả dụng"),
    ("accounts.login.title", "Đăng nhập"),
    ("accounts.login.unavailable", "Bản dựng này chưa hỗ trợ đăng nhập. Màn hình này không thu thập email hay mật khẩu."),
    ("accounts.login.link_unavailable", "Đăng nhập · chưa khả dụng"),
    ("accounts.register.title", "Tạo tài khoản"),
    ("accounts.register.unavailable", "Bản dựng này chưa hỗ trợ đăng ký tài khoản. Màn hình này không thể tạo tài khoản hay phiên đăng nhập."),
    ("accounts.register.link_unavailable", "Tạo tài khoản · chưa khả dụng"),
    ("accounts.friends.title", "Bạn bè"),
    ("accounts.friends.unavailable", "Dịch vụ bạn bè chưa khả dụng. Màn hình này chưa thể kiểm tra danh sách bạn bè, lời mời hay trạng thái online."),
    ("accounts.friends.link_unavailable", "Bạn bè · chưa khả dụng"),
    ("accounts.profile.other_unavailable", "Chưa hỗ trợ tra cứu hồ sơ người khác. Địa chỉ này không chọn hồ sơ của bạn hay xác nhận tài khoản có tồn tại."),
    ("app.title", "Tabula"),
    ("app.skip", "Tới nội dung chính"),
    ("app.locale", "Ngôn ngữ"),
    ("nav.home", "Trang chính"),
    ("nav.library", "Trò chơi"),
    ("home.heading", "Chơi tiếp hoặc bắt đầu ván mới"),
    ("home.resume.none", "Không có ván nào đang chờ chơi tiếp."),
    (
        "home.resume.unavailable",
        "Bản dựng này chưa kiểm tra được ván đang dở vì chưa có dịch vụ phiên chơi.",
    ),
    ("home.browse", "Xem tất cả trò chơi"),
    ("home.featured", "Có sẵn tại đây"),
    ("library.heading", "Trò chơi"),
    ("library.search.label", "Tìm trò chơi"),
    ("library.search.clear", "Xoá tìm kiếm"),
    ("library.filter.heading", "Bộ lọc"),
    ("library.filter.category", "Thể loại"),
    ("library.filter.players", "Số người chơi"),
    ("library.filter.duration", "Tối đa"),
    ("library.filter.complexity", "Độ phức tạp"),
    ("library.filter.mode", "Chế độ"),
    ("library.filter.any", "Tất cả"),
    ("library.filter.reset", "Xoá bộ lọc"),
    ("library.results.heading", "Kết quả"),
    ("library.results.count", "{0} trên {1} trò chơi"),
    (
        "library.empty.catalog",
        "Bản dựng này chưa có trò chơi nào dành cho bạn.",
    ),
    ("library.empty.filtered", "Không có trò chơi nào khớp bộ lọc này."),
    (
        "library.filter.invalid",
        "Bộ lọc {0} trong địa chỉ không phải giá trị danh mục này có, nên đã không được áp dụng.",
    ),
    ("card.details", "Xem chi tiết {0}"),
    ("card.unavailable", "Chưa có chế độ nào bắt đầu được ván tại đây."),
    ("detail.back", "Về danh sách trò chơi"),
    ("detail.capabilities", "Trò chơi này hỗ trợ gì"),
    ("detail.modes", "Các cách chơi"),
    ("detail.resources", "Luật chơi và tài nguyên"),
    ("detail.rules.unavailable", "Chưa có tài liệu luật chơi cho trò này."),
    ("detail.setup", "Thiết lập ván"),
    ("detail.notfound.heading", "Không có trò chơi này"),
    (
        "detail.notfound.body",
        "Địa chỉ này không trỏ tới trò chơi nào trong danh mục.",
    ),
    ("detail.invalid.heading", "Địa chỉ này không phải định danh trò chơi"),
    ("detail.version", "Gói {0}, phiên bản luật {1}"),
    ("detail.seats", "Số người chơi"),
    ("detail.duration", "Thời lượng ước tính"),
    ("detail.duration.range", "{0}–{1} phút"),
    ("detail.complexity", "Độ phức tạp"),
    ("detail.rating", "Phân loại nội dung"),
    ("detail.hidden_information", "Thông tin ẩn"),
    ("detail.spectators", "Người xem"),
    ("detail.ranked", "Luật xếp hạng"),
    ("detail.async", "Lượt không đồng bộ"),
    ("detail.declared", "Theo khai báo của luật chơi"),
    (
        "detail.declared.hint",
        "Khai báo của luật chơi không phải tính năng trực tuyến đang chạy; hãy xem phần chế độ ở trên.",
    ),
    ("value.yes", "Có"),
    ("value.no", "Không"),
    ("seats.exact", "{0}"),
    ("seats.range", "{0}–{1}"),
    ("setup.heading", "Thiết lập ván"),
    ("setup.back", "Về trang trò chơi"),
    ("setup.mode", "Chế độ"),
    ("setup.seats", "Số người chơi"),
    ("setup.bot", "Mức máy chơi"),
    ("setup.options", "Tuỳ chọn trò chơi"),
    ("setup.summary.heading", "Tóm tắt"),
    ("setup.summary.game", "Trò chơi"),
    ("setup.summary.version", "Phiên bản"),
    ("setup.state.editing", "Chưa kiểm tra"),
    ("setup.state.validating", "Đang kiểm tra cấu hình"),
    ("setup.state.opening", "Đang mở ván cục bộ"),
    ("setup.state.ready", "Sẵn sàng bắt đầu"),
    ("setup.state.rejected", "Cấu hình này bị từ chối"),
    ("setup.state.unavailable", "Chưa thể bắt đầu ván tại đây"),
    ("setup.validate", "Kiểm tra cấu hình"),
    ("setup.start", "Bắt đầu ván này"),
    ("setup.resume", "Quay lại cấu hình"),
    ("setup.notvalidated", "Giá trị đã nhập, chưa kiểm tra"),
    ("setup.nomode", "Không có chế độ nào của trò chơi này bắt đầu được ván."),
    ("category.abstract", "Trừu tượng"),
    ("category.cards", "Bài"),
    ("category.social_deduction", "Suy luận xã hội"),
    ("category.tile_placement", "Xếp mảnh"),
    ("category.party", "Vui nhộn"),
    ("complexity.light", "Nhẹ"),
    ("complexity.medium", "Vừa"),
    ("complexity.heavy", "Nặng"),
    ("rating.everyone", "Mọi lứa tuổi"),
    ("rating.teen", "Thiếu niên"),
    ("rating.mature", "Người lớn"),
    ("missing", "[chưa có bản dịch: {0}]"),
];

/// The merged lookup for one locale.
#[derive(Clone, Debug)]
pub struct Messages {
    locale: Locale,
    table: Vec<(&'static str, &'static str)>,
}

impl Messages {
    /// Shell copy first, then the platform and per-game tables of every game
    /// this build links.
    #[must_use]
    pub fn new(locale: Locale) -> Self {
        let shell = match locale {
            Locale::En => SHELL_EN,
            Locale::Vi => SHELL_VI,
        };
        let mut table = shell.to_vec();
        table.extend(tabula_registry::platform_messages(locale));
        for game in tabula_registry::registered_games() {
            table.extend_from_slice(game.messages(locale));
        }
        Self { locale, table }
    }

    #[must_use]
    pub const fn locale(&self) -> Locale {
        self.locale
    }

    /// The raw entry for a key, if this locale has one.
    #[must_use]
    pub fn lookup(&self, key: &str) -> Option<&'static str> {
        self.table
            .iter()
            .find_map(|(candidate, text)| (*candidate == key).then_some(*text))
    }

    /// Copy for a key, or a visibly labeled fallback naming the missing key.
    #[must_use]
    pub fn text(&self, key: &str) -> String {
        self.lookup(key).map_or_else(
            || {
                self.lookup("missing")
                    .unwrap_or("[{0}]")
                    .replace("{0}", key)
            },
            ToOwned::to_owned,
        )
    }

    /// Copy for a key with `{0}`, `{1}`, … replaced in order.
    ///
    /// Substituting into one complete sentence keeps word order translatable;
    /// concatenating fragments does not.
    #[must_use]
    pub fn format(&self, key: &str, args: &[&str]) -> String {
        let mut text = self.text(key);
        for (index, arg) in args.iter().enumerate() {
            text = text.replace(&format!("{{{index}}}"), arg);
        }
        text
    }
}

impl Localizer for Messages {
    fn text(&self, key: &I18nKey) -> Option<&str> {
        self.lookup(key.as_str())
    }
}

/// The catalog and message table for one locale.
///
/// Ordering is locale-dependent, so the two are always built together.
#[must_use]
pub fn shell(locale: Locale) -> (Messages, tabula_registry::Catalog) {
    let messages = Messages::new(locale);
    let catalog = tabula_registry::Catalog::new(tabula_registry::registered_games(), &messages);
    (messages, catalog)
}

#[cfg(test)]
mod tests {
    use super::{Messages, SHELL_EN, SHELL_VI};
    use std::collections::BTreeSet;
    use tabula_registry::Locale;

    fn arguments(text: &str) -> BTreeSet<&str> {
        text.split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(argument, _)| argument))
            .collect()
    }

    #[test]
    fn shell_locales_have_unique_matching_keys_and_argument_shapes() {
        let en: BTreeSet<_> = SHELL_EN.iter().map(|(key, _)| *key).collect();
        let vi: BTreeSet<_> = SHELL_VI.iter().map(|(key, _)| *key).collect();
        assert_eq!(en.len(), SHELL_EN.len(), "duplicate English key");
        assert_eq!(vi.len(), SHELL_VI.len(), "duplicate Vietnamese key");
        assert_eq!(en, vi, "a shell message is missing in one locale");
        for (key, english) in SHELL_EN {
            let vietnamese = SHELL_VI
                .iter()
                .find_map(|(candidate, text)| (candidate == key).then_some(*text))
                .expect("Vietnamese key parity");
            assert!(!english.is_empty(), "{key}");
            assert!(!vietnamese.is_empty(), "{key}");
            assert_eq!(arguments(english), arguments(vietnamese), "{key}");
        }
    }

    #[test]
    fn account_copy_is_installed_and_missing_keys_remain_visibly_labelled() {
        for locale in Locale::ALL {
            let messages = Messages::new(locale);
            for (key, _) in SHELL_EN
                .iter()
                .filter(|(key, _)| key.starts_with("accounts."))
            {
                assert!(messages.lookup(key).is_some(), "{key}");
                assert!(!messages.text(key).contains("{0}"), "{key}");
            }
            let missing = messages.text("accounts.not_installed");
            assert!(missing.contains("accounts.not_installed"));
            assert!(missing.starts_with('['));
            assert!(missing.ends_with(']'));
        }
    }
}
