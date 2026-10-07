# Tabula — Ma sói và Cờ vua redesign

Historical artifact notice: removed raw evidence/design files remain in the pinned
[pre-cleanup archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/issue-82-game-feel).
Commands and results below describe that original source/build, not current runtime
acceptance. Use ignored `verification/` output for new captures and receipts.


Thiết kế ngày 06/10/2026, tuần tự Ma sói trước Cờ vua. Giữ Rust/Macroquad và primary tím. Lượt này tạo design/source/issue, chưa sửa runtime hoặc luật game.

1. [Ma sói #84](https://github.com/loveoverflowcom/tabula/issues/84): cảnh làng đêm/bình minh, avatar tài khoản đồng bộ dashboard, mobile responsive, bài riêng drawer và phiếu/chuyển pha.
2. [Cờ vua #85](https://github.com/loveoverflowcom/tabula/issues/85): bàn có chiều sâu, Staunton upright, move/capture/promotion và góc nhìn quân đen.

## Historical design archive

The lightweight Markdown requirements, prompts, attribution and implementation handoffs
remain here. HTML/CSS/MJS, exporter scripts, sample JSON/avatar assets, static preview
images and the ZIP design export are available in the pinned
[pre-cleanup design archive](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/ui/issue-82-game-feel).
Those are sample-data designs, not Macroquad runtime screenshots or gameplay acceptance.
To reproduce the historical design, use that exact archived tree outside the current
checkout; keep exported images/bundles outside Git. Runtime game assets remain in
`games/<game>/assets/`, independently of the archived design copies.

## Evidence và artwork

- Đã review source và inspect các preview Ma sói night/day/vote, mobile/compact/landscape và avatar-sync; Chess desktop/mobile/black-orientation/promotion giữ bản đã inspect trước.
- Syntax MJS, Python compile, XML/dimensions và fixture/source checks đã chạy. Browser playback, Macroquad/native runtime, frame pacing, privacy/network acceptance chưa chạy (NOT_RUN).
- Nền Ma sói là original imagegen night và dawn; prompt nguyên văn trong PROMPTS.md, source/hash trong werewolf/ASSETS.md. Avatar SVG không mặt nạ là tài khoản mẫu; avatar-fixtures.json + account-avatars.mjs dùng chung cho mini dashboard, header và bàn, không encode role assignment. Role art lấy nguyên vẹn từ asset Tabula hiện có. Source hiện chỉ có account ID, chưa có avatar API; contract nguồn avatar công khai cần bổ sung tại host/resource layer.
- Chess tái dùng Staunton SVG nguyên bản; PIECE-PROVENANCE.md giữ attribution. Không dùng Unicode chess glyph hoặc ảnh stock.
- Asset/version/hash/runtime budgets cần đi qua pack per-game khi implementation. Animation thuộc Local và không làm authority/clock/phase chờ frame.

Source đối chiếu: develop@e75624ae870a74f62f0f734fbcf2f12043047dd4. Gói werewolf-assets.zip chứa asset tối ưu và source preview Ma sói để tải từ #84. Các issue giữ liên kết artifact immutable và nghiệm thu runtime riêng. #82 có comment dẫn hai issue; receipt/ảnh chạy thật gốc giữ nguyên.
