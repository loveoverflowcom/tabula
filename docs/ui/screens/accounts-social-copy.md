# Accounts/social — proposed en/vi copy catalogue

PR A of issue #54; [shared contract](accounts-social.md). These keys and
complete sentences are specification copy, **not an installed runtime table**.
PR B/C add the supported subset to the existing `apps/web/src/i18n.rs`
`Messages` pattern and test en/vi coverage, argument parity and missing-key
behavior. Safe structured API errors map to these keys; never display raw
server text or interpolate email, password, credential, account-existence
diagnostics or private denial reasons. Locale switches retain focus/drafts.

## Shared form, navigation and state copy

| Proposed key | English | Vietnamese |
|---|---|---|
| `accounts.login.title` | Sign in | Đăng nhập |
| `accounts.register.title` | Create account | Tạo tài khoản |
| `accounts.profile.title` | Profile | Hồ sơ |
| `accounts.friends.title` | Friends | Bạn bè |
| `accounts.field.email` | Email | Email |
| `accounts.field.password` | Password | Mật khẩu |
| `accounts.field.display_name` | Display name | Tên hiển thị |
| `accounts.password.show` | Show password | Hiện mật khẩu |
| `accounts.password.hide` | Hide password | Ẩn mật khẩu |
| `accounts.action.login` | Sign in | Đăng nhập |
| `accounts.action.register` | Create account | Tạo tài khoản |
| `accounts.action.library` | Browse games | Xem thư viện trò chơi |
| `accounts.action.back` | Back | Quay lại |
| `accounts.action.retry` | Retry | Thử lại |
| `accounts.action.cancel` | Cancel | Hủy |
| `accounts.local.explanation` | Supported local games can be played without an account. | Những trò chơi hỗ trợ chơi local không cần tài khoản. |
| `accounts.session.checking` | Checking your session… | Đang kiểm tra phiên đăng nhập… |
| `accounts.session.signed_out` | You're signed out. | Bạn chưa đăng nhập. |
| `accounts.session.expired` | Your session has ended. Sign in to continue with account features. | Phiên đăng nhập đã kết thúc. Đăng nhập để tiếp tục dùng tính năng tài khoản. |
| `accounts.service.unavailable` | Account services aren't available. You can still browse games. | Dịch vụ tài khoản chưa khả dụng. Bạn vẫn có thể xem thư viện trò chơi. |
| `accounts.connection.offline` | Can't connect right now. Try again when your connection is available. | Hiện chưa thể kết nối. Thử lại khi có kết nối. |
| `accounts.form.errors` | Check the marked fields. | Kiểm tra các trường được đánh dấu. |
| `accounts.field.required` | Enter a value for this field. | Nhập giá trị cho trường này. |
| `accounts.field.email_invalid` | Enter an email in the supported format. | Nhập email đúng định dạng được hỗ trợ. |
| `accounts.field.value_invalid` | This value doesn't meet the stated requirements. | Giá trị này chưa đáp ứng yêu cầu đã nêu. |
| `accounts.auth.pending` | Signing in… | Đang đăng nhập… |
| `accounts.auth.rejected` | Couldn't sign in with these details. Check them and try again. | Chưa thể đăng nhập với thông tin này. Kiểm tra lại rồi thử lại. |
| `accounts.auth.confirmed` | You're signed in. | Bạn đã đăng nhập. |
| `accounts.register.pending` | Submitting account request… | Đang gửi yêu cầu tạo tài khoản… |
| `accounts.register.rejected` | Couldn't complete this account request. Check the entered details and try again. | Chưa thể hoàn tất yêu cầu tạo tài khoản. Kiểm tra thông tin đã nhập rồi thử lại. |
| `accounts.operation.unknown` | Couldn't confirm whether this request completed. Check its status before trying again. | Chưa xác nhận được yêu cầu đã hoàn tất hay chưa. Kiểm tra trạng thái trước khi thử lại. |
| `accounts.rate_limited` | Please wait before trying again. | Vui lòng chờ trước khi thử lại. |
| `accounts.rate_limited_until` | Try again after {time}. | Thử lại sau {time}. |
| `accounts.action.unavailable` | This action isn't available. | Thao tác này chưa khả dụng. |

