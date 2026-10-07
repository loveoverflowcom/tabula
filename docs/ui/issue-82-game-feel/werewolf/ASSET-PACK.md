# Gói tải assets Ma sói

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/issue-82-game-feel/werewolf).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


`../werewolf-assets.zip` chứa asset tối ưu và source preview cho #84:

- 12 SVG avatar tài khoản mẫu không mặt nạ; `avatar-fixtures.json`, resolver dùng chung dashboard/header/game và adapter tương thích.
- `village.webp`, `village-dawn.webp` và role art `werewolf.png` để mở preview offline qua static server.
- HTML/CSS/MJS, exporter Python, README/ASSETS/IMPLEMENTATION và prompt gốc trong `../PROMPTS.md`.

Ảnh tham chiếu SVG/PNG/JPG desktop/mobile/compact/landscape/avatar-sync nằm trực tiếp trong folder source GitHub của issue. Gói tối ưu không lặp các ảnh export hoặc PNG gốc cỡ lớn; chạy `python3 export_preview.py` để xuất lại từ WebP và fixture. ZIP tổng bàn giao gồm hai game vẫn giữ PNG gốc và các ảnh export.

Sau giải nén, chạy `python3 -m http.server 8000` tại folder `tabula-redesign`, mở `http://localhost:8000/werewolf/` hoặc `http://localhost:8000/werewolf/dashboard-avatar-sync.html`.

Đây là source design dùng dữ liệu mẫu; nguồn account avatar thật còn cần contract host/resource layer như IMPLEMENTATION.md. Syntax, mapping và ảnh tĩnh đã kiểm; browser/Macroquad playback chưa chạy.
