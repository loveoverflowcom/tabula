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
    ("shell.navigation", "Main navigation"),
    ("shell.context", "Your play space"),
    ("shell.context.game", "Game details"),
    ("shell.menu.open", "Open navigation"),
    ("shell.menu.close", "Close navigation"),
    ("shell.note", "Slow down. Connect a little more."),
    ("shell.note.detail", "A small pause, just for you."),
    ("shell.upcoming", "Coming to your play space"),
    ("shell.history", "My games"),
    ("shell.rooms", "Play rooms"),
    ("shell.upcoming.reason", "History and rooms are not available in this build."),
    ("home.eyebrow", "Your play space"),
    ("home.lead", "A little pause. A new move."),
    ("home.hero.eyebrow", "A little discovery"),
    // Discretionary syllable breaks preserve the authored words at large text.
    ("home.hero.heading", "Every move,\na little dis\u{00ad}cov\u{00ad}ery."),
    ("home.hero.body", "Find a game that feels right. Explore the ways you can play."),
    ("home.hero.action", "Explore games"),
    ("home.continue.heading", "Continue playing"),
    ("home.continue.status", "Saved games are unavailable here yet."),
    ("home.continue.body", "Start a new game whenever you’re ready."),
    ("card.players", "{0} players"),
    ("online.heading", "Play with a friend"),
    ("online.safety_cleanup_failed", "The result above is confirmed, but this browser could not clear duplicate-request protection. New admission requests stay blocked. Keep this page open to retain the result."),
    ("online.signin_unavailable", "Sign-in is required for online play and is unavailable in this build. You can check Account or keep playing locally."),
    ("online.check_account", "Check Account"),
    ("online.signin_hint", "Keep the shared code handy: provider sign-in returns to Account in a new document, so you’ll need to enter it again."),
    ("online.creating", "Creating match…"),
    ("online.joining", "Joining match…"),
    ("online.code.placeholder", "Enter the shared code"),
    ("online.session_pending", "Checking your session…"),
    ("online.session_unavailable", "Your session could not be checked. Check again before creating or joining."),
    ("online.context_changed", "Your session changed before admission was sent. Check your session again."),
    ("online.recheck", "Check session again"),
    ("online.unknown", "The request outcome is unconfirmed. Creating or joining again is blocked in this tab, including after reload, to avoid a duplicate. This service has no admission-recovery check yet."),
    ("online.safety_unavailable", "This browser could not save duplicate-request protection. No new admission request was sent. Allow session storage before trying again."),
    ("online.handoff_failed", "The game document could not be opened. Your match and code are kept here. Try Open board again or return to the library."),
    ("online.local_hint", "Supported local play is still available in game setup."),
    ("online.quick.heading", "Quick match"),
    ("online.quick.unavailable", "Not supported"),
    ("online.quick.reason", "Opponent matching is unavailable in this build."),
    ("online.scope", "Two people · unranked · untimed"),
    ("online.ready", "Create a match, or enter the code shared with you."),
    ("online.pending", "Checking your session and match…"),
    ("online.create", "Create match and get code"),
    ("online.code.label", "Already have a join code?"),
    ("online.join", "Join"),
    ("online.code.share", "Share this code with the other player:"),
    ("online.seat", "Your seat:"),
    ("online.enter", "Open board"),
    ("online.account", "Sign in"),
    ("online.waiting", "Match created. Share the code, then open the board. The board waits until the other player joins."),
    ("online.joined", "You joined. Open the board to play."),
    ("online.incompatible", "This game or configuration is unavailable for direct play in this build."),
    ("online.unavailable", "The match service could not complete this request."),
    ("online.signin", "Sign in to create or join an online match."),
    ("online.invalid_code", "You can’t join with this code. Check it with the other player."),
    ("online.disconnected", "Connection lost before admission was sent. Check your session to try again."),
    ("nav.account", "Account"),
    ("accounts.title", "Account"),
    ("accounts.intro.account", "Manage this browser's session and view your profile."),
    ("accounts.intro.login", "Sign in with your Kanidm identity, or keep playing locally."),
    ("accounts.intro.profile", "Read the profile information available to your account."),
    ("accounts.login.already_signed_in", "You're already signed in. Open your profile, or sign out below to use a different account."),
    ("accounts.action.profile", "View your profile"),
    ("accounts.action.signin", "Go to sign-in"),
    ("accounts.local.title", "Play without an account"),
    ("accounts.features.unavailable", "Unavailable in this build"),
    ("accounts.register.reason", "New account creation is not supported. Invited accounts can use sign-in."),
    ("accounts.friends.reason", "Friend lists, requests and presence need a supported social service."),
    ("accounts.scope", "Check your session and read your account ID. Invited-account sign-in is available when the provider is configured. Registration and social features remain unavailable."),
    ("accounts.profile.title", "Profile"),
    ("accounts.profile.self", "Your profile"),
    ("accounts.profile.read_only", "Read-only self profile"),
    ("accounts.profile.id", "Account ID"),
    ("accounts.profile.only_id", "Your account ID is read-only."),
    ("accounts.action.check", "Check session"),
    ("accounts.action.login", "Continue with Kanidm"),
    ("accounts.login.invited", "Continue to Kanidm to sign in with your identity. You will return to Account."),
    ("accounts.login.switch", "To use a different account, sign out of this browser session first."),
    ("accounts.login.starting", "Preparing sign-in…"),
    ("accounts.login.redirecting", "Opening Kanidm sign-in…"),
    ("accounts.login.failed", "Sign-in couldn't be started. Account data stays hidden."),
    ("accounts.login.retry", "Check the session again before retrying sign-in."),
    ("accounts.logout.storage_unavailable", "Browser sign-out protection is unavailable. Account data stays hidden."),
    ("accounts.logout.storage_help", "Allow this site's browser storage, then check the session again. If sign-out was interrupted, reloading may lose its local protection; server sign-out is not confirmed by this warning."),
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
    ("accounts.logout.context_changed", "The session no longer matches the earlier sign-out target. That sign-out remains unconfirmed, and its retry cannot target this session. Account data stays hidden. Check again or contact the test operator for recovery."),
    ("accounts.features.title", "Unavailable account features"),
    ("accounts.login.title", "Sign in"),
    ("accounts.login.unavailable", "Sign-in cannot start from this session check. Check the session again; sign-in requires a configured provider. No email or password is collected here."),
    ("accounts.login.link_unavailable", "Sign in · unavailable"),
    ("accounts.full.checking", "Checking account information…"),
    ("accounts.full.ready", "Account information is ready."),
    ("accounts.full.error", "This request could not be checked."),
    ("accounts.full.pending", "Sending your request…"),
    ("accounts.full.unknown", "The result is not confirmed. Reconcile or retry the same request."),
    ("accounts.full.conflict", "The profile changed. Refetch the current profile before saving again."),
    ("accounts.full.saved", "Your changes were saved."),
    ("accounts.full.safe_error", "Account details stay hidden unless the current session permits them."),
    ("accounts.full.enrollment_intro", "Confirm your identity with Kanidm, then choose your account handle and display name."),
    ("accounts.full.accepted_without_session", "Your Tabula account is ready. You are still signed out; choose Sign in."),
    ("accounts.full.register_rejected", "This registration could not be accepted. Check your details or start again."),
    ("accounts.full.fields_invalid", "Check the handle and display name requirements below."),
    ("accounts.full.handle", "Handle"),
    ("accounts.full.handle_help", "3–32 lowercase ASCII letters, digits or underscores. Your handle cannot be edited later."),
    ("accounts.full.display_name", "Display name"),
    ("accounts.full.name_help", "1–64 characters, at most 256 UTF-8 bytes. Control characters are not allowed."),
    ("accounts.full.enrollment_start", "Confirm identity with Kanidm"),
    ("accounts.full.reconcile", "Check registration result"),
    ("accounts.full.missing_profile", "This account has no registration profile. When registration is available, sign out, then use Create account to confirm the same identity and complete the missing profile."),
    ("accounts.full.profile_details", "Profile details"),
    ("accounts.full.visibility", "Profile visibility"),
    ("accounts.full.visibility.private", "Private"),
    ("accounts.full.visibility.friends", "Friends"),
    ("accounts.full.visibility.public", "Public"),
    ("accounts.full.edit", "Edit profile"),
    ("accounts.full.save", "Save changes"),
    ("accounts.full.refetch", "Refetch current profile"),
    ("accounts.full.retry_same", "Retry the same request"),
    ("accounts.full.friends_intro", "Find accounts by handle and manage your friend requests."),
    ("accounts.full.search", "Search accounts"),
    ("accounts.full.search_help", "Enter a lowercase account handle. Search results contain only permitted information."),
    ("accounts.full.friends_list", "Friends"),
    ("accounts.full.requests", "Friend requests"),
    ("accounts.full.empty", "Nothing to show."),
    ("accounts.full.send", "Send friend request"),
    ("accounts.full.accept", "Accept request"),
    ("accounts.full.decline", "Decline request"),
    ("accounts.full.cancel_request", "Cancel request"),
    ("accounts.full.observed_at", "Observed at"),
    ("accounts.full.last_seen", "Last seen"),
    ("accounts.full.pending_request", "Pending"),
    ("accounts.full.accepted_request", "Accepted"),
    ("accounts.full.declined_request", "Declined"),
    ("accounts.full.expired_request", "Expired"),
    ("accounts.full.cancelled_request", "Cancelled"),
    ("accounts.full.presence.unknown", "Presence unknown"),
    ("accounts.full.presence.online", "Online"),
    ("accounts.full.presence.offline", "Offline"),
    ("accounts.full.presence.stale", "Presence is stale"),
    ("accounts.full.resync", "Reload friends"),
    ("accounts.full.friends_available", "Manage friends and requests."),
    ("accounts.full.register_available", "Confirm identity and create your account."),
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
    ("nav.library", "Library"),
    ("home.heading", "Shall we play?"),
    ("home.browse", "Browse all games"),
    ("home.featured", "Find your next little joy"),
    ("library.heading", "Game library"),
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
    ("missing", "[no translation: {0}]"),
];

