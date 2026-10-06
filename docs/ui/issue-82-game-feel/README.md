# Tabula — Ma sói và Cờ vua redesign

Thiết kế ngày 06/10/2026, tuần tự Ma sói trước Cờ vua. Giữ Rust/Macroquad và primary tím. Lượt này tạo design/source/issue, chưa sửa runtime hoặc luật game.

1. [Ma sói #84](https://github.com/loveoverflowcom/tabula/issues/84): cảnh làng đêm/bình minh, avatar tài khoản đồng bộ dashboard, mobile responsive, bài riêng drawer và phiếu/chuyển pha.
2. [Cờ vua #85](https://github.com/loveoverflowcom/tabula/issues/85): bàn có chiều sâu, Staunton upright, move/capture/promotion và góc nhìn quân đen.

Mỗi folder có index.html, style.css, preview.mjs, IMPLEMENTATION.md, ISSUE.md, exporter và SVG/PNG/JPG tham chiếu. PNG/JPG là ảnh xuất từ source SVG với dữ liệu mẫu, không phải screenshot Macroquad hoặc browser. Ma sói bổ sung avatar dùng chung dashboard/header/game, mobile390×844, compact320×640 và landscape844×390; màn tham chiếu ưu tiên ghế + CTA, bài riêng mở drawer. Chess giữ bản trước với ảnh full-page390×1050.

## Xem preview có animation

Giải nén ZIP, mở terminal tại folder này rồi chạy:

```sh
python3 -m http.server 8000
```

Mở http://localhost:8000/werewolf/ và http://localhost:8000/chess/. Dùng các nút/công cụ mẫu để thử mở bài, đổi pha, bỏ phiếu, di chuyển quân, capture, đảo bàn và phong cấp. Các demo dùng fixture, không có network/game rules/authority; không dùng để nghiệm thu luật hoặc quyền.

## Xuất lại ảnh tĩnh

Cần Python3, Pillow và Inkscape:

```sh
python3 werewolf/export_preview.py
python3 chess/export_preview.py
```

Các SVG xuất ra tự chứa; exporter dùng chung artwork/model/portrait dữ liệu với prototype. Một số hiệu ứng bóng browser dùng ellipse thay thế trong static export để Inkscape hiển thị ổn định.

## Evidence và artwork

- Đã review source và inspect các preview Ma sói night/day/vote, mobile/compact/landscape và avatar-sync; Chess desktop/mobile/black-orientation/promotion giữ bản đã inspect trước.
- Syntax MJS, Python compile, XML/dimensions và fixture/source checks đã chạy. Browser playback, Macroquad/native runtime, frame pacing, privacy/network acceptance chưa chạy (NOT_RUN).
- Nền Ma sói là original imagegen night và dawn; prompt nguyên văn trong PROMPTS.md, source/hash trong werewolf/ASSETS.md. Avatar SVG không mặt nạ là tài khoản mẫu; avatar-fixtures.json + account-avatars.mjs dùng chung cho mini dashboard, header và bàn, không encode role assignment. Role art lấy nguyên vẹn từ asset Tabula hiện có. Source hiện chỉ có account ID, chưa có avatar API; contract nguồn avatar công khai cần bổ sung tại host/resource layer.
- Chess tái dùng Staunton SVG nguyên bản; PIECE-PROVENANCE.md giữ attribution. Không dùng Unicode chess glyph hoặc ảnh stock.
- Asset/version/hash/runtime budgets cần đi qua pack per-game khi implementation. Animation thuộc Local và không làm authority/clock/phase chờ frame.

Source đối chiếu: develop@e75624ae870a74f62f0f734fbcf2f12043047dd4. Gói werewolf-assets.zip chứa asset tối ưu và source preview Ma sói để tải từ #84. Các issue giữ liên kết artifact immutable và nghiệm thu runtime riêng. #82 có comment dẫn hai issue; receipt/ảnh chạy thật gốc giữ nguyên.
