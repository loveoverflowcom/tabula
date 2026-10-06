# Ma Sói — Đêm ở làng Sương

Bản thiết kế tương tác, dữ liệu mẫu, mô phỏng cục bộ. Mở `index.html` qua một server static. Không có luật game, authority, mạng, chat hoặc voice.

## Hướng thiết kế

Làng dưới trăng và vòng chân dung là bàn chơi chính. 12 chân dung là SVG trung tính với mặt nạ giống nhau; tóc, da và trang phục không mã hóa vai trò. Bài của ghế khác có chung mặt sau. Chỉ lá bài mẫu của bạn được lật. Chọn mục tiêu qua chân dung; hành động chính nằm ở một dock ngắn. Điều khiển demo được thu gọn ở cuối trang.

Desktop dùng vòng 12 ghế; mobile 390 px dùng lưới 4×3 và cuộn trang để giữ cỡ chân dung/chữ rõ ràng. Lá bài riêng có vùng riêng, không đè bàn chơi. Không giả lập online/voice.

## Chuyển động

- Lật bài riêng: 480 ms. Blur, Escape hoặc đổi cảnh che ngay. Giảm chuyển động đổi mặt tức thì.
- Chọn chân dung: 160 ms nâng nhẹ + viền focus; giữ trạng thái chọn cuối.
- Lá phiếu công khai: 160 ms từ trung tâm tới người được chọn; không áp dụng cho lựa chọn đêm.
- Đổi đêm/ngày/biểu quyết: tonal wash 480 ms. Không chặn input.
- Tàn lửa nền có giới hạn 12 hạt; giảm chuyển động dừng hẳn.

Các duration tham chiếu `docs/ui/tokens.json` và doc 04 §9. Đây là choreography mẫu cho Macroquad, không phải evidence Rust runtime. Khi triển khai, giữ `Local`, `ViewEvent`, reduced-motion, quyền riêng tư và invariant I-10.

## Nguồn ảnh

- `assets/village.png`: cảnh nền nguyên bản do ImageGen tạo trong phiên redesign này, bản sao asset từ `generated_images/exec-7c159190-5976-4b13-a976-58a083eb5808.png`.
- `assets/werewolf.png`: art runtime hiện có, lấy nguyên vẹn từ `games/werewolf/assets/werewolf@2x.png` ở `develop@e75624ae870a74f62f0f734fbcf2f12043047dd4`. Giữ upright, không xoay source để chữa lỗi UV.
- Chân dung SVG được tạo trực tiếp trong `preview.mjs`, không chứa artwork bên thứ ba.

Các chữ, status, nhãn và nút đều là typography live, không được bake vào ảnh.

`export_preview.py` xuất SVG tự chứa + PNG/JPG tham chiếu bằng Inkscape; không phải ảnh chụp browser hoặc Rust runtime. `portraits.mjs` và `portrait-data.json` dùng chung SVG chân dung cho HTML/export. `assets/village.webp`/`village-dawn.webp` là derivatives tối ưu cho trình diễn; PNG gốc chỉ giữ trong gói nguồn cuối. Dawn là asset riêng đã được tạo trong cùng phiên.
