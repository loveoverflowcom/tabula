# Tabula — T Portal / Lavender v2

Bộ logo theo ảnh lavender người dùng chọn ngày 06/10/2026. Đây là asset bàn giao, chưa tích hợp runtime.

## Nguồn và màu

Hình T liền khối: chữ T / bàn chơi chung / cánh cổng vào nhiều game. Vector đã được chuẩn hóa từ ảnh; không phải pixel trace. Chỉ tham khảo tinh thần Wonderous, không dùng asset Wonderous. Wordmark thường `tabula` đã outline từ Inter Display ExtraBold, không kèm font.

`palette.json`: #5E4B8B (tím mực), #B9A7F3 (mark nền tối), #7C63ED (mark nền sáng), #EBE6F7 (nền lilac). Màu hỗ trợ #473D66, #F7F4FF, #E2DAFA tách rõ khỏi bốn màu brand. Logo phẳng, không glow/gradient/3D.

## Asset chính

- `tabula-mark-primary.svg` và `tabula-mark-primary.png`: SVG nguồn và PNG trong suốt 256×256.
- `tabula-mark-on-dark.svg`, `tabula-mark-mono.svg`: biến thể màu cùng một hình.
- `tabula-mark-micro.svg`: optical variant cho 16–24 px.
- `tabula-wordmark.svg`, `tabula-lockup-light.svg`, `tabula-lockup-dark.svg`: chữ outline và logo ngang.
- `tabula-app-icon.svg`: artwork vuông tràn nền; hệ điều hành sở hữu mặt nạ, không bo góc sẵn trong source.
- `preview.html`: tự chứa; mở offline hoặc tải raw về. Không cần font/CDN; có đổi nền, phát motion theo yêu cầu, reduced motion.

GitHub chứa các asset chính. Gói ZIP bàn giao trong cuộc trò chuyện có thêm PNG 1024, PNG logo ngang/app icon, bảng nhận diện PNG, các cỡ 16–512, ảnh desktop/mobile và script tái tạo. SVG là nguồn cho mọi cỡ PNG; không upscale PNG 256.

## Ràng buộc tích hợp

Chuyển nguồn dùng chung vào `assets/brand/` khi triển khai; dùng semantic tokens/generator của `crates/tabula-design`, không hardcode màu từng màn, không thêm crate chỉ vì logo. Thay `Brand()` trong `apps/web/src/views/parts.rs` và audit mọi consumer hiện hữu. Logo/palette brand mới ưu tiên hơn logo cũ trong #87, nhưng giữ layout dashboard của #87. Không nhuộm lại artwork game hoặc thay avatar người dùng.

Logo không kéo theo tính năng/auth/mobile host mới. Chỉ cập nhật surface đang tồn tại; ghi rõ phần chưa có. App icon native cần export đúng target; không coi mockup là icon đã tích hợp.

Preview đã kiểm tra 1440/1100/768/390/320 px, logo nằm trong card, đổi nền, motion, reduced motion, không page error/request mạng. SVG không nhúng raster hoặc phần tử text; PNG có alpha. Chưa chạy Rust/CMP/runtime, chưa đổi develop/main. Khi triển khai cần ảnh ứng dụng thật.
