# Ma Sói — Đêm ở làng Sương

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/issue-82-game-feel/werewolf).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


Bản thiết kế • dữ liệu mẫu • mô phỏng cục bộ. Mở `index.html` qua một server static. Không có luật game, authority, mạng, chat hoặc voice.

## Avatar tài khoản chung

Bàn Ma Sói, avatar nhỏ ở header và `dashboard-avatar-sync.html` dùng chung `avatar-fixtures.json` cùng resolver `account-avatars.mjs`. Subject ID, tên, avatar reference/version và chữ cái fallback đều lấy từ cùng public profile mẫu. Các hình SVG gốc ở `assets/account-avatars/` không có mặt nạ, role art hoặc sigil. Vai trò riêng nằm ở lá bài, không nằm trên avatar.

`dashboard-avatar-sync.html` chỉ là đối chiếu sáu hồ sơ/ghế dùng đúng cùng asset; không phải thiết kế lại dashboard đầy đủ. Fixture là hợp đồng hiển thị mục tiêu. Source hiện tại chưa có API/avatar resolver hoàn chỉnh; không coi ảnh này là luồng tài khoản đã được tích hợp.

Prototype tải managed asset mẫu cùng origin. Profile không có avatar hoặc ảnh lỗi hiển thị initials cùng kiểu ở cả hai surface. Remount có generation guard để callback cũ không thay avatar/fallback của occupant mới. Ghế của bạn được xác định bằng `fixture.ownSubjectId`, không bằng số ghế cố định. `portraits.mjs` và `portrait-data.json` chỉ là adapter tương thích, trỏ về fixture này, không còn artwork mặt nạ cũ.

## Bố cục theo viewport

- Desktop: 12 avatar quanh bàn làng, vùng bài riêng bên cạnh; chọn một mục tiêu và một CTA theo pha.
- 390×844: header 56 px, phần tên cảnh gọn, roster 4×3, thanh mở bài riêng thu gọn và dock hành động có slot riêng. 12 người và CTA chính nằm trong màn đầu. Bài riêng mở bằng native dialog, đóng bằng nút hoặc Escape; đóng trả focus về nút mở.
- 320×640: giảm khoảng cách và avatar; 4×3 vẫn giữ tên/số ghế 12 px. Dock sticky nằm trong flow và có padding safe-area. Footer/công cụ mẫu có vùng riêng dưới bàn; trang có thể cuộn.
- Landscape 844×390: roster 6×2 ở cột trái; vùng mở bài và hành động nằm ở cột phải đã dành chỗ. Header không nổi đè lên bàn.

Tên đầy đủ nằm trong label/title của ghế; focus và selected có marker riêng, không dùng vai trò để phân biệt người chơi. Blur, hidden, Escape và đổi pha che bài riêng ngay và cancel token; card front được loại khỏi accessibility tree khi che. Reduced motion giữ trạng thái reveal đã chọn, snap mặt bài và cancel token.

## Chuyển động

- Lật bài riêng: 480 ms; giảm chuyển động đổi mặt tức thì.
- Chọn avatar: 160 ms nâng nhẹ + focus/selected.
- Phiếu công khai: token 160 ms; không dùng cho lựa chọn đêm.
- Đổi ngày/đêm: artwork bình minh thật và tonal wash 480 ms.
- Tàn lửa giới hạn 12 hạt; giảm chuyển động tắt, hidden pause.

Duration tham chiếu semantic tokens hiện tại. Đây là choreography mẫu cho Macroquad, không phải evidence Rust runtime. Giữ `Local`, `ViewEvent`, reduced motion, projection privacy và invariant I-10 khi triển khai.

## Reference và kiểm chứng

`export_preview.py` đọc cùng avatar fixture và asset SVG. Desktop night/day/vote 1440×960, mobile 390×844, compact 320×640, landscape 844×390 và `dashboard-avatar-sync` 1440×960 đều có SVG/PNG/JPG. Đây là **static vector references**, không phải browser screenshots hay ảnh Macroquad thật; exporter bố trí thiết kế và cùng dữ liệu, không thay thế browser layout engine.

Source syntax, fixture mapping, XML/dimensions và kiểm tra hình static: PASS. Browser interaction/layout/accessibility tree/animation playback: **NOT_RUN** vì browser runtime không có trong môi trường. Rust/Macroquad/native/network checks: **NOT_RUN**; lượt này chỉ cập nhật thiết kế.

Nền `village.webp` và `village-dawn.webp` là derivatives tối ưu cho trình diễn. PNG gốc chỉ giữ trong ZIP tổng, không commit vào thư mục asset GitHub; dawn là asset riêng tạo trong cùng phiên. Cảnh đêm gốc từ `generated_images/exec-7c159190-5976-4b13-a976-58a083eb5808.png`; role art giữ nguyên upright từ pack Tabula. Chữ/status/nút là typography live, không bake vào ảnh.

Nguồn ảnh, size/hash và dữ liệu ở `ASSETS.md`; ràng buộc tích hợp ở `IMPLEMENTATION.md`.
