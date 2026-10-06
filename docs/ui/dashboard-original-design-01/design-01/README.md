# Tabula · Design 01

Bộ prototype tương tác cho nền tảng board game Tabula. Có **22 màn hình đại diện**, HTML/CSS/MJS gốc và ảnh PNG desktop/mobile. Bản thiết kế ngày 01/10/2026.

## Mở và xem ngay

1. Giải nén toàn bộ thư mục `tabula-design`.
2. Mở **`index.html`** bằng trình duyệt để bấm thử prototype. Không cần Node, npm, tài khoản hay server.
3. Mở **`gallery.html`** để xem danh mục 22 màn và chọn ảnh desktop/mobile.

**`standalone.html`** là phiên bản một file, đã nhúng CSS và JavaScript. Có thể mang riêng file này sang máy khác; không cần thư mục `assets` hoặc `styles`. Trình xem trước file đính kèm của một số dịch vụ không chạy JavaScript: hãy mở file bằng trình duyệt sau khi giải nén.

Trong ứng dụng, nút **Tất cả màn hình** mở bộ chuyển màn. Trên mobile, mở menu ở góc trên để truy cập các mục bổ sung. Các URL hash, ví dụ `index.html#learn/xiangqi`, cho phép mở trực tiếp một màn.

## Các màn hình

| # | Màn hình | Route mẫu |
|---|---|---|
| 01 | Thư viện game | `#library` |
| 02 | Chi tiết game | `#game/xiangqi` |
| 03 | Thiết lập ván | `#setup/xiangqi` |
| 04 | Bàn chơi · Cờ vua | `#play/chess` |
| 05 | Bàn chơi · Caro | `#play/caro` |
| 06 | Bàn chơi · Tiles | `#play/tiles` |
| 07 | Bàn chơi · Ma sói | `#play/werewolf` |
| 08 | Bàn chơi · Cờ tướng | `#play/xiangqi` |
| 09 | Kết quả ván | `#result/xiangqi` |
| 10 | Ván của tôi | `#history` |
| 11 | Phân tích | `#analysis/xiangqi` |
| 12 | Gia sư cờ tướng | `#learn/xiangqi` |
| 13 | Cài đặt | `#settings` |
| 14 | Tài nguyên game | `#resources/xiangqi` |
| 15 | Đăng nhập | `#login` |
| 16 | Tạo tài khoản | `#register` |
| 17 | Phòng chơi | `#rooms` |
| 18 | Phòng chờ | `#room` |
| 19 | Ghép trận | `#queue` |
| 20 | Hồ sơ | `#profile` |
| 21 | Bạn bè | `#friends` |
| 22 | Bộ thành phần | `#design-system` |

`Chơi / Phân tích / Gia sư` nằm trong Xiangqi, không phải ba ứng dụng độc lập. Chi tiết/thiết lập/kết quả còn có thể mở cho các game khác qua UI. Các ảnh đại diện không phải toàn bộ tổ hợp trạng thái.

## Có gì trong ZIP?

```text
tabula-design/
├── index.html              # Mở trực tiếp; dùng bundle đã dựng
├── standalone.html         # HTML tự chứa toàn bộ prototype
├── index.dev.html          # Entry dùng ES modules khi chạy qua HTTP
├── gallery.html            # Bộ chọn màn + PNG desktop/mobile
├── src/
│   ├── icons.mjs           # Icon SVG và dấu hiệu nhận diện
│   ├── data.mjs            # Dữ liệu mẫu, danh mục game, danh sách màn
│   ├── boards.mjs          # SVG/CSS minh họa và bàn chơi
│   ├── screens.mjs         # Component và template các màn
│   └── app.mjs             # Router, state, sự kiện và modal
├── styles/
│   ├── tokens.css          # Semantic tokens và phần mở rộng prototype
│   └── app.css             # Responsive layout và component styles
├── assets/                 # Favicon SVG + bundle JS được sinh
├── previews/
│   ├── desktop/            # 22 PNG toàn trang, rộng 1440 px
│   ├── mobile/             # 22 PNG khung nhìn 390 × 844 px
│   ├── mobile-full/        # 22 PNG toàn bộ nội dung, rộng 390 px
│   ├── states/             # Dark settings, loading, reconnect
│   ├── overview-desktop.png
│   ├── overview-mobile.png
│   └── cover.png
├── docs/                   # Ghi chú thiết kế, screen map và báo cáo QA
└── tools/                  # Build, capture, interaction tests
```

Ảnh `mobile/` giữ nguyên bottom navigation cố định như ứng dụng. Ảnh `mobile-full/` chỉ tạm đưa navigation xuống cuối luồng tài liệu khi chụp để không che phần nội dung dài; mã nguồn ứng dụng vẫn giữ navigation cố định. Ảnh desktop có thể cao hơn 1080 px vì chụp toàn trang.