`{time}` is a localized actual permitted retry instant, never a fabricated
estimate. A timer announcement is throttled, not read every second. Public
registration completion copy depends on the approved service disclosure/
session contract; no “Account created”, “Check your email” or auto-login
message is selected before that contract exists. Mandatory agreement text
and URLs come from the actual agreement, not this catalogue. Password/length
requirements are supplied by the approved field contract, not invented here.

## Profile, list, presence and request copy

| Proposed key | English | Vietnamese |
|---|---|---|
| `accounts.profile.self` | Your profile | Hồ sơ của bạn |
| `accounts.profile.loading` | Loading profile… | Đang tải hồ sơ… |
| `accounts.profile.unavailable` | This profile isn't available. | Hồ sơ này chưa khả dụng. |
| `accounts.profile.no_history` | No eligible match history is available. | Chưa có lịch sử ván đấu phù hợp. |
| `accounts.profile.metric_unavailable` | This statistic isn't available. | Số liệu này chưa khả dụng. |
| `accounts.data.checked_at` | Checked at {time}. | Đã kiểm tra lúc {time}. |
| `accounts.data.stale` | This information may be out of date. | Thông tin này có thể đã cũ. |
| `accounts.friends.loading` | Loading friends… | Đang tải danh sách bạn bè… |
| `accounts.friends.empty` | You don't have any friends in this list yet. | Bạn chưa có bạn bè trong danh sách này. |
| `accounts.friends.filter_label` | Filter your friends | Lọc danh sách bạn bè |
| `accounts.friends.search_label` | Find a player by supported handle or ID | Tìm người chơi bằng tên tài khoản hoặc ID được hỗ trợ |
| `accounts.friends.search` | Search | Tìm kiếm |
| `accounts.friends.no_matches` | No permitted matches for this search. | Không có kết quả được phép hiển thị cho tìm kiếm này. |
| `accounts.friends.unavailable` | Friends services aren't available. | Dịch vụ bạn bè chưa khả dụng. |
| `accounts.friends.requests` | Friend requests | Lời mời kết bạn |
| `accounts.friends.profile` | View profile | Xem hồ sơ |
| `accounts.friends.request` | Request friendship | Gửi lời mời kết bạn |
| `accounts.presence.unknown` | Status unknown | Chưa rõ trạng thái |
| `accounts.presence.online` | Online | Đang online |
| `accounts.presence.offline` | Offline | Đang offline |
| `accounts.presence.stale` | Status may be out of date | Trạng thái có thể đã cũ |
| `accounts.presence.last_seen` | Last seen at {time}. | Online lần cuối lúc {time}. |
| `accounts.invite.incoming` | Incoming request | Lời mời nhận được |
| `accounts.invite.outgoing` | Sent request | Lời mời đã gửi |
| `accounts.invite.pending` | Awaiting a response | Đang chờ phản hồi |
| `accounts.invite.accept` | Accept | Chấp nhận |
| `accounts.invite.decline` | Decline | Từ chối |
| `accounts.invite.cancel` | Cancel request | Hủy lời mời |
| `accounts.invite.accepted` | Accepted | Đã chấp nhận |
| `accounts.invite.declined` | Declined | Đã từ chối |
| `accounts.invite.expired` | This request has expired. | Lời mời này đã hết hạn. |
| `accounts.invite.cancelled` | This request was cancelled. | Lời mời này đã bị hủy. |
| `accounts.invite.updating` | Updating request… | Đang cập nhật lời mời… |
| `accounts.invite.conflict` | This request has changed. Review its current status. | Lời mời này đã thay đổi. Kiểm tra trạng thái hiện tại. |

Request heading identifies friendship vs room/game kind using the actual
supported contract. Labels must not conflate the two. Last-seen/checked time
appears only for a known, permitted observation. Do not fill missing time with
Now, infer Offline from withheld data, or disclose who restricted the viewer.
Counts/plurals, if later needed, use complete localized message variants and
arguments; never concatenate a name/count with an English status fragment.
