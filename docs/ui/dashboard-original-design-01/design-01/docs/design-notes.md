# Tabula · Design handoff

## 1. Phạm vi thiết kế

Một trải nghiệm thống nhất: **Khám phá → Thiết lập → Chơi → Kết quả → Xem lại / Học**. Home và Catalog được gộp thành Thư viện để không có hai màn đầu gần giống nhau. Lịch sử tách ván đang dở với ván đã kết thúc. Profile/friends/rooms là các khu vực online bổ sung, không là điều kiện để dùng local trong đề xuất này.

Bộ thiết kế gồm 22 route đại diện. Mỗi game có thể cung cấp layout và capability riêng. Không ép Werewolf và Tiles vào layout hai người của cờ vua; không đưa gia sư thành chatbot toàn cục độc lập.

## 2. Căn cứ repository

Mốc tham chiếu: `develop @ 44f6b74e07648abc7191363d7582efc1fceab262`.

- `tokens.toml`: identity M3 Expressive-inspired; semantic purple, sáng/tối, display serif và body sans.
- `docs/architecture/04-frontend-and-design-system.md`: shell, gameplay runtime, route map và handoff giữa hai phần.
- `apps/web/src/main.rs`: danh sách route trong skeleton, **không phải router đã triển khai**.
- `apps/game-client/src/main.rs`: vòng chơi local, lựa chọn game/cấu hình từ entry point.
- `games/README.md`: Chess, Caro, Tiles, Werewolf và khác biệt về board/phase/hidden information.
- `loveoverflowcom/tabula#48`: hướng Xiangqi Tutor với Play / Analyze / Learn, engine và LLM tùy chọn.

Đề xuất UI bổ sung không thay đổi quyết định kiến trúc của repository. Snapshot source và trạng thái issue không chứng minh chức năng mới đã được implementation.

## 3. Ngôn ngữ thiết kế

**Nhận diện:** primary tím `#5634BE`, sáng/tối bám vai trò semantic có sẵn. Giữ màu active/selection nhất quán, màu xanh cho local/healthy, sắc ấm cho bàn chơi. Không dùng một palette game để thay thế màu ứng dụng toàn cục.

**Canvas:** giấy ấm nhẹ `#F9F7F4` với panel trắng; đây là phần mở rộng prototype, khác token `surface` gốc. Lavender, sage, peach và mint chỉ dùng làm tonal surface/art. Tất cả phần thêm được gom trong `styles/tokens.css` và gắn chú thích, không trình bày là output `xtask gen-tokens`.

**Kiểu chữ:** serif ở tiêu đề lớn để có cảm giác thư viện/bàn chơi; sans cho nội dung điều khiển; mono cho thời gian/mã phòng. Font là fallback hệ thống, không đóng gói font.

**Hình khối:** card mềm, CTA tròn vừa phải, hạn chế border trong các khối nội dung. Bàn chơi có ranh giới rõ; đường lưới không bị thay thế bằng shadow trang trí. Focus/selection không chỉ dựa vào thay đổi màu mờ.

**Layout:** desktop sidebar 224 px, topbar 80 px, nội dung có max-width; màn bàn chơi ưu tiên diện tích bàn. Từ chiều rộng nhỏ, các cột xếp dọc, phần phụ chuyển xuống dưới, sidebar thành drawer và thanh điều hướng đáy. 320 px là độ rộng nhỏ nhất đã audit overflow.

**Chuyển động:** motion ngắn, có reduced-motion. Dark theme và high-contrast toggle là mẫu component; chưa qua chứng nhận WCAG. Không tuyên bố canvas runtime native tự có accessibility tương đương HTML.

## 4. Màn hình và ownership khi triển khai thật

| Khu vực | Owner đề xuất | Ranh giới cần giữ |
|---|---|---|
| Thư viện, Chi tiết, Setup, Lịch sử, Kết quả, Profile, Settings | Application shell | Dùng metadata/capability và protocol đã định kiểu; không tự suy luật |
| Bàn chơi, lượt/phase, clocks, lựa chọn/preview | Game presentation + runtime | Authoritative view không trộn với UI selection/optimistic preview |
| Rooms, ready, invitations, queue | Lobby / match service | Server xác thực quyền và ready state; UI không tự cấp phép |
| Replay | Rules version + replay authority | Tái dựng từ accepted events/moves, không generate lại quá khứ bằng AI |
| Xiangqi analysis | Host engine adapter + game state | Gắn position digest, rules/engine version, budget, cancellation và stale result |
| Tutor | Xiangqi presentation + optional explanation provider | Evidence ở đúng position; không trực tiếp sửa game state |
| Engine/resource page | Host + asset manager | Integrity/license/provision status thật; không tự tải hoặc đổi sang cloud |