## Tương tác đã có và giới hạn

**Có thể bấm thử:** tìm/lọc game, yêu thích, cấu hình ván, chuyển màn, chọn/di chuyển một số quân minh họa, đặt X/O, xoay mảnh ghép, chọn phiếu Ma sói, chuyển vị trí replay mẫu, hội thoại gia sư dựng sẵn, cài engine giả lập, modal, profile mẫu, phòng chờ và đổi giao diện sáng/tối.

**Đây không phải game/client production:**

- Không có server, WebSocket, đăng nhập thật, lưu ván thật, gửi lời mời hoặc matchmaking thật. Các dữ liệu profile, số liệu, thời gian và thành tích là minh họa.
- Không có bộ luật đầy đủ. Chess chỉ có thao tác mẫu; Caro không xác định chiến thắng; Tiles/Werewolf không chạy luật hoàn chỉnh. Xiangqi chỉ thể hiện chọn quân/đường gợi ý mẫu.
- Replay điều khiển một timeline minh họa, **không tái tạo chính xác bàn cờ theo lịch sử**. Nhánh phân tích, score và phương án đều không phải đầu ra engine.
- Tutor trả lời từ kịch bản, không gọi LLM/RAG. Form nhập FEN/replay chỉ mở dữ liệu có sẵn, không parse dữ liệu nhập.
- Tải/cài tài nguyên và giới hạn CPU là trạng thái giao diện, không tải binary/weights hay điều khiển tài nguyên máy.
- `localStorage` chỉ lưu một số sở thích, game yêu thích, tên hiển thị và thiết lập mẫu. Mật khẩu mẫu không được lưu hoặc gửi đi. Có thể xóa dữ liệu trang trong trình duyệt để reset.
- Âm thanh chỉ là trạng thái lựa chọn trong prototype; không kèm tệp audio.

Tất cả minh họa và icon được dựng bằng SVG/CSS trong mã nguồn. Không dùng CDN, tracking hay tài nguyên từ Internet. Không kèm font; UI dùng font hệ thống nên chữ có thể khác đôi chút giữa các máy. Ký tự quân cờ tướng cần font CJK hệ thống phù hợp.

## Chỉnh sửa source

Sửa `src/*.mjs` và `styles/*.css`, sau đó chạy từ thư mục gốc:

```bash
python3 tools/build.py
```

Lệnh trên sinh lại `assets/app.bundle.js`, `standalone.html` và `index.dev.html`. Không sửa bundle trực tiếp. Builder tối giản chỉ phù hợp cấu trúc import/export hiện tại; khi mở rộng dự án thật, thay bằng pipeline build của ứng dụng.

Để phát triển trực tiếp bằng ES modules:

```bash
python3 -m http.server 8000
# Mở http://localhost:8000/index.dev.html
```

Không mở `index.dev.html` qua `file://`; trình duyệt thường hạn chế tải ES modules theo cách đó. Bản `index.html`/`standalone.html` đã được chuẩn bị để không cần module loader.

## Chạy lại kiểm tra và xuất PNG

Phần xem prototype không cần dependency. Các script QA tùy chọn cần Python + Playwright:

```bash
python3 -m pip install playwright
python3 -m playwright install chromium
python3 tools/build.py
python3 tools/verify.py
python3 tools/capture.py --mode desktop
python3 tools/capture.py --mode mobile
python3 tools/capture.py --mode audit
```

`capture.py` dùng Chromium hệ thống nếu có, nếu không dùng Chromium do Playwright cài. Có thể thêm `--base-url http://localhost:8000` để chụp `index.html` qua server đang chạy. Mặc định script nạp trực tiếp bản standalone vào browser.

**Kết quả lần đóng gói:** 23/23 bài smoke test tương tác đạt; 22 màn × 5 độ rộng (320, 600, 768, 1024, 1440) không phát hiện lỗi JavaScript hoặc tràn ngang. Báo cáo JSON nằm trong `docs/`. Kiểm tra này không chứng nhận mọi tương tác, accessibility production, luật game, backend, native hoặc tất cả trình duyệt. Runtime kiểm tra là Chromium với `standalone.html` nạp qua Playwright `set_content`; không kiểm thử chính sách `file://` trên từng máy. Các MJS và bundle cũng đã được kiểm tra cú pháp bằng Node.

## Căn cứ và bàn giao

Đọc cùng `docs/design-notes.md`. Thiết kế bám danh sách màn đã thảo luận, frontend architecture trên `develop` tại `44f6b74e07648abc7191363d7582efc1fceab262`, `tokens.toml` và hướng Xiangqi Tutor trong issue #48. Đây là **đề xuất giao diện và prototype độc lập**, không phải báo cáo chức năng đã có trong repository; chưa sửa source hoặc tạo PR trên GitHub.