const SHELL_VI: &[(&str, &str)] = &[
    ("shell.navigation", "Điều hướng chính"),
    ("shell.context", "Không gian chơi"),
    ("shell.context.game", "Chi tiết game"),
    ("shell.menu.open", "Mở điều hướng"),
    ("shell.menu.close", "Đóng điều hướng"),
    ("shell.note", "Chơi chậm lại. Kết nối nhiều hơn."),
    ("shell.note.detail", "Một khoảng nghỉ cho riêng bạn."),
    ("shell.upcoming", "Góc chơi sắp có"),
    ("shell.history", "Ván của tôi"),
    ("shell.rooms", "Phòng chơi"),
    ("shell.upcoming.reason", "Lịch sử và phòng chơi chưa khả dụng trong bản này."),
    ("home.eyebrow", "Không gian chơi của bạn"),
    ("home.lead", "Một khoảng nghỉ nhỏ. Một nước đi mới."),
    ("home.hero.eyebrow", "Góc khám phá mới"),
    ("home.hero.heading", "Mỗi nước đi,\nmột điều để học."),
    ("home.hero.body", "Tìm game hợp với bạn. Chọn cách chơi được hỗ trợ."),
    ("home.hero.action", "Khám phá game"),
    ("home.continue.heading", "Tiếp tục ván"),
    ("home.continue.status", "Chưa hỗ trợ tìm ván đã lưu ở đây."),
    ("home.continue.body", "Bạn có thể bắt đầu một ván mới khi sẵn sàng."),
    ("card.players", "{0} người"),
    ("online.heading", "Chơi cùng bạn"),
    ("online.safety_cleanup_failed", "Kết quả ở trên đã được xác nhận, nhưng trình duyệt chưa xóa được cơ chế chống gửi trùng. Yêu cầu tạo hoặc tham gia mới vẫn bị khóa. Giữ trang này mở để giữ kết quả."),
    ("online.signin_unavailable", "Chơi trực tuyến cần đăng nhập, nhưng bản này chưa hỗ trợ đăng nhập. Kiểm tra Tài khoản hoặc tiếp tục chơi cục bộ."),
    ("online.check_account", "Kiểm tra Tài khoản"),
    ("online.signin_hint", "Giữ mã được chia sẻ: đăng nhập qua nhà cung cấp quay lại Tài khoản trong trang mới, nên cần nhập lại mã."),
    ("online.creating", "Đang tạo trận…"),
    ("online.joining", "Đang tham gia…"),
    ("online.code.placeholder", "Nhập mã được chia sẻ"),
    ("online.session_pending", "Đang kiểm tra phiên…"),
    ("online.session_unavailable", "Chưa kiểm tra được phiên. Kiểm tra lại trước khi tạo hoặc tham gia trận."),
    ("online.context_changed", "Phiên đã thay đổi trước khi gửi yêu cầu. Kiểm tra lại phiên."),
    ("online.recheck", "Kiểm tra lại phiên"),
    ("online.unknown", "Chưa xác nhận kết quả yêu cầu. Tab này khóa tạo và tham gia lại, kể cả sau khi tải lại, để tránh gửi trùng. Dịch vụ chưa hỗ trợ kiểm tra khôi phục yêu cầu."),
    ("online.safety_unavailable", "Trình duyệt chưa lưu được cơ chế chống gửi trùng. Chưa gửi yêu cầu tạo hoặc tham gia trận mới. Cho phép lưu trữ phiên trước khi thử lại."),
    ("online.handoff_failed", "Chưa mở được trang chơi. Trận và mã vẫn được giữ ở đây. Thử Mở bàn cờ lần nữa hoặc quay lại thư viện."),
    ("online.local_hint", "Vẫn có thể chơi cục bộ trong phần thiết lập trò chơi."),
    ("online.quick.heading", "Ghép nhanh"),
    ("online.quick.unavailable", "Chưa hỗ trợ"),
    ("online.quick.reason", "Bản này chưa có dịch vụ ghép đối thủ."),
    ("online.scope", "Hai người · không xếp hạng · không đồng hồ"),
    ("online.ready", "Tạo ván mới hoặc nhập mã được chia sẻ."),
    ("online.pending", "Đang kiểm tra phiên và ván…"),
    ("online.create", "Tạo trận và lấy mã"),
    ("online.code.label", "Đã có mã tham gia?"),
    ("online.join", "Tham gia"),
    ("online.code.share", "Chia sẻ mã này với người chơi còn lại:"),
    ("online.seat", "Chỗ của bạn:"),
    ("online.enter", "Mở bàn cờ"),
    ("online.account", "Đăng nhập"),
    ("online.waiting", "Đã tạo ván. Chia sẻ mã rồi mở bàn cờ. Bàn cờ đợi người còn lại tham gia."),
    ("online.joined", "Đã tham gia. Mở bàn cờ để chơi."),
    ("online.incompatible", "Trò chơi hoặc thiết lập này chưa hỗ trợ chơi trực tiếp."),
    ("online.unavailable", "Dịch vụ ván chưa thể hoàn tất yêu cầu."),
    ("online.signin", "Đăng nhập để tạo hoặc tham gia ván trực tuyến."),
    ("online.invalid_code", "Không thể tham gia bằng mã này. Kiểm tra mã với người chơi còn lại."),
    ("online.disconnected", "Mất kết nối trước khi gửi yêu cầu tham gia. Kiểm tra phiên để thử lại."),
    ("nav.account", "Tài khoản"),
    ("accounts.title", "Tài khoản"),
    ("accounts.intro.account", "Quản lý phiên của trình duyệt này và xem hồ sơ của bạn."),
    ("accounts.intro.login", "Đăng nhập bằng tài khoản được mời, hoặc tiếp tục chơi local."),
    ("accounts.intro.profile", "Đọc thông tin hồ sơ được cung cấp cho tài khoản của bạn."),
    ("accounts.login.already_signed_in", "Bạn đã đăng nhập. Mở hồ sơ, hoặc đăng xuất bên dưới để dùng tài khoản khác."),
    ("accounts.action.profile", "Xem hồ sơ của bạn"),
    ("accounts.action.signin", "Đến màn hình đăng nhập"),
    ("accounts.local.title", "Chơi không cần tài khoản"),
    ("accounts.features.unavailable", "Chưa khả dụng trong bản dựng này"),
    ("accounts.register.reason", "Chưa hỗ trợ tạo tài khoản mới. Tài khoản được mời có thể đăng nhập."),
    ("accounts.friends.reason", "Danh sách, lời mời và trạng thái bạn bè cần dịch vụ xã hội được hỗ trợ."),
    ("accounts.scope", "Kiểm tra phiên đăng nhập và đọc ID tài khoản của bạn. Tài khoản được cấp hoặc mời có thể đăng nhập khi nhà cung cấp đã được cấu hình. Đăng ký và tính năng xã hội chưa khả dụng."),
    ("accounts.profile.title", "Hồ sơ"),
    ("accounts.profile.self", "Hồ sơ của bạn"),
    ("accounts.profile.read_only", "Hồ sơ của bạn, chỉ đọc"),
    ("accounts.profile.id", "ID tài khoản"),
    ("accounts.profile.only_id", "ID tài khoản của bạn chỉ có thể đọc."),
    ("accounts.action.check", "Kiểm tra phiên đăng nhập"),
    ("accounts.action.login", "Tiếp tục với Kanidm"),
    ("accounts.login.invited", "Tiếp tục đến Kanidm để đăng nhập bằng danh tính của bạn. Sau đó bạn sẽ quay lại Tài khoản."),
    ("accounts.login.switch", "Muốn dùng tài khoản khác, hãy đăng xuất phiên trình duyệt này trước."),
    ("accounts.login.starting", "Đang chuẩn bị đăng nhập…"),
    ("accounts.login.redirecting", "Đang mở trang đăng nhập Kanidm…"),
    ("accounts.login.failed", "Chưa thể bắt đầu đăng nhập. Dữ liệu tài khoản vẫn được ẩn."),
    ("accounts.login.retry", "Hãy kiểm tra lại phiên trước khi thử đăng nhập lại."),
    ("accounts.logout.storage_unavailable", "Chưa thể dùng cơ chế bảo vệ đăng xuất của trình duyệt. Dữ liệu tài khoản vẫn được ẩn."),
    ("accounts.logout.storage_help", "Cho phép trang này dùng bộ nhớ trình duyệt rồi kiểm tra lại phiên. Nếu đăng xuất bị gián đoạn, tải lại có thể làm mất trạng thái bảo vệ cục bộ; cảnh báo này không xác nhận đăng xuất trên máy chủ."),
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
    ("accounts.logout.context_changed", "Phiên này không còn khớp với phiên cần đăng xuất trước đó. Chưa xác nhận lần đăng xuất ấy và không thể dùng yêu cầu cũ để đăng xuất phiên này. Dữ liệu tài khoản vẫn được ẩn. Hãy kiểm tra lại hoặc liên hệ người vận hành thử nghiệm để khôi phục."),
    ("accounts.features.title", "Tính năng tài khoản chưa khả dụng"),
    ("accounts.login.title", "Đăng nhập"),
    ("accounts.login.unavailable", "Chưa thể bắt đầu đăng nhập từ lần kiểm tra này. Hãy kiểm tra lại phiên; đăng nhập cần nhà cung cấp đã được cấu hình. Màn hình này không thu thập email hay mật khẩu."),
    ("accounts.login.link_unavailable", "Đăng nhập · chưa khả dụng"),
    ("accounts.full.checking", "Đang kiểm tra thông tin tài khoản…"),
    ("accounts.full.ready", "Thông tin tài khoản đã sẵn sàng."),
    ("accounts.full.error", "Không thể kiểm tra yêu cầu này."),
    ("accounts.full.pending", "Đang gửi yêu cầu…"),
    ("accounts.full.unknown", "Kết quả chưa được xác nhận. Kiểm tra lại hoặc thử lại cùng yêu cầu."),
    ("accounts.full.conflict", "Hồ sơ đã thay đổi. Tải lại hồ sơ hiện tại trước khi lưu."),
    ("accounts.full.saved", "Đã lưu thay đổi."),
    ("accounts.full.safe_error", "Thông tin tài khoản được ẩn khi phiên hiện tại không cho phép."),
    ("accounts.full.enrollment_intro", "Xác nhận danh tính với Kanidm, rồi chọn tên tài khoản và tên hiển thị."),
    ("accounts.full.accepted_without_session", "Tài khoản Tabula của bạn đã sẵn sàng. Bạn vẫn chưa đăng nhập; hãy chọn Đăng nhập."),
    ("accounts.full.register_rejected", "Không thể chấp nhận đăng ký. Kiểm tra thông tin hoặc bắt đầu lại."),
    ("accounts.full.fields_invalid", "Kiểm tra yêu cầu về tên tài khoản và tên hiển thị bên dưới."),
    ("accounts.full.handle", "Tên tài khoản"),
    ("accounts.full.handle_help", "3–32 chữ cái ASCII viết thường, chữ số hoặc dấu gạch dưới. Không thể đổi tên tài khoản sau này."),
    ("accounts.full.display_name", "Tên hiển thị"),
    ("accounts.full.name_help", "1–64 ký tự, tối đa 256 byte UTF-8. Không cho phép ký tự điều khiển."),
    ("accounts.full.enrollment_start", "Xác nhận danh tính với Kanidm"),
    ("accounts.full.reconcile", "Kiểm tra kết quả đăng ký"),
    ("accounts.full.missing_profile", "Tài khoản chưa có hồ sơ đăng ký. Khi đăng ký khả dụng, đăng xuất, rồi dùng Tạo tài khoản để xác nhận cùng danh tính và hoàn thành hồ sơ còn thiếu."),
    ("accounts.full.profile_details", "Thông tin hồ sơ"),
    ("accounts.full.visibility", "Quyền xem hồ sơ"),
    ("accounts.full.visibility.private", "Riêng tư"),
    ("accounts.full.visibility.friends", "Bạn bè"),
    ("accounts.full.visibility.public", "Công khai"),
    ("accounts.full.edit", "Sửa hồ sơ"),
    ("accounts.full.save", "Lưu thay đổi"),
    ("accounts.full.refetch", "Tải lại hồ sơ hiện tại"),
    ("accounts.full.retry_same", "Thử lại cùng yêu cầu"),
    ("accounts.full.friends_intro", "Tìm tài khoản theo tên và quản lý lời mời kết bạn."),
    ("accounts.full.search", "Tìm tài khoản"),
    ("accounts.full.search_help", "Nhập tên tài khoản viết thường. Kết quả chỉ có thông tin được phép xem."),
    ("accounts.full.friends_list", "Bạn bè"),
    ("accounts.full.requests", "Lời mời kết bạn"),
    ("accounts.full.empty", "Chưa có mục nào."),
    ("accounts.full.send", "Gửi lời mời kết bạn"),
    ("accounts.full.accept", "Chấp nhận lời mời"),
    ("accounts.full.decline", "Từ chối lời mời"),
    ("accounts.full.cancel_request", "Hủy lời mời"),
    ("accounts.full.observed_at", "Quan sát lúc"),
    ("accounts.full.last_seen", "Hoạt động lần cuối"),
    ("accounts.full.pending_request", "Đang chờ"),
    ("accounts.full.accepted_request", "Đã chấp nhận"),
    ("accounts.full.declined_request", "Đã từ chối"),
    ("accounts.full.expired_request", "Đã hết hạn"),
    ("accounts.full.cancelled_request", "Đã hủy"),
    ("accounts.full.presence.unknown", "Chưa rõ trạng thái"),
    ("accounts.full.presence.online", "Trực tuyến"),
    ("accounts.full.presence.offline", "Ngoại tuyến"),
    ("accounts.full.presence.stale", "Trạng thái đã cũ"),
    ("accounts.full.resync", "Tải lại bạn bè"),
    ("accounts.full.friends_available", "Quản lý bạn bè và lời mời."),
    ("accounts.full.register_available", "Xác nhận danh tính và tạo tài khoản."),
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
    ("nav.library", "Thư viện"),
    ("home.heading", "Chơi một ván nhé?"),
    ("home.browse", "Xem tất cả trò chơi"),
    ("home.featured", "Tìm niềm vui tiếp theo"),
    ("library.heading", "Thư viện game"),
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
        for game in tabula_registry::registered_discovery_games() {
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
pub fn shell(locale: Locale) -> (Messages, tabula_registry::DiscoveryCatalog) {
    let messages = Messages::new(locale);
    let catalog = tabula_registry::DiscoveryCatalog::new(
        tabula_registry::registered_discovery_games(),
        &messages,
    );
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
    fn hero_discretionary_breaks_preserve_the_authored_sentence() {
        let english = Messages::new(Locale::En).text("home.hero.heading");
        assert_eq!(
            english.replace('\u{00ad}', ""),
            "Every move,\na little discovery."
        );
        assert!(english.contains("dis\u{00ad}cov\u{00ad}ery"));
        assert_eq!(
            Messages::new(Locale::Vi).text("home.hero.heading"),
            "Mỗi nước đi,\nmột điều để học."
        );
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