HTML/SVG trong ZIP là **reference thiết kế**, không đề xuất đưa gameplay Macroquad vào Leptos hoặc WebView. Khi port, giữ interface/ownership của Tabula; mapping component sang Leptos cho shell và presenter/RenderList cho game runtime. Hình minh họa trong catalog không cần dùng cùng renderer với bàn chơi.

## 5. Bàn chơi theo game

- **Chess:** board, đồng hồ, turn và notation bên cạnh. Tương tác mẫu ưu tiên chọn Mã g1 hoặc Tốt d2; không có bộ luật hoàn chỉnh.
- **Caro:** board ô lớn, chỉ báo lượt, lịch sử đơn giản. Prototype nhận X/O nhưng không kiểm tra thắng/thua.
- **Tiles:** bàn mở rộng, zoom, mảnh chờ và xoay/đặt, bảng điểm. Những cập nhật điểm và vị trí trong prototype chỉ là hình dung UI.
- **Werewolf:** trung tâm là người chơi, pha, role cá nhân và vote. Khi triển khai, thông tin ẩn phải bị chặn tại projection/authority, không chỉ hidden trong DOM.
- **Xiangqi:** Play / Analyze / Learn cùng một vùng bàn. Chế độ phân tích có nhánh riêng và panel engine; gia sư có hội thoại + đề xuất xem phương án. Trợ giúp chỉ xuất hiện trong mode được phép, không mở mặc định trong trận cạnh tranh.

Replay không phải Chess/Xiangqi có giao diện sự kiện chung, không hiển thị engine score hoặc notation cờ không phù hợp.

## 6. Trạng thái cần nối với dữ liệu thật

Các trạng thái được thiết kế hoặc minh họa trong prototype: search rỗng, không có ván, room chưa ready, loading tài nguyên, chưa có engine, engine đã cài mẫu, xác nhận rời/reset, mất kết nối, input form không hợp lệ, snackbar phản hồi.

Khi triển khai cần thêm những nghiệm thu backend: token hết hạn, không đủ quyền xem replay, game/version không tương thích, engine crash/timeout/cancel, assets hỏng, upload quá lớn, room đầy/ván đã bắt đầu, khôi phục sau restart, vị trí phân tích đã thay đổi. Những lỗi này không được coi là đã xử lý chỉ vì có modal mẫu.

## 7. Dữ liệu mẫu và an toàn

Không dùng tài khoản hay phiên của người dùng. Tên/mốc thời gian/rating/kết quả/room player đều là nội dung minh họa. Không gửi email, mời người thật, đọc file ngoài trình duyệt, lưu password, gọi model hoặc tải engine. Bộ demo không có API secret.

SVG chỉ là tài sản hình ảnh minh họa do prototype tạo, không chứa engine, font, model weights hoặc art trích từ game thương mại. Bộ ZIP không gán giấy phép cho repository gốc; cần chủ dự án quyết định license khi đưa những file này vào repo.

## 8. QA và giới hạn của bằng chứng

Bằng chứng đi kèm:

- `interaction-report.json`: 23 smoke tests UI đạt; phần scope trong file nêu rõ không test backend/AI/rules.
- `capture-desktop.json`, `capture-mobile.json`: 22 route mỗi loại, không phát hiện lỗi JavaScript hoặc horizontal overflow.
- `capture-audit.json`: 110 tổ hợp route/width, cùng tiêu chí trên.

Ảnh được render từ HTML thực bằng Chromium, không phải ảnh giao diện dựng độc lập. Lần kiểm tra dùng `standalone.html` qua `set_content` vì môi trường hạn chế điều hướng URL. Các báo cáo không xác nhận toàn bộ luồng `file://`, trình duyệt khác, native, concurrency, accessibility hay correctness của game. Không có build/run repository Rust trong lượt thiết kế này.

Ảnh mobile viewport giữ fixed nav. Ảnh mobile-full đổi duy nhất vị trí nav khi chụp để phục vụ handoff toàn nội dung, không phải một implementation responsive khác.
